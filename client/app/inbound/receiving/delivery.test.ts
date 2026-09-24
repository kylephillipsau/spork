import { strict as assert } from "node:assert";
import { test } from "node:test";
import { baseUnits, fold, levelNamed, overDelivery, receiptKey, whatIsMissing } from "./delivery.ts";
import type { ExpectedLine } from "@domain/types";

function line(over: Partial<ExpectedLine> = {}): ExpectedLine {
  return {
    expected_supply_id: "01a05b03-0eeb-7423-aac4-fe6dac905e8a",
    item_id: "17e10000-0000-0000-0000-000000000001",
    item_code: "GLOVE-M",
    description: null,
    order_number: "PO-2026-0031",
    supplier: "Gloveco",
    expected_from: null,
    expected: 130,
    received: 97,
    outstanding: 33,
    requires_lot: false,
    levels: [
      { level: "each", units: 1, item_packing_config_id: null, baseline: null },
      { level: "carton", units: 12, item_packing_config_id: "cfg-1", baseline: null },
    ],
    owner_id: "9a247000-0000-0000-0000-000000000001",
    owners: [],
    picture: null,
    ...over,
  };
}

test("six cartons of twelve is seventy-two", () => {
  assert.equal(baseUnits("6", levelNamed(line(), "carton")), 72);
});

test("an each is an each", () => {
  assert.equal(baseUnits("6", levelNamed(line(), "each")), 6);
});

test("an unknown level falls back to each rather than to nothing", () => {
  // A screen holding a level the read did not offer would otherwise multiply by
  // undefined and send a NaN, and `each` is the one level every item has.
  assert.equal(levelNamed(line(), "pallet")?.level, "each");
});

test("a count that is not a count converts to nothing, not to zero", () => {
  // Zero would be a quantity, and `POST /receipts` would take it.
  for (const bad of ["", " ", "no", "-4", "0"]) {
    assert.equal(baseUnits(bad, levelNamed(line(), "each")), null, bad);
  }
});

test("more than expected is a fact, and this says only that", () => {
  // 33 outstanding. Forty arrived: seven more than promised. Whether that is a
  // finding is the tolerance's decision and the tolerance is the server's.
  assert.equal(overDelivery(line(), 40), 7);
  assert.equal(overDelivery(line(), 33), 0);
  assert.equal(overDelivery(line(), 10), 0);
});

test("a required lot is asked for before the press, not after the refusal", () => {
  // `disposition` refuses outright: accepting a line with no lot puts stock on
  // the floor that cannot be recalled. Discovering that from a 400 means the
  // pallet is already broken down.
  assert.match(
    whatIsMissing(line({ requires_lot: true }), "6", "", "owner") ?? "",
    /lot/i,
  );
  assert.equal(whatIsMissing(line({ requires_lot: true }), "6", "L2026-021", "owner"), null);
});

test("an owner nobody named is asked for, and a promise that names one is not", () => {
  assert.match(whatIsMissing(line({ owner_id: null }), "6", "", null) ?? "", /owner/i);
  assert.equal(whatIsMissing(line({ owner_id: null }), "6", "", "a-party"), null);
  assert.equal(whatIsMissing(line(), "6", "", null), null);
});

test("nothing missing is null, not an empty string", () => {
  assert.equal(whatIsMissing(line(), "6", "", "owner"), null);
});

test("a satisfied promise leaves the list", () => {
  assert.equal(fold([line({ outstanding: 33 })], "01a05b03-0eeb-7423-aac4-fe6dac905e8a", 33).length, 0);
});

test("a part delivery stays, with what is still owed", () => {
  const after = fold([line({ received: 97, outstanding: 33 })], "01a05b03-0eeb-7423-aac4-fe6dac905e8a", 10);
  assert.equal(after[0]?.outstanding, 23);
  assert.equal(after[0]?.received, 107);
});

test("lines this delivery did not serve are untouched", () => {
  const other = line({ expected_supply_id: "b", outstanding: 5 });
  assert.deepEqual(fold([other], "a", 99)[0], other);
});

test("the act's name carries the delivery, the line and what was counted", () => {
  const l = line();
  const one = receiptKey(l, "truck-1", "6", "carton", "L1");
  assert.equal(one, receiptKey(l, "truck-1", " 6 ", "carton", " L1 "));
  assert.notEqual(one, receiptKey(l, "truck-2", "6", "carton", "L1"));
  assert.notEqual(one, receiptKey(l, "truck-1", "7", "carton", "L1"));
  assert.notEqual(one, receiptKey(l, "truck-1", "6", "each", "L1"));
  assert.notEqual(one, receiptKey(l, "truck-1", "6", "carton", "L2"));
});

test("no line and no delivery is still a name", () => {
  assert.equal(receiptKey(null, "truck-1", "6", "each", ""), "receipt:none");
  assert.equal(receiptKey(line(), null, "6", "each", ""), "receipt:none");
});
