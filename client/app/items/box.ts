import type { CaptureSubject } from "@domain/types";

import { FACES, type Face } from "./subjects.ts";

/**
 * An item as a box: its six faces, which way each faces, and how a photograph
 * of one sits on it. No three.js here, so a test runner can read it.
 *
 * **A box unless it said otherwise.** Anything is one until somebody says it is
 * packed in something without six sides, shrink-wrap or a bag (D191), or that
 * it has no size (D138). Then it is a thing with a photo, and what else is
 * worth taking, rather than six sides.
 */

/** A box's six sides: every face a photo can be of, but its label. */
export type BoxFace = Exclude<Face, "label" | "detail">;
export const BOX_FACES: readonly BoxFace[] = FACES.filter((f) => f !== "label") as BoxFace[];

/** Whether to treat it as a box: the server says, from what it is packed in. */
export function isBox(subject: Pick<CaptureSubject, "box_shaped">): boolean {
  return subject.box_shaped;
}

/** A thing that is not a box: its photo, then its back, label and a close-up, any of them skipped. */
export const THING_FACES: readonly Face[] = ["front", "back", "label", "detail"];

/**
 * What to photograph: a box's six sides and its label, or a thing's photo and
 * what else is worth taking. `sides` asks a thing for every side as well.
 */
export function facesToAsk(subject: Pick<CaptureSubject, "box_shaped">, sides = false): readonly Face[] {
  return isBox(subject) ? FACES : sides ? [...FACES, "detail"] : THING_FACES;
}

/** A face's name on screen: a thing that is not a box has a photo, not a front. */
export function faceName(face: string, subject: Pick<CaptureSubject, "box_shaped">): string {
  if (!isBox(subject) && face === "front") return "Photo";
  if (face === "detail") return "Close-up";
  return face.charAt(0).toUpperCase() + face.slice(1);
}

/** Length, width and height in millimetres, or nothing until all three are known. */
export function boxSize(s: Pick<CaptureSubject, "length_mm" | "width_mm" | "height_mm">): [number, number, number] | null {
  return s.length_mm && s.width_mm && s.height_mm ? [s.length_mm, s.width_mm, s.height_mm] : null;
}

/**
 * A face's width over its height. The length runs across the front and back,
 * the width across the sides, and the top is length by width.
 */
export function faceAspect(face: BoxFace, [l, w, h]: [number, number, number]): number {
  switch (face) {
    case "front":
    case "back":
      return l / h;
    case "left":
    case "right":
      return w / h;
    case "top":
    case "bottom":
      return l / w;
  }
}

/**
 * How a photograph covers a face without stretching: the share of it shown
 * across and up, and where that share starts, centred. A photo wider than the
 * face loses its sides; a taller one, its top and bottom.
 */
export function cover(faceAspectRatio: number, imageAspectRatio: number): { repeat: [number, number]; offset: [number, number] } {
  if (imageAspectRatio > faceAspectRatio) {
    const x = faceAspectRatio / imageAspectRatio;
    return { repeat: [x, 1], offset: [(1 - x) / 2, 0] };
  }
  const y = imageAspectRatio / faceAspectRatio;
  return { repeat: [1, y], offset: [0, (1 - y) / 2] };
}

/**
 * A face's width over its height from what was measured, or nothing: a label
 * has no measured size, and nor does a box nobody has measured. A face cut
 * from a photograph is straightened to this when there is one (D176).
 */
export function measuredAspect(
  subject: Pick<CaptureSubject, "box_shaped" | "length_mm" | "width_mm" | "height_mm">,
  face: string,
): number | null {
  const size = boxSize(subject);
  if (!isBox(subject) || !size || !(BOX_FACES as readonly string[]).includes(face)) return null;
  return faceAspect(face as BoxFace, size);
}

/** The order three.js's box takes its six materials in: +x, −x, +y, −y, +z, −z. */
export const MATERIAL_ORDER: readonly BoxFace[] = ["right", "left", "top", "bottom", "front", "back"];

/**
 * Where to look from to show a face, as a direction from the box's middle: in
 * front of it and a little off square, so it fills the view and still reads as
 * a box. The sides are seen from a little above and toward the front, the top
 * and bottom from toward the front only. The front faces the viewer (+z), the
 * top up (+y).
 */
const VIEW: Record<BoxFace, [number, number, number]> = {
  front: [0.35, 0.3, 1],
  back: [-0.35, 0.3, -1],
  right: [1, 0.3, 0.35],
  left: [-1, 0.3, 0.35],
  top: [0, 1, 0.5],
  bottom: [0, -1, 0.5],
};

export function toward(face: BoxFace): [number, number, number] {
  return VIEW[face];
}
