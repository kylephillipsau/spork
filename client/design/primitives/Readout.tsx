import { cx } from "../cx";
import styles from "../materials/face.module.css";
import { groupDigits } from "../format";

/**
 * The number the operator walked over to read.
 *
 * A weight or a carton count is not body text containing digits, so it gets
 * its own type role: tabular figures, thin-space grouping, and the unit set
 * small beside the figure rather than as part of it.
 *
 * `value` arrives already formatted to the right number of decimals — the
 * client formats values and the server phrases judgements (D114), so
 * "412.500" is ours and "overdue" is not.
 *
 * # Readout or [`Fact`] (D167)
 *
 * **A Readout is an instrument: the number the operator walked over to read.**
 * The live weight on the scale, the count being captured now. One per bench,
 * occasionally two — it is 2.5rem on the floor because it is meant to be
 * legible from arm's length with a carton in the other hand.
 *
 * **A number inside a list row is a `Fact` about that record**, not an
 * instrument. Despatch drew four Readouts per consignment and the screen was
 * 1,967px of eight jobs: nothing there was walked over to, because you cannot
 * walk over to eight things at once. Fact keeps the figure tabular and bound
 * to its label, at the row's own size.
 */
export function Readout({
  label,
  value,
  unit,
  size = "regular",
}: {
  label: string;
  value: string;
  unit?: string;
  size?: "small" | "regular" | "large";
}) {
  return (
    <div
      data-layer="instrument"
      className={cx(styles.readout, size === "large" && styles.readoutLarge)}
    >
      <span className={styles.readoutKey}>{label}</span>
      <span className={styles.readoutValue}>
        {groupDigits(value)}
        {unit && <span className={styles.readoutUnit}>{unit}</span>}
      </span>
    </div>
  );
}

/**
 * A dimension line. Ornament that measures something — the figures come off
 * the record, not out of a layout.
 */
export function Dim({ from, to, label }: { from: string; to: string; label: string }) {
  return (
    <div data-layer="instrument" className={styles.dim}>
      <span className={styles.dimValue}>{groupDigits(from)}</span>
      <span className={styles.dimLine} aria-hidden="true" />
      <span className={styles.dimValue}>{label}</span>
      <span className={styles.dimLine} aria-hidden="true" />
      <span className={styles.dimValue}>{groupDigits(to)}</span>
    </div>
  );
}
