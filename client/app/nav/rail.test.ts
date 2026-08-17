import { test } from "node:test";
import assert from "node:assert/strict";
import { SCREENS } from "../routing/manifest.ts";
import { RAIL, currentDestination, deadLinks, destinations, unreachable } from "./rail.ts";

test("the rail groups by job, and every group has something in it", () => {
  // D110's grouping is load-bearing, and an empty group is a promise rather
  // than a destination — absent, not disabled.
  assert.ok(RAIL.length > 0);
  for (const g of RAIL) {
    assert.ok(g.items.length > 0, `${g.group} is empty and should not be drawn at all`);
  }
});

test("no rail entry points nowhere", () => {
  // The defect a hand-written rail accumulates, and one a grep cannot see.
  const live = SCREENS.map((s) => s.path);
  assert.deepEqual(deadLinks(live), []);
});

test("every screen is reachable, or says how it is reached instead", () => {
  // The demo failure in miniature: a screen you can only get to by typing its
  // URL. Adding one now forces a decision rather than going unnoticed.
  assert.deepEqual(unreachable(SCREENS.map((s) => s.id)), []);
});

test("a subject's screen is still the destination it belongs to", () => {
  // The reason this is a function rather than `===`: /pack/f01f… is Pack.
  assert.equal(currentDestination("/pack")?.id, "pack");
  assert.equal(currentDestination("/pack/f01f0000-0000-0000-0000-000000000004")?.id, "pack");
  assert.equal(currentDestination("/findings/d15c0000")?.id, "findings");
  assert.equal(currentDestination("/"), null);
  assert.equal(currentDestination("/nonsense"), null);
});

test("a name that merely starts the same way is not the same destination", () => {
  assert.equal(currentDestination("/packing-list"), null);
});

test("the rail carries no maud page any more", () => {
  // **This test was written to fail on this day.** It used to assert that
  // passkeys was external, with a comment saying that an entry which stops
  // being external must stop being treated as one — and passkeys was the last
  // one there was. `/app/keys` is now a page the router owns.
  //
  // Kept rather than deleted, inverted: an `external` entry reappearing is a
  // page going back to maud, which would be a decision rather than an edit.
  for (const d of destinations()) {
    assert.equal(d.external, undefined, `${d.id} is still a maud page`);
  }
  // And the router answers for it, which is what "owns" means here.
  assert.equal(currentDestination("/keys")?.id, "keys");
  assert.equal(currentDestination("/app/keys"), null);
});

test("every badge names a count the work endpoint actually returns", () => {
  const counts = new Set(["pack", "pick", "despatch", "findings"]);
  for (const d of destinations()) {
    if (d.badge) assert.ok(counts.has(d.badge), `${d.badge} is not a count`);
  }
});
