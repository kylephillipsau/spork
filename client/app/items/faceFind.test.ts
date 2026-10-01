import { strict as assert } from "node:assert";
import { test } from "node:test";
import type { Point, Quad } from "./cut.ts";
import { SAM_SIZE, biggestQuad, choose, faceFrom, fromTopLeft, hull, outline, pieceAt, quadFit, samInput } from "./faceFind.ts";

/** From a photograph to the model's question, and from its outlines to a face's corners. */

const inQuad = (q: Quad, x: number, y: number) => {
  for (let i = 0; i < 4; i++) {
    const [a, b] = [q[i]!, q[(i + 1) % 4]!];
    if ((b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]) < 0) return false;
  }
  return true;
};

/** A mask of `w` × `h` set wherever `inside` says. */
const mask = (w: number, h: number, inside: (x: number, y: number) => boolean) => {
  const m = new Uint8Array(w * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) if (inside(x + 0.5, y + 0.5)) m[y * w + x] = 1;
  return m;
};

/** A face photographed from above: wider at the bottom. */
const LEANING: Quad = [
  [300, 150],
  [700, 160],
  [820, 600],
  [190, 590],
];

test("the photograph goes to the model normalised, a channel at a time, padded square", () => {
  const px = { width: 2, height: 1, data: new Uint8ClampedArray([255, 255, 255, 255, 0, 0, 0, 255]) };
  const t = samInput(px);
  assert.equal(t.length, 3 * SAM_SIZE * SAM_SIZE);
  assert.ok(Math.abs(t[0]! - (1 - 0.485) / 0.229) < 1e-6, "white, red channel");
  assert.ok(Math.abs(t[1]! - (0 - 0.485) / 0.229) < 1e-6, "black, red channel");
  assert.ok(Math.abs(t[SAM_SIZE * SAM_SIZE]! - (1 - 0.456) / 0.224) < 1e-6, "white, green channel");
  assert.equal(t[2], 0, "padding is zero");
});

test("an outline is read between the model's cells at the photograph's size", () => {
  // One outline, yes on the left half of its 256 × 256 cells.
  const logits = new Float32Array(256 * 256).map((_, i) => (i % 256 < 128 ? 5 : -5));
  const m = outline(logits, 0, 1024, 768);
  assert.equal(m[0], 1);
  assert.equal(m[1023], 0);
  assert.equal(m[400 * 1024 + 500], 1, "512 photograph pixels to a side of the line");
  assert.equal(m[400 * 1024 + 520], 0);
});

test("the piece under the point is the one taken, and nothing when the point is on nothing", () => {
  const m = mask(100, 50, (x) => x < 30 || x > 60);
  assert.equal(pieceAt(m, 100, 50, [10, 10]).area, 30 * 50);
  assert.equal(pieceAt(m, 100, 50, [80, 10]).area, 40 * 50);
  assert.equal(pieceAt(m, 100, 50, [45, 10]).area, 0);
});

test("the corners of a leaning face are where its outline turns, named from the top-left", () => {
  const m = mask(1024, 768, (x, y) => inQuad(LEANING, x, y));
  const q = biggestQuad(hull(m, 1024, 768));
  assert.ok(q);
  const named = fromTopLeft(q);
  named.forEach((p, i) => assert.ok(Math.hypot(p[0] - LEANING[i]![0], p[1] - LEANING[i]![1]) < 6, `corner ${i}: ${p} near ${LEANING[i]}`));
});

test("four corners describe a face well, and a face with the shelf under it badly", () => {
  const face = mask(1024, 768, (x, y) => inQuad(LEANING, x, y));
  const area = face.reduce((s, v) => s + v, 0);
  assert.ok(quadFit(face, area, LEANING, 1024, 768) > 0.97);
  const withShelf = mask(1024, 768, (x, y) => inQuad(LEANING, x, y) || (y > 590 && y < 700 && x > 100 && x < 400));
  const shelf = withShelf.reduce((s, v) => s + v, 0);
  const q = biggestQuad(hull(withShelf, 1024, 768))!;
  assert.ok(quadFit(withShelf, shelf, q, 1024, 768) < 0.9);
});

test("of the four-sided outlines the largest is the face; when none is, the best described", () => {
  const c = (share: number, fit: number, score = 0.9) => ({ quad: LEANING, share, fit, score });
  const lid = c(0.3, 0.97), top = c(0.55, 0.95), over = c(0.7, 0.8);
  assert.equal(choose([lid, top, over]), top, "the whole top, not its lid, not the top and the shelf");
  assert.equal(choose([c(0.5, 0.7, 0.9), c(0.4, 0.8, 0.9)])?.fit, 0.8);
  assert.equal(choose([]), null);
});

test("from the model's three outlines to the face's corners as fractions", () => {
  // At the model's 256-cell scale: a label (a part), the face, and the face
  // with the shelf under it. The photograph is 1024 × 768.
  const cells = (q: Quad, extra?: (x: number, y: number) => boolean) =>
    Float32Array.from(mask(256, 256, (x, y) => inQuad(q.map(([a, b]) => [a / 4, b / 4]) as Quad, x, y) || !!extra?.(x, y)), (v) => (v ? 8 : -8));
  const label: Quad = [[420, 300], [600, 300], [600, 420], [420, 420]];
  const logits = new Float32Array(3 * 256 * 256);
  logits.set(cells(label), 0);
  logits.set(cells(LEANING), 256 * 256);
  logits.set(cells(LEANING, (x, y) => y > 590 / 4 && y < 700 / 4 && x > 25 && x < 100), 2 * 256 * 256);
  const face = faceFrom(logits, new Float32Array([0.9, 0.95, 0.85]), 1024, 768, [512, 384] as Point);
  assert.ok(face, "a face");
  face.forEach(([x, y], i) => {
    const [ex, ey] = [LEANING[i]![0] / 1024, LEANING[i]![1] / 768];
    assert.ok(Math.abs(x - ex) < 0.012 && Math.abs(y - ey) < 0.012, `corner ${i}: ${x},${y} near ${ex},${ey}`);
  });
  assert.equal(faceFrom(logits, new Float32Array([0.9, 0.95, 0.85]), 1024, 768, [20, 20]), null, "nothing at a corner of the photograph");
});
