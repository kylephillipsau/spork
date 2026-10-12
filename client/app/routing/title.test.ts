import { strict as assert } from "node:assert";
import { test } from "node:test";

import { titleOf } from "./title.ts";

test("the tab says what is open, in front first, then the screen and Spork", () => {
  assert.equal(titleOf("Order", [{ text: "SO-1042 · Acme Foods", front: false }]), "SO-1042 · Acme Foods · Order · Spork");
  assert.equal(
    titleOf("Items", [
      { text: "Buckets", front: false },
      { text: "YSW-6409 · Wipes", front: true },
    ]),
    "YSW-6409 · Wipes · Buckets · Items · Spork",
  );
  assert.equal(titleOf("Dashboard", []), "Dashboard · Spork");
});
