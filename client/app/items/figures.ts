/**
 * What the operator typed, and what it amounts to.
 *
 * **Pure, and separate from the hook, so a test runner can read it.** The unit
 * a measurement is recorded in is not a detail that can live only in a suffix
 * somebody might change — this screen asked for millimetres against a tape
 * marked in centimetres, and every length went in at a tenth of its size with
 * nothing on screen to notice. `figures.test.ts` pins it.
 */

/** What the operator has typed, before any of it is a fact. */
export interface Figures {
  weight: string;
  length: string;
  width: string;
  height: string;
  /** One of [`PRESENTATIONS`], or empty before it is chosen. D138. */
  presentation: string;
  /** **The answer that is not a number.** A two-part set has no bounding box
   *  and an apron has as many as ways of folding it, so *there is none* has to
   *  be sayable or the worklist asks for ever. D138. */
  noDimensions: boolean;
}

/**
 * Which metrics the typed figures amount to.
 *
 * **A blank is not a zero and not a metric.** Sending an empty string would be
 * refused by the writer as "not a decimal number", which is right, but the
 * refusal belongs here: an operator who weighed a box and could not reach a
 * tape has recorded a weight, and the request should say that rather than fail.
 */
export function measurementsOf(figures: Figures): {
  metric: string;
  entered_value?: string;
  unit?: string;
  absent_reason?: string;
}[] {
  const out: {
    metric: string;
    entered_value?: string;
    unit?: string;
    absent_reason?: string;
  }[] = [];
  const push = (metric: string, value: string, unit: string) => {
    const trimmed = value.trim();
    if (trimmed) out.push({ metric, entered_value: trimmed, unit });
  };
  // **Kilograms and centimetres, because that is what the instruments read.**
  //
  // This asked for millimetres, and the tape in the warehouse is marked in
  // centimetres — so `23.5` off the tape went in as 23.5mm and was stored as
  // 23mm, a tenth of the real length, with no error and nothing on screen. The
  // printed sheet this replaces says it in its own header: *"measure the
  // selling unit shown in Unit · centimetres and kilograms, one decimal"*.
  //
  // Nothing is converted here. `unit` travels with the value and the writer
  // applies `unit.factor_num/factor_den` — cm is 10/1 to canonical mm — so the
  // number the operator wrote and the unit they wrote it in are both kept, and
  // arithmetic stays on the side that owns the vocabulary (Principle 5).
  push("gross_weight", figures.weight, "kg");
  if (figures.noDimensions) {
    // **All three, because two of three is not an answer.** The worklist reads
    // a declared absence as complete only when every length carries one, on the
    // same argument that two of three measurements is not a cube. D138.
    for (const metric of ["length", "width", "height"]) {
      out.push({ metric, absent_reason: "not_applicable" });
    }
  } else {
    push("length", figures.length, "cm");
    push("width", figures.width, "cm");
    push("height", figures.height, "cm");
  }
  return out;
}
