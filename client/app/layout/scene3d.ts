import {
  BufferAttribute,
  BufferGeometry,
  EdgesGeometry,
  ExtrudeGeometry,
  Float32BufferAttribute,
  Group,
  Line,
  LineBasicMaterial,
  LineDashedMaterial,
  LineLoop,
  LineSegments,
  MOUSE,
  Mesh,
  MeshBasicMaterial,
  OrthographicCamera,
  Raycaster,
  Shape,
  ShapeGeometry,
  Vector2,
  Vector3,
  type Color,
  type Object3D,
} from "three";

import { Stage, type Pose } from "@app/common/stage3d";

import type { Block, Point3, Scene3D } from "./blocks";

/**
 * The site in 3D, drawn by three.js (D173). Turned, moved and zoomed; nothing
 * is edited.
 *
 * An imperative class, not React: the meshes live here, React only hosts the
 * element and says what is chosen. The canvas, the camera's controls and the
 * frame drawn on demand are the shared [`Stage`]'s.
 *
 * The site's axes become three.js's as x → x, y → −z, z → y: the site's y runs
 * away from the viewer, as it runs up the 2D plan, so the two read the same
 * way round.
 *
 * **Colours are the theme's tokens**, read from the page, so dark mode and any
 * later palette apply without a line here. Faces are shaded by which way they
 * face rather than lit, so a token's colour is exactly the colour of a top.
 */

export interface SceneEvents {
  /** A place was clicked (a click, not the end of a drag). */
  choose: (placeId: string) => void;
  /** The pointer is over a place, or over none. */
  hover: (placeId: string | null) => void;
  /** A frame was drawn: anything pinned to a point in the scene should follow. */
  drawn: () => void;
}

/** Which way the view looks at first: from the front left, above. */
const AZIMUTH = (-28 * Math.PI) / 180;
const ELEVATION = (50 * Math.PI) / 180;
/** Room around the site when it is fitted to the pane. */
const MARGIN = 1.12;
/** How much one press of zoom in or out zooms. */
const STEP = 1.6;

const DASH_PX = 4;
const GAP_PX = 3;

/** A face's shade by which way it faces, in the working (linear) colour space. */
function shade(nx: number, ny: number, nz: number): number {
  if (ny > 0.5) return 1;
  if (ny < -0.5) return 0.4;
  // Light from the front left: faces towards it lighter, the far ones darker.
  const toLight = (-0.45 * nx + 0.89 * nz) / Math.hypot(nx, nz || 1e-9);
  return 0.62 + 0.26 * toLight;
}

/** The token colours the scene is drawn in, read from the page. */
interface Palette {
  outer: Color;
  floor: Color;
  solid: Color;
  line: Color;
  floorLine: Color;
  outerLine: Color;
  chosen: Color;
  chosenLine: Color;
  chosenGrid: Color;
  hover: Color;
}

const TOKENS: Record<keyof Palette, string> = {
  outer: "--ui-surface-sunken",
  floor: "--ui-surface",
  solid: "--ui-border-strong",
  line: "--ui-text-faint",
  floorLine: "--ui-border-strong",
  outerLine: "--ui-border-strong",
  chosen: "--ui-accent",
  chosenLine: "--ui-accent-hover",
  chosenGrid: "--ui-accent-fg",
  hover: "--ui-accent",
};

function readPalette(stage: Stage<OrthographicCamera>): Palette {
  const out = {} as Palette;
  for (const [key, token] of Object.entries(TOKENS) as [keyof Palette, string][]) out[key] = stage.token(token);
  return out;
}

/** One place, as drawn: its body, its outline and its grid. */
interface Drawn {
  block: Block;
  body: Mesh;
  outline: Line | LineSegments;
  grid: LineSegments | null;
}

export class SiteScene {
  static supported = Stage.supported;

  private readonly stage: Stage<OrthographicCamera>;
  private readonly world = new Group();
  private readonly raycaster = new Raycaster();

  private palette: Palette;
  private drawn: Drawn[] = [];
  private site: Scene3D = { blocks: [], min: [0, 0, 0], max: [0, 0, 0] };
  /** Half the height of the view that fits the site, before any zoom. */
  private fitted = 1;
  private chosen: string | null = null;
  private hovered: string | null = null;
  private press: { x: number; y: number; at: number } | null = null;

  private readonly materials = {
    solid: new MeshBasicMaterial({ vertexColors: true, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 }),
    floor: new MeshBasicMaterial({ polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 }),
    outer: new MeshBasicMaterial({ polygonOffset: true, polygonOffsetFactor: 2, polygonOffsetUnits: 2 }),
    chosenSolid: new MeshBasicMaterial({ vertexColors: true, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 }),
    chosenFloor: new MeshBasicMaterial({ polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 }),
    line: new LineBasicMaterial(),
    floorLine: new LineDashedMaterial(),
    outerLine: new LineBasicMaterial(),
    grid: new LineBasicMaterial({ transparent: true, opacity: 0.55 }),
    chosenLine: new LineBasicMaterial(),
    chosenGrid: new LineBasicMaterial({ transparent: true, opacity: 0.45 }),
    hover: new LineBasicMaterial(),
  };

  constructor(
    private readonly host: HTMLElement,
    private readonly events: SceneEvents,
  ) {
    this.stage = new Stage(host, new OrthographicCamera(-1, 1, 1, -1, 0.1, 1000), {
      frame: () => {
        // Dashes stay the plan's four pixels and three, however far in.
        const cam = this.stage.camera;
        const perUnit = (this.host.clientHeight * cam.zoom) / (cam.top - cam.bottom);
        this.materials.floorLine.dashSize = DASH_PX / perUnit;
        this.materials.floorLine.gapSize = GAP_PX / perUnit;
        return false;
      },
      drawn: () => this.events.drawn(),
      resized: () => this.fit(false),
      themed: () => {
        this.palette = readPalette(this.stage);
        this.paint();
      },
    });
    this.palette = readPalette(this.stage);
    this.paint();
    this.stage.scene.add(this.world);

    const controls = this.stage.controls;
    // A drag with the middle or right button, or with Ctrl or ⌘ held, moves
    // the view, as in most CAD and 3D tools. It slides over the floor rather
    // than up the screen, so turning afterwards still turns about a point on
    // the floor.
    controls.enablePan = true;
    controls.screenSpacePanning = false;
    controls.mouseButtons = { LEFT: MOUSE.ROTATE, MIDDLE: MOUSE.PAN, RIGHT: MOUSE.PAN };
    controls.zoomToCursor = true;
    controls.minZoom = 0.6;
    controls.maxZoom = 24;
    // Never under the floor, and never quite flat on it.
    controls.minPolarAngle = 0.05;
    controls.maxPolarAngle = Math.PI / 2 - 0.08;

    const canvas = this.stage.canvas;
    canvas.addEventListener("pointerdown", this.onDown);
    canvas.addEventListener("mousedown", this.onMiddle);
    canvas.addEventListener("pointerup", this.onUp);
    canvas.addEventListener("pointermove", this.onMove);
    canvas.addEventListener("pointerleave", this.onLeave);
  }

  /** Draw this site, replacing whatever was drawn, and fit it to the pane. */
  set(site: Scene3D): void {
    const first = this.site.blocks.length === 0;
    this.clear();
    this.site = site;
    for (const block of site.blocks) {
      const d = this.draw(block);
      this.drawn.push(d);
      this.world.add(d.body, d.outline);
      if (d.grid) this.world.add(d.grid);
    }
    this.style();
    this.fit(first, true);
  }

  /** Mark this place as the chosen one, or none. */
  choose(placeId: string | null): void {
    if (placeId === this.chosen) return;
    this.chosen = placeId;
    this.style();
    this.stage.invalidate();
  }

  /** Back to the whole site from the front left. */
  reset(): void {
    this.fit(true, true);
  }

  zoomIn(): void {
    this.zoomBy(STEP);
  }

  zoomOut(): void {
    this.zoomBy(1 / STEP);
  }

  /** Where a point on the site is in the pane, in CSS pixels. */
  project([x, y, z]: Point3): { x: number; y: number } {
    const v = new Vector3(x, z, -y).project(this.stage.camera);
    const { clientWidth: w, clientHeight: h } = this.stage.canvas;
    return { x: ((v.x + 1) / 2) * w, y: ((1 - v.y) / 2) * h };
  }

  dispose(): void {
    const canvas = this.stage.canvas;
    canvas.removeEventListener("pointerdown", this.onDown);
    canvas.removeEventListener("mousedown", this.onMiddle);
    canvas.removeEventListener("pointerup", this.onUp);
    canvas.removeEventListener("pointermove", this.onMove);
    canvas.removeEventListener("pointerleave", this.onLeave);
    this.clear();
    for (const m of Object.values(this.materials)) m.dispose();
    this.stage.dispose();
  }

  // ── drawing ────────────────────────────────────────────────────────────

  private draw(block: Block): Drawn {
    const shape = new Shape(block.ring.map(([x, y]) => new Vector2(x, y)));
    let body: Mesh;
    let outline: Line | LineSegments;
    if (block.solid && block.height > 0) {
      const g = new ExtrudeGeometry(shape, { depth: block.height, bevelEnabled: false });
      // The shape is drawn in x and y and pushed out along z; standing it up
      // turns y into −z and the push into up.
      g.rotateX(-Math.PI / 2);
      g.translate(0, block.z, 0);
      shadeFaces(g);
      body = new Mesh(g, this.materials.solid);
      outline = new LineSegments(new EdgesGeometry(g, 1), this.materials.line);
    } else {
      const g = new ShapeGeometry(shape);
      g.rotateX(-Math.PI / 2);
      g.translate(0, block.z, 0);
      body = new Mesh(g, block.nesting === 0 ? this.materials.outer : this.materials.floor);
      const ring = new BufferGeometry().setAttribute(
        "position",
        new Float32BufferAttribute(block.ring.flatMap(([x, y]) => [x, block.z, -y]), 3),
      );
      // The outermost place's edge is its walls, drawn solid as on the plan;
      // a floor inside it is dashed.
      outline = new LineLoop(ring, block.nesting === 0 ? this.materials.outerLine : this.materials.floorLine);
      outline.computeLineDistances();
    }
    body.userData.placeId = block.place_id;

    let grid: LineSegments | null = null;
    if (block.grid.length > 0) {
      const g = new BufferGeometry().setAttribute(
        "position",
        new Float32BufferAttribute(
          block.grid.flatMap(([p, q]) => [p[0], p[2], -p[1], q[0], q[2], -q[1]]),
          3,
        ),
      );
      grid = new LineSegments(g, this.materials.grid);
    }
    return { block, body, outline, grid };
  }

  /** Materials by what each place is now: chosen, under the pointer, or neither. */
  private style(): void {
    const m = this.materials;
    for (const d of this.drawn) {
      const chosen = d.block.place_id === this.chosen && d.block.target;
      const hovered = d.block.place_id === this.hovered && d.block.target;
      const solid = d.block.solid && d.block.height > 0;
      d.body.material = chosen ? (solid ? m.chosenSolid : m.chosenFloor) : solid ? m.solid : d.block.nesting === 0 ? m.outer : m.floor;
      d.outline.material = chosen ? m.chosenLine : hovered ? m.hover : solid ? m.line : d.block.nesting === 0 ? m.outerLine : m.floorLine;
      if (d.grid) d.grid.material = chosen ? m.chosenGrid : m.grid;
      // The chosen place is drawn last, so its outline is never under a
      // neighbour's, as on the plan.
      d.outline.renderOrder = chosen || hovered ? 1 : 0;
    }
  }

  private paint(): void {
    const p = this.palette;
    const m = this.materials;
    m.solid.color.copy(p.solid);
    m.floor.color.copy(p.floor);
    m.outer.color.copy(p.outer);
    m.chosenSolid.color.copy(p.chosen);
    m.chosenFloor.color.copy(p.chosen);
    m.line.color.copy(p.line);
    m.floorLine.color.copy(p.floorLine);
    m.outerLine.color.copy(p.outerLine);
    m.grid.color.copy(p.line);
    m.chosenLine.color.copy(p.chosenLine);
    m.chosenGrid.color.copy(p.chosenGrid);
    m.hover.color.copy(p.hover);
  }

  private clear(): void {
    for (const d of this.drawn) {
      d.body.geometry.dispose();
      d.outline.geometry.dispose();
      d.grid?.geometry.dispose();
    }
    this.world.clear();
    this.drawn = [];
  }

  // ── the view ───────────────────────────────────────────────────────────

  /**
   * Size the view to the pane. With `home`, also go back to looking at the
   * whole site from the front left; without, keep where the viewer has turned
   * to and only follow the pane's shape.
   */
  private fit(home: boolean, animate = false): void {
    const w = this.host.clientWidth;
    const h = this.host.clientHeight;
    if (w === 0 || h === 0) return;
    const aspect = w / h;
    const { min, max } = this.site;
    const centre = new Vector3((min[0] + max[0]) / 2, (min[2] + max[2]) / 2, -(min[1] + max[1]) / 2);
    const reach = Math.max(1, Math.hypot(max[0] - min[0], max[1] - min[1], max[2] - min[2]));

    // How big the site looks from the home view, so the whole of it fits.
    const dir = new Vector3(Math.sin(AZIMUTH) * Math.cos(ELEVATION), Math.sin(ELEVATION), Math.cos(AZIMUTH) * Math.cos(ELEVATION));
    const look = new OrthographicCamera();
    look.position.copy(centre).addScaledVector(dir, reach * 2);
    look.lookAt(centre);
    look.updateMatrixWorld();
    let halfW = 0.5;
    let halfH = 0.5;
    for (const x of [min[0], max[0]])
      for (const y of [min[1], max[1]])
        for (const z of [min[2], max[2]]) {
          const v = new Vector3(x, z, -y).applyMatrix4(look.matrixWorldInverse);
          halfW = Math.max(halfW, Math.abs(v.x));
          halfH = Math.max(halfH, Math.abs(v.y));
        }
    this.fitted = Math.max(halfH, halfW / aspect) * MARGIN;

    const cam = this.stage.camera;
    cam.top = this.fitted;
    cam.bottom = -this.fitted;
    cam.left = -this.fitted * aspect;
    cam.right = this.fitted * aspect;
    cam.near = 0.1;
    cam.far = reach * 6;
    // However far it is moved, the middle of the view stays over the site.
    const controls = this.stage.controls;
    controls.cursor.copy(centre);
    controls.maxTargetRadius = Math.max(1, Math.hypot(max[0] - min[0], max[1] - min[1]) / 2);
    cam.updateProjectionMatrix();
    if (home) {
      const to: Pose = { position: centre.clone().addScaledVector(dir, reach * 2), target: centre, zoom: 1 };
      if (animate && this.site.blocks.length > 0 && cam.position.lengthSq() > 0) this.stage.animate(to);
      else this.stage.pose(to);
    }
    this.stage.invalidate();
  }

  private zoomBy(factor: number): void {
    const { camera, controls } = this.stage;
    const zoom = Math.min(controls.maxZoom, Math.max(controls.minZoom, camera.zoom * factor));
    this.stage.animate({ position: camera.position.clone(), target: controls.target.clone(), zoom });
  }

  // ── the pointer ────────────────────────────────────────────────────────

  private placeAt(e: PointerEvent): string | null {
    const rect = this.stage.canvas.getBoundingClientRect();
    const ndc = new Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.stage.camera);
    const targets: Object3D[] = this.drawn.filter((d) => d.block.target).map((d) => d.body);
    const [hit] = this.raycaster.intersectObjects(targets, false);
    return (hit?.object.userData.placeId as string | undefined) ?? null;
  }

  private readonly onDown = (e: PointerEvent): void => {
    this.press = { x: e.clientX, y: e.clientY, at: performance.now() };
    this.stage.hold();
  };

  /** A middle press is a move here, not the browser's autoscroll. */
  private readonly onMiddle = (e: MouseEvent): void => {
    if (e.button === 1) e.preventDefault();
  };

  private readonly onUp = (e: PointerEvent): void => {
    const p = this.press;
    this.press = null;
    // Only the main button chooses; the others move the view.
    if (e.button !== 0) return;
    if (!p || Math.hypot(e.clientX - p.x, e.clientY - p.y) > 5 || performance.now() - p.at > 600) return;
    const id = this.placeAt(e);
    if (id) this.events.choose(id);
  };

  private readonly onMove = (e: PointerEvent): void => {
    if (e.buttons !== 0) return;
    const id = this.placeAt(e);
    this.hover(id);
    // Holding the key that moves the view says so before the drag starts.
    this.stage.canvas.style.cursor = e.ctrlKey || e.metaKey || e.shiftKey ? "move" : id ? "pointer" : "";
  };

  private readonly onLeave = (): void => {
    this.stage.canvas.style.cursor = "";
    this.hover(null);
  };

  private hover(id: string | null): void {
    if (id === this.hovered) return;
    this.hovered = id;
    this.style();
    this.stage.invalidate();
    this.events.hover(id);
  }

}

/** A shade per vertex from its face's normal, which the solid material multiplies by the colour. */
function shadeFaces(g: BufferGeometry): void {
  g.computeVertexNormals();
  const n = g.getAttribute("normal");
  const shades = new Float32Array(n.count * 3);
  for (let i = 0; i < n.count; i++) {
    const s = shade(n.getX(i), n.getY(i), n.getZ(i));
    shades[i * 3] = shades[i * 3 + 1] = shades[i * 3 + 2] = s;
  }
  g.setAttribute("color", new BufferAttribute(shades, 3));
}

