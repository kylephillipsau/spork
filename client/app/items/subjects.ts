import type { CaptureSubject, ItemPacking, ItemView, SubjectPhoto } from "@domain/types";

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

/** The seven faces, in the order somebody photographs a box: round it from the
 *  front, turning it right, then its top and bottom; `label` last because it is
 *  the one that is not a geometric face. */
export const FACES = ["front", "right", "back", "left", "top", "bottom", "label"] as const;
/** A side, or its label; `detail` is a close-up that is not a side (D140, D191). */
export type Face = (typeof FACES)[number] | "detail";

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
  top: "",
  base: "",
  topHeight: "",
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

/** A subject's identity: whose it is and at what level. A part and a run have no level. */
export function subjectKey(
  s: Pick<CaptureSubject, "item_id" | "item_style_id" | "item_part_id" | "lot_id" | "packaging_level">,
): string {
  if (s.lot_id) return `${s.lot_id}:lot`;
  return `${s.item_id ?? s.item_style_id ?? s.item_part_id}:${s.packaging_level ?? "part"}`;
}

/**
 * What to call it: the item's own levels in what it is sold as (D218), "Box
 * of 100", "Carton of 10 boxes", "Single item"; a family's "Carton of the
 * STY-7720 family"; a part by its name; or "Variant: O/N 123".
 */
export function nameOf(s: CaptureSubject, item?: Pick<ItemView, "item_id" | "unit" | "packing">): string {
  if (s.lot_id) return `Variant: ${s.lot_code ?? "unnamed"}`;
  if (s.item_part_id) return s.part_label ? sentence(s.part_label) : "Part";
  const level = s.packaging_level;
  if (item && !s.item_style_id && s.item_id === item.item_id && (level === "each" || level === "inner" || level === "carton")) {
    return levelName(level, item);
  }
  if (level === "inner" && !s.item_style_id) return "Inner pack";
  const named = sentence(level ?? "item");
  return s.item_style_id ? `${named} of the ${s.code} family` : named;
}

/**
 * What one of it is called in NetSuite (D218): its Pack Unit as a word, a
 * CTN being a carton; or, with none, its level's name.
 */
export function unitWord(item: Pick<ItemView, "unit">): string {
  const said = item.unit.netsuite_unit?.trim();
  if (said) {
    const u = said.toUpperCase();
    if (u === "CTN" || u === "CS" || u === "CASE") return "Carton";
    if (u === "UNT") return "Unit";
    return sentence(said.toLowerCase());
  }
  return { each: "Each", inner: "Pack", carton: "Carton" }[item.unit.level];
}

/** A word for so many of a thing: "boxes", "packs", "each". */
function plural(word: string, n: number): string {
  const w = word.toLowerCase();
  if (n === 1 || w === "each") return w;
  return /(x|s|sh|ch)$/.test(w) ? `${w}es` : `${w}s`;
}

/**
 * One of the item's own levels, named from what it is sold as (D218): the
 * unit by NetSuite's word and what it holds, "Box of 100"; a carton by how
 * many of that it holds, "Carton of 10 boxes"; a pack by its count; and the
 * single product inside a pack or carton it is sold as, "Single item".
 */
export function levelName(level: "each" | "inner" | "carton", item: Pick<ItemView, "unit" | "packing">): string {
  const unit = item.unit.level;
  const word = unitWord(item);
  const per = item.packing?.units_per_inner ?? null;
  const inners = item.packing?.inners_per_carton ?? null;
  const count = (n: number) => n.toLocaleString();
  if (level === unit) {
    if (unit === "inner" && per && per > 1) return `${word} of ${count(per)}`;
    if (unit === "carton" && inners) {
      const n = inners * (per ?? 1);
      return n > 1 ? `${word} of ${count(n)}` : word;
    }
    return word;
  }
  if (level === "carton") {
    if (!inners) return "Carton";
    if (unit === "inner") return `Carton of ${count(inners)} ${plural(word, inners)}`;
    return per && per > 1 ? `Carton of ${count(inners)} packs` : `Carton of ${count(inners)}`;
  }
  if (level === "inner") return per && per > 1 ? `Pack of ${count(per)}` : "Pack";
  return "Single item";
}

/** Its newest photograph of each face, its own only. */
/** The picture to show for a photograph: its face cut out, once somebody has (D176). */
export function shown(photo: Pick<SubjectPhoto, "digest" | "cut">): string {
  return photo.cut?.digest ?? photo.digest;
}

export function photosOf(item: Pick<ItemView, "photos">, s: CaptureSubject): Map<string, SubjectPhoto> {
  const own = subjectKey(s);
  // A carton standing in for by a variant shows the variant's, until it has its own (D184).
  const key =
    s.variant_lot_id && !item.photos.some((p) => subjectKey(p) === own) ? `${s.variant_lot_id}:lot` : own;
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

/**
 * Whether this is the item's own carton: a box of so many of the item, whose
 * count is said at the item (D178). A family's carton is said by its family.
 */
export function isOwnCarton(s: Pick<CaptureSubject, "item_id" | "packaging_level">): boolean {
  return s.item_id !== null && s.packaging_level === "carton";
}

type Counts = Pick<ItemPacking, "units_per_inner" | "inners_per_carton">;

/** How many of the item a carton holds, by the case pack in force; null when nobody has said. */
export function cartonHolds(p: Counts | null): number | null {
  if (!p || p.inners_per_carton === null) return null;
  return p.inners_per_carton * (p.units_per_inner ?? 1);
}

/** What a carton holds, in words: "16 × each", "6 packs of 50 (300 × each)", or that nobody has said. */
export function holdsInWords(p: Counts | null, pack = "pack"): string {
  if (!p) return "No carton on file yet";
  const n = p.inners_per_carton;
  const u = p.units_per_inner;
  if (n === null) return "Not said yet";
  if (u === 1) return `${n.toLocaleString()} × each`;
  const packs = plural(pack, n);
  if (u === null) return `${n} ${packs}, how many in each not said`;
  return `${n} ${packs} of ${u} (${(n * u).toLocaleString()} × each)`;
}

/**
 * How many a carton holds, as typed: the whole carton's count of the item, a
 * whole number from 1, or nothing said; and, when it holds packs, how many are
 * in each, so the packs are the count over that (D185). 1,000 in packs of 50
 * is 20 packs: the count is what a person reads off the carton's label.
 */
export function readHolds(typed: string, perTyped = ""): { holds: number | null; per: number | null } | { problem: string } {
  const t = typed.trim();
  const p = perTyped.trim();
  const whole = (v: string) => /^\d+$/.test(v) && Number(v) >= 1;
  if (t && !whole(t)) return { problem: "How many it holds is a whole number, 1 or more." };
  if (p && !whole(p)) return { problem: "How many are in a pack is a whole number, 1 or more." };
  if (p && !t) return { problem: "Say how many are in it altogether, as well as how many in each pack." };
  if (!p) return { holds: t ? Number(t) : null, per: null };
  const [total, per] = [Number(t), Number(p)];
  if (total % per !== 0) {
    return { problem: `${total.toLocaleString()} isn’t a whole number of packs of ${per.toLocaleString()}.` };
  }
  return { holds: total / per, per };
}

/**
 * A carton's count as typed for an item sold by the pack (D218): how many
 * packs are in the carton, and, when said, how many are in a pack. Left blank,
 * a pack holds what it held. Any other item's carton is typed as the whole
 * carton's count ([`readHolds`]).
 */
export function readHoldsFor(
  unit: "each" | "inner" | "carton",
  typed: string,
  perTyped: string,
  packing: Pick<ItemPacking, "units_per_inner"> | null,
): { holds: number | null; per: number | null } | { problem: string } {
  if (unit !== "inner") return readHolds(typed, perTyped);
  const t = typed.trim();
  const p = perTyped.trim();
  const whole = (v: string) => /^\d+$/.test(v) && Number(v) >= 1;
  if (t && !whole(t)) return { problem: "How many packs it holds is a whole number, 1 or more." };
  if (p && !whole(p)) return { problem: "How many are in a pack is a whole number, 1 or more." };
  return { holds: t ? Number(t) : null, per: p ? Number(p) : (packing?.units_per_inner ?? null) };
}

/**
 * Whether the carton has to be said before it is weighed, measured or
 * photographed: there is no carton on file (the writer refuses one, D23), or
 * a count was typed that is not the one on file.
 */
export function sayFirst(packing: Counts | null, typed: number | null, per: number | null = null): boolean {
  if (!packing) return true;
  if (typed === null) return false;
  return typed !== packing.inners_per_carton || (per ?? 1) !== (packing.units_per_inner ?? 1);
}
