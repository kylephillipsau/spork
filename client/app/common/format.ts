/**
 * Units, formatted on the client (D114). Canonical values are grams and
 * millimetres; these say them the way a person reads them.
 */

/** Grams as kilograms, to the gram: 41800 -> "41.800". */
export function grams(value: number): string {
  return (value / 1000).toFixed(3);
}

/** Millimetres, whole. Nothing in the packaging model is finer than 1mm. */
export function millimetres(value: number): string {
  return String(Math.round(value));
}

/** Millimetres as centimetres, one decimal: what the tape is marked in. */
export function centimetres(value: number): string {
  return (value / 10).toFixed(1);
}

/** Grams as "41.800 kg", or a dash when unknown. */
export function kg(value: number | null): string {
  return value === null ? "—" : `${grams(value)} kg`;
}
