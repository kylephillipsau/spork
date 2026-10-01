import { strict as assert } from "node:assert";
import { test } from "node:test";
import { bindable, nameOf, photosOf, presentationNeeded, presentationOffered, subjectKey, weighable } from "./subjects.ts";
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
  packaging_level: "each",
  face: "front",
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
