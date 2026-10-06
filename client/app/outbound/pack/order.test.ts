import { strict as assert } from "node:assert";
import { test } from "node:test";

import type { BenchScreen } from "@domain/types";

import { arrange } from "./arrange.ts";
import { PACK_FILLING, PACK_FIXTURE } from "./fixture.ts";
import { wholeOrder } from "./order.ts";

/**
 * The whole order (D202). What matters is the sum: every unit of every line
 * is in a carton, planned, listed, or picked and not boxed, and anything else
 * is said to be missing rather than left out of the count.
 */

function view(screen: BenchScreen, open: Parameters<typeof arrange>[2] = null) {
  return wholeOrder(screen, arrange(screen.lines, screen.presets, open));
}

test("every unit of the order is somewhere, and the sum says so", () => {
  const o = view(PACK_FIXTURE);
  // Gloves 8 and hair nets 3 are in cartons; six aprons go in the small box;
  // ten oversleeves are in their own carton and twenty more ship as two.
  assert.equal(o.committed, 8 + 3 + 6 + 30);
  assert.equal(o.packed, 8 + 3 + 10);
  assert.equal(o.planned, 6 + 20);
  assert.equal(o.missing, 0);
  const done = o.lines.filter((l) => l.done).map((l) => l.item_code);
  assert.deepEqual(done, ["GLV-NIT-BLU-M", "HRN-DSP-WHT"]);
  const sleeves = o.lines.find((l) => l.item_code === "SLV-PE-BLU")!;
  assert.deepEqual(
    sleeves.where.map((w) => [w.parcel, w.units, w.state]),
    [
      ["Carton 4", 10, "in"],
      ["2 parcels as they are", 20, "planned"],
    ],
  );
});

test("each parcel says what is in it and what is to go in", () => {
  const o = view(PACK_FIXTURE);
  const titles = o.parcels.map((p) => [p.title, p.state]);
  assert.deepEqual(titles, [
    ["Carton 1", "sealed"],
    ["Carton 2", "open"],
    ["Carton 3", "open"],
    ["Carton 4", "sealed"],
    ["small box", "planned"],
    ["SLV-PE-BLU carton", "as-is"],
  ]);
  const own = o.parcels.find((p) => p.title === "Carton 4")!;
  assert.equal(own.detail, "SLV-PE-BLU carton", "a product's own carton says so");
  const asIs = o.parcels.find((p) => p.state === "as-is")!;
  assert.equal(asIs.count, 2);
});

test("a product's own carton looks like the product's carton, and a box type like a box", () => {
  const screen = structuredClone(PACK_FIXTURE);
  const sleeves = screen.lines.find((l) => l.item_code === "SLV-PE-BLU")!;
  const front = { front: "f0", top: "70" };
  sleeves.packs = sleeves.packs.map((p) => (p.level === "carton" ? { ...p, faces: front } : p));
  const o = view(screen);
  assert.deepEqual(o.parcels.find((p) => p.title === "Carton 4")!.faces, front, "its own carton, already made");
  assert.deepEqual(o.parcels.find((p) => p.state === "as-is")!.faces, front, "and the ones still to ship");
  assert.deepEqual(o.parcels.find((p) => p.title === "Carton 1")!.faces, {}, "a small box is a box");
});

test("filling the open carton, what is in it is packed and the rest is planned in it (D198)", () => {
  const c = PACK_FILLING.cartons[1]!;
  const o = view(PACK_FILLING, {
    id: c.id,
    sequence: c.sequence,
    name: c.package_type!,
    size: c.stated_size!,
    max_payload_g: null,
    tare_weight_g: null,
    contents: c.contents.map((r) => ({ item_id: r.item_id, quantity: r.quantity })),
  });
  const carton = o.parcels.find((p) => p.title === "Carton 2")!;
  assert.deepEqual(
    carton.lines.map((l) => [l.item_code, l.units, l.state]),
    [
      ["APR-PE-CLR-L", 2, "in"],
      ["APR-PE-CLR-L", 4, "planned"],
    ],
  );
  assert.ok(!o.parcels.some((p) => p.state === "planned"), "no other box for what still fits");
  const aprons = o.lines.find((l) => l.item_code === "APR-PE-CLR-L")!;
  assert.deepEqual([aprons.packed, aprons.planned, aprons.missing, aprons.done], [2, 4, 0, false]);
});

test("what cannot be placed is listed, and what is in no carton nor plan is missing", () => {
  const screen: BenchScreen = {
    ...PACK_FIXTURE,
    cartons: [],
    lines: [
      // Nothing measured: listed, not missing.
      { ...PACK_FIXTURE.lines[0]!, remaining: 5, committed: 5, packs: [] },
      // Picked here, two of five, and in no carton.
      { ...PACK_FIXTURE.lines[1]!, remaining: 3, committed: 5, packs: [] },
    ],
  };
  const o = view(screen);
  const [unmeasured, picked] = o.lines;
  assert.deepEqual([unmeasured!.unplaced, unmeasured!.missing], [5, 0]);
  assert.deepEqual([picked!.unplaced, picked!.notBoxed, picked!.missing], [3, 2, 0]);
  assert.equal(o.missing, 0);
});

test("every parcel says its size and what one weighs, and the booking is made from them (D224)", () => {
  const o = view(PACK_FIXTURE);
  const weighs = o.parcels.map((p) => [p.title, p.weight_g, p.weighed, p.weightNote]);
  assert.deepEqual(weighs, [
    ["Carton 1", 4200, true, null],
    ["Carton 2", 25360, false, null],
    ["Carton 3", null, false, "not weighed"],
    ["Carton 4", 3600, false, null],
    ["small box", 1080, false, "box not weighed"],
    ["SLV-PE-BLU carton", 3600, false, null],
  ]);
  const asIs = o.freight.find((f) => f.count === 2)!;
  assert.deepEqual(asIs, { count: 2, size: [420, 310, 260], weight_g: 3600 }, "two alike, each its own weight");
  assert.equal(o.freight.reduce((t, f) => t + f.count, 0), 7, "every parcel the order leaves as");
});
