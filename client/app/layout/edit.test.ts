import { strict as assert } from "node:assert";
import { test } from "node:test";

import type { LayoutView } from "@domain/types";

import { boundsOf, byName, changeCount, changesOf, cleared, clearances, draftsOf, freeName, inRow, makeOf, moved, planOf, sameMake, shown, sizedFrom, snap, stepOf, stored, turned, unitOf, type Draft, type PlaceBox } from "./edit.ts";
import { DRAFTED_SITE, LAID_OUT } from "./fixture.ts";

const near = (a: number[][], b: number[][], what: string) =>
  a.forEach((p, i) => p.forEach((n, j) => assert.ok(Math.abs(n - b[i]![j]!) < 1e-9, `${what}: ${JSON.stringify(a)} against ${JSON.stringify(b)}`)));

const box = (over: Partial<PlaceBox> = {}): PlaceBox => ({ x: 0, y: 0, z: 0, length: 4, depth: 2, height: 3, turn: 0, ...over });
const draft = (place_id: string, parent_id: string | null, over: Partial<PlaceBox> = {}): Draft => ({
  place_id,
  parent_id,
  name: place_id,
  solid: parent_id !== null,
  box: box(over),
  outline: null,
  sides: 1,
  bays: 1,
  levels: 1,
  bins: 0,
  fresh: false,
});

test("the plan is composed as the server composes it", () => {
  // From places_http: Rack LE at (10, 4) in a building at the origin, a quarter turned.
  const plan = planOf([draft("b", null, { length: 30, depth: 20, height: 8 }), draft("r", "b", { x: 10, y: 4, turn: 90 })]);
  const rack = plan.find((s) => s.place_id === "r")!;
  near(rack.corners, [[10, 4], [10, 8], [8, 8], [8, 4]], "turned about its corner");
  assert.deepEqual(rack.frame, { x: 10, y: 4, z: 0, turn: 90 });
  assert.equal(rack.nesting, 1);

  // A building moved and turned carries what is in it.
  const carried = planOf([draft("b", null, { x: 5, y: 5, turn: 90, length: 30, depth: 20 }), draft("r", "b", { x: 1, y: 1 })]);
  near(carried.find((s) => s.place_id === "r")!.corners, [[4, 6], [4, 10], [2, 10], [2, 6]], "inside a turned building");
});

test("the fixtures' plans are what their places compose to", () => {
  for (const view of [LAID_OUT, DRAFTED_SITE] as LayoutView[]) {
    const plan = planOf(draftsOf(view));
    for (const shape of view.plan) {
      const mine = plan.find((s) => s.place_id === shape.place_id);
      assert.ok(mine, shape.name);
      near(mine.corners, shape.corners, shape.name);
    }
  }
});

test("a turn is about the middle, and four of them come back", () => {
  const start = box({ x: 2, y: 3, length: 18, depth: 2 });
  const once = turned(start, 90);
  const middle = (b: PlaceBox) => planOf([draft("p", null, b)])[0]!.corners.reduce(([x, y], [u, v]) => [x + u / 4, y + v / 4], [0, 0]);
  near([middle(once)], [middle(start)], "the middle stays put");
  assert.equal(once.turn, 90);
  assert.deepEqual(turned(turned(turned(once, 90), 90), 90), start, "round to where it began");
  assert.deepEqual(turned(start, -90).turn, 270);
});

test("a drag snaps to the half cell, and follows the pointer inside a turned building", () => {
  assert.equal(snap(1.26), 1.5);
  assert.equal(snap(-0.2), 0);
  assert.deepEqual(moved(box({ x: 1, y: 1 }), { x: 0, y: 0, z: 0, turn: 0 }, 2.2, -0.7), box({ x: 3, y: 0.5 }));
  // The building is turned a quarter: dragging up the site is along its x.
  assert.deepEqual(moved(box({ x: 1, y: 1 }), { x: 5, y: 5, z: 0, turn: 90 }, 0, 3), box({ x: 4, y: 1 }));
});

test("a save sends what changed, what was drawn, and what was taken away", () => {
  const drafts = draftsOf(LAID_OUT);
  assert.equal(changeCount(LAID_OUT, drafts), 0, "nothing yet");
  const [main, rackC, , returns] = drafts;
  const edited: Draft[] = [
    ...drafts.filter((d) => d !== returns && d !== rackC),
    { ...rackC!, box: turned(rackC!.box, 90) },
    { ...draft("wall", main!.place_id, { length: 10, depth: 0.5 }), name: "Wall", fresh: true },
  ];
  const c = changesOf(LAID_OUT, edited);
  assert.deepEqual(c.changed.map((p) => [p.name, p.turn]), [["Rack C", 90]]);
  assert.deepEqual(c.added.map((p) => [p.name, p.parent_id]), [["Wall", main!.place_id]]);
  assert.deepEqual(c.removed, [returns!.place_id]);
});

test("a new place takes a name nothing beside it has", () => {
  const drafts = [draft("a", null), { ...draft("w", "a"), name: "Wall" }, { ...draft("w2", "a"), name: "Wall 2" }];
  assert.equal(freeName(drafts, "a", "Wall"), "Wall 3");
  assert.equal(freeName(drafts, null, "Wall"), "Wall", "a wall elsewhere is no clash");
});

test("once a cell is a metre, numbers read in metres and a nudge is ten centimetres", () => {
  assert.equal(unitOf(null), "cells");
  assert.equal(shown(2.5, null), 2.5, "not to scale: cells as they are");
  assert.equal(unitOf(1000), "m");
  assert.equal(shown(2.4, 1000), 2.4);
  assert.equal(shown(18, 900), 16.2, "any scale reads in metres");
  assert.equal(stored(16.2, 900), 18);
  assert.equal(stepOf(1000), 0.1);
  assert.equal(stepOf(1000, true), 1);
  assert.equal(stepOf(null), 0.5);
  assert.deepEqual(moved(box({ x: 1, y: 1 }), { x: 0, y: 0, z: 0, turn: 0 }, 0.26, 0.04, stepOf(1000)), box({ x: 1.3, y: 1 }));
});

const rack = (id: string, over: Partial<PlaceBox>, grid: Partial<Draft> = {}): Draft => ({
  ...draft(id, "b", over),
  name: `Rack ${id}`,
  solid: true,
  bays: 18,
  levels: 4,
  sides: 2,
  ...grid,
});

test("a rack is sized from its bays, growing from its front left corner", () => {
  const e = rack("E", { x: 3, y: 4, length: 18, depth: 2, height: 4 });
  const box = sizedFrom(e, { bay: 0.9, side: 0.6, level: 0.5 });
  assert.deepEqual([box.x, box.y, box.length, box.depth, box.height], [3, 4, 16.2, 1.2, 2]);
  assert.deepEqual(makeOf({ ...e, box }), { bay: 0.9, side: 0.6, level: 0.5 });
  const pallet = rack("A", {}, { bays: 14, levels: 6 });
  assert.deepEqual(sameMake([e, pallet, rack("F", {})], e).map((d) => d.name), ["Rack E", "Rack F"], "racks of its make, not A");
});

test("a rack's clearances are to whatever faces it across the gap, or the wall", () => {
  const building = draft("b", null, { length: 30, depth: 20 });
  const e = rack("E", { x: 2, y: 4, length: 16, depth: 1.2 });
  const f = rack("F", { x: 2, y: 7.6, length: 16, depth: 1.2 });
  const elsewhere = rack("Z", { x: 25, y: 10, length: 3, depth: 1 });
  const c = clearances([building, e, f, elsewhere], e);
  assert.deepEqual(c, [
    { side: "left", gap: 2, to: null },
    { side: "right", gap: 12, to: null },
    { side: "front", gap: 4, to: null },
    { side: "back", gap: 2.4, to: "Rack F" },
  ]);
  assert.deepEqual(cleared(e.box, "back", 2.4, 2.8), { ...e.box, y: 3.6 }, "a wider aisle moves it away from F");
  assert.deepEqual(cleared(e.box, "left", 2, 1.5), { ...e.box, x: 1.5 });
});

test("a turned rack is measured by where it lies, not by its corner", () => {
  const building = draft("b", null, { length: 30, depth: 20 });
  // A quarter turn: from (5, 2) it runs 4 up the plan and lies 1 to the left.
  const t = rack("T", { x: 5, y: 2, length: 4, depth: 1, turn: 90 });
  assert.deepEqual(boundsOf(t), { left: 4, front: 2, right: 5, back: 6 });
  assert.equal(clearances([building, t], t).find((c) => c.side === "left")!.gap, 4);
});

test("racks set out in a row stand an aisle apart, ends lined up, each keeping its turn", () => {
  const a = rack("A", { x: 9, y: 9, length: 14, depth: 2 });
  const b = rack("B", { x: 0, y: 0, length: 14, depth: 2, turn: 180 });
  const c = rack("C", { x: 4, y: 1, length: 16, depth: 1.2 });
  const row = inRow([a, b, c].sort(byName), { left: 1.5, front: 2 }, 2.4, "up");
  const at = (d: Draft) => boundsOf({ ...d, box: row.get(d.place_id)! });
  assert.deepEqual(at(a), { left: 1.5, front: 2, right: 15.5, back: 4 });
  assert.deepEqual(at(b), { left: 1.5, front: 6.4, right: 15.5, back: 8.4 }, "turned the other way, still in line");
  assert.equal(row.get(b.place_id)!.turn, 180);
  assert.deepEqual(at(c), { left: 1.5, front: 10.8, right: 17.5, back: 12 });
  assert.deepEqual([rack("10", {}), rack("9", {})].sort(byName).map((d) => d.name), ["Rack 9", "Rack 10"]);
});

test("a bin from the tray goes in as a spot of its own, not a place", () => {
  const drafts = draftsOf(LAID_OUT);
  const main = drafts[0]!;
  const spot: Draft = { ...draft("s", main.place_id, { length: 2, depth: 2, height: 1 }), name: "PACK", solid: false, fresh: true, holds: { location_id: "loc-pack", code: "PACK" } };
  const c = changesOf(LAID_OUT, [...drafts, spot]);
  assert.deepEqual(c.added, []);
  assert.deepEqual(c.spots.map((s) => [s.place_id, s.location_id, s.parent_id]), [["s", "loc-pack", main.place_id]]);
  assert.equal(changeCount(LAID_OUT, [...drafts, spot]), 1);
});
