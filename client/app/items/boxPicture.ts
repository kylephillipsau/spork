import { anAct } from "@domain/acts";
import { api } from "@domain/api";
import type { CaptureSubject, ItemView, SubjectPhoto } from "@domain/types";
import { encodeWebp } from "@domain/webp";

import { handheld, loadPhoto } from "./crop";
import { photosOf, shown } from "./subjects";

/**
 * An item drawn as its box (D186): its front, right and top, each cut to its
 * face, drawn at an angle at the box's measured proportions, and kept as the
 * picture a list shows.
 *
 * **Drawn by a computer, from cuts a person checked.** Never on a phone,
 * which has neither the memory to spare nor the cuts. A drawing names the
 * three cuts it was drawn from, so a face cut again makes it out of date and
 * the next computer to see the item draws it again.
 */

/** The size it is drawn at, square. */
const SIDE = 512;
/** Room round the box. */
const MARGIN = 24;

/** The three faces a drawing is made of, in the order it names them. */
const DRAWN = ["front", "right", "top"] as const;

/** What to draw an item from: the subject whose front, right and top are all cut, carton first. */
export function boxFaces(item: Pick<ItemView, "photos" | "subjects">): { subject: CaptureSubject; faces: SubjectPhoto[] } | null {
  for (const subject of item.subjects) {
    if (subject.dimensions_absent) continue;
    const photos = photosOf(item, subject);
    const faces = DRAWN.map((f) => photos.get(f));
    if (faces.every((p): p is SubjectPhoto => p !== undefined && p.cut !== null)) return { subject, faces };
  }
  return null;
}

/** The cuts a drawing would be made from now, by content address. */
export const madeFrom = (faces: SubjectPhoto[]) => faces.map(shown);

/** Whether the item wants drawing: three cut faces, and no drawing of those cuts. */
export function wantsDrawing(item: ItemView): { subject: CaptureSubject; faces: SubjectPhoto[] } | null {
  const found = boxFaces(item);
  if (!found) return null;
  const now = madeFrom(found.faces);
  const drawn = item.box_picture?.made_from;
  return drawn && drawn.length === 3 && drawn.every((d, i) => d === now[i]) ? null : found;
}

type V = { x: number; y: number };
const add = (a: V, b: V): V => ({ x: a.x + b.x, y: a.y + b.y });

/**
 * The box's three edges on the page, from its lengths: its length runs down to
 * the right, its depth up to the right, its height straight down, each at the
 * angle an isometric drawing uses.
 */
export function axes(length: number, depth: number, height: number): { u: V; w: V; v: V } {
  const c = Math.cos(Math.PI / 6);
  const s = Math.sin(Math.PI / 6);
  return { u: { x: length * c, y: length * s }, w: { x: depth * c, y: -depth * s }, v: { x: 0, y: height } };
}

/** Draw the box from its three cut faces, as WebP. */
export async function drawBox(subject: CaptureSubject, faces: SubjectPhoto[]): Promise<Blob> {
  const [front, right, top] = await Promise.all(faces.map((p) => loadPhoto(shown(p))));
  // Its measured size when there is one; otherwise the cut faces' own shapes.
  const measured = subject.length_mm && subject.width_mm && subject.height_mm;
  const height = measured ? subject.height_mm! : 1;
  const length = measured ? subject.length_mm! : front!.naturalWidth / front!.naturalHeight;
  const depth = measured ? subject.width_mm! : right!.naturalWidth / right!.naturalHeight;
  const raw = axes(length, depth, height);
  // Every corner, to fit the drawing to the page.
  const corners = [{ x: 0, y: 0 }, raw.u, raw.w, add(raw.u, raw.w)].flatMap((p) => [p, add(p, raw.v)]);
  const xs = corners.map((p) => p.x);
  const ys = corners.map((p) => p.y);
  const scale = (SIDE - 2 * MARGIN) / Math.max(Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys));
  const at = (p: V): V => ({ x: p.x * scale, y: p.y * scale });
  const { u, w, v } = { u: at(raw.u), w: at(raw.w), v: at(raw.v) };
  const width = (Math.max(...xs) - Math.min(...xs)) * scale;
  const tall = (Math.max(...ys) - Math.min(...ys)) * scale;
  const o: V = { x: (SIDE - width) / 2 - Math.min(...xs) * scale, y: (SIDE - tall) / 2 - Math.min(...ys) * scale };

  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = SIDE;
  const g = canvas.getContext("2d")!;
  g.imageSmoothingQuality = "high";
  // A face: the image's x along one edge, its y along another, from a corner.
  const face = (image: HTMLImageElement, origin: V, across: V, down: V, shade: string | null) => {
    g.setTransform(across.x / image.naturalWidth, across.y / image.naturalWidth, down.x / image.naturalHeight, down.y / image.naturalHeight, origin.x, origin.y);
    g.drawImage(image, 0, 0);
    g.setTransform(1, 0, 0, 1, 0, 0);
    const path = new Path2D();
    path.moveTo(origin.x, origin.y);
    path.lineTo(origin.x + across.x, origin.y + across.y);
    path.lineTo(origin.x + across.x + down.x, origin.y + across.y + down.y);
    path.lineTo(origin.x + down.x, origin.y + down.y);
    path.closePath();
    if (shade) {
      g.fillStyle = shade;
      g.fill(path);
    }
    g.strokeStyle = "rgba(0, 0, 0, 0.35)";
    g.lineWidth = 1.5;
    g.stroke(path);
  };
  face(front!, o, u, v, null);
  // The right side in a little shadow, the top in a little light: a box, not three pictures.
  face(right!, add(o, u), w, v, "rgba(0, 0, 0, 0.12)");
  // The top's own top edge is the box's back edge.
  face(top!, add(o, w), u, { x: -w.x, y: -w.y }, "rgba(255, 255, 255, 0.06)");
  try {
    return await encodeWebp(canvas);
  } finally {
    canvas.width = canvas.height = 0;
  }
}

/**
 * Draw and keep the item's box when it wants one (D186). At a computer only.
 * True when a drawing was kept.
 */
export async function ensureBoxPicture(item: ItemView): Promise<boolean> {
  if (handheld()) return false;
  const wanted = wantsDrawing(item);
  if (!wanted) return false;
  const kept = await api.storeImage(await drawBox(wanted.subject, wanted.faces));
  await api.recordBoxPicture(item.item_id, { digest: kept.digest, made_from: madeFrom(wanted.faces), act: anAct() });
  return true;
}
