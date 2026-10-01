import { test } from "node:test";
import assert from "node:assert/strict";
import { measurementsOf } from "./figures.ts";
import type { Figures } from "./figures.ts";

/** Only the fields a measurement is made of. `presentation` is D138's "how it
 *  was arranged" and never becomes a metric, so it stays empty here. */
const figures = (over: Partial<Figures>): Figures => ({
  weight: "",
  length: "",
  width: "",
  height: "",
  presentation: "",
  noDimensions: false,
  ...over,
});


/**
 * What the operator wrote, and in which unit.
 *
 * This exists because the screen asked for millimetres while the tape in the
 * warehouse — and the printed sheet it replaces — is marked in centimetres. A
 * length read as `23.5` went in as 23.5mm and was stored as 23mm: a tenth of
 * the real box, with no error raised and nothing on screen to notice.
 *
 * A unit is not a detail that can be left to a suffix somebody might change.
 */

test("lengths go up in centimetres, because that is what the tape reads", () => {
  const out = measurementsOf(figures({ weight: "0.896", length: "23.5", width: "16", height: "30" }));
  const unit = (metric: string) => out.find((m) => m.metric === metric)?.unit;
  assert.equal(unit("length"), "cm");
  assert.equal(unit("width"), "cm");
  assert.equal(unit("height"), "cm");
});

test("weight goes up in kilograms, because that is what the scale reads", () => {
  const out = measurementsOf(figures({ weight: "8.446" }));
  assert.equal(out.find((m) => m.metric === "gross_weight")?.unit, "kg");
});

test("nothing is scaled on this side", () => {
  // Principle 5: the writer converts, reading `unit.factor_num/factor_den`.
  // A client that divided by ten here would be a second place the conversion
  // lives, and the two would disagree the day one of them changed.
  const out = measurementsOf(figures({ weight: "0.896", length: "23.5" }));
  assert.equal(out.find((m) => m.metric === "length")?.entered_value, "23.5");
  assert.equal(out.find((m) => m.metric === "gross_weight")?.entered_value, "0.896");
});

test("an empty field is not a measurement of nothing", () => {
  const out = measurementsOf(figures({ weight: "1.2", width: "  " }));
  assert.deepEqual(out.map((m) => m.metric), ["gross_weight"]);
});

test("declaring no dimensions declares all three, never two of them", () => {
  // D138: two of three is not an answer, and the worklist reads a declared
  // absence as complete only when every length carries one.
  const out = measurementsOf(figures({ weight: "1.2", length: "9", noDimensions: true }));
  const absent = out.filter((m) => m.absent_reason === "not_applicable").map((m) => m.metric);
  assert.deepEqual(absent.sort(), ["height", "length", "width"]);
  // And the typed length is not sent beside its own absence.
  assert.equal(out.filter((m) => m.metric === "length").length, 1);
});
