import { test } from "node:test";
import assert from "node:assert/strict";
import { SCREENS, wantsChromeLocator } from "./manifest.ts";
import { resolve } from "../../domain/routing.ts";

test("no two screens share an id or a path", () => {
  // The id is what the rail marks as current and what pairs a spec with its
  // component; two of either would make one of them unreachable.
  assert.equal(new Set(SCREENS.map((s) => s.id)).size, SCREENS.length);
  assert.equal(new Set(SCREENS.map((s) => s.path)).size, SCREENS.length);
});

test("every path resolves to its own entry, in declaration order", () => {
  // Declaration order is match order, and this is what says nothing above
  // shadows anything below.
  for (const screen of SCREENS) {
    assert.equal(resolve(SCREENS, screen.path)?.route.id, screen.id, screen.path);
  }
});

test("the screens that own the scanner are the ones built around one (D117)", () => {
  // **Named, not counted.** This asserted `["capture"]` under the heading
  // "exactly one", and the reasoning under it — *two would fight over the
  // caret* — is about two locators on **one** screen, which is a different
  // claim and the one the render gate makes. Two screens each owning their own
  // never fight: they are never mounted together.
  //
  // Picking joined with D166 and put-away with the read behind it. All three
  // are floor screens whose whole shape is one scan field asking whichever
  // question is open, and a chrome locator beside one would be the second place
  // to aim a reader.
  const owners = SCREENS.filter((s) => s.claimsScan);
  assert.deepEqual(owners.map((s) => s.id).sort(), ["capture", "picking", "putaway", "receiving"]);
});

test("the chrome's locator is on every screen except those that must not have it", () => {
  // D111 wants it everywhere; D117 exempts the screen that claims the scanner;
  // and the two screens that run without a session are exempt because
  // resolving an identifier is a read behind one — a scan bar that can only
  // answer 401 is worse than no scan bar.
  const exempt = ["capture", "picking", "putaway", "receiving", "setup", "sign-in"];
  const without = SCREENS.filter((s) => !wantsChromeLocator(s)).map((s) => s.id);
  assert.deepEqual(without.sort(), [...exempt].sort());
  for (const s of SCREENS) {
    if (exempt.includes(s.id)) continue;
    assert.ok(wantsChromeLocator(s), `${s.id} should carry the chrome locator`);
  }
});

test("every screen names a surface the shells know", () => {
  const surfaces = new Set(["bench", "floor", "desk", "plain"]);
  for (const s of SCREENS) assert.ok(surfaces.has(s.surface), `${s.id}: ${s.surface}`);
});
