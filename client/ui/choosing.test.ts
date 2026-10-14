import { strict as assert } from "node:assert";
import { test } from "node:test";
import { carried, choose, dropHow } from "./choosing.ts";

test("a click chooses one, Shift or Ctrl and a click chooses more, and a drag carries what is chosen (D245)", () => {
  assert.deepEqual(choose(["a", "b"], "c", "only"), ["c"]);
  assert.deepEqual(choose(["a", "b"], "b", "toggle"), ["a"], "chosen again, it is let go");
  assert.deepEqual(choose(["a"], "b", "toggle"), ["a", "b"]);
  assert.deepEqual(choose(["a"], "a", "add"), ["a"], "added twice, it is there once");
  assert.deepEqual(carried(["a", "b"], "b"), ["a", "b"], "one of those chosen: all of them go");
  assert.deepEqual(carried(["a", "b"], "c"), ["c"], "one not chosen: it goes alone");
  assert.deepEqual(dropHow({ shiftKey: true, altKey: false }), { one: true, ask: false });
});
