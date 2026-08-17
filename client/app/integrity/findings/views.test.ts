import { test } from "node:test";
import assert from "node:assert/strict";
import { VIEWS, shows, statesOf, viewFor } from "./views.ts";

test("every state the server can hold is on some tab", () => {
  // `discrepancy.state` in `findings.rs` is these four and nothing else. A
  // fifth arriving with no tab would be a finding nobody can reach from the
  // queue, which is the failure a deep link makes visible rather than fixes.
  for (const state of ["open", "investigating", "resolved", "accepted"]) {
    assert.ok(
      VIEWS.some((v) => shows(v.key, state)),
      `${state} is on no tab`,
    );
  }
});

test("Live is the two states that are still somebody's job", () => {
  assert.equal(statesOf("live"), "open,investigating");
  assert.equal(statesOf("closed"), "resolved,accepted");
});

test("a link to a finding the tab already shows does not move the tab", () => {
  // The case that made this a function of two arguments: reading the Open tab
  // and following a link to another open finding.
  assert.equal(viewFor("open", "open"), "open");
  assert.equal(viewFor("investigating", "investigating"), "investigating");
  assert.equal(viewFor("open", "live"), "live");
  assert.equal(viewFor("accepted", "closed"), "closed");
});

test("a link to a finding the tab cannot show moves to one that can", () => {
  // Landing on Live with an accepted finding would draw a queue that does not
  // contain the row on the rail.
  assert.equal(viewFor("accepted", "live"), "closed");
  assert.equal(viewFor("resolved", "open"), "closed");
  assert.equal(viewFor("investigating", "open"), "live");
  assert.equal(viewFor("open", "closed"), "live");
});

test("a state no tab covers leaves the tab where it was", () => {
  // Better than guessing: the rail shows the finding, and the queue stays
  // where the operator left it rather than emptying for no stated reason.
  assert.equal(viewFor("something_new", "open"), "open");
});
