import { strict as assert } from "node:assert";
import { test } from "node:test";

import { anAct, pressing } from "./acts.ts";

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
