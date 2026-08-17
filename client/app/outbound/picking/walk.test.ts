import { strict as assert } from "node:assert";
import { test } from "node:test";
import { claimFor, fold, offered } from "./walk.ts";
import type { PickLine, PickListScreen } from "@domain/types";

/**
 * The arithmetic that decides what reaches the ledger.
 *
 * Two of these three functions have a wrong answer that raises a finding on
 * somebody else's screen, and neither wrong answer looks wrong: over-claiming
 * is a 400 the picker sees, but under-claiming is silent here and surfaces days
 * later as J56 saying `picked > covered` about a warehouse that did nothing.
 */

function line(over: Partial<PickLine> = {}): PickLine {
  return {
    fulfilment_line_id: "f11e0000-0000-0000-0000-000000000001",
    item_id: "17e10000-0000-0000-0000-000000000001",
    item_code: "GLOVE-M",
    description: null,
    reference: null,
    stock_id: "01a00d1f-45dc-7bd0-876b-e8d74d909f09",
    location_code: "B-04-2",
    pick_sequence: 22,
    lot_code: null,
    picked: 0,
    covered: 0,
    remaining: 10,
    available: 10,
    allocated: false,
    picture: null,
    ...over,
  };
}

test("a line planning already covered is picked without claiming again", () => {
  // The common case, and the one that made `always claim` untenable: the order
  // was covered when it arrived, so there is nothing left to claim and
  // `POST /allocations` would refuse the attempt with OverCovers.
  assert.equal(claimFor(line({ covered: 10 }), 10), 0);
});

test("a line nobody covered claims the whole pick", () => {
  assert.equal(claimFor(line({ covered: 0 }), 10), 10);
});

test("a partly covered line claims only the shortfall", () => {
  // Sixty wanted, twenty picked, thirty ever claimed. Picking ten more takes
  // picked to thirty, which covered already reaches — so nothing is claimed.
  assert.equal(claimFor(line({ picked: 20, covered: 30 }), 10), 0);
  // Eleven would take picked to thirty-one, and one unit is uncovered.
  assert.equal(claimFor(line({ picked: 20, covered: 30 }), 11), 1);
});

test("a claim is never negative", () => {
  // Over-covered lines exist — planning may claim more than it needed — and a
  // negative here would be sent as a quantity.
  assert.equal(claimFor(line({ picked: 0, covered: 40 }), 5), 0);
});

test("the offer is the smaller of what is wanted and what is there", () => {
  assert.equal(offered(line({ remaining: 40, available: 12 })), 12);
  assert.equal(offered(line({ remaining: 4, available: 12 })), 4);
});

test("a line with nowhere to pick from offers what the line wants", () => {
  // `available` is null when no cell was found. Offering zero would say the
  // line is satisfied, which is the opposite of what that row means.
  assert.equal(offered(line({ remaining: 6, available: null })), 6);
});

function screen(lines: PickLine[]): PickListScreen {
  return { site: "MEL", lines };
}

test("a served line keeps its place and loses what was picked", () => {
  const before = screen([
    line({ fulfilment_line_id: "a", remaining: 10, picked: 0, covered: 10 }),
    line({ fulfilment_line_id: "b", remaining: 5 }),
  ]);
  const after = fold(before, "a", 4, 10);
  assert.deepEqual(
    after.lines.map((l) => l.fulfilment_line_id),
    ["a", "b"],
    "the walk is not reordered under somebody standing in an aisle",
  );
  assert.equal(after.lines[0]?.remaining, 6);
  assert.equal(after.lines[0]?.picked, 4);
});

test("a satisfied line leaves the walk", () => {
  // The read's own filter is `picked_quantity < quantity`, so a line with
  // nothing left is not on the list. Leaving it would be the client
  // disagreeing with the list it was handed.
  const after = fold(screen([line({ fulfilment_line_id: "a", remaining: 10 })]), "a", 10, 10);
  assert.equal(after.lines.length, 0);
});

test("the fold follows the ledger rather than what was asked for", () => {
  // The response says four landed. Had the client subtracted the amount it
  // requested, a pick the server trimmed would leave the row claiming more
  // progress than the ledger holds.
  const after = fold(screen([line({ fulfilment_line_id: "a", remaining: 10, picked: 2 })]), "a", 6, 10);
  assert.equal(after.lines[0]?.remaining, 6, "10 - (6 - 2)");
});

test("lines the pick did not serve are untouched", () => {
  const other = line({ fulfilment_line_id: "b", remaining: 5, picked: 1, covered: 2 });
  const after = fold(screen([other]), "a", 99, 99);
  assert.deepEqual(after.lines[0], other);
});
