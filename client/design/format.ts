/**
 * Formatting values (D114).
 *
 * The client formats values; the server phrases judgements. Grouping, units
 * and decimals are presentation and belong where the pixels are. Anything
 * that reads as a judgement — "due tomorrow", "overdue", "three weeks ago"
 * — is phrased by the server and arrives as text, because that is a
 * business rule with tests behind it already and a second implementation
 * here is a second thing that can disagree.
 */

/** U+2009 THIN SPACE. Wider than nothing, narrower than a word space, and
 *  it does not read as a decimal separator to anyone. */
const THIN = " ";

/**
 * Group the integer part in threes with thin spaces: `1840` → `1 840`.
 * The fractional part is left alone — `412.500` is a measurement to three
 * decimals, not a number wanting separators after the point.
 */
export function groupDigits(value: string): string {
  const match = /^(-?)(\d+)(\.\d+)?$/.exec(value.trim());
  if (!match) return value;

  const [, sign = "", whole = "", fraction = ""] = match;
  if (whole.length < 5) return value;

  let grouped = "";
  for (let i = whole.length; i > 0; i -= 3) {
    const start = Math.max(0, i - 3);
    grouped = whole.slice(start, i) + (grouped ? THIN + grouped : "");
  }
  return sign + grouped + fraction;
}

/** Grams to kilograms, three decimals: the warehouse reads kg, the ledger
 *  stores whole grams (see `rust_decimal_from` on the server). */
export function grams(value: number): string {
  return (value / 1000).toFixed(3);
}

/** Millimetres, whole. Nothing in the packaging model is finer than 1mm. */
export function millimetres(value: number): string {
  return String(Math.round(value));
}

/**
 * Canonical millimetres shown as centimetres, one decimal.
 *
 * **What the tape is marked in.** The ledger stores millimetres because that is
 * the canonical unit for length, and every instrument in the warehouse — and
 * the printed sheet this replaces — is in centimetres. Showing a stored value
 * in a different unit from the one the field asks for is how a number gets
 * retyped a tenth of its size.
 *
 * One decimal, matching the sheet: a tape read closer than a millimetre is a
 * tape being read optimistically.
 */
export function centimetres(value: number): string {
  return (value / 10).toFixed(1);
}
