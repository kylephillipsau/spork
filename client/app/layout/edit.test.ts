import { strict as assert } from "node:assert";
import { test } from "node:test";

import type { LayoutView } from "@domain/types";

import { changeCount, changesOf, draftsOf, freeName, moved, planOf, snap, turned, type Draft, type PlaceBox } from "./edit.ts";
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
