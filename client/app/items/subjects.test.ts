import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  bindable,
  cartonHolds,
  holdsInWords,
  isOwnCarton,
  nameOf,
  photosOf,
  readHoldsTyped,
  holdsOf,
  holdsTypedFrom,
  levelName,
  singleOffer,
  unitWord,
  sayFirst,
  shown,
  presentationNeeded,
  presentationOffered,
  subjectKey,
  weighable,
} from "./subjects.ts";
import type { CaptureSubject, SubjectPhoto } from "@domain/types";

/**
 * An item's subjects, in words: what each is called, which writes take it,
 * and which photographs are its own.
 */

const subject = (over: Partial<CaptureSubject>): CaptureSubject => ({
  item_id: "item-1",
  item_style_id: null,
  item_part_id: null,
  part_label: null,
  lot_id: null,
  lot_code: null,
  variant_lot_id: null,
  variant_code: null,
  code: "GLOVE-M",
  description: null,
  packaging_level: "each",
  parts: 0,
  gross_weight_g: null,
  length_mm: null,
  width_mm: null,
  height_mm: null,
  diameter_mm: null,
  base_diameter_mm: null,
  top_height_mm: null,
  weight_absent: false,
  dimensions_absent: false,
  packed_in: null,
  packed_in_name: null,
  packed_in_source: null,
  ships_as_is: false,
  ships_as_is_source: "default",
  upright: false,
  upright_source: "default",
  box_shaped: true,
  round: false,
  wrap: null,
  is_unit: false,
  offered: false,
  source: null,
  style_code: null,
  method: null,
  observed_at: null,
  faces: [],
  wants: [],
  demand: 0,
  because: "nothing",
  location_code: null,
  soh: 0,
  ...over,
});

const photo = (over: Partial<SubjectPhoto>): SubjectPhoto => ({
  item_id: "item-1",
  item_style_id: null,
  item_part_id: null,
  lot_id: null,
  packaging_level: "each",
  face: "front",
  image_id: "image-1",
  cut: null,
  same_as: null,
  digest: "d".repeat(64),
  captured_at: "2026-10-01T00:00:00Z",
  ...over,
});

test("a subject is called what it is: its level, its family's carton, or its part", () => {
  assert.equal(nameOf(subject({})), "Each");
  assert.equal(nameOf(subject({ packaging_level: "carton" })), "Carton");
  assert.equal(nameOf(subject({ item_id: null, item_style_id: "style-1", code: "STY-7720", packaging_level: "carton" })), "Carton of the STY-7720 family");
  assert.equal(nameOf(subject({ item_id: null, item_part_id: "part-1", part_label: "handle", packaging_level: null })), "Handle");
});

test("the scale weighs an item or a family at a level, and a label binds to an item's own", () => {
  const part = subject({ item_id: null, item_part_id: "part-1", packaging_level: null });
  const family = subject({ item_id: null, item_style_id: "style-1", packaging_level: "carton" });
  assert.equal(weighable(subject({})), true);
  assert.equal(weighable(family), true);
  assert.equal(weighable(part), false, "a part's weight goes with its size");
  assert.equal(bindable(subject({})), true);
  assert.equal(bindable(family), false, "a family has no barcode of its own");
  assert.equal(bindable(part), false);
});

test("an each needs its arrangement with a length; a part is offered it; a carton is not", () => {
  assert.equal(presentationNeeded(subject({})), true);
  assert.equal(presentationOffered(subject({ packaging_level: "carton" })), false);
  assert.equal(presentationOffered(subject({ item_id: null, item_part_id: "p", packaging_level: null })), true);
});

test("a subject's photographs are its own, never another level's or its family's", () => {
  const each = subject({});
  const photos = [
    photo({ face: "front" }),
    photo({ face: "top" }),
    photo({ face: "front", packaging_level: "carton" }),
    photo({ face: "back", item_id: null, item_style_id: "style-1", packaging_level: "each" }),
  ];
  const mine = photosOf({ photos }, each);
  assert.deepEqual([...mine.keys()].sort(), ["front", "top"]);
  assert.equal(subjectKey(each), subjectKey(photos[0]!), "a photograph names its subject the way a subject does");
  assert.equal(subjectKey(subject({ item_id: null, item_part_id: "p", packaging_level: null })), "p:part");
});

test("a photograph shows its face cut out once somebody has, and itself until then", () => {
  assert.equal(shown(photo({})), "d".repeat(64));
  assert.equal(shown(photo({ cut: { digest: "c".repeat(64), corners: [0, 0, 1, 0, 1, 1, 0, 1] } })), "c".repeat(64));
});

test("an item's own carton is said at the item; a family's is not", () => {
  assert.equal(isOwnCarton(subject({ packaging_level: "carton" })), true);
  assert.equal(isOwnCarton(subject({ packaging_level: "each" })), false);
  assert.equal(isOwnCarton(subject({ item_id: null, item_style_id: "sty-1", packaging_level: "carton" })), false);
});

test("a carton holds its packs times what is in each, or nobody has said", () => {
  assert.equal(cartonHolds({ units_per_inner: 1, inners_per_carton: 16 }), 16);
  assert.equal(cartonHolds({ units_per_inner: 50, inners_per_carton: 6 }), 300);
  assert.equal(cartonHolds({ units_per_inner: null, inners_per_carton: null }), null);
  assert.equal(cartonHolds(null), null);
  assert.equal(holdsInWords({ units_per_inner: 1, inners_per_carton: 16 }), "16 × each");
  assert.equal(holdsInWords({ units_per_inner: 50, inners_per_carton: 6 }), "6 packs of 50 (300 × each)");
  assert.equal(holdsInWords({ units_per_inner: null, inners_per_carton: null }), "Not said yet");
  assert.equal(holdsInWords(null), "No carton on file yet");
  assert.equal(holdsInWords({ units_per_inner: 100, inners_per_carton: 10 }, "box"), "10 boxes of 100 (1,000 × each)", "in what it is sold as (D218)");
});

const by = (by: "all" | "packs", count: string, per = "", inPairs = false) => ({ by, count, per, in: inPairs ? 2 : 1 });

test("a count typed is the whole carton's, or so many packs of so many, or nothing said (D185, D233)", () => {
  const brush = { unit: { level: "each" as const, said: false, netsuite_unit: "Each", netsuite_level: "each" as const, singles: 1 }, packing: null };
  assert.deepEqual(readHoldsTyped(by("all", " 16 "), brush), { holds: 16, per: null });
  assert.deepEqual(readHoldsTyped(by("all", ""), brush), { holds: null, per: null });
  assert.deepEqual(readHoldsTyped(by("packs", "6", "24"), brush), { holds: 6, per: 24 }, "six packs of 24");
  for (const wrong of ["0", "1.5", "a dozen"]) assert.ok("problem" in readHoldsTyped(by("all", wrong), brush), wrong);
});

test("a pair is two single ones, counted either way (D233)", () => {
  const gloves = { unit: { level: "inner" as const, said: false, netsuite_unit: "Pair", netsuite_level: "each" as const, singles: 2 }, packing: null };
  assert.deepEqual(readHoldsTyped(by("all", "140"), gloves), { holds: 70, per: 2 }, "140 gloves is 70 pairs");
  assert.deepEqual(readHoldsTyped(by("all", "70", "", true), gloves), { holds: 70, per: 2 }, "and so is 70 pairs");
  assert.ok("problem" in readHoldsTyped(by("all", "141"), gloves), "no whole number of pairs");
  assert.deepEqual(readHoldsTyped(by("packs", "10", "12", true), gloves), { holds: 10, per: 24 }, "ten bags of twelve pairs");
  const packed = { ...gloves, packing: { units_per_inner: 2, inners_per_carton: 70, effective_from: "2026-10-01" } };
  assert.equal(holdsOf(packed), "70 pairs (140 single)");
  assert.deepEqual(holdsTypedFrom(packed), by("all", "70", "", true), "on file, in pairs");
  assert.equal(levelName("inner", packed), "Pair");
  assert.equal(levelName("each", packed), "Single one");
  assert.equal(levelName("carton", packed), "Carton of 70 pairs");
  const bagged = {
    unit: { level: "each" as const, said: false, netsuite_unit: "Pair", netsuite_level: "each" as const, singles: 2 },
    packing: { units_per_inner: 24, inners_per_carton: 10, effective_from: "2026-10-01" },
  };
  assert.equal(holdsOf(bagged), "10 packs of 12 pairs (240 single)");
  assert.deepEqual(holdsTypedFrom(bagged), by("packs", "10", "12", true));
  assert.equal(levelName("inner", bagged), "Pack of 24 (12 pairs)");
});

test("the carton is said first when none is on file, or a different count is typed", () => {
  const sixteen = { units_per_inner: 1, inners_per_carton: 16 };
  assert.equal(sayFirst(null, null), true, "the writer refuses a carton with no case pack");
  assert.equal(sayFirst(null, 16), true);
  assert.equal(sayFirst(sixteen, null), false, "nothing typed changes nothing");
  assert.equal(sayFirst(sixteen, 16), false, "the same count is no act");
  assert.equal(sayFirst(sixteen, 12), true);
  assert.equal(sayFirst({ units_per_inner: null, inners_per_carton: null }, 6), true, "an unsaid count, said");
  assert.equal(sayFirst({ units_per_inner: 24, inners_per_carton: 6 }, 6, 24), false, "six packs of 24, as on file");
  assert.equal(sayFirst(sixteen, 16, 24), true, "loose sixteen is not sixteen packs");
});

test("an item's levels are named from what it is sold as (D218)", () => {
  const plugs = { unit: { level: "inner" as const, said: false, netsuite_unit: "Box", netsuite_level: "inner" as const, singles: null }, packing: { units_per_inner: 100, inners_per_carton: 10, effective_from: "2026-10-01" } };
  assert.equal(levelName("inner", plugs), "Box of 100");
  assert.equal(levelName("carton", plugs), "Carton of 10 boxes");
  assert.equal(levelName("each", plugs), "Single item");
  const gloves = { unit: { level: "carton" as const, said: false, netsuite_unit: "CTN", netsuite_level: "carton" as const, singles: null }, packing: { units_per_inner: 1, inners_per_carton: 1000, effective_from: "2026-10-01" } };
  assert.equal(unitWord(gloves), "Carton", "a CTN is a carton");
  assert.equal(levelName("carton", gloves), "Carton of 1,000");
  assert.equal(levelName("each", gloves), "Single item");
  const brush = { unit: { level: "each" as const, said: false, netsuite_unit: "Each", netsuite_level: "each" as const, singles: 1 }, packing: { units_per_inner: 1, inners_per_carton: 16, effective_from: "2026-10-01" } };
  assert.equal(levelName("each", brush), "Each");
  assert.equal(levelName("carton", brush), "Carton of 16");
  const boots = { unit: { level: "each" as const, said: false, netsuite_unit: "Pair", netsuite_level: "each" as const, singles: 2 }, packing: { units_per_inner: 24, inners_per_carton: 6, effective_from: "2026-10-01" } };
  assert.equal(levelName("each", boots), "Single one", "a pair is two of it (D233)");
  assert.equal(levelName("inner", boots), "Pack of 24 (12 pairs)");
  assert.equal(levelName("carton", boots), "Carton of 6 packs (72 pairs)");
  const book = { unit: { level: "each" as const, said: false, netsuite_unit: null, netsuite_level: null, singles: 1 }, packing: null };
  assert.equal(levelName("each", book), "Each");
  assert.equal(levelName("carton", book), "Carton", "a carton nobody has counted");
});

test("NetSuite's word names the level it means, whatever Spork says it is sold as (D239)", () => {
  const packing = (per: number, inners: number) => ({ units_per_inner: per, inners_per_carton: inners, effective_from: "2026-10-01" });
  // NetSuite says "Roll"; it is sold by the carton of eight.
  const tissue = { unit: { level: "carton" as const, said: true, netsuite_unit: "Roll", netsuite_level: "each" as const, singles: null }, packing: packing(1, 8) };
  assert.equal(levelName("carton", tissue), "Carton of 8 rolls");
  assert.equal(levelName("each", tissue), "Roll");
  assert.equal(holdsOf(tissue), "8 rolls");
  assert.equal(singleOffer(tissue), "Measure a single roll");
  // NetSuite says "Each"; it is sold in its own carton of one.
  const stand = { unit: { level: "carton" as const, said: true, netsuite_unit: "Each", netsuite_level: "each" as const, singles: null }, packing: packing(1, 1) };
  assert.equal(levelName("carton", stand), "Carton");
  assert.equal(levelName("each", stand), "Single item", "an each names nothing");
  assert.equal(singleOffer(stand), "Measure a single one from the carton");
});

test("a round carton is called what it is packed in (D240)", () => {
  const bucket = {
    unit: { level: "carton" as const, said: true, netsuite_unit: "Each", netsuite_level: "each" as const, singles: null },
    packing: { units_per_inner: 1, inners_per_carton: 400, effective_from: "2026-10-01" },
    subjects: [subject({ packaging_level: "carton", round: true, box_shaped: false, packed_in: "BJ", packed_in_name: "Bucket" })],
  };
  assert.equal(levelName("carton", bucket), "Bucket of 400");
  assert.equal(singleOffer(bucket), "Measure a single one from the bucket");
  const boxed = { ...bucket, subjects: [subject({ packaging_level: "carton", packed_in: "BX", packed_in_name: "Box" })] };
  assert.equal(levelName("carton", boxed), "Carton of 400", "a box is the carton");
});

test("a carton of an item sold by the box is counted in boxes (D218)", () => {
  const plugs = {
    unit: { level: "inner" as const, said: false, netsuite_unit: "Box", netsuite_level: "inner" as const, singles: null },
    packing: { units_per_inner: 100, inners_per_carton: null, effective_from: "2026-10-01" },
  };
  assert.deepEqual(readHoldsTyped(by("packs", "10"), plugs), { holds: 10, per: 100 }, "ten boxes, each as it was");
  assert.deepEqual(readHoldsTyped(by("packs", "10", "50"), plugs), { holds: 10, per: 50 });
  assert.ok("problem" in readHoldsTyped(by("packs", "ten"), plugs));
  assert.deepEqual(readHoldsTyped(by("all", "1000"), plugs), { holds: 10, per: 100 }, "a thousand earplugs, in its boxes of 100");
});
