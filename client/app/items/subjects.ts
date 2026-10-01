import type { CaptureSubject, ItemView, SubjectPhoto } from "@domain/types";

import type { Figures } from "./figures.ts";

/**
 * What gets measured for an item, in words, with no React in it so a test
 * runner can read it.
 *
 * An item's properties are recorded against **subjects**, and an item has
 * several: its carton, its each, its family's carton when the family is what
 * gets measured (D108), and its parts when it is a set with no box of its own
 * (D139). Each is the pair `POST /observations` takes, and each is shown and
 * edited on its own.
 */

/** The seven faces, in the order somebody walks round the box; `label` last
 *  because it is the one that is not a geometric face. */
export const FACES = ["front", "back", "left", "right", "top", "bottom", "label"] as const;
export type Face = (typeof FACES)[number];

/**
 * The seven arrangements, likeliest first. D138: a shipped vocabulary, so this
 * list and `presentation` in the database are the same seven words.
 */
export const PRESENTATIONS = [
  { value: "as_supplied", label: "As supplied" },
  { value: "folded", label: "Folded" },
  { value: "flat", label: "Laid flat" },
  { value: "rolled", label: "Rolled" },
  { value: "assembled", label: "Assembled" },
  { value: "knocked_down", label: "Knocked down" },
  { value: "compressed", label: "Compressed" },
] as const;

export const NO_FIGURES: Figures = {
  weight: "",
  length: "",
  width: "",
  height: "",
  presentation: "",
  noDimensions: false,
};

/**
 * Whether this subject's lengths need the arrangement recorded with them.
 *
 * **The writer's rule exactly.** `POST /observations` requires a presentation
 * for a length at `each` and nowhere else; a client asking for more would
 * refuse writes the server accepts.
 */
export function presentationNeeded(subject: CaptureSubject): boolean {
  return subject.packaging_level === "each";
}

/** Whether to offer it at all: never for a carton, which has one arrangement. */
export function presentationOffered(subject: CaptureSubject): boolean {
  return presentationNeeded(subject) || subject.item_part_id !== null;
}

/** A subject's identity: whose it is and at what level. A part has no level. */
export function subjectKey(s: Pick<CaptureSubject, "item_id" | "item_style_id" | "item_part_id" | "packaging_level">): string {
  return `${s.item_id ?? s.item_style_id ?? s.item_part_id}:${s.packaging_level ?? "part"}`;
}

/** What to call it: "Carton", "Each", "Carton of the STY-7720 family", or the part's own name. */
export function nameOf(s: CaptureSubject): string {
  if (s.item_part_id) return s.part_label ? sentence(s.part_label) : "Part";
  const level = sentence(s.packaging_level ?? "item");
  return s.item_style_id ? `${level} of the ${s.code} family` : level;
}

/** Its newest photograph of each face, its own only. */
/** The picture to show for a photograph: its face cut out, once somebody has (D176). */
export function shown(photo: Pick<SubjectPhoto, "digest" | "cut">): string {
  return photo.cut?.digest ?? photo.digest;
}

export function photosOf(item: Pick<ItemView, "photos">, s: CaptureSubject): Map<string, SubjectPhoto> {
  const key = subjectKey(s);
  return new Map(item.photos.filter((p) => subjectKey(p) === key).map((p) => [p.face, p]));
}

/**
 * Whether the scale's own write takes it. `POST /weighings` weighs an item or
 * a family at a level; a part's weight is recorded with its size instead.
 */
export function weighable(s: CaptureSubject): boolean {
  return s.item_part_id === null && s.packaging_level !== null;
}

/** Whether a label can be bound to it (D164): an item of its own, at a level. */
export function bindable(s: CaptureSubject): boolean {
  return s.item_id !== null && s.packaging_level !== null;
}

/** The figures on file, as a form starts from: nothing typed yet. */
export function wanted(s: CaptureSubject): { weight: boolean; size: boolean; photos: boolean } {
  return {
    weight: s.wants.includes("weight"),
    size: s.wants.includes("dimensions"),
    photos: s.wants.includes("photographs"),
  };
}

function sentence(word: string): string {
  const w = word.replace(/_/g, " ");
  return w.charAt(0).toUpperCase() + w.slice(1);
}
