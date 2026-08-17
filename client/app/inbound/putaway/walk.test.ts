import { strict as assert } from "node:assert";
import { test } from "node:test";
import { awayKey, fold, nearestHome, offered } from "./walk.ts";
import type { PutawayCell, PutawayScreen } from "@domain/types";

function cell(over: Partial<PutawayCell> = {}): PutawayCell {
  return {
    stock_id: "01a05ac2-0bcd-7c85-a7cc-cd8a9877ea17",
    item_id: "17e10000-0000-0000-0000-000000000001",
    item_code: "GLOVE-M",
    description: null,
    location_code: "DOCK-1",
    lot_code: null,
    quantity: 120,
    available: 120,
    homes: [],
    picture: null,
    ...over,
  };
}

const screen = (cells: PutawayCell[]): PutawayScreen => ({ site: "MEL", cells });

test("the whole cell is offered, because that is the usual trip", () => {
  assert.equal(offered(cell({ quantity: 120 })), 120);
});

test("the offer ignores what is claimed", () => {
  // A claim is a promise to an order; it does not pin the goods to the dock.
  // Offering only the free part would strand the rest where nobody looks.
  assert.equal(offered(cell({ quantity: 30, available: 12 })), 30);
});

test("the nearest home is the first, because the list arrives in walk order", () => {
  const c = cell({
    homes: [
      { location_id: "a", location_code: "A-01-1", quantity: 40, pick_sequence: 1 },
      { location_id: "b", location_code: "B-01-1", quantity: 18, pick_sequence: 12 },
    ],
  });
  assert.equal(nearestHome(c)?.location_code, "A-01-1");
});

test("an item never stored here has no home, and that is an answer", () => {
  assert.equal(nearestHome(cell({ homes: [] })), undefined);
});

test("a cell moved in full leaves the dock", () => {
  const after = fold(screen([cell({ stock_id: "a", quantity: 120 })]), "a", 120);
  assert.equal(after.cells.length, 0);
});

test("a cell moved in part stays, with what is left", () => {
  // The rest still needs a home, so hiding the row would lose the work.
  const after = fold(screen([cell({ stock_id: "a", quantity: 120 })]), "a", 40);
  assert.equal(after.cells[0]?.quantity, 80);
});

test("moving goods shrinks the free part first", () => {
  // Thirty on the dock, eighteen claimed, twelve free. Move twelve and nothing
  // free is left — the claim did not move, and `available` must not go
  // negative or exceed what is still there.
  const after = fold(screen([cell({ stock_id: "a", quantity: 30, available: 12 })]), "a", 12);
  assert.equal(after.cells[0]?.quantity, 18);
  assert.equal(after.cells[0]?.available, 0);
});

test("available never exceeds what is left", () => {
  const after = fold(screen([cell({ stock_id: "a", quantity: 30, available: 30 })]), "a", 25);
  assert.equal(after.cells[0]?.quantity, 5);
  assert.equal(after.cells[0]?.available, 5);
});

test("cells the put-away did not move are untouched", () => {
  const other = cell({ stock_id: "b", quantity: 7, available: 7 });
  const after = fold(screen([other]), "a", 99);
  assert.deepEqual(after.cells[0], other);
});

test("the act's name carries the cell, the bin and the amount", () => {
  const c = cell({ stock_id: "a" });
  const one = awayKey(c, { id: "bin-1" }, "40");
  // Same press twice is one act; a different bin or a different amount is not.
  assert.equal(one, awayKey(c, { id: "bin-1" }, " 40 "));
  assert.notEqual(one, awayKey(c, { id: "bin-2" }, "40"));
  assert.notEqual(one, awayKey(c, { id: "bin-1" }, "41"));
});

test("nothing selected is still a name, and a distinct one", () => {
  assert.equal(awayKey(null, { id: "bin-1" }, "40"), "putaway:none");
  assert.equal(awayKey(cell(), null, "40"), "putaway:none");
});
