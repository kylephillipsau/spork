import { strict as assert } from "node:assert";
import { test } from "node:test";

import type { BenchCarrier, BenchScreen, BuildRule, PlannedParcel, RuleSet } from "@domain/types";
import { boardOf, dropOn, parcelKey } from "./board.ts";
import { marksOf, proposedHollow } from "./carrying.ts";
import { PACK_FILLING } from "./fixture.ts";

/** What a parcel carries and what its rules allow, judged with no positions (D249, D250, settled 10). */

const OPEN = "ca470000-0000-0000-0000-000000000005";
const PALLET = "9a7e0000-0000-0000-0000-0000000000c1";

const rule = (kind: BuildRule["kind"], amount: number, strength: BuildRule["strength"] = "must"): BuildRule => ({
  kind,
  strength,
  weight: strength === "prefer" ? 2 : null,
  amount,
  with_base: null,
  top_counts: null,
});
const set = (...rules: BuildRule[]): RuleSet => ({ id: "s1", version_id: "v1", name: "Rules", rules });
const usual = (breakdown: RuleSet | null, as_one: RuleSet | null): BenchCarrier => ({ id: "c1", name: "Road", usual: true, breakdown, as_one });
const pallet: PlannedParcel = { id: "pl", package_type_id: PALLET, as_is: null, on_parcel_id: null, on_package_id: null, at: null, placed_by_hand: true, carrier_id: null, lines: [] };
const screenWith = (carriers: BenchCarrier[], plan: PlannedParcel[] = []): BenchScreen => ({ ...PACK_FILLING, carriers, plan });

test("a carton on no pallet is judged by the usual carrier's breakdown rules: heavy, and sent on a pallet", () => {
  // The open carton weighed 0.95 kg.
  const screen = screenWith([usual(set(rule("heavy", 900), rule("loose", 900)), null)]);
  const board = boardOf(screen);
  const marks = marksOf(screen, board, board.parcels.find((p) => p.id === OPEN)!);
  assert.match(marks.heavy ?? "", /mark it heavy/);
  assert.match(marks.pallet ?? "", /send it on a pallet/);
  const light = screenWith([usual(set(rule("heavy", 1000, "off"), rule("loose", 1000)), null)]);
  const calm = marksOf(light, boardOf(light), boardOf(light).parcels.find((p) => p.id === OPEN)!);
  assert.deepEqual(calm, { heavy: null, pallet: null, over: null }, "an off rule says nothing, and under the limit is fine");
});

test("a pallet heavier than a must is refused, and than a preference said (settled 10)", () => {
  const strict = screenWith([usual(null, set(rule("weight", 500)))], [pallet]);
  const refused = dropOn(strict, boardOf(strict), [parcelKey(OPEN)], parcelKey("pl"), { one: false }, () => "x");
  assert.equal(refused.ok, false);
  assert.match(refused.verdict, /over the 0\.5 kg its rules allow/);
  const loose = screenWith([usual(null, set(rule("weight", 500, "prefer")))], [pallet]);
  const said = dropOn(loose, boardOf(loose), [parcelKey(OPEN)], parcelKey("pl"), { one: false }, () => "x");
  assert.ok(said.ok, said.verdict);
  assert.match(said.verdict, /heavier than its rules prefer/);
});

test("a full carton of things that take weight is proposed solid; anything that takes less, or room left, hollow (D249)", () => {
  const board = boardOf(screenWith([]));
  const carton = board.parcels.find((p) => p.id === OPEN)!;
  assert.ok(carton.arranged, "the fixture's open carton is arranged");
  const full = { ...carton, arranged: { ...carton.arranged!, fill: 0.9, loose: [] } };
  assert.equal(proposedHollow(full), false, "full, and everything in it takes weight");
  assert.equal(proposedHollow({ ...full, arranged: { ...full.arranged, fill: 0.5 } }), true, "half empty");
  const soft = {
    ...full,
    goods: full.goods.map((g) => ({ ...g, line: { ...g.line, packs: g.line.packs.map((u) => ({ ...u, on_top_g: 0 })) } })),
  };
  assert.equal(proposedHollow(soft), true, "full of something that takes nothing on top");
});
