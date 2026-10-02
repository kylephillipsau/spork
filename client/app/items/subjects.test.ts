import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  bindable,
  cartonHolds,
  holdsInWords,
  isOwnCarton,
  nameOf,
  photosOf,
  readHolds,
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
  code: "GLOVE-M",
  description: null,
  packaging_level: "each",
  parts: 0,
  gross_weight_g: null,
  length_mm: null,
  width_mm: null,
  height_mm: null,
  weight_absent: false,
  dimensions_absent: false,
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
});

test("a count typed is a whole number from one, or nothing said", () => {
  assert.deepEqual(readHolds(" 16 "), { holds: 16 });
  assert.deepEqual(readHolds(""), { holds: null });
  assert.ok("problem" in readHolds("0"));
  assert.ok("problem" in readHolds("1.5"));
  assert.ok("problem" in readHolds("a dozen"));
});

test("the carton is said first when none is on file, or a different count is typed", () => {
  const sixteen = { units_per_inner: 1, inners_per_carton: 16 };
  assert.equal(sayFirst(null, null), true, "the writer refuses a carton with no case pack");
  assert.equal(sayFirst(null, 16), true);
  assert.equal(sayFirst(sixteen, null), false, "nothing typed changes nothing");
  assert.equal(sayFirst(sixteen, 16), false, "the same count is no act");
  assert.equal(sayFirst(sixteen, 12), true);
  assert.equal(sayFirst({ units_per_inner: null, inners_per_carton: null }, 6), true, "an unsaid count, said");
});
