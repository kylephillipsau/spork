import { strict as assert } from "node:assert";
import { test } from "node:test";
import { START, aspectOf, cutSize, fromCorners, isFace, lieOf, projection, straighten, toCorners, turn, type Quad } from "./cut.ts";

/** A photograph cut to its face: the corners, which way up, and the straightening. */

const WHOLE: Quad = [
  [0, 0],
  [1, 0],
  [1, 1],
  [0, 1],
];

test("the corners go to the server as eight numbers and come back the same", () => {
  assert.deepEqual(fromCorners(toCorners(START)), START);
  assert.equal(toCorners(START).length, 8);
});

test("a turn names the corners from the next one round, and four turns are none", () => {
  const once = turn(WHOLE);
  assert.deepEqual(once[0], [0, 1], "the photograph's bottom-left is the face's top-left");
  assert.deepEqual(turn(turn(turn(once))), WHOLE);
  assert.ok(isFace(once), "turned is still a face");
});

test("a face is clockwise, convex, and a real share of the photograph, as the server says", () => {
  assert.ok(isFace(START));
  assert.ok(isFace([[0.3, 0.2], [0.7, 0.2], [0.85, 0.8], [0.15, 0.8]]), "leaning away");
  assert.ok(!isFace([[0, 0], [0, 1], [1, 1], [1, 0]]), "anticlockwise: seen from behind");
  assert.ok(!isFace([[0, 0], [1, 1], [1, 0], [0, 1]]), "crossed");
  assert.ok(!isFace([[0, 0], [1, 0], [0.3, 0.3], [0, 1]]), "folded in");
  assert.ok(!isFace([[0.5, 0.5], [0.51, 0.5], [0.51, 0.51], [0.5, 0.51]]), "dropped in a heap");
});

test("the projection puts each corner of the straightened face on its corner in the photograph", () => {
  const leaning: Quad = [[0.3, 0.2], [0.7, 0.2], [0.85, 0.8], [0.15, 0.8]];
  const at = projection(leaning, 1000, 800);
  const near = (a: [number, number], b: [number, number]) => assert.ok(Math.hypot(a[0] - b[0], a[1] - b[1]) < 1e-6, `${a} is ${b}`);
  near(at(0, 0), [300, 160]);
  near(at(1, 0), [700, 160]);
  near(at(1, 1), [850, 640]);
  near(at(0, 1), [150, 640]);
  // A camera keeps straight lines straight: the middle of the top edge is on it.
  assert.ok(Math.abs(at(0.5, 0)[1] - 160) < 1e-6);
});

test("a face's proportions come from its edges, and its size from the pixels it has", () => {
  // Half the width and a quarter of the height of a 4000 × 3000 photograph.
  const q: Quad = [[0.25, 0.25], [0.75, 0.25], [0.75, 0.5], [0.25, 0.5]];
  assert.equal(aspectOf(q, 4000, 3000), 2000 / 750);
  assert.deepEqual(cutSize(q, 4000, 3000, 2), [2000, 1000], "the longest edge's pixels, at the measured proportions");
  assert.deepEqual(cutSize(q, 4000, 3000, 0.5), [1000, 2000], "taller than wide");
  assert.deepEqual(cutSize(WHOLE, 6000, 4000, 1.5), [2048, 1365], "never more than 2048 on a side");
});

test("corners a quarter turn out from the measured face are told from the face, and a turn puts them right (D214)", () => {
  // A carton front 52 cm wide and 24 tall, its corners marked from its side.
  const front = 52 / 24;
  const sideways: Quad = [[0.3, 0.1], [0.7, 0.1], [0.7, 0.9], [0.3, 0.9]];
  const shot = aspectOf(sideways, 3000, 3000);
  assert.equal(lieOf(shot, front), "turned");
  assert.equal(lieOf(aspectOf(turn(sideways), 3000, 3000), front), "matches", "one turn and it lies right");
  assert.equal(lieOf(1.7, 2), "matches", "photographed a little off square, it is still the face");
  assert.equal(lieOf(0.5, 2), "turned");
  assert.equal(lieOf(1.2, 3), "neither", "a face of another shape is not put right by turning it");
  assert.equal(lieOf(0.95, 1.05), "matches");
  assert.equal(lieOf(0.62, 1.1), "neither", "a nearly square face is never called turned: it looks the same either way");
});

test("straightening the whole photograph onto itself is the photograph, and a turn turns it", () => {
  // 2 × 2: red, green / blue, white.
  const src = {
    width: 2,
    height: 2,
    data: new Uint8ClampedArray([255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255]),
  };
  assert.deepEqual([...straighten(src, WHOLE, 2, 2)], [...src.data]);
  // A quarter turn clockwise: blue comes to the top-left, red to the top-right.
  const turned = straighten(src, turn(WHOLE), 2, 2);
  assert.deepEqual([...turned.slice(0, 4)], [0, 0, 255, 255], "blue, top-left");
  assert.deepEqual([...turned.slice(4, 8)], [255, 0, 0, 255], "red, top-right");
});
