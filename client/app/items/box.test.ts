import { strict as assert } from "node:assert";
import { test } from "node:test";
import { BOX_FACES, MATERIAL_ORDER, boxSize, cover, faceAspect, faceName, facesToAsk, isBox, toward } from "./box.ts";

/** An item as a box: which faces to ask for, and how a photo sits on one. */

test("a box is asked for its six sides, walked round from the front, then its label; a thing with no box shape for a photo and its label", () => {
  assert.equal(isBox({ dimensions_absent: false }), true);
  assert.deepEqual(facesToAsk({ dimensions_absent: false }), ["front", "right", "back", "left", "top", "bottom", "label"]);
  assert.deepEqual(facesToAsk({ dimensions_absent: true }), ["front", "label"]);
  assert.equal(faceName("front", { dimensions_absent: true }), "Photo", "a thing that is not a box has a photo, not a front");
  assert.equal(faceName("front", { dimensions_absent: false }), "Front");
  assert.equal(faceName("label", { dimensions_absent: true }), "Label");
});

test("a size is all three lengths or nothing", () => {
  assert.deepEqual(boxSize({ length_mm: 400, width_mm: 300, height_mm: 200 }), [400, 300, 200]);
  assert.equal(boxSize({ length_mm: 400, width_mm: null, height_mm: 200 }), null);
});

test("each face has the proportions of the sides it spans", () => {
  const size: [number, number, number] = [400, 300, 200];
  assert.equal(faceAspect("front", size), 2, "length across, height up");
  assert.equal(faceAspect("left", size), 1.5, "width across, height up");
  assert.equal(faceAspect("top", size), 400 / 300, "length across, width up");
});

test("a photo covers its face without stretching, centred", () => {
  // A square photo on a face twice as wide as it is tall loses its top and bottom.
  assert.deepEqual(cover(2, 1), { repeat: [1, 0.5], offset: [0, 0.25] });
  // A wide photo on a square face loses its sides.
  assert.deepEqual(cover(1, 2), { repeat: [0.5, 1], offset: [0.25, 0] });
  assert.deepEqual(cover(1.5, 1.5), { repeat: [1, 1], offset: [0, 0] });
});

test("the six faces are three.js's six, and each is seen from its own side", () => {
  assert.deepEqual([...MATERIAL_ORDER].sort(), [...BOX_FACES].sort());
  for (const face of BOX_FACES) {
    const [x, y, z] = toward(face);
    const axis = MATERIAL_ORDER.indexOf(face);
    // +x, −x, +y, −y, +z, −z: the face's own axis and sign.
    const along = [x, y, z][Math.floor(axis / 2)]!;
    assert.ok(axis % 2 === 0 ? along > 0.5 : along < -0.5, `${face} is looked at from its own side`);
  }
});
