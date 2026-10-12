import { test } from "node:test";
import assert from "node:assert/strict";

import type { BenchLine, ExpectedWeight, ItemUnit, PackUnit, PackedRow } from "@domain/types";
import { checkOf, expectedIn, explain, onTheBench, uncheckedIn } from "./check.ts";

const EACH: ItemUnit = { level: "each", said: false, netsuite_unit: null, netsuite_level: null, singles: 1 };
const pack = (level: PackUnit["level"], units: number) => ({ level, units }) as PackUnit;
const line = (over: Partial<BenchLine>) =>
  ({ item_id: "i1", item_code: "ABC-1234", committed: 29, packs: [], unit: EACH, check: null, ...over }) as BenchLine;

test("what should be there is said in the units on the box", () => {
  assert.equal(expectedIn(line({ packs: [pack("each", 1), pack("carton", 12)] })), "2 cartons of 12 and 5 each");
  assert.equal(expectedIn(line({ committed: 14, packs: [pack("inner", 6)] })), "2 packs of 6 and 2 each");
  // Counted in boxes, ten to a carton: the box is NetSuite's word.
  const boxes: ItemUnit = { ...EACH, level: "inner", netsuite_unit: "Box", netsuite_level: "inner", singles: null };
  assert.equal(expectedIn(line({ committed: 10, unit: boxes, packs: [pack("carton", 10)] })), "1 carton of 10 boxes");
  assert.equal(expectedIn(line({ committed: 3 })), "3 each");
});

test("a check agrees, is short, or was made against another count", () => {
  const at = "2026-10-12T09:05:00Z";
  assert.equal(checkOf(line({})).kind, "unchecked");
  assert.equal(checkOf(line({ check: { expected: 29, seen: 29, by: "Alex", at } })).kind, "agrees");
  assert.deepEqual(checkOf(line({ check: { expected: 29, seen: 27, by: "Alex", at } })).kind, "short");
  assert.equal(checkOf(line({ committed: 30, check: { expected: 29, seen: 29, by: "Alex", at } })).kind, "stale");
  // Short two: the plan is for the 27 that are there.
  assert.equal(onTheBench([line({ remaining: 29, check: { expected: 29, seen: 27, by: "Alex", at } })])[0]!.remaining, 27);
  const row = { item_id: "i1", item_code: "ABC-1234" } as PackedRow;
  assert.deepEqual(uncheckedIn({ contents: [row] }, [line({})]), ["ABC-1234"]);
  assert.deepEqual(uncheckedIn({ contents: [row] }, [line({ check: { expected: 29, seen: 27, by: null, at } })]), []);
});

test("a weight that is off is explained by so many of one thing, within the tolerance", () => {
  const expected: ExpectedWeight = {
    grams: 13_200, n: 9, borrowed: false, established: true,
    delta_g: 1_400, delta_per_mille: 106, tolerance_per_mille: 30, agrees: false,
  };
  const row = (item_code: string, quantity: number, each_g: number) => ({ item_code, quantity, each_g }) as PackedRow;
  // Two wipes, not nine gloves: the fewest miscounted is the likeliest.
  assert.deepEqual(explain(expected, [row("WPE-0750", 14, 700), row("GLV-2200M", 10, 160)]), [{ code: "WPE-0750", by: 2, each_g: 700 }]);
  assert.deepEqual(explain({ ...expected, delta_g: -700 }, [row("WPE-0750", 14, 700)]), [{ code: "WPE-0750", by: -1, each_g: 700 }]);
  assert.deepEqual(explain({ ...expected, agrees: true }, [row("WPE-0750", 14, 700)]), [], "it agrees");
  assert.deepEqual(explain(expected, [row("BKT-1500", 2, 2_300)]), [], "nothing in it explains 1.4 kg");
});
