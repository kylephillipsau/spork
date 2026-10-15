import { strict as assert } from "node:assert";
import { test } from "node:test";

import type { Uuid } from "@domain/types";
import { PACK_FIXTURE } from "./fixture.ts";
import { sendOn, writer, writesOf, type BenchWrite, type HeldPress } from "./writes.ts";

/** A press at the bench, kept on the device and sent on (D207). */

const ORDER = "0f000000-0000-0000-0000-000000000001";
const APRONS = "f11e0000-0000-0000-0000-000000000003";
const PLANNED = "01a20000-0000-7000-8000-00000000b001";

test("two cartons filled by one press are not filled from the same units", () => {
  const w = writer(PACK_FIXTURE, ORDER);
  w.add({ kind: "put", line: APRONS, units: 6, carton: "c1" }, "a");
  w.add({ kind: "put", line: APRONS, units: 6, carton: "c2" }, "b");
  const picks = w.writes.map((x) => (x.kind === "pickInto" ? [x.carton, x.stock.slice(-1), x.quantity] : x.kind));
  // Nine in the first bin and three in the second.
  assert.deepEqual(picks, [["c1", "3", 6], ["c2", "3", 3], ["c2", "4", 3]]);
  assert.deepEqual(w.notes, []);
});

test("a made carton is taken apart: opened, emptied, off its pallet, discarded, then the pallet", () => {
  // Carton 1: sealed, eight gloves in it, on pallet 2.
  const [carton, pallet] = ["ca470000-0000-0000-0000-000000000001", "ca470000-0000-0000-0000-000000000002"];
  const { writes } = writesOf([{ kind: "unmake", carton, pallet }], PACK_FIXTURE, ORDER);
  assert.deepEqual(
    writes.map((w) => (w.kind === "takeOut" ? `takeOut ${w.quantity}` : `${w.kind} ${"carton" in w ? w.carton.slice(-1) : ""}`)),
    ["open 1", "takeOut 8", "place 1", "discard 1", "discard 2"],
  );
});

test("a thing as it is, planned onto a pallet, is made as Ship makes one, onto its pallet", () => {
  const [OWN, PALLET] = ["01a20000-0000-7000-8000-00000000a001", "ca470000-0000-0000-0000-000000000003"];
  const as_is = { item_id: "01990000-0000-7000-8000-00000000a004", level: "carton" as const, config_id: "9ac40000-0000-0000-0000-00000000000b" };
  const plan = [{ id: OWN, package_type_id: null, as_is, on_parcel_id: null, on_package_id: PALLET, at: null, placed_by_hand: true, lines: [{ line_id: "f11e0000-0000-0000-0000-000000000004", quantity: 10 }] }];
  const { writes } = writesOf([{ kind: "make", parcel: OWN }], { ...PACK_FIXTURE, plan }, ORDER);
  const [first, second] = writes;
  assert.ok(first?.kind === "startAsIs" && first.id === OWN && first.level === "carton", "made with the plan's id");
  assert.ok(second?.kind === "contain" && second.pallet === PALLET, "onto its pallet");
  assert.equal(writes.at(-1)?.kind, "seal", "sealed as it is made");
});

test("a press stopped part way is sent on from where it stopped, as the same act", async () => {
  const { writes } = writesOf([{ kind: "make", parcel: PLANNED }], PACK_FIXTURE, ORDER);
  assert.deepEqual(writes.map((x) => x.kind), ["startCarton", "pickInto"]);

  let n = 0;
  const mint = (): Uuid => `id-${++n}`;
  const body: HeldPress = { label: "Make", order: "S1", at: "2026-10-15T00:00:00Z", ids: {}, writes, done: 0 };
  // The device's storage: what `keep` saved last is what a reload finds.
  let device = "";
  const keep = () => (device = JSON.stringify(body));

  const sent: string[] = [];
  let wifi = false;
  const send = async (w: BenchWrite, act: { id: (name: string) => Uuid }) => {
    // As the calls do: the ids are named before the request leaves.
    const ids = w.kind === "pickInto" ? [act.id("allocation"), act.id("event")] : [act.id("event")];
    if (w.kind === "pickInto" && !wifi) throw new TypeError("Failed to fetch");
    sent.push(`${w.kind}:${ids.join(",")}`);
  };

  await assert.rejects(sendOn(body, send, keep, mint));
  wifi = true;
  // Reloaded: only what was saved.
  const again = JSON.parse(device) as HeldPress;
  assert.equal(again.done, 1, "the carton is made");
  await sendOn(again, send, () => {});
  assert.deepEqual(sent, ["startCarton:id-1", "pickInto:id-2,id-3"], "the carton once, and the pick with the ids it first went with");
});
