import { strict as assert } from "node:assert";
import { test } from "node:test";
import { FLOOR_LIFT, gridLines, sceneOf } from "./blocks.ts";
import type { LayoutPlace, PlanShape } from "@domain/types";

/**
 * The 3D view's reading of a plan: blocks where the plan has solid places,
 * floor where it has walk-through ones, and a rack's grid on its faces.
 */

const rect = (x: number, y: number, l: number, d: number): [number, number][] => [
  [x, y],
  [x + l, y],
  [x + l, y + d],
  [x, y + d],
];

const shape = (over: Partial<PlanShape>): PlanShape => ({
  place_id: "p",
  name: "P",
  solid: false,
  nesting: 0,
  corners: rect(0, 0, 10, 8),
  z: 0,
  height: 4,
  ...over,
});

const place = (place_id: string, bays: number, levels: number, rows = 1): LayoutPlace => ({
  place_id,
  parent_id: null,
  name: place_id,
  solid: true,
  bays,
  levels,
  rows,
  sides: 1,
  pattern: null,
  bins: 0,
});

test("a solid place stands up; a walk-through one lies flat, however tall it is", () => {
  const scene = sceneOf(
    [
      shape({ place_id: "main", height: 6 }),
      shape({ place_id: "rack", solid: true, nesting: 1, corners: rect(1, 1, 5, 1), height: 4 }),
      shape({ place_id: "room", nesting: 1, corners: rect(7, 1, 2, 2), height: 3 }),
    ],
    [],
  );
  const [main, rack, room] = scene.blocks;
  assert.equal(main!.height, 0);
  assert.equal(rack!.height, 4);
  assert.equal(room!.height, 0);
  assert.deepEqual(rack!.top, [3.5, 1.5, 4]);
});

test("only what is inside the outermost place can be chosen", () => {
  const scene = sceneOf([shape({ place_id: "main" }), shape({ place_id: "dock", nesting: 1, corners: rect(1, 6, 8, 1) })], []);
  assert.deepEqual(
    scene.blocks.map((b) => b.target),
    [false, true],
  );
});

test("a floor inside a floor is lifted by how deep it is, and a mezzanine keeps its height", () => {
  const scene = sceneOf(
    [
      shape({ place_id: "main" }),
      shape({ place_id: "zone", nesting: 1, corners: rect(1, 1, 4, 4) }),
      shape({ place_id: "spot", nesting: 2, corners: rect(2, 2, 1, 1) }),
      shape({ place_id: "mezz", nesting: 1, corners: rect(6, 1, 3, 3), z: 3 }),
    ],
    [],
  );
  assert.deepEqual(
    scene.blocks.map((b) => b.z),
    [0, FLOOR_LIFT, 2 * FLOOR_LIFT, 3 + FLOOR_LIFT],
  );
});

test("a footprint drawn clockwise is turned round, and one with no area is left out", () => {
  const clockwise = [...rect(0, 0, 4, 2)].reverse();
  const scene = sceneOf(
    [
      shape({ place_id: "cw", corners: clockwise }),
      shape({ place_id: "flat", corners: [[0, 0], [4, 0], [8, 0]] }),
      shape({ place_id: "two", corners: [[0, 0], [4, 0]] }),
      shape({ place_id: "nan", corners: [[0, 0], [Number.NaN, 0], [4, 4]] }),
    ],
    [],
  );
  assert.deepEqual(
    scene.blocks.map((b) => b.place_id),
    ["cw"],
  );
  const [[x0, y0], [x1, y1], [x2, y2]] = scene.blocks[0]!.ring as [[number, number], [number, number], [number, number]];
  assert.ok((x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0) > 0, "counter-clockwise");
});

test("the bounds hold every footprint and the top of the tallest block", () => {
  const scene = sceneOf(
    [
      shape({ place_id: "main", corners: rect(-2, -1, 14, 12), height: 6 }),
      shape({ place_id: "rack", solid: true, nesting: 1, corners: rect(1, 1, 5, 1), z: 0.5, height: 4 }),
    ],
    [],
  );
  assert.deepEqual(scene.min, [-2, -1, 0]);
  assert.deepEqual(scene.max, [12, 11, 4.5]);
  assert.deepEqual(sceneOf([], []), { blocks: [], min: [0, 0, 0], max: [0, 0, 0] });
});

test("a rack's bays are drawn down its front and back and across its top, its levels round it", () => {
  // Five bays and four levels: four lines between bays, three between levels.
  const lines = gridLines(rect(0, 0, 5, 1), 0, 4, { bays: 5, levels: 4, rows: 1 });
  const upright = lines.filter(([p, q]) => p[2] !== q[2]);
  const level = lines.filter(([p, q]) => p[2] === q[2] && p[2] > 0 && p[2] < 4);
  const across = lines.filter(([p, q]) => p[2] === 4 && q[2] === 4);
  assert.equal(upright.length, 8, "four on the front and four on the back");
  assert.equal(level.length, 12, "three levels, four faces each");
  assert.equal(across.length, 4);
  assert.deepEqual(upright[0], [
    [1, 0, 0],
    [1, 0, 4],
  ]);
  assert.deepEqual(
    [...new Set(level.map(([p]) => p[2]))],
    [1, 2, 3],
  );
});

test("a turned rack's grid turns with it", () => {
  // Rack C turned a quarter: its front runs up the site from (5, 0).
  const turned: [number, number][] = [
    [5, 0],
    [5, 4],
    [4, 4],
    [4, 0],
  ];
  const lines = gridLines(turned, 0, 2, { bays: 2, levels: 1, rows: 1 });
  assert.deepEqual(lines, [
    [
      [5, 2, 0],
      [5, 2, 2],
    ],
    [
      [4, 2, 0],
      [4, 2, 2],
    ],
    [
      [5, 2, 2],
      [4, 2, 2],
    ],
  ]);
});

test("a rack with a face on each side has a spine down its top", () => {
  // Rack E: eighteen columns, two sides, one row deep from each.
  const lines = gridLines(rect(0, 0, 18, 2), 0, 4, { bays: 18, levels: 4, rows: 1, sides: 2 });
  const along = lines.filter(([p, q]) => p[2] === 4 && q[2] === 4 && p[1] === q[1]);
  assert.deepEqual(along, [
    [
      [0, 1, 4],
      [18, 1, 4],
    ],
  ]);
});

test("a floor's doors and rows are drawn on it, and one cell draws nothing", () => {
  const dock = gridLines(rect(0, 0, 6, 2), 0.02, 0, { bays: 6, levels: 1, rows: 2 });
  assert.equal(dock.length, 5 + 1);
  assert.ok(dock.every(([p, q]) => p[2] === 0.02 && q[2] === 0.02));
  assert.deepEqual(gridLines(rect(0, 0, 1, 1), 0, 3, { bays: 1, levels: 1, rows: 1 }), []);
});

test("the grid comes from the site's list, and only for a rectangle", () => {
  const l: [number, number][] = [
    [0, 0],
    [4, 0],
    [4, 1],
    [1, 1],
    [1, 4],
    [0, 4],
  ];
  const scene = sceneOf(
    [
      shape({ place_id: "rack", solid: true, nesting: 1, corners: rect(0, 0, 3, 1), height: 2 }),
      shape({ place_id: "odd", solid: true, nesting: 1, corners: l, height: 2 }),
      shape({ place_id: "unlisted", solid: true, nesting: 1, corners: rect(0, 3, 3, 1), height: 2 }),
    ],
    [place("rack", 3, 2), place("odd", 3, 2)],
  );
  assert.deepEqual(
    scene.blocks.map((b) => b.grid.length > 0),
    [true, false, false],
  );
});
