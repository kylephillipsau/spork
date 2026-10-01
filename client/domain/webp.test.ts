import { strict as assert } from "node:assert";
import { test } from "node:test";
import { LONGEST_PX, fit } from "./webp.ts";

/** A photograph's size as it is sent. */

test("a photo that fits is sent at its own size", () => {
  assert.deepEqual(fit(4032, 3024), [4032, 3024], "a 12-megapixel phone photo goes whole");
  assert.deepEqual(fit(640, 480), [640, 480], "nothing is scaled up");
});

test("a bigger photo is scaled down to the longest side, keeping its shape", () => {
  assert.deepEqual(fit(5712, 4284), [LONGEST_PX, 3072], "a 24-megapixel photo, held sideways");
  assert.deepEqual(fit(4284, 5712), [3072, LONGEST_PX], "and held upright");
  assert.ok(LONGEST_PX * 3072 < 16_777_216, "within the largest canvas iOS will draw");
});
