import { strict as assert } from "node:assert";
import { test } from "node:test";

import { anAct, partOf, pressing, uuid } from "./acts.ts";

/** A counter and a clock, so the assertions are about the rule. */
function fake() {
  let n = 0;
  let tick = 0;
  return {
    mintId: () => `id-${++n}` as `${string}-${string}-${string}-${string}-${string}`,
    clock: () => `2026-08-31T00:00:0${tick++}Z`,
    minted: () => n,
  };
}

test("an act mints each name once and answers the same thing after", () => {
  const f = fake();
  const act = anAct(f.mintId, f.clock);
  assert.equal(act.id("event"), act.id("event"), "one name, one id");
  assert.notEqual(act.id("event"), act.id("package"), "two names, two ids");
  assert.equal(f.minted(), 2, "and only the two");
});

test("the parts of one act name apart, and keep its moment", () => {
  // Three cartons from one press: each part's `event` is its own, a retry of
  // the press answers every part the same, and nobody pressed three times.
  const f = fake();
  const act = anAct(f.mintId, f.clock);
  const one = partOf(act, "carton-1:make");
  const two = partOf(act, "carton-2:make");
  assert.notEqual(one.id("event"), two.id("event"), "two parts, two events");
  assert.equal(one.id("event"), partOf(act, "carton-1:make").id("event"), "a retry is the same part");
  assert.equal(one.at, act.at);
  assert.equal(two.at, act.at, "one press, one moment");
});

test("the moment is read once, not once per attempt", () => {
  // **The half that would make a partial fix worse than none.** A stable id
  // carrying a moving timestamp is a body that disagrees with the one already
  // stored, which the server refuses — so a retry would fail loudly having
  // already succeeded.
  const f = fake();
  const act = anAct(f.mintId, f.clock);
  const first = act.at;
  act.id("event");
  assert.equal(act.at, first, "the clock was re-read inside the act");
});

test("pressing the same thing twice is one act", () => {
  // The bug, stated: an operator presses Record, the response is lost, they
  // press again. The second attempt has to arrive wearing the same name.
  const press = pressing();
  const first = press.attempt("accept:f-1");
  const retry = press.attempt("accept:f-1");
  assert.equal(first.id("event"), retry.id("event"));
  assert.equal(first.at, retry.at);
});

test("pressing it against something else is a different act", () => {
  // The key is (what is being done, to what). Accepting one finding and then
  // another are two acts, and sharing an identity between them would be
  // refused by the server as a body that disagrees — loudly, but wrongly.
  const press = pressing();
  assert.notEqual(
    press.attempt("accept:f-1").id("event"),
    press.attempt("accept:f-2").id("event"),
  );
  assert.notEqual(
    press.attempt("accept:f-1").id("event"),
    press.attempt("investigate:f-1").id("event"),
  );
});

test("one press can be two acts, and both are stable", () => {
  // The bench's weigh: a weight off an instrument and a height off a keyboard
  // are two `observation_event`s, because `method` is on the event and one
  // event cannot say both.
  const press = pressing();
  const weight = press.attempt("weigh:c-1:weight");
  const height = press.attempt("weigh:c-1:height");
  assert.notEqual(weight.id("event"), height.id("event"));
  assert.equal(press.attempt("weigh:c-1:weight").id("event"), weight.id("event"));
});

test("once it lands, the next press is a new act", () => {
  // Otherwise measuring the same carton twice on purpose — a re-weigh after a
  // repack — would replay the first measurement and record nothing.
  const press = pressing();
  const first = press.attempt("weigh:c-1:weight").id("event");
  press.landed("weigh:c-1:weight");
  assert.notEqual(press.attempt("weigh:c-1:weight").id("event"), first);
});

test("what has not landed is what is still held", () => {
  // A screen that never landed anything would grow a map for ever. It is
  // bounded by success, which is the same thing the retry semantics are.
  const press = pressing();
  assert.equal(press.open(), 0);
  press.attempt("a");
  press.attempt("b");
  press.attempt("a");
  assert.equal(press.open(), 2, "one entry per key, not per attempt");
  press.landed("a");
  assert.equal(press.open(), 1);
});

test("an act's id is a version 4 UUID made without randomUUID, which plain HTTP lacks", () => {
  const ids = new Set(Array.from({ length: 1000 }, () => uuid()));
  assert.equal(ids.size, 1000, "no two alike");
  for (const id of ids) assert.match(id, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);

  // A phone on the WiFi by address is not a secure context: no randomUUID.
  const randomUUID = crypto.randomUUID;
  Object.defineProperty(crypto, "randomUUID", { value: undefined, configurable: true });
  try {
    assert.match(anAct().id("event"), /^[0-9a-f-]{36}$/);
  } finally {
    Object.defineProperty(crypto, "randomUUID", { value: randomUUID, configurable: true });
  }
});
