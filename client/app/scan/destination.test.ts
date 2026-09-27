import { test } from "node:test";
import assert from "node:assert/strict";
import { destinationFor, screenFor } from "./destination.ts";
import type { Resolution, Subject } from "../../domain/types.ts";

const item = (code: string, captures = 1): Subject => ({
  kind: "item",
  id: `id-${code}`,
  code,
  description: `${code} description`,
  via: "item_barcode",
  capture: Array.from({ length: captures }, () => ({}) as never),
});

const resolution = (over: Partial<Resolution>): Resolution => ({
  outcome: "resolved",
  scanned: "9312345678907",
  gtin: null,
  sscc: null,
  lot: null,
  expiry: null,
  subjects: [],
  ...over,
});

test("one subject goes straight there, with no results page", () => {
  // D111: "One subject. Navigate. No intermediate results page, ever."
  const landing = destinationFor(resolution({ subjects: [item("STY-7720-08")] }));
  assert.deepEqual(landing, { kind: "go", path: "/capture" });
});

test("several subjects are a choice, never a silent preference", () => {
  // "That last is how a scan gets recorded against the wrong product."
  const landing = destinationFor(
    resolution({ subjects: [item("STY-7720-08"), item("STY-7720-12")] }),
  );
  assert.equal(landing.kind, "choose");
  if (landing.kind !== "choose") return;
  assert.equal(landing.options.length, 2);
  assert.deepEqual(
    landing.options.map((o) => o.label),
    ["STY-7720-08", "STY-7720-12"],
  );
  // The scanned string travels with it, to be echoed verbatim in mono.
  assert.equal(landing.scanned, "9312345678907");
});

test("ambiguous is a choice even when the server called it that itself", () => {
  const landing = destinationFor(
    resolution({ outcome: "identifier_ambiguous", subjects: [item("A"), item("B")] }),
  );
  assert.equal(landing.kind, "choose");
});

test("a well-formed identifier nobody holds is not the same as a smudge", () => {
  // The distinction D111 insists on, and the reason there are four outcomes.
  const unknown = destinationFor(resolution({ outcome: "identifier_unknown" }));
  const smudge = destinationFor(resolution({ outcome: "identifier_unrecognised" }));
  assert.equal(unknown.kind, "unknown");
  assert.equal(smudge.kind, "unrecognised");
  assert.notEqual(unknown.kind, smudge.kind);
});

test("something real with no screen says so rather than going nowhere", () => {
  const location: Subject = { ...item("A-01-1"), kind: "location", capture: [] };
  const landing = destinationFor(resolution({ subjects: [location] }));
  assert.equal(landing.kind, "nowhere");
  if (landing.kind !== "nowhere") return;
  assert.equal(landing.what, "location");
});

test("an item nothing can be measured about has no screen either", () => {
  // Not a default to /capture: sending it there because that is the only screen
  // built would be the silent preference D111 forbids, one level up.
  assert.equal(screenFor(item("X", 0)), null);
  assert.equal(screenFor({ ...item("P"), kind: "package" }), null);
});
