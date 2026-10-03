import {
  BoxGeometry,
  CanvasTexture,
  EdgesGeometry,
  Group,
  ImageLoader,
  LineBasicMaterial,
  LineSegments,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  PerspectiveCamera,
  SRGBColorSpace,
  Texture,
  Vector3,
  type Color,
} from "three";

import { Stage } from "@app/common/stage3d";
import { MATERIAL_ORDER, cover, faceAspect, type BoxFace } from "@app/items/box";

import type { Dims, Kind, Layer, Placement } from "./arrange";
import { tone } from "./tones";

/**
 * A suggested arrangement in 3D (D195): the box as an outline, and each thing
 * in it as a box made of its own photographs where its sides are cut (D176),
 * or plain in its line's colour with its code on it where they are not. Built
 * up to a layer, so the view follows the steps.
 *
 * Built on the shared [`Stage`]; the faces go on as the item page's box puts
 * them on ([`cover`], [`faceAspect`]), so a thing looks the same in both.
 */

/** Where the camera stands, from the box's middle: in front, to the right, above. */
const HOME: [number, number, number] = [0.8, 0.9, 1.2];
const DISTANCE = 6.5;
/** The longest side of a photograph on a thing, in pixels: there may be dozens. */
const TEXTURE_PX = 256;

/** Pack axes (along the length, across, up) as three.js axes: x, z, y. */
const AXIS = [new Vector3(1, 0, 0), new Vector3(0, 0, 1), new Vector3(0, 1, 0)] as const;

interface Dressed {
  geometry: BoxGeometry;
  edges: EdgesGeometry;
  materials: MeshBasicMaterial[];
}

export class PackScene {
  static supported = Stage.supported;

  private readonly stage: Stage<PerspectiveCamera>;
  private readonly loader = new ImageLoader();
  private readonly root = new Group();
  private readonly lineMaterial = new LineBasicMaterial();
  private readonly boxMaterial = new LineBasicMaterial();
  /** Outlines what is still to put into the carton being filled (D198). */
  private readonly toAddMaterial = new LineBasicMaterial();
  private outline: LineSegments | null = null;
  private dressed = new Map<Kind, Dressed>();
  private textures: Texture[] = [];
  private layered: { layer: number; objects: (Mesh | LineSegments)[] }[] = [];
  private size: Dims = [1, 1, 1];
  private layers: Layer[] = [];
  private shown = Infinity;
  private filling = false;
  private painting = 0;

  constructor(
    host: HTMLElement,
    private readonly imageUrl: (digest: string) => string,
  ) {
    this.stage = new Stage(host, new PerspectiveCamera(30, 1, 0.1, 100), {
      resized: (w, h) => {
        this.stage.camera.aspect = w / h;
        this.stage.camera.updateProjectionMatrix();
      },
      themed: () => this.build(),
    });
    this.stage.controls.enablePan = false;
    this.stage.controls.minDistance = 2;
    this.stage.controls.maxDistance = 14;
    this.stage.scene.add(this.root);
    this.stage.pose({ position: new Vector3(...HOME).normalize().multiplyScalar(DISTANCE), target: new Vector3(), zoom: 1 });
  }

  /** Draw this box and what goes in it. */
  set(size: Dims, layers: Layer[]): void {
    this.filling = layers.some((l) => l.placements.some((p) => p.packed !== undefined));
    this.size = size;
    this.layers = layers;
    this.build();
  }

  /** Show the layers below `count`, and nothing above. */
  upTo(count: number): void {
    this.shown = count;
    for (const { layer, objects } of this.layered) for (const o of objects) o.visible = layer < count;
    this.stage.invalidate();
  }

  dispose(): void {
    this.clear();
    this.lineMaterial.dispose();
    this.boxMaterial.dispose();
    this.toAddMaterial.dispose();
    this.stage.dispose();
  }

  private clear(): void {
    this.root.clear();
    this.outline?.geometry.dispose();
    this.outline = null;
    for (const d of this.dressed.values()) {
      d.geometry.dispose();
      d.edges.dispose();
      for (const m of d.materials) m.dispose();
    }
    this.dressed.clear();
    for (const t of this.textures) t.dispose();
    this.textures = [];
    this.layered = [];
  }

  private build(): void {
    this.clear();
    const painting = ++this.painting;
    const [L, W, H] = this.size;
    // The longest side is two units, so every box fills the view alike.
    const scale = 2 / Math.max(L, W, H);
    this.lineMaterial.color.copy(this.stage.token("--ui-border-strong"));
    this.boxMaterial.color.copy(this.stage.token("--ui-text-muted"));
    this.toAddMaterial.color.copy(this.stage.token("--ui-accent"));

    const box = new BoxGeometry(L * scale, H * scale, W * scale);
    this.outline = new LineSegments(new EdgesGeometry(box), this.boxMaterial);
    box.dispose();
    this.root.add(this.outline);

    this.layers.forEach((layer, index) => {
      const objects: (Mesh | LineSegments)[] = [];
      for (const p of layer.placements) {
        const d = this.dress(p.kind, scale, painting);
        const mesh = new Mesh(d.geometry, d.materials);
        const edges = new LineSegments(d.edges, this.filling && !p.packed ? this.toAddMaterial : this.lineMaterial);
        for (const o of [mesh, edges]) {
          o.quaternion.setFromRotationMatrix(rotation(p));
          o.position.set((p.x + p.dims[0] / 2 - L / 2) * scale, (p.z + p.dims[2] / 2 - H / 2) * scale, (p.y + p.dims[1] / 2 - W / 2) * scale);
          o.visible = index < this.shown;
          this.root.add(o);
          objects.push(o);
        }
      }
      this.layered.push({ layer: index, objects });
    });
    this.stage.invalidate();
  }

  /** A kind's shape and its six faces, made once however many of it there are. */
  private dress(kind: Kind, scale: number, painting: number): Dressed {
    const made = this.dressed.get(kind);
    if (made) return made;
    const [l, w, h] = kind.size;
    // Its own length across, its height up and its width deep, as the item page draws it.
    const geometry = new BoxGeometry(l * scale, h * scale, w * scale);
    const tint = this.stage.token(`--ui-${tone(kind.index)}`);
    const materials = MATERIAL_ORDER.map((face) => {
      const material = new MeshBasicMaterial({ map: this.blank(kind, face, tint) });
      const digest = kind.faces[face];
      if (digest) {
        this.loader.load(this.imageUrl(digest), (image) => {
          if (painting !== this.painting) return;
          const texture = new Texture(shrink(image, TEXTURE_PX));
          texture.colorSpace = SRGBColorSpace;
          const fit = cover(faceAspect(face, kind.size), image.width / image.height);
          texture.repeat.set(...fit.repeat);
          texture.offset.set(...fit.offset);
          texture.needsUpdate = true;
          this.textures.push(texture);
          material.map = texture;
          material.needsUpdate = true;
          this.stage.invalidate();
        });
      }
      return material;
    });
    const dressed = { geometry, edges: new EdgesGeometry(geometry), materials };
    this.dressed.set(kind, dressed);
    return dressed;
  }

  /** A plain face in its line's colour, with the code on the biggest faces. */
  private blank(kind: Kind, face: BoxFace, tint: Color): Texture {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 128;
    const g = canvas.getContext("2d")!;
    // The surface, then the line's colour over it: a material ignores a
    // texture's transparency, so a tint on nothing shows at full strength.
    g.fillStyle = `#${this.stage.token("--ui-surface").getHexString()}`;
    g.fillRect(0, 0, 128, 128);
    g.fillStyle = `#${tint.getHexString()}`;
    g.globalAlpha = 0.3;
    g.fillRect(0, 0, 128, 128);
    g.globalAlpha = 1;
    if (face === "top" || face === "front") {
      g.fillStyle = `#${this.stage.token("--ui-text").getHexString()}`;
      g.font = "600 16px system-ui, sans-serif";
      g.textAlign = "center";
      g.textBaseline = "middle";
      g.fillText(kind.item_code, 64, 64, 120);
    }
    const texture = new CanvasTexture(canvas);
    texture.colorSpace = SRGBColorSpace;
    this.textures.push(texture);
    return texture;
  }
}

/**
 * Turn a thing drawn the way the item page draws it (length along x, height
 * up, width toward the viewer) so that its sides run along the box's as
 * placed. A turn, never a mirror: when the axes swap an odd number of times one
 * of them is reversed, which a box does not show.
 */
function rotation(p: Placement): Matrix4 {
  // Where each of its own sides goes: the box's axis that side runs along.
  const to = (side: 0 | 1 | 2) => AXIS[p.axes.indexOf(side)]!.clone();
  const x = to(0);
  const y = to(2);
  const z = to(1);
  const m = new Matrix4().makeBasis(x, y, z);
  if (m.determinant() < 0) m.makeBasis(x, y, z.negate());
  return m;
}

/** The image itself when it is small enough, or a copy whose longest side is `px`. */
function shrink(image: HTMLImageElement, px: number): HTMLImageElement | HTMLCanvasElement {
  const scale = px / Math.max(image.width, image.height);
  if (scale >= 1) return image;
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(image.width * scale);
  canvas.height = Math.round(image.height * scale);
  canvas.getContext("2d")!.drawImage(image, 0, 0, canvas.width, canvas.height);
  return canvas;
}
