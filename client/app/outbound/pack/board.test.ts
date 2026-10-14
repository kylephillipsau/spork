import { strict as assert } from "node:assert";
import { test } from "node:test";

import type { BenchScreen, PlannedParcel } from "@domain/types";
import { boardOf, dropOn, goodsKey, parcelKey, sourcesFor, suggest } from "./board.ts";
import { PACK_FILLING } from "./fixture.ts";

/** The board (D245): what is where, and what a drop does. */

const APRONS = "f11e0000-0000-0000-0000-000000000003";
const SEALED = "ca470000-0000-0000-0000-000000000001";
const OPEN = "ca470000-0000-0000-0000-000000000005";
const SMALL_BOX = "9a7e0000-0000-0000-0000-0000000000b1";
const PALLET = "9a7e0000-0000-0000-0000-0000000000c1";

let n = 0;
const newId = () => `01a20000-0000-7000-8000-${String(++n).padStart(12, "0")}`;
const planned = (id: string, type: string, lines: [string, number][], byHand = true): PlannedParcel => ({
  id,
  package_type_id: type,
  on_parcel_id: null,
  on_package_id: null,
  at: null,
  placed_by_hand: byHand,
  lines: lines.map(([line_id, quantity]) => ({ line_id, quantity })),
});
const withPlan = (plan: PlannedParcel[]): BenchScreen => ({ ...PACK_FILLING, plan });

test("the board counts each line's units in cartons, planned, and still to place", () => {
  const board = boardOf(withPlan([planned("p1", SMALL_BOX, [[APRONS, 1]])]));
  const aprons = board.lines.find((b) => b.line.line_id === APRONS)!;
  assert.equal(aprons.toPack, 4, "six committed, two in the open carton");
  assert.equal(aprons.planned, 1);
  assert.equal(aprons.left, 3, "what To pack offers to drag");
  assert.ok(board.parcels.some((p) => p.id === "p1" && !p.made && p.kind === "box"));
});

test("goods dropped into the open carton are the act, as many as fit, and undone as a correction (D246)", () => {
  const screen = withPlan([]);
  const out = dropOn(screen, boardOf(screen), [goodsKey(APRONS)], parcelKey(OPEN), { one: false }, newId);
  assert.ok(out.ok, out.verdict);
  assert.deepEqual(out.acts, [{ kind: "put", line: APRONS, units: 4, carton: OPEN }], "six lie flat in it, two are in");
  assert.equal(out.plan, null, "no plan changes");
  assert.deepEqual(out.undo?.acts, [{ kind: "unput", line: APRONS, units: 4, carton: OPEN }], "undone straight after, a correction (settled 2)");
  assert.match(out.verdict, /into Carton 2/);
});

test("into a planned parcel a drop changes the plan only; Shift puts in one", () => {
  const screen = withPlan([planned("p1", SMALL_BOX, [], false)]);
  const out = dropOn(screen, boardOf(screen), [goodsKey(APRONS)], parcelKey("p1"), { one: true }, newId);
  assert.ok(out.ok, out.verdict);
  assert.deepEqual(out.acts, []);
  assert.deepEqual(out.plan?.[0]?.lines, [{ line_id: APRONS, quantity: 1 }]);
  assert.equal(out.plan?.[0]?.placed_by_hand, true, "touched, it is the packer's");
  assert.deepEqual(out.undo?.plan, screen.plan, "undone, the plan as it was");
});

test("a sealed carton takes nothing, and says why", () => {
  const screen = withPlan([]);
  const out = dropOn(screen, boardOf(screen), [goodsKey(APRONS)], parcelKey(SEALED), { one: false }, newId);
  assert.equal(out.ok, false);
  assert.match(out.verdict, /sealed: open it/);
});

test("a made carton onto a planned pallet makes the pallet first; goods out of a carton go onto the bench", () => {
  const screen = withPlan([planned("pl", PALLET, [])]);
  const board = boardOf(screen);
  const on = dropOn(screen, board, [parcelKey(OPEN)], parcelKey("pl"), { one: false }, newId);
  assert.deepEqual(on.acts, [
    { kind: "make", parcel: "pl" },
    { kind: "contain", carton: OPEN, pallet: "pl" },
  ]);
  const out = dropOn(screen, board, [goodsKey(APRONS, OPEN)], "bench", { one: false }, newId);
  assert.deepEqual(out.acts, [{ kind: "repack", line: APRONS, units: 2, from: OPEN, to: null }]);
});

test("Suggest plans again round what the packer placed, and keeps it", () => {
  const screen = withPlan([planned("mine", SMALL_BOX, [[APRONS, 1]]), planned("its", SMALL_BOX, [[APRONS, 3]], false)]);
  const plan = suggest(screen, boardOf(screen), newId);
  assert.equal(plan[0]?.id, "mine", "the packer's stays");
  assert.ok(!plan.some((p) => p.id === "its"), "Suggest's own is planned afresh");
  const aprons = plan.slice(1).flatMap((p) => p.lines).filter((l) => l.line_id === APRONS);
  assert.equal(aprons.reduce((t, l) => t + l.quantity, 0), 3, "the other three aprons");
});

test("units for a carton come from the bench first, then a bin, then handed over (settled 2)", () => {
  const line = {
    ...PACK_FILLING.lines[2]!,
    on_bench: 2,
    cells: [{ stock_id: "s1", location: "K.36.05", lot: null, available: 3 }],
    elsewhere: { reported: 10, handed: 4, document: "IF1", picked_by: null, provenance: "" },
  };
  assert.deepEqual(sourcesFor(line, 10), {
    sources: [
      { kind: "bench", units: 2 },
      { kind: "cell", stock: "s1", units: 3 },
      { kind: "handover", units: 5 },
    ],
    short: 0,
  });
  assert.equal(sourcesFor(line, 20).short, 9, "two, three and six here: nine more are nowhere");
});
