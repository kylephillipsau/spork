import {
  BufferGeometry,
  Float32BufferAttribute,
  FrontSide,
  ImageLoader,
  Mesh,
  MeshBasicMaterial,
  OrthographicCamera,
  PerspectiveCamera,
  SRGBColorSpace,
  Scene,
  Texture,
  Vector3,
  WebGLRenderer,
  type Color,
} from "three";

import type { Wrap } from "@domain/types";
import { Stage } from "@app/common/stage3d";

import { profile, slant, type RoundSize } from "./round";

/**
 * A round thing as three.js draws it (D240): its side, tapering and then
 * straight under the rim, with its photographs wrapped round it, and its lid
 * and base as discs. **One shape** for its page, its list picture and the
 * packing plan, so it looks the same in all three.
 *
 * Millimetres times `scale`, the front (the middle of the side's picture)
 * facing +z, standing on its base at the origin or, `centred`, about its
 * middle as a box is.
 */

/** Around it, in this many flat strips: smooth at any size it is shown. */
const AROUND = 96;

/**
 * The parts of its geometry, in the order its materials are given: an open
 * thing (D241) has an inside wall and a floor where a lidded one has a lid.
 */
export const PARTS = ["side", "lid", "base", "inside", "floor"] as const;
export type Part = (typeof PARTS)[number];

/** Whether a wrapping is of an open thing: its inside, not a lid, was made (D241). */
export const isOpen = (wrap: Wrap | null) => wrap?.inside != null;

/**
 * Its surface: the side, then the lid and the base as discs, each a group of
 * its own for its own picture; `open`, no lid but its inside wall and floor.
 * Every part faces out of the solid it bounds, and is drawn from that side
 * only, so the inside and the outside of an open thing's wall show their own.
 */
export function roundGeometry(size: RoundSize, scale = 1, centred = false, open = false): BufferGeometry {
  const rings = profile(size);
  const total = slant(rings);
  const lift = centred ? -size.height / 2 : 0;
  const positions: number[] = [], uvs: number[] = [], index: number[] = [];
  const at = (x: number, y: number, z: number) => positions.push(x * scale, (y + lift) * scale, z * scale);
  // The side: a ring at each turn of its outline, the picture's top row the rim.
  let up = 0;
  rings.forEach((ring, k) => {
    if (k > 0) up += Math.hypot(ring.r - rings[k - 1]!.r, ring.y - rings[k - 1]!.y);
    for (let i = 0; i <= AROUND; i++) {
      const u = i / AROUND, theta = 2 * Math.PI * u - Math.PI;
      at(ring.r * Math.sin(theta), ring.y, ring.r * Math.cos(theta));
      uvs.push(u, up / total);
    }
  });
  // The wall, facing out; and again facing in, for an open thing's inside.
  const wall = (inward: boolean) => {
    for (let k = 0; k < rings.length - 1; k++) {
      for (let i = 0; i < AROUND; i++) {
        const a = k * (AROUND + 1) + i, b = a + 1, c = a + AROUND + 1, d = c + 1;
        index.push(...(inward ? [a, c, b, b, c, d] : [a, b, c, b, d, c]));
      }
    }
  };
  // A disc at a height, facing up or down: its middle, then its rim. Looking
  // down on it, the picture's bottom edge is toward the front, as a lid is
  // photographed by somebody in front of it; looking up at a base, its top
  // edge is, as a base is photographed once the thing is tipped over onto
  // its top, front to back.
  const disc = (r: number, y: number, up: boolean) => {
    const middle = positions.length / 3;
    at(0, y, 0);
    uvs.push(0.5, 0.5);
    for (let i = 0; i <= AROUND; i++) {
      const theta = (2 * Math.PI * i) / AROUND;
      const x = r * Math.sin(theta), z = r * Math.cos(theta);
      at(x, y, z);
      uvs.push(0.5 + x / (2 * r), up ? 0.5 - z / (2 * r) : 0.5 + z / (2 * r));
    }
    for (let i = 1; i <= AROUND; i++) index.push(...(up ? [middle, middle + i, middle + i + 1] : [middle, middle + i + 1, middle + i]));
  };
  const geometry = new BufferGeometry();
  // Each part a group, its material by its place in `PARTS`.
  const part = (which: Part, draw: () => void) => {
    const from = index.length;
    draw();
    geometry.addGroup(from, index.length - from, PARTS.indexOf(which));
  };
  const top = rings[rings.length - 1]!;
  part("side", () => wall(false));
  if (!open) part("lid", () => disc(top.r, top.y, true));
  part("base", () => disc(rings[0]!.r, 0, false));
  if (open) {
    part("inside", () => wall(true));
    part("floor", () => disc(rings[0]!.r, 0, true));
  }
  geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
  geometry.setAttribute("uv", new Float32BufferAttribute(uvs, 2));
  geometry.setIndex(index);
  geometry.computeVertexNormals();
  return geometry;
}

/** The digest of each part's picture in a wrapping, where there is one. */
export function pictures(wrap: Wrap | null): Record<Part, string | null> {
  return {
    side: wrap?.side ?? null,
    lid: wrap?.lid ?? null,
    base: wrap?.base ?? null,
    inside: wrap?.inside ?? null,
    floor: wrap?.floor ?? null,
  };
}

/**
 * Its three materials, plain in `tint` until each part's picture has loaded:
 * then `loaded` is told, to draw again. The pictures are kept as made, at
 * their own size: a side is a few thousand pixels round and reads as sharp
 * as the photographs it came from.
 */
export function roundMaterials(
  wrap: Wrap | null,
  imageUrl: (digest: string) => string,
  tint: Color | number,
  loaded: (texture: Texture) => void,
): MeshBasicMaterial[] {
  const loader = new ImageLoader();
  const digests = pictures(wrap);
  return PARTS.map((part) => {
    const material = new MeshBasicMaterial({ color: tint, side: FrontSide });
    const digest = digests[part];
    if (digest) {
      loader.load(imageUrl(digest), (image) => {
        const texture = new Texture(image);
        texture.colorSpace = SRGBColorSpace;
        texture.anisotropy = 8;
        texture.needsUpdate = true;
        material.map = texture;
        material.color.set(0xffffff);
        material.needsUpdate = true;
        loaded(texture);
      });
    }
    return material;
  });
}

/** Where the camera stands, from its middle: in front, a little to the right and above. */
const HOME: [number, number, number] = [0.35, 0.45, 1];
const DISTANCE = 7;

/**
 * A round thing turned by dragging, on its page (D240): the item page's box
 * view (`BoxScene`), for a tub.
 */
export class RoundScene {
  static supported = Stage.supported;

  private readonly stage: Stage<PerspectiveCamera>;
  private mesh: Mesh | null = null;
  private textures: Texture[] = [];
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
      themed: () => this.stage.invalidate(),
    });
    this.stage.controls.enablePan = false;
    this.stage.controls.enableZoom = false;
    this.stage.pose({ position: new Vector3(...HOME).normalize().multiplyScalar(DISTANCE), target: new Vector3(), zoom: 1 });
  }

  /** Draw this shape with this wrapping, or plain. */
  set(size: RoundSize, wrap: Wrap | null): void {
    this.clear();
    const painting = ++this.painting;
    // Its height or width, whichever is more, two units, so every one fills the view alike.
    const scale = 2 / Math.max(size.top, size.base, size.height);
    const materials = roundMaterials(wrap, this.imageUrl, this.stage.token("--ui-surface-sunken"), (texture) => {
      if (painting !== this.painting) return texture.dispose();
      this.textures.push(texture);
      this.stage.invalidate();
    });
    this.mesh = new Mesh(roundGeometry(size, scale, true, isOpen(wrap)), materials);
    this.stage.scene.add(this.mesh);
    this.stage.invalidate();
  }

  dispose(): void {
    this.clear();
    this.stage.dispose();
  }

  private clear(): void {
    if (!this.mesh) return;
    this.stage.scene.remove(this.mesh);
    this.mesh.geometry.dispose();
    for (const m of this.mesh.material as MeshBasicMaterial[]) m.dispose();
    for (const t of this.textures) t.dispose();
    this.textures = [];
    this.mesh = null;
  }
}

/**
 * Its picture for lists (D186, D240): drawn from in front, a little to the
 * right and above, square on in its proportions as a box's drawing is, on
 * nothing, `side` pixels square. The wrapping's pictures are given loaded;
 * given an inside, it is drawn open (D241).
 */
export function drawRound(size: RoundSize, images: Partial<Record<Part, HTMLImageElement | HTMLCanvasElement>>, side = 512): HTMLCanvasElement {
  const renderer = new WebGLRenderer({ antialias: true, alpha: true, preserveDrawingBuffer: true });
  try {
    renderer.setSize(side, side);
    renderer.outputColorSpace = SRGBColorSpace;
    const scene = new Scene();
    const textures: Texture[] = [];
    const materials = PARTS.map((part) => {
      const image = images[part];
      if (!image) return new MeshBasicMaterial({ color: 0xe8e8e8, side: FrontSide });
      const texture = new Texture(image);
      texture.colorSpace = SRGBColorSpace;
      texture.anisotropy = 8;
      texture.needsUpdate = true;
      textures.push(texture);
      return new MeshBasicMaterial({ map: texture, side: FrontSide });
    });
    const geometry = roundGeometry(size, 1, true, images.inside !== undefined);
    const mesh = new Mesh(geometry, materials);
    // Turned so its front faces the camera, which stands off to its right.
    mesh.rotation.y = Math.PI / 4;
    scene.add(mesh);
    const reach = Math.max(size.top, size.height) * 0.78;
    const camera = new OrthographicCamera(-reach, reach, reach, -reach, 1, 100_000);
    camera.position.copy(new Vector3(1, 0.8, 1).normalize().multiplyScalar(10_000));
    camera.lookAt(0, 0, 0);
    renderer.render(scene, camera);
    // A copy that outlives the renderer, which is let go now.
    const out = document.createElement("canvas");
    out.width = out.height = side;
    out.getContext("2d")!.drawImage(renderer.domElement, 0, 0);
    geometry.dispose();
    for (const m of materials) m.dispose();
    for (const t of textures) t.dispose();
    return out;
  } finally {
    renderer.dispose();
  }
}
