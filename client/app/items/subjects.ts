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
  // Two single ones is a pair, whatever NetSuite's word (D233).
  if (singlesOf(item) === 2) return "Pair";
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
export function plural(word: string, n: number): string {
  const w = word.toLowerCase();
  if (n === 1 || w === "each") return w;
  return /(x|s|sh|ch)$/.test(w) ? `${w}es` : `${w}s`;
}

/**
 * How many single ones one of what NetSuite counts is, where it is counted in
 * them (D233): two for a pair, packed as one or not; one otherwise.
 */
export function singlesOf(item: Pick<ItemView, "unit">): number {
  return Math.max(item.unit.singles ?? 1, 1);
}

/** "pairs", for an item sold by the pair; null for one counted in single ones. */
function severalWord(item: Pick<ItemView, "unit">): string | null {
  return singlesOf(item) > 1 ? plural(unitWord(item), 2) : null;
}

/** "12 pairs": so many single ones in what NetSuite counts, where that is several; null where it isn't whole. */
function inUnits(item: Pick<ItemView, "unit">, singles: number): string | null {
  const k = singlesOf(item);
  if (k <= 1 || singles % k !== 0) return null;
  return `${(singles / k).toLocaleString()} ${plural(unitWord(item), singles / k)}`;
}

/**
 * One of the item's own levels, named from what it is sold as (D218): the
 * unit by NetSuite's word and what it holds, "Box of 100"; a carton by how
 * many of that it holds, "Carton of 10 boxes"; a pack by its count; and the
 * single product inside a pack or carton it is sold as, "Single item". A pair
 * is two single ones (D233): its pack, when it is packed as one, is "Pair",
 * and the glove or boot inside it "Single one".
 */
export function levelName(level: "each" | "inner" | "carton", item: Pick<ItemView, "unit" | "packing">): string {
  const unit = item.unit.level;
  const word = unitWord(item);
  const per = item.packing?.units_per_inner ?? null;
  const inners = item.packing?.inners_per_carton ?? null;
  const count = (n: number) => n.toLocaleString();
  const several = singlesOf(item) > 1;
  if (several) {
    if (level === "each") return "Single one";
    if (level === unit) return word;
    if (level === "inner") {
      const pairs = per ? inUnits(item, per) : null;
      return per && per > 1 ? `Pack of ${count(per)}${pairs ? ` (${pairs})` : ""}` : "Pack";
    }
  }
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
    const pairs = several ? inUnits(item, inners * (per ?? 1)) : null;
    const said = per && per > 1 ? `Carton of ${count(inners)} packs` : `Carton of ${count(inners)}`;
    return pairs ? `${said} (${pairs})` : said;
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

/**
 * Whether this is the item's own pack: so many of it in a pack or inner box,
 * said at the item (D185, D234). A family's or a variant's is not.
 */
export function isOwnPack(s: Pick<CaptureSubject, "item_id" | "packaging_level" | "lot_id" | "item_style_id">): boolean {
  return s.item_id !== null && s.packaging_level === "inner" && !s.lot_id && !s.item_style_id;
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
  if (u === null) return `${n.toLocaleString()} ${packs}, how many in each not said`;
  return `${n.toLocaleString()} ${packs} of ${u.toLocaleString()} (${(n * u).toLocaleString()} × each)`;
}

/**
 * What its carton holds, in words, counted as the item is sold (D218, D233):
 * "16 × each", "10 boxes of 100 (1,000 × each)", "70 pairs (140 single)",
 * "10 packs of 12 pairs (240 single)".
 */
export function holdsOf(item: Pick<ItemView, "unit" | "packing">): string {
  const p = item.packing;
  const several = severalWord(item);
  if (!several || !p || p.inners_per_carton === null) {
    return holdsInWords(p, item.unit.level === "inner" ? unitWord(item) : "pack");
  }
  const n = p.inners_per_carton;
  const u = p.units_per_inner;
  // Packed as pairs, the carton is so many of them.
  if (item.unit.level === "inner") return `${n.toLocaleString()} ${plural(unitWord(item), n)}${u ? ` (${(n * u).toLocaleString()} single)` : ""}`;
  const all = n * (u ?? 1);
  const pairs = inUnits(item, all);
  if (u && u > 1) {
    const each = inUnits(item, u);
    return `${n.toLocaleString()} ${plural("pack", n)} of ${each ?? `${u} single`} (${all.toLocaleString()} single)`;
  }
  return pairs ? `${pairs} (${all.toLocaleString()} single)` : `${all.toLocaleString()} single`;
}

/** What a pack of it holds, in words: "12 × each", "12 pairs (24 single)"; null when nobody has said. */
export function packHolds(item: Pick<ItemView, "unit" | "packing">): string | null {
  const u = item.packing?.units_per_inner ?? null;
  if (u === null || u <= 1) return null;
  const pairs = inUnits(item, u);
  return pairs ? `${pairs} (${u.toLocaleString()} single)` : `${u.toLocaleString()} × each`;
}

/**
 * What a carton holds, as somebody types it (D185, D233): how many are in it
 * altogether, or so many packs of so many; each count in single ones or, for
 * an item sold by the pair, in pairs. A carton of 70 pairs is typed as 140
 * single or 70 pairs; ten bags of twelve pairs, as 10 packs of 12 pairs.
 */
export interface HoldsTyped {
  by: "all" | "packs";
  /** How many in it altogether, or how many packs. */
  count: string;
  /** How many in a pack, by packs, and on a pack's own card. */
  per: string;
  /** What the counts are counted in, as so many single ones: 1, or 2 for pairs. */
  in: number;
}

export const NO_HOLDS: HoldsTyped = { by: "all", count: "", per: "", in: 1 };

/**
 * The item's carton as it is on file, to keep or correct: by packs where it
 * has packs of its own (a box sold as one, ten bags of twelve pairs), else
 * altogether, counted in pairs where it is sold by the pair and that is whole.
 */
export function holdsTypedFrom(item: Pick<ItemView, "unit" | "packing">): HoldsTyped {
  const k = singlesOf(item);
  const per = item.packing?.units_per_inner ?? null;
  const n = item.packing?.inners_per_carton ?? null;
  const counted = (singles: number) => (k > 1 && singles % k === 0 ? { v: singles / k, in: k } : { v: singles, in: 1 });
  const boxSold = item.unit.level === "inner" && k <= 1;
  // A pack of its own: not loose, and not the pair itself.
  if (boxSold || (per !== null && per > 1 && per !== k)) {
    const each = per === null ? null : counted(per);
    return { by: "packs", count: n === null ? "" : String(n), per: each ? String(each.v) : "", in: each?.in ?? k };
  }
  if (n === null) return { ...NO_HOLDS, in: k };
  const all = counted(n * (per ?? 1));
  return { by: "all", count: String(all.v), per: "", in: all.in };
}

type Read = { holds: number | null; per: number | null } | { problem: string };

const whole = (v: string) => /^\d+$/.test(v) && Number(v) >= 1;

/**
 * The case pack's two counts from what was typed: how many packs a carton
 * holds and how many single ones are in a pack, either null where nothing is
 * said. Altogether, a pair item's pairs are its packs (D233), a box sold as
 * one is counted in its boxes of what is on file, and anything else is loose.
 */
export function readHoldsTyped(typed: HoldsTyped, item: Pick<ItemView, "unit" | "packing">): Read {
  const k = singlesOf(item);
  const t = typed.count.trim();
  const p = typed.per.trim();
  const counted = typed.in > 1 ? plural(unitWord(item), 2) : "single ones";
  if (t && !whole(t)) return { problem: "How many it holds is a whole number, 1 or more." };
  if (typed.by === "packs") {
    if (p && !whole(p)) return { problem: "How many are in a pack is a whole number, 1 or more." };
    const per = p ? Number(p) * typed.in : null;
    return { holds: t ? Number(t) : null, per: per ?? (item.unit.level === "inner" ? (item.packing?.units_per_inner ?? null) : null) };
  }
  if (!t) return { holds: null, per: null };
  const all = Number(t) * typed.in;
  if (k > 1) {
    if (all % k !== 0) return { problem: `${all.toLocaleString()} single isn’t a whole number of ${plural(unitWord(item), 2)}.` };
    return { holds: all / k, per: k };
  }
  if (item.unit.level === "inner") {
    const per = item.packing?.units_per_inner ?? null;
    if (per === null) return { problem: `Say how many ${counted} each ${unitWord(item).toLowerCase()} holds: count it in packs.` };
    if (all % per !== 0) return { problem: `${all.toLocaleString()} isn’t a whole number of ${plural(unitWord(item), 2)} of ${per}.` };
    return { holds: all / per, per };
  }
  return { holds: all, per: null };
}

/**
 * What a pack of it holds, as typed on the pack's own card (D234): so many
 * single ones, or pairs; a pair packed as one holds its two. Null where
 * nothing is typed.
 */
export function readPackTyped(typed: HoldsTyped, item: Pick<ItemView, "unit" | "packing">): number | null | { problem: string } {
  const k = singlesOf(item);
  if (item.unit.level === "inner" && k > 1) return k;
  const p = typed.per.trim();
  if (!p) return null;
  if (!whole(p)) return { problem: "How many are in a pack is a whole number, 1 or more." };
  return Number(p) * typed.in;
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
