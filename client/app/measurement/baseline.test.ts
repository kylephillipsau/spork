import { strict as assert } from "node:assert";
import { test } from "node:test";
import { agreement, implied, provenance, witness } from "./baseline.ts";
import type { ExpectedWeight, WeightBaseline } from "@domain/types";

/**
 * The sentences that stop a number being believed more than it deserves.
 *
 * Every case here is one the bench will actually meet: a code weighed forty
 * times, a code weighed twice, and a size whose figure belongs to its style.
 * The last is the ordinary case for a styled catalogue — 58 measured styles
 * standing for 252 sellable codes — so the wording for it is not an edge.
 */

function expected(over: Partial<ExpectedWeight> = {}): ExpectedWeight {
  return {
    grams: 104_600,
    n: 41,
    borrowed: false,
    established: true,
    delta_g: null,
    delta_per_mille: null,
    ...over,
  };
}

test("a well-measured code says how many times, and no more", () => {
  assert.equal(provenance(expected()), "from 41 weighings");
});

test("one weighing is singular, and it is not enough to go on", () => {
  assert.equal(
    provenance(expected({ n: 1, established: false })),
    "from 1 weighing — not enough to go on",
  );
});

test("a borrowed figure says whose it is", () => {
  assert.equal(
    provenance(expected({ n: 13, borrowed: true })),
    "from 13 weighings of this style",
  );
});

test("a thin borrowed figure says both things", () => {
  assert.equal(
    provenance(expected({ n: 2, borrowed: true, established: false })),
    "from 2 weighings of this style — not enough to go on",
  );
});

test("nothing is said about agreement before anything is weighed", () => {
  assert.equal(agreement(expected()), null);
});

test("the gap is stated, and never judged", () => {
  // The bench artboard's figures: 104.9 weighed against 104.6 expected.
  assert.equal(agreement(expected({ delta_g: 300 })), "0.300 kg over expected");
  assert.equal(agreement(expected({ delta_g: -1240 })), "1.240 kg under expected");
});

test("dead on is worth saying out loud", () => {
  assert.equal(agreement(expected({ delta_g: 0 })), "exactly as expected");
});

test("a weight says how many, rounded to the nearer whole one", () => {
  // The receive artboard: 18.4 kg over a 400 g carton.
  assert.equal(implied(18_400, 400), 46);
  assert.equal(implied(18_240, 400), 46);
  assert.equal(implied(18_000, 400), 45);
});

test("a broken per-unit figure implies nothing at all", () => {
  assert.equal(implied(18_400, 0), null);
  assert.equal(implied(18_400, -400), null);
});

test("the scale corroborates the count, or names the disagreement", () => {
  const b: WeightBaseline = { grams: 400, n: 14, borrowed: false, established: true };
  assert.deepEqual(witness(18_400, b, 46, "carton"), {
    units: 46,
    agrees: true,
    sentence: "= 46 cartons, and so does the count",
  });
  assert.deepEqual(witness(19_200, b, 46, "carton"), {
    units: 48,
    agrees: false,
    sentence: "= 48 cartons, not 46",
  });
});

test("one of a thing is singular", () => {
  const b: WeightBaseline = { grams: 18_400, n: 5, borrowed: false, established: true };
  assert.equal(witness(18_400, b, 1, "pallet")?.sentence, "= 1 pallet, and so does the count");
});

test("with nothing counted yet it still says what it implies", () => {
  const b: WeightBaseline = { grams: 400, n: 14, borrowed: false, established: true };
  const w = witness(18_400, b, null, "carton");
  assert.equal(w?.sentence, "= 46 cartons");
  assert.equal(w?.agrees, null, "nothing counted is not a disagreement");
});

test("no weight and no baseline is nothing to say, not a zero", () => {
  const b: WeightBaseline = { grams: 400, n: 14, borrowed: false, established: true };
  assert.equal(witness(null, b, 46, "carton"), null);
  assert.equal(witness(18_400, null, 46, "carton"), null);
});
