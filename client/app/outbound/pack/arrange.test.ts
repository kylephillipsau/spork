import { strict as assert } from "node:assert";
import { test } from "node:test";
import { MOST_PIECES, arrange, contentsOf, pack, piecesOf, type Dims, type Kind, type OpenCarton, type Placement } from "./arrange.ts";
import type { BenchLine, PackUnit, Preset } from "@domain/types";

/**
 * The suggested arrangement (D195). What matters most is that it never claims
 * something fits that does not: two things in one place, or a thing through
 * the side of the box, is a suggestion a packer finds out about with the goods
 * in hand. After that, that it picks the box a packer would.
 */

let lines = 0;

function each(size: Dims | null, over: Partial<PackUnit> = {}): PackUnit {
  return {
    level: "each",
    units: 1,
    ships_as_is: false,
    size: size && { length_mm: size[0], width_mm: size[1], height_mm: size[2] },
    no_size: false,
    gross_weight_g: 100,
    source: "own",
    style_code: null,
    faces: {},
    ...over,
  };
}

function line(code: string, remaining: number, packs: PackUnit[], over: Partial<BenchLine> = {}): BenchLine {
  lines += 1;
  return {
    line_id: `f11e0000-0000-0000-0000-${String(lines).padStart(12, "0")}`,
    item_id: `17e10000-0000-0000-0000-${String(lines).padStart(12, "0")}`,
    item_code: code,
    description: null,
    remaining,
    cells: [],
    elsewhere: null,
    own_carton: null,
    picture: null,
    packs,
    ...over,
  };
}

function box(name: string, [l, w, h]: Dims | [number, number, number], suggested = true): Preset {
  return { id: `9a7e0000-0000-0000-0000-${name.padStart(12, "0").slice(-12)}`, name, size: { length_mm: l, width_mm: w, height_mm: h }, suggested };
}

const SMALL = box("small", [400, 300, 190]);
const MEDIUM = box("medium", [450, 340, 410]);
const LARGE = box("large", [660, 440, 460]);

/** Every placement inside its box, and no two sharing any space. */
function sound(size: Dims, placements: Placement[]): void {
  for (const p of placements) {
    assert.ok(p.x >= 0 && p.y >= 0 && p.z >= 0, `${p.kind.item_code} starts inside`);
    assert.ok(p.x + p.dims[0] <= size[0] && p.y + p.dims[1] <= size[1] && p.z + p.dims[2] <= size[2], `${p.kind.item_code} ends inside`);
    const turned = [p.kind.size[p.axes[0]], p.kind.size[p.axes[1]], p.kind.size[p.axes[2]]];
    assert.deepEqual(turned, p.dims, "placed as itself, turned");
  }
  for (let i = 0; i < placements.length; i++) {
    for (let j = i + 1; j < placements.length; j++) {
      const a = placements[i]!;
      const b = placements[j]!;
      const apart = [0, 1, 2].some((k) => {
        const [as, bs] = [[a.x, a.y, a.z][k]!, [b.x, b.y, b.z][k]!];
        return as + a.dims[k]! <= bs || bs + b.dims[k]! <= as;
      });
      assert.ok(apart, `${a.kind.item_code} and ${b.kind.item_code} overlap`);
    }
  }
}

test("six aprons lie flat in the small box, one to a layer", () => {
  const a = arrange([line("APR", 6, [each([280, 220, 30])])], [MEDIUM, SMALL]);
  assert.equal(a.boxes.length, 1);
  assert.equal(a.boxes[0]!.preset.name, "small", "the smaller box that takes them all");
  assert.equal(a.boxes[0]!.layers.length, 6);
  assert.ok(a.boxes[0]!.layers.every((l) => l.height === 30), "each laid on its biggest side");
  sound([400, 300, 190], a.boxes[0]!.layers.flatMap((l) => l.placements));
});

test("what does not fit the small box goes in the next one up", () => {
  const a = arrange([line("APR", 7, [each([280, 220, 30])])], [SMALL, MEDIUM]);
  assert.equal(a.boxes.length, 1);
  assert.equal(a.boxes[0]!.preset.name, "medium");
});

test("when no box takes it all, the biggest is filled and the rest goes in the smallest that takes it", () => {
  const cube = each([300, 300, 300]);
  const a = arrange([line("CUBE", 3, [cube])], [box("snug", [310, 310, 310])]);
  assert.equal(a.boxes.length, 3);
  assert.deepEqual(a.oversize, []);

  // Four to a layer and fifteen layers in the large box; two to a layer in the
  // medium one, which is the smallest that takes the twenty left.
  const b = arrange([line("APR", 80, [each([280, 220, 30])])], [SMALL, MEDIUM, LARGE]);
  assert.deepEqual(
    b.boxes.map((x) => x.preset.name),
    ["large", "medium"],
  );
  const placed = b.boxes.map((x) => x.layers.reduce((u, l) => u + l.placements.length, 0));
  assert.deepEqual(placed, [60, 20], "every apron somewhere");
  for (const x of b.boxes) sound([x.preset.size.length_mm, x.preset.size.width_mm, x.preset.size.height_mm], x.layers.flatMap((l) => l.placements));
});

test("small things fill round and on top of a big one in its layer", () => {
  const a = arrange([line("TRAY", 1, [each([400, 300, 100])]), line("CUP", 8, [each([90, 90, 40])])], [box("tight", [400, 380, 100])]);
  assert.equal(a.boxes.length, 1);
  const [layer] = a.boxes[0]!.layers;
  assert.equal(a.boxes[0]!.layers.length, 1, "the cups go beside the tray, not on a layer of their own");
  assert.deepEqual(
    contentsOf(layer!).map((c) => [c.kind.item_code, c.count]),
    [
      ["TRAY", 1],
      ["CUP", 8],
    ],
  );
  sound([400, 380, 100], layer!.placements);
});

test("a mixed order never overlaps or leaves the box", () => {
  const order = [
    line("A", 5, [each([250, 160, 90])]),
    line("B", 12, [each([120, 80, 60])]),
    line("C", 3, [each([380, 120, 120])]),
    line("D", 20, [each([60, 60, 200])]),
  ];
  const a = arrange(order, [SMALL, MEDIUM, LARGE]);
  assert.deepEqual(a.oversize, []);
  for (const x of a.boxes) sound([x.preset.size.length_mm, x.preset.size.width_mm, x.preset.size.height_mm], x.layers.flatMap((l) => l.placements));
  const placed = a.boxes.flatMap((x) => x.layers.flatMap((l) => l.placements));
  assert.equal(placed.length, 40);
});

test("whole cartons ship as they are; inner packs while the count fills one; eaches after", () => {
  const inner: PackUnit = { ...each([200, 100, 100]), level: "inner", units: 10 };
  const carton: PackUnit = { ...each(null), level: "carton", units: 50, ships_as_is: true };
  const { pieces, asIs } = piecesOf([line("GLV", 125, [each([100, 50, 20]), inner, carton])]);
  assert.deepEqual(asIs.map((o) => [o.level, o.count, o.units]), [["carton", 2, 100]]);
  assert.deepEqual(
    pieces.map((p) => [p.kind.level, p.count]),
    [
      ["inner", 2],
      ["each", 5],
    ],
  );
});

test("an each that ships as it is goes on its own, and only the rest is boxed (D196)", () => {
  // Three rolls in their own boxes, and gloves in inner packs: the rolls go
  // to the carrier as they are, and the box is chosen for the gloves alone.
  const roll = each([350, 350, 320], { ships_as_is: true, gross_weight_g: 5338 });
  const glove: PackUnit = { ...each([260, 125, 110], { gross_weight_g: 838 }), level: "inner", units: 24 };
  const a = arrange([line("JWR", 3, [roll]), line("DGC", 48, [each(null, { no_size: true }), glove])], [SMALL, MEDIUM, LARGE]);
  assert.deepEqual(a.asIs.map((x) => [x.item_code, x.level, x.count, x.weight_g]), [["JWR", "each", 3, 3 * 5338]]);
  assert.equal(a.boxes.length, 1);
  assert.equal(a.boxes[0]!.preset.name, "small");
  assert.ok(a.boxes[0]!.layers.flatMap((l) => l.placements).every((p) => p.kind.item_code === "DGC"));
});

test("a carton somebody said goes in a box is boxed, by its size", () => {
  const carton: PackUnit = { ...each([300, 200, 150]), level: "carton", units: 6, ships_as_is: false };
  const a = arrange([line("BRM", 12, [each([300, 50, 50]), carton])], [SMALL, MEDIUM, LARGE]);
  assert.deepEqual(a.asIs, []);
  assert.deepEqual(
    a.boxes.flatMap((b) => b.layers.flatMap((l) => l.placements)).map((p) => p.kind.level),
    ["carton", "carton"],
  );
});

test("a box the workspace does not suggest is never chosen, however well it fits (D196)", () => {
  const shovel = box("shovel", [1400, 340, 400], false);
  const a = arrange([line("ROLL", 3, [each([350, 350, 320])])], [shovel, LARGE, box("xl", [860, 560, 460])]);
  assert.ok(a.boxes.every((b) => b.preset.name !== "shovel"), a.boxes.map((b) => b.preset.name).join());
});

test("loose things count toward the weight of the box they go round", () => {
  const a = arrange(
    [line("APR", 1, [each([280, 220, 30], { gross_weight_g: 180 })]), line("GLOVE", 4, [each(null, { no_size: true, gross_weight_g: 28 })]), line("CLOTH", 2, [each(null, { no_size: true, gross_weight_g: null })])],
    [SMALL],
  );
  assert.equal(a.boxes[0]!.weight_g, 180 + 4 * 28);
  assert.equal(a.boxes[0]!.unweighed, 2, "the two cloths, which have no weight");
});

test("an each with no size is listed, not guessed; one with no size to measure goes in round the rest", () => {
  const a = arrange(
    [line("APR", 2, [each([280, 220, 30])]), line("UNMEASURED", 3, []), line("WEIGHED", 1, [each(null)]), line("GLOVE", 4, [each(null, { no_size: true })])],
    [SMALL],
  );
  assert.deepEqual(
    a.unmeasured.map((u) => [u.item_code, u.units]),
    [
      ["UNMEASURED", 3],
      ["WEIGHED", 1],
    ],
  );
  assert.deepEqual(a.boxes[0]!.loose.map((u) => [u.item_code, u.units]), [["GLOVE", 4]]);
  assert.deepEqual(a.loose, [], "said once, with the box it goes in");
});

test("a thing too big for every box is said, and nothing else is lost", () => {
  const a = arrange([line("POLE", 1, [each([1800, 60, 60])]), line("APR", 1, [each([280, 220, 30])])], [SMALL, MEDIUM]);
  assert.deepEqual(a.oversize.map((o) => o.item_code), ["POLE"]);
  assert.equal(a.boxes.length, 1);
  assert.equal(a.boxes[0]!.preset.name, "small");
});

test("a preset with no size is never suggested", () => {
  const a = arrange([line("APR", 1, [each([280, 220, 30])])], [{ id: "9a7e0000-0000-0000-0000-0000000000c1", name: "PALLET", size: null, suggested: true }]);
  assert.deepEqual(a.boxes, []);
  assert.deepEqual(a.oversize.map((o) => o.item_code), ["APR"]);
});

test("a thing goes in on its side when that is the only way it fits", () => {
  const kind: Kind = {
    line: "l",
    item_id: "i",
    item_code: "TALL",
    level: "each",
    units: 1,
    size: [100, 100, 300],
    weight_g: null,
    faces: {},
    index: 0,
  };
  const p = pack([320, 120, 110], [{ kind, count: 1 }]);
  assert.equal(p.left.length, 0);
  assert.deepEqual(p.layers[0]!.placements[0]!.dims, [300, 100, 100]);
});

test("weight is what the record says, and pieces with none are counted, not zeroed silently", () => {
  const a = arrange([line("A", 2, [each([100, 100, 100], { gross_weight_g: 500 })]), line("B", 1, [each([100, 100, 100], { gross_weight_g: null })])], [SMALL]);
  assert.equal(a.boxes[0]!.weight_g, 1000);
  assert.equal(a.boxes[0]!.unweighed, 1);
});

test("as many pieces as it will arrange go in without running out of stack", () => {
  // Fifteen hundred cubes fill this box exactly: 20 by 15 by 5 of them. Filling
  // nests a step per piece along a row, so this is the deepest it goes.
  const a = arrange([line("DICE", MOST_PIECES, [each([20, 20, 20])])], [box("dice", [400, 300, 100])]);
  assert.equal(a.tooMany, false);
  assert.equal(a.boxes.length, 1);
  assert.equal(
    a.boxes[0]!.layers.reduce((t, l) => t + l.placements.length, 0),
    MOST_PIECES,
  );
  assert.equal(arrange([line("DICE", MOST_PIECES + 1, [each([20, 20, 20])])], [box("dice", [400, 300, 100])]).tooMany, true);
});

/** The small box, open on the bench with these in it. */
function opened(contents: { item_id: string; quantity: number }[]): OpenCarton {
  return { id: "ca470000-0000-0000-0000-000000000009", sequence: "2", name: "small", size: SMALL.size!, contents };
}

/** Where everything is, and whether it is in already, in the order the layers go in. */
function where(a: ReturnType<typeof arrange>) {
  return a.boxes[0]!.layers.flatMap((l) => l.placements.map((p) => [p.kind.item_code, p.x, p.y, p.z, !!p.packed]));
}

test("filling the open carton, the plan stays the same as things go in (D198)", () => {
  const tray = line("TRAY", 1, [each([380, 280, 60])]);
  const cups = line("CUP", 8, [each([90, 90, 40])]);
  const start = arrange([tray, cups], [SMALL, MEDIUM], opened([]));
  assert.equal(start.boxes[0]!.carton?.sequence, "2", "the open carton first");
  assert.ok(where(start).every(([, , , , packed]) => !packed), "nothing in it yet");

  // The tray goes in, then three cups: what is left shrinks, the plan does not move.
  const later = arrange(
    [{ ...tray, remaining: 0 }, { ...cups, remaining: 5 }],
    [SMALL, MEDIUM],
    opened([
      { item_id: tray.item_id, quantity: 1 },
      { item_id: cups.item_id, quantity: 3 },
    ]),
  );
  assert.deepEqual(
    where(later).map(([code, x, y, z]) => [code, x, y, z]),
    where(start).map(([code, x, y, z]) => [code, x, y, z]),
    "the same places",
  );
  assert.deepEqual(
    where(later).filter(([, , , , packed]) => packed).map(([code]) => code),
    ["TRAY", "CUP", "CUP", "CUP"],
    "the first of each, in the order the layers go in, are in",
  );
  assert.equal(later.boxes.length, 1, "and no second box for what still fits");
});

test("what does not fit the open carton goes in the next box, and what is in it never does (D198)", () => {
  const apron = line("APR", 10, [each([280, 220, 30])]);
  const a = arrange([apron], [SMALL, MEDIUM], opened([{ item_id: apron.item_id, quantity: 2 }]));
  // Six aprons fit the small box: two are in, four more go in, and of the ten
  // still to pack, six are left.
  assert.equal(a.boxes[0]!.layers.flatMap((l) => l.placements).length, 6);
  assert.equal(a.boxes[0]!.layers.flatMap((l) => l.placements).filter((p) => p.packed).length, 2);
  const after = a.boxes.slice(1).flatMap((b) => b.layers.flatMap((l) => l.placements));
  assert.equal(after.length, 6, "the six left, in a box of their own");
  assert.ok(after.every((p) => !p.packed));
});
