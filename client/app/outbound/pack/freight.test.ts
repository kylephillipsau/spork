import { strict as assert } from "node:assert";
import { test } from "node:test";

import { OBJECTIVES, bookingCm, bookingText, chargeable_g, gathered, type Freight } from "./freight.ts";

/** What an order leaves as, and what makes one way of sending it better (D224). */

const parcel = (size: [number, number, number] | null, weight_g: number | null, count = 1): Freight => ({ count, size, weight_g });

test("a carrier charges a parcel by its weight or its cubic weight, whichever is more", () => {
  // 400 × 300 × 190 mm is 0.0228 m³: 5.7 kg cubic at 250 kg/m³.
  assert.equal(Math.round(chargeable_g(parcel([400, 300, 190], 2000))), 5700, "light for its size: its cubic weight");
  assert.equal(chargeable_g(parcel([400, 300, 190], 9000)), 9000, "heavy for its size: its weight");
  assert.equal(chargeable_g(parcel(null, 1500)), 1500, "no size: its weight");
  assert.equal(Math.round(chargeable_g(parcel([400, 300, 190], 2000), 333)), 7592, "and a carrier's own factor");
});

test("fewest parcels prefers one parcel, and of as many, the least air; cheapest freight prefers the lightest charge", () => {
  const one = [parcel([620, 620, 320], 2000)];
  const two = [parcel([310, 310, 310], 1000, 2)];
  assert.ok(OBJECTIVES.parcels.cost(one) < OBJECTIVES.parcels.cost(two), "one box before two");
  assert.ok(OBJECTIVES.parcels.cost([parcel([400, 300, 190], 1000)]) < OBJECTIVES.parcels.cost([parcel([450, 340, 410], 1000)]), "the smaller of one");
  assert.ok(OBJECTIVES.chargeable.cost(two) < OBJECTIVES.chargeable.cost(one), "two small boxes charge less than one big one");
});

test("parcels alike are one line of a booking, in whole centimetres rounded up", () => {
  const sent = [parcel([400, 300, 190], 4200), parcel([400, 300, 190], 4200), parcel([520, 420, 221], null)];
  assert.deepEqual(
    gathered(sent).map((p) => p.count),
    [2, 1],
  );
  assert.equal(bookingCm(221), 23, "a part centimetre is a whole one");
  assert.equal(bookingText(sent), "2\t40\t30\t19\t4.200\n1\t52\t42\t23\t");
  assert.equal(sent[0]!.count, 1, "gathering leaves what it was given alone");
});
