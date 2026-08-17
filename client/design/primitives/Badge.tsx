import { cx } from "../cx";
import styles from "../materials/nylon.module.css";

/**
 * L1b — a count of work waiting.
 *
 * **Nothing at zero, and that is the component rather than a rule.** D112 says
 * *"a badge counts work waiting for you, at this site, now. Zero hides. Totals
 * never appear"* — and the reason is stated in the weigh screen it was
 * generalised from: *"a badge showing nothing to do is a badge people stop
 * reading."*
 *
 * That rule was already written out by hand in two places. Returning `null`
 * here makes it structural: a call site cannot draw a nought by forgetting,
 * because there is no way to ask for one.
 *
 * A badge carries a number, so by D122 it is nylon rather than an instrument
 * face — a tag sewn to the chassis, not a readout. The number it carries is a
 * quantity of work rather than a measurement, which is the distinction that
 * keeps it off the paper.
 */
export function Badge({ count }: { count: number }) {
  if (!Number.isFinite(count) || count <= 0) return null;
  return (
    <span data-layer="nylon" data-material="nylon" className={cx(styles.badge)}>
      {count}
    </span>
  );
}
