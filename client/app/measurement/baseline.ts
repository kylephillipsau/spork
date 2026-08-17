import type { ExpectedWeight, WeightBaseline } from "@domain/types";

/**
 * What a weight baseline is worth, in words, for whichever screen is asking.
 *
 * Both the pack bench and the dock read `WeightBaseline`, so both would
 * otherwise keep their own copy of "from 41 weighings" and drift apart on the
 * day one of them learns to say something better.
 *
 * **The number is the least of it.** `grams` alone is a figure the packer will
 * read as authoritative whether it rests on forty weighings of this code or on
 * two of a different size in the same box — and the server sends `n`,
 * `borrowed` and `established` precisely so the screen does not have to present
 * those two the same way. Putting the wording here rather than inline in the
 * component is the split `walk.ts` and `figures.ts` already make: `node --test`
 * reads neither React nor a Vite alias, so the sentences can be asserted on
 * directly.
 */

/** Where the figure came from, and whether there is enough of it. */
export function provenance(e: WeightBaseline): string {
  const weighings = `${e.n} weighing${e.n === 1 ? "" : "s"}`;
  const whose = e.borrowed ? " of this style" : "";
  // **Said plainly rather than hedged.** A thin baseline is not a slightly
  // weaker strong one: two weighings cannot outvote a mistyped weight, so the
  // figure they produce is not evidence about anything yet, and the screen says
  // so in the same breath as it shows the number.
  return e.established
    ? `from ${weighings}${whose}`
    : `from ${weighings}${whose} — not enough to go on`;
}

/**
 * The gap between the scale and the expectation, or nothing before it is weighed.
 *
 * **No verdict, and deliberately.** Whether three hundred grams is fine is a
 * tolerance, a tolerance is policy, and `specification_policy` is where policy
 * lives — it does not exist yet, and inventing a threshold in a React component
 * is the worst of the available places to put one. So this states the gap and
 * the packer, who is holding the carton, decides.
 */
export function agreement(e: ExpectedWeight): string | null {
  if (e.delta_g === null) return null;
  if (e.delta_g === 0) return "exactly as expected";
  const kg = (Math.abs(e.delta_g) / 1000).toFixed(3);
  return `${kg} kg ${e.delta_g > 0 ? "over" : "under"} expected`;
}


/**
 * How many of a thing a weight accounts for, or nothing if it cannot say.
 *
 * *18.4 kg is 46 cartons.* Rounded to nearest rather than truncated, and
 * refusing a non-positive divisor — the same two rules as `baseline::
 * implied_units` on the server, because they are the same arithmetic and an
 * answer that differed between the two would be worse than either.
 *
 * **Computed here at all** for the reason `delivery.ts` gives about `baseUnits`:
 * *"the screen multiplies to show a total before the press; the server does the
 * conversion that counts."* This is a preview for somebody standing at a scale,
 * not a figure anything is written from.
 */
export function implied(weighedG: number, perUnitG: number): number | null {
  if (perUnitG <= 0 || weighedG < 0 || !Number.isFinite(weighedG)) return null;
  return Math.round(weighedG / perUnitG);
}

/**
 * What the scale has to say about a count, as a sentence.
 *
 * **The third witness, and it is only ever a witness.** The paperwork says one
 * number and the receiver says another; the scale is the one observation that
 * belongs to neither of them, which is what makes a disagreement worth
 * recording and an agreement worth trusting. So it reports what it implies and
 * whether that matches — never whether the gap is acceptable, which is a
 * tolerance, and a tolerance is the server's (the same cut the `over` pill on
 * this screen already makes).
 *
 * `null` when there is nothing to say: no weight typed, no baseline, or no
 * count to compare against.
 */
export function witness(
  weighedG: number | null,
  baseline: WeightBaseline | null,
  counted: number | null,
  /** What is being counted — `carton`, `each`. Pluralised here. */
  noun: string,
): { units: number; agrees: boolean | null; sentence: string } | null {
  if (weighedG === null || baseline === null) return null;
  const units = implied(weighedG, baseline.grams);
  if (units === null) return null;
  // **The whole sentence, noun included.** It read `= 46, and so does the
  // count` with the caller appending the noun, which renders as "= 46, and so
  // does the count cartons". A sentence assembled in two places is a sentence
  // neither place can read.
  const many = `${units} ${noun}${units === 1 ? "" : "s"}`;
  // `null` rather than `false`: nothing has been counted yet, which is not the
  // same answer as counted-and-disagrees.
  if (counted === null) return { units, agrees: null, sentence: `= ${many}` };
  const agrees = units === counted;
  return {
    units,
    agrees,
    sentence: agrees ? `= ${many}, and so does the count` : `= ${many}, not ${counted}`,
  };
}
