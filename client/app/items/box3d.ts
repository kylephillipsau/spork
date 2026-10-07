import {
  BoxGeometry,
  CanvasTexture,
  EdgesGeometry,
  ImageLoader,
  LineBasicMaterial,
  LineSegments,
  Mesh,
  MeshBasicMaterial,
  PerspectiveCamera,
  SRGBColorSpace,
  Texture,
  Vector3,
} from "three";

import { Stage } from "@app/common/stage3d";

import { MATERIAL_ORDER, cover, faceAspect, toward, type BoxFace } from "./box";

/**
 * An item as a box made of its own photographs, turned by dragging. Sized by
 * what was measured, or a cube until then. A side nobody has photographed is a
 * plain face with its name on it, so what is missing shows where it is missing.
 *
 * Built on the shared [`Stage`]: this keeps only the box.
 */

/** Where the camera stands, from the box's middle: in front, to the right, a little above. */
const HOME: [number, number, number] = [0.62, 0.42, 1];
const DISTANCE = 7;

/**
 * The longest side of a photo on the box, in pixels. The box is a few hundred
 * pixels across, and six full-size photos from a phone's camera would ask a
 * phone for hundreds of megabytes of graphics memory.
 */
const TEXTURE_PX = 1024;

export class BoxScene {
  static supported = Stage.supported;

  private readonly stage: Stage<PerspectiveCamera>;
  private readonly loader = new ImageLoader();
  private mesh: Mesh | null = null;
  private edges: LineSegments | null = null;
  private readonly materials = MATERIAL_ORDER.map(() => new MeshBasicMaterial());
  private readonly edgeMaterial = new LineBasicMaterial();
  private textures: Texture[] = [];
  private painting = 0;
  private faces: Partial<Record<BoxFace, string>> = {};
  private size: [number, number, number] = [1, 1, 1];

  constructor(
    host: HTMLElement,
    private readonly imageUrl: (digest: string) => string,
  ) {
    this.stage = new Stage(host, new PerspectiveCamera(30, 1, 0.1, 100), {
      resized: (w, h) => {
        this.stage.camera.aspect = w / h;
        this.stage.camera.updateProjectionMatrix();
      },
      themed: () => this.paint(),
    });
    const controls = this.stage.controls;
    controls.enablePan = false;
    controls.enableZoom = false;
    this.stage.pose({ position: new Vector3(...HOME).normalize().multiplyScalar(DISTANCE), target: new Vector3(), zoom: 1 });
  }

  /** Draw this box: its photographs by face, and its size in millimetres, or a cube. */
  set(faces: Partial<Record<BoxFace, string>>, size: [number, number, number] | null): void {
    this.faces = faces;
    this.size = size ?? [1, 1, 1];
    // The longest side is two units, so every box fills the view alike.
    const [l, w, h] = this.size;
    const scale = 2 / Math.max(l, w, h);
    this.mesh?.geometry.dispose();
    this.edges?.geometry.dispose();
    if (this.mesh) this.stage.scene.remove(this.mesh);
    if (this.edges) this.stage.scene.remove(this.edges);
    const geometry = new BoxGeometry(l * scale, h * scale, w * scale);
    this.mesh = new Mesh(geometry, this.materials);
    this.edges = new LineSegments(new EdgesGeometry(geometry), this.edgeMaterial);
    this.stage.scene.add(this.mesh, this.edges);
    this.paint();
  }

  /** Turn the box to show this face square on. */
  show(face: BoxFace): void {
    const position = new Vector3(...toward(face)).normalize().multiplyScalar(DISTANCE);
    this.stage.animate({ position, target: new Vector3(), zoom: 1 }, 480);
  }

  dispose(): void {
    this.mesh?.geometry.dispose();
    this.edges?.geometry.dispose();
    for (const t of this.textures) t.dispose();
    for (const m of this.materials) m.dispose();
    this.edgeMaterial.dispose();
    this.stage.dispose();
  }

  /**
   * Each face's picture, or its name on a plain face; colours from the theme.
   * A face shows its name until its photo arrives, and keeps it if the photo
   * never does, rather than going black.
   */
  private paint(): void {
    for (const t of this.textures) t.dispose();
    this.textures = [];
    const painting = ++this.painting;
    this.edgeMaterial.color.copy(this.stage.token("--ui-border-strong"));
    MATERIAL_ORDER.forEach((face, i) => {
      const material = this.materials[i]!;
      this.dress(material, this.blank(face));
      const digest = this.faces[face];
      if (!digest) return;
      this.loader.load(this.imageUrl(digest), (image) => {
        // Painted again since (other photos, or the theme): this one is stale.
        if (painting !== this.painting) return;
        this.dress(material, faceTexture(image, face, this.size, TEXTURE_PX));
      });
    });
  }

  private dress(material: MeshBasicMaterial, texture: Texture): void {
    this.textures.push(texture);
    material.map = texture;
    material.needsUpdate = true;
    this.stage.invalidate();
  }

  private blank(face: BoxFace): Texture {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 256;
    const g = canvas.getContext("2d")!;
    g.fillStyle = `#${this.stage.token("--ui-surface-sunken").getHexString()}`;
    g.fillRect(0, 0, 256, 256);
    g.fillStyle = `#${this.stage.token("--ui-text-faint").getHexString()}`;
    g.font = "600 30px system-ui, sans-serif";
    g.textAlign = "center";
    g.textBaseline = "middle";
    g.fillText(face.charAt(0).toUpperCase() + face.slice(1), 128, 128);
    const texture = new CanvasTexture(canvas);
    texture.colorSpace = SRGBColorSpace;
    return texture;
  }
}

/**
 * A photograph as one face of a box this size, its longest side `px` at most,
 * covering the face without stretching. The one way a photo goes on a box, so
 * a thing looks the same on its page and in the packing plan.
 */
export function faceTexture(image: HTMLImageElement, face: BoxFace, size: [number, number, number], px: number): Texture {
  const texture = new Texture(shrink(image, px));
  texture.colorSpace = SRGBColorSpace;
  const fit = cover(faceAspect(face, size), image.width / image.height);
  texture.repeat.set(...fit.repeat);
  texture.offset.set(...fit.offset);
  texture.needsUpdate = true;
  return texture;
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
