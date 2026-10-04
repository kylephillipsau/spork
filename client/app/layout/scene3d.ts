import {
  BoxGeometry,
  BufferAttribute,
  BufferGeometry,
  CircleGeometry,
  Color,
  DoubleSide,
  EdgesGeometry,
  ExtrudeGeometry,
  Float32BufferAttribute,
  Group,
  InstancedMesh,
  Line,
  LineBasicMaterial,
  LineDashedMaterial,
  LineLoop,
  LineSegments,
  MOUSE,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  OrthographicCamera,
  Quaternion,
  Raycaster,
  Shape,
  ShapeGeometry,
  Vector2,
  Vector3,
  type Object3D,
} from "three";

import { Stage, type Pose } from "@app/common/stage3d";

import type { BinCell, Block, Point3, Scene3D } from "./blocks";
import { MIX, toneOf, type Layer, type Tone } from "./layers";

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
  /** A bin was clicked, or the empty floor was, which chooses none (D208). */
  chooseBin?: ((locationId: string | null) => void) | undefined;
  /** The pointer is over a bin, or over none. */
  hoverBin?: ((locationId: string | null) => void) | undefined;
}

/** How far above level a flight to a bin looks from, and how far round from straight on. */
const FLY_ELEVATION = (24 * Math.PI) / 180;
const FLY_ASIDE = (16 * Math.PI) / 180;
/** How much of the site, in cells either side of the bin, a flight ends showing. */
const FLY_SPAN = 6;

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
    bin: new MeshBasicMaterial({ vertexColors: true, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 }),
    ghost: new MeshBasicMaterial({ vertexColors: true, transparent: true, opacity: 0.13, depthWrite: false }),
    route: new MeshBasicMaterial({ side: DoubleSide, polygonOffset: true, polygonOffsetFactor: -2, polygonOffsetUnits: -2 }),
  };

  // ── the bins (D208) ──────────────────────────────────────────────────
  private cells: BinCell[] = [];
  private binIndex = new Map<string, BinCell>();
  /** Places whose bins are drawn: their block steps back to an outline. */
  private withBins = new Set<string>();
  /**
   * The bins as drawn: one solid group, and, while a bin is chosen, a faint
   * one of every other rack's, so the racks across the aisle don't hide it.
   */
  private groups: { mesh: InstancedMesh; cells: BinCell[] }[] = [];
  /** The place whose bins are in front, while a bin is chosen there. */
  private focus: string | null = null;
  /** The walk drawn on the floor, and where it starts (D211). */
  private walk: Mesh[] = [];
  private layer: Layer = "stock";
  private chosenBin: string | null = null;
  private hoveredBin: string | null = null;
  private tones = {} as Record<Tone, Color>;
  private readonly chosenMark = new LineSegments(new EdgesGeometry(new BoxGeometry(1, 1, 1)), this.materials.chosenLine);
  private readonly hoverMark = new LineSegments(new EdgesGeometry(new BoxGeometry(1, 1, 1)), this.materials.hover);

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
    this.chosenMark.visible = this.hoverMark.visible = false;
    this.chosenMark.renderOrder = this.hoverMark.renderOrder = 2;
    this.stage.scene.add(this.chosenMark, this.hoverMark);

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

  /**
   * Draw these bins in their cells (D208), replacing any drawn before. The
   * racks they sit on step back to their outlines, so the bins are what is seen.
   */
  setBins(cells: BinCell[]): void {
    this.cells = cells;
    this.binIndex = new Map(cells.map((c) => [c.bin.location_id, c]));
    this.withBins = new Set(cells.map((c) => c.bin.place_id));
    this.focus = this.chosenBin ? (this.binIndex.get(this.chosenBin)?.bin.place_id ?? null) : null;
    this.build();
    this.style();
    this.mark();
    this.stage.invalidate();
  }

  /** The groups for the bins and the focus as they are. */
  private build(): void {
    this.dropGroups();
    const solid = this.focus ? this.cells.filter((c) => c.bin.place_id === this.focus) : this.cells;
    const faint = this.focus ? this.cells.filter((c) => c.bin.place_id !== this.focus) : [];
    for (const [cells, material] of [
      [solid, this.materials.bin],
      [faint, this.materials.ghost],
    ] as const) {
      if (cells.length === 0) continue;
      const box = new BoxGeometry(1, 1, 1);
      shadeFaces(box);
      const mesh = new InstancedMesh(box, material, cells.length);
      const m = new Matrix4();
      cells.forEach((c, i) => mesh.setMatrixAt(i, cellMatrix(c, 1, m)));
      mesh.instanceMatrix.needsUpdate = true;
      mesh.computeBoundingSphere();
      // The faint ones after the solid, so they are laid over it, not under.
      mesh.renderOrder = material === this.materials.ghost ? 1 : 0;
      this.groups.push({ mesh, cells });
      this.world.add(mesh);
    }
    this.colourBins();
  }

  /**
   * Draw a walk on the floor (D211): a ribbon along its path, and a disc
   * where it starts. Nothing, to take it away.
   */
  setRoute(path: [number, number][] | null): void {
    for (const m of this.walk) {
      this.stage.scene.remove(m);
      m.geometry.dispose();
    }
    this.walk = [];
    if (path && path.length > 1) {
      const { min, max } = this.site;
      // A ribbon a fraction of the site wide, so it reads at any size.
      const width = Math.max(0.12, Math.hypot(max[0] - min[0], max[1] - min[1]) / 250);
      const lift = 0.05;
      const positions: number[] = [];
      for (let k = 0; k + 1 < path.length; k++) {
        const [ax, ay] = path[k]!;
        const [bx, by] = path[k + 1]!;
        const len = Math.hypot(bx - ax, by - ay) || 1;
        const [nx, ny] = [(-(by - ay) / len) * (width / 2), ((bx - ax) / len) * (width / 2)];
        // Extended by half a width at each end, so the bends close.
        const [ex, ey] = [((bx - ax) / len) * (width / 2), ((by - ay) / len) * (width / 2)];
        const c = [
          [ax - ex + nx, ay - ey + ny],
          [bx + ex + nx, by + ey + ny],
          [bx + ex - nx, by + ey - ny],
          [ax - ex - nx, ay - ey - ny],
        ] as const;
        for (const i of [0, 1, 2, 0, 2, 3]) positions.push(c[i]![0], lift, -c[i]![1]);
      }
      const ribbon = new BufferGeometry().setAttribute("position", new Float32BufferAttribute(positions, 3));
      const start = new CircleGeometry(width * 2.2, 24);
      start.rotateX(-Math.PI / 2);
      start.translate(path[0]![0], lift, -path[0]![1]);
      this.walk = [new Mesh(ribbon, this.materials.route), new Mesh(start, this.materials.route)];
      for (const m of this.walk) {
        m.renderOrder = 3;
        this.stage.scene.add(m);
      }
    }
    this.stage.invalidate();
  }

  /** Colour the bins by this layer. */
  setLayer(layer: Layer): void {
    if (layer === this.layer) return;
    this.layer = layer;
    this.colourBins();
    this.stage.invalidate();
  }

  /** Mark this bin as the chosen one, or none. */
  chooseBin(locationId: string | null): void {
    if (locationId === this.chosenBin) return;
    this.chosenBin = locationId;
    const focus = locationId ? (this.binIndex.get(locationId)?.bin.place_id ?? null) : null;
    if (focus !== this.focus) {
      this.focus = focus;
      this.build();
    } else {
      this.colourBins();
    }
    this.mark();
    this.stage.invalidate();
  }

  /**
   * Fly to a bin: face it from the aisle it opens onto, a little above and a
   * little to one side, close enough to read the bins around it.
   */
  flyTo(locationId: string): void {
    const cell = this.binIndex.get(locationId);
    if (!cell) return;
    const target = new Vector3(cell.centre[0], cell.centre[2], -cell.centre[1]);
    const [fx, fy] = cell.facing;
    const side = Math.atan2(-fy, fx) + FLY_ASIDE;
    const dir = new Vector3(Math.cos(side) * Math.cos(FLY_ELEVATION), Math.sin(FLY_ELEVATION), Math.sin(side) * Math.cos(FLY_ELEVATION));
    const { min, max } = this.site;
    const reach = Math.max(1, Math.hypot(max[0] - min[0], max[1] - min[1], max[2] - min[2]));
    const controls = this.stage.controls;
    const zoom = Math.min(controls.maxZoom, Math.max(controls.minZoom, this.fitted / FLY_SPAN));
    this.stage.animate({ position: target.clone().addScaledVector(dir, reach * 2), target, zoom }, 700);
  }

  /** The top of a bin's cell, for a label pinned to it. */
  binTop(locationId: string): Point3 | null {
    const cell = this.binIndex.get(locationId);
    return cell ? [cell.centre[0], cell.centre[1], cell.centre[2] + cell.size[2] / 2] : null;
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
    this.dropGroups();
    this.setRoute(null);
    for (const mark of [this.chosenMark, this.hoverMark]) mark.geometry.dispose();
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
      // A place whose bins are drawn is seen as its bins and its outline.
      const binned = this.withBins.has(d.block.place_id);
      d.body.visible = !binned;
      if (d.grid) d.grid.visible = !binned;
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
    m.route.color.copy(p.chosen);
    const surface = this.stage.token("--ui-surface");
    for (const [tone, { token, share }] of Object.entries(MIX) as [Tone, { token: string; share: number }][]) {
      this.tones[tone] = mix(surface, this.stage.token(token), share);
    }
    this.colourBins();
  }

  /** Each bin in its layer's tone, and the chosen one in the accent. */
  private colourBins(): void {
    const c = new Color();
    for (const { mesh, cells } of this.groups) {
      cells.forEach((cell, i) => {
        c.copy(cell.bin.location_id === this.chosenBin ? this.palette.chosen : this.tones[toneOf(cell.bin, this.layer)]);
        mesh.setColorAt(i, c);
      });
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    }
  }

  /** The outlines round the chosen bin and the one under the pointer. */
  private mark(): void {
    for (const [mark, id] of [
      [this.chosenMark, this.chosenBin],
      [this.hoverMark, this.hoveredBin === this.chosenBin ? null : this.hoveredBin],
    ] as const) {
      const cell = id === null ? undefined : this.binIndex.get(id);
      mark.visible = !!cell;
      if (cell) {
        cellMatrix(cell, 1.12, mark.matrix);
        mark.matrixAutoUpdate = false;
        mark.matrixWorldNeedsUpdate = true;
      }
    }
  }

  private dropGroups(): void {
    for (const { mesh } of this.groups) {
      this.world.remove(mesh);
      mesh.geometry.dispose();
      mesh.dispose();
    }
    this.groups = [];
  }

  private clear(): void {
    for (const d of this.drawn) {
      d.body.geometry.dispose();
      d.outline.geometry.dispose();
      d.grid?.geometry.dispose();
    }
    this.world.clear();
    for (const { mesh } of this.groups) this.world.add(mesh);
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

  private aim(e: PointerEvent): void {
    const rect = this.stage.canvas.getBoundingClientRect();
    const ndc = new Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.stage.camera);
  }

  private placeAt(e: PointerEvent): string | null {
    this.aim(e);
    const targets: Object3D[] = this.drawn.filter((d) => d.block.target && d.body.visible).map((d) => d.body);
    const [hit] = this.raycaster.intersectObjects(targets, false);
    return (hit?.object.userData.placeId as string | undefined) ?? null;
  }

  /** The bin under the pointer, when bins are drawn. */
  private binAt(e: PointerEvent): string | null {
    if (this.groups.length === 0) return null;
    this.aim(e);
    // The solid group first: a faint bin in front doesn't take the pointer
    // from the rack in focus behind it.
    for (const { mesh, cells } of this.groups) {
      const [hit] = this.raycaster.intersectObject(mesh, false);
      const cell = hit?.instanceId === undefined ? undefined : cells[hit.instanceId];
      if (cell) return cell.bin.location_id;
    }
    return null;
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
    if (this.events.chooseBin) {
      this.events.chooseBin(this.binAt(e));
      return;
    }
    const id = this.placeAt(e);
    if (id) this.events.choose(id);
  };

  private readonly onMove = (e: PointerEvent): void => {
    if (e.buttons !== 0) return;
    const bin = this.binAt(e);
    this.hoverOnBin(bin);
    const id = bin ? null : this.placeAt(e);
    this.hover(id);
    // Holding the key that moves the view says so before the drag starts.
    this.stage.canvas.style.cursor = e.ctrlKey || e.metaKey || e.shiftKey ? "move" : bin || id ? "pointer" : "";
  };

  private readonly onLeave = (): void => {
    this.stage.canvas.style.cursor = "";
    this.hover(null);
    this.hoverOnBin(null);
  };

  private hoverOnBin(id: string | null): void {
    if (id === this.hoveredBin) return;
    this.hoveredBin = id;
    this.mark();
    this.stage.invalidate();
    this.events.hoverBin?.(id);
  }

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


/** A cell's box as a matrix, grown by `grow` (an outline sits just outside it). */
function cellMatrix(c: BinCell, grow: number, out: Matrix4): Matrix4 {
  return out.compose(
    new Vector3(c.centre[0], c.centre[2], -c.centre[1]),
    new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), c.angle),
    new Vector3(c.size[0] * grow, c.size[2] * grow, c.size[1] * grow),
  );
}

/** `share` of `token` mixed into `surface`, as CSS `color-mix` in sRGB does it. */
function mix(surface: Color, token: Color, share: number): Color {
  const a = surface.clone().convertLinearToSRGB();
  const b = token.clone().convertLinearToSRGB();
  return a.lerp(b, share).convertSRGBToLinear();
}
