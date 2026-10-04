import { strict as assert } from "node:assert";
import { test } from "node:test";

import type { MapBin } from "@domain/types";

import { findBins, LEGEND, MIX, swatch, toneOf } from "./layers.ts";

const bin = (code: string, over: Partial<MapBin> = {}): MapBin => ({
  location_id: code,
  code,
  place_id: "p",
  side: 1,
  bay: 1,
  level: 1,
  row: 1,
  position: 1,
  within_reach: true,
  reported_items: 0,
  reported_on_hand: 0,
  held: 0,
  ...over,
});

test("a shelf is toned by how many items are on it, by either record", () => {
  assert.equal(toneOf(bin("A"), "stock"), "empty");
  assert.equal(toneOf(bin("A", { reported_items: 1 }), "stock"), "one");
  assert.equal(toneOf(bin("A", { reported_items: 2 }), "stock"), "two");
  assert.equal(toneOf(bin("A", { reported_items: 7 }), "stock"), "many");
  assert.equal(toneOf(bin("A", { held: 4 }), "stock"), "one", "Spork's ledger counts when NetSuite's report is silent");
});

test("reach is the floor or a ladder", () => {
  assert.equal(toneOf(bin("A"), "reach"), "reach");
  assert.equal(toneOf(bin("A", { within_reach: false }), "reach"), "ladder");
});

test("every tone a legend shows has a mix, and the swatch is that mix in CSS", () => {
  for (const entries of Object.values(LEGEND)) for (const { tone } of entries) assert.ok(MIX[tone]);
  assert.equal(swatch("two"), "color-mix(in srgb, var(--ui-info) 60%, var(--ui-surface))");
});

test("a search finds the code exactly first, then by its start, then anywhere, without minding dashes", () => {
  const bins = [bin("E-36-01"), bin("E-03-01"), bin("E-36-010"), bin("BE-36-01"), bin("A-01-01")];
  assert.deepEqual(findBins(bins, "e3601").map((b) => b.code), ["E-36-01", "E-36-010", "BE-36-01"]);
  assert.deepEqual(findBins(bins, "  "), []);
  assert.equal(findBins(bins, "0", 2).length, 2, "at most as many as asked for");
});
