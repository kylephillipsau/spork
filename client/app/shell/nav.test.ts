import { test } from "node:test";
import assert from "node:assert/strict";

import { SCREENS } from "../routing/manifest.ts";
import { NAV, REACHED_ANOTHER_WAY, SETTINGS, USER_MENU_SCREENS, allItems, currentItem } from "./nav.ts";

test("every item opens a screen that exists, at that screen's path", () => {
  for (const item of allItems()) {
    const screen = SCREENS.find((s) => s.id === item.id);
    assert.ok(screen, `${item.label}: no screen ${item.id}`);
    assert.equal(screen.path, item.path, `${item.label} links somewhere its screen is not`);
  }
});

test("every screen can be reached, and the ones in no menu say so", () => {
  const reached = new Set<string>([
    ...allItems().map((i) => i.id),
    ...USER_MENU_SCREENS,
    ...REACHED_ANOTHER_WAY,
  ]);
  for (const s of SCREENS) assert.ok(reached.has(s.id), `${s.id} is in no menu and not listed as reached another way`);
});

test("no screen is in two menus", () => {
  const ids = allItems().map((i) => i.id);
  assert.equal(new Set(ids).size, ids.length);
  for (const id of USER_MENU_SCREENS) assert.ok(!ids.includes(id), `${id} is in the sidebar and the user menu`);
});

test("no group is empty", () => {
  for (const g of [...NAV, SETTINGS]) assert.ok(g.items.length > 0, `${g.label ?? "top"} is empty`);
});

test("the current item is the longest match, and home only matches home", () => {
  assert.equal(currentItem("/")?.item.id, "home");
  assert.equal(currentItem("/pack/1234")?.item.id, "pack");
  assert.equal(currentItem("/findings/abc")?.item.id, "findings");
  assert.equal(currentItem("/packing-list"), null, "a path that only shares a prefix is not a match");
  assert.equal(currentItem("/account"), null);
});
