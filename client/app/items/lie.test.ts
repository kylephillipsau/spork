import { strict as assert } from "node:assert";
import { test } from "node:test";

import { lieSaid } from "./lie.ts";

test("a cut is said against its face as measured, and one a quarter turn out says so (D214)", () => {
  const front: [number, number] = [520, 240];
  assert.equal(lieSaid("matches", "front", front).out, false);
  assert.match(lieSaid("matches", "front", front).text, /52\.0 × 24\.0 cm/);
  const turned = lieSaid("turned", "front", front);
  assert.equal(turned.out, true);
  assert.match(turned.text, /front is wider than tall .* mark it taller than wide/);
  assert.equal(lieSaid("neither", "front", front).out, true);
  assert.equal(lieSaid(null, "label", null).out, false, "a label has no measured shape to be out of");
});
