import type { ReactNode } from "react";
import { cx } from "../cx";
import { groupDigits } from "../format";
import styles from "./record.module.css";

/**
 * Whether a slot was actually filled.
 *
 * Call sites write `meta={x && <Faint>{x}</Faint>}`, which is `null` — not
 * `undefined` — when `x` is absent. Guarding on `!== undefined` alone let that
 * through and drew an empty band with a gap above it, which is the same bug the
 * grid-areas version had and the reason this is one function rather than care
 * at six call sites.
 *
 * **It cannot see inside a fragment.** A `<>{a && …}{b && …}</>` where both are
 * false is still an element, so a slot that may be *entirely* conditional is
 * passed as `cond ? <>…</> : undefined` instead.
 */
const has = (slot: ReactNode) => slot !== undefined && slot !== null && slot !== false;

/**
 * ONE ROW OF A WORKLIST, SAID ONCE (D167).
 *
 * # Why this exists
 *
 * Six screens hand-composed a list row out of `Face`, `FaceWell`, `Row`,
 * `Spacer` and `Stack` — seventy-nine containers between them — and all six
 * reached a different answer. The design system had given them *materials* and
 * no *patterns*, so every worklist re-derived the row, and the differences
 * were not decisions: the action sat right on one row and left on the next of
 * the same list, because `Spacer` pushes to the end of the line only until the
 * line wraps. Nobody chose that, and nobody could have — it is invisible in
 * the source and visible only at 390px.
 *
 * # Slots rather than children
 *
 * Every part arrives as a prop. A call site cannot reach inside and put the
 * action somewhere else, which is the whole point: the layout is decided here,
 * once, and six screens inherit it. Passing `children` would have made it a
 * suggestion, and a suggestion is what those screens were already ignoring.
 */
export function Record({
  media,
  lead,
  name,
  tags,
  facts,
  note,
  meta,
  action,
}: {
  /**
   * A thumbnail, beside the whole record rather than inside one of its bands.
   *
   * The picker checks the photograph against what is on the shelf, so it sits
   * where the eye lands first and stays put while the text beside it wraps.
   */
  media?: ReactNode;
  /** Where it is: a bin, a lamp, a kind. The first thing read on a walk. */
  lead?: ReactNode;
  /** What it is. The identifier somebody would say out loud. */
  name: ReactNode;
  /** Level, state, provenance — the words that qualify the name. */
  tags?: ReactNode;
  /** Figures, trailing. [`Fact`] keeps each one whole. */
  facts?: ReactNode;
  /** A description. Given the full width, because the distinguishing part of
   *  one is usually its end ("… - Green - AU8 EU42") and truncating loses it. */
  note?: ReactNode;
  /** What is outstanding about it, and why. */
  meta?: ReactNode;
  /**
   * **One.** A row with two things to do is a row that has not been designed;
   * the second belongs behind the first, or on the panel. A constraint the
   * type enforces rather than a convention a reviewer has to catch.
   */
  action?: ReactNode;
}) {
  // **Nothing but a name and one thing to do: one line.** The action joins the
  // top band rather than drawing a second one under it — the guarantee is that
  // the action trails the record, and on a single-line record that is here.
  const alone = has(action) && !has(facts) && !has(meta);

  const body = (
    <>
      <div className={styles.top}>
        <div className={styles.head}>
          {lead}
          {name}
          {tags}
        </div>
        {has(facts) && <div className={styles.facts}>{facts}</div>}
        {alone && <div className={styles.action}>{action}</div>}
      </div>

      {has(note) && <div className={styles.note}>{note}</div>}

      {!alone && (has(meta) || has(action)) && (
        <div className={styles.foot}>
          <div className={styles.meta}>{meta}</div>
          {has(action) && <div className={styles.action}>{action}</div>}
        </div>
      )}
    </>
  );

  if (!has(media)) return <div className={styles.record}>{body}</div>;
  return (
    <div className={cx(styles.record, styles.withMedia)}>
      {media}
      <div className={styles.body}>{body}</div>
    </div>
  );
}

/**
 * A list of records, separated by a line rather than by a card each.
 *
 * Seven cards is seven borders, seven backgrounds and six gaps. Seven records
 * is six hairlines — which is most of the difference between a screen that
 * looks dense and one that looks padded.
 */
export function Records({ children }: { children?: ReactNode }) {
  return <div className={styles.records}>{children}</div>;
}

/**
 * A number and the word for what it counts, which break together or not at all.
 *
 * See [`Readout`] for the other form a number takes here, and the rule for
 * which: a Fact is *about* a record, a Readout is an instrument on a bench.
 */
export function Fact({
  value,
  label,
  unit,
}: {
  value: string | number;
  label: string;
  unit?: string;
}) {
  return (
    <span className={styles.fact}>
      <span className={styles.factValue}>
        {groupDigits(String(value))}
        {unit}
      </span>
      <span className={styles.factLabel}>{label}</span>
    </span>
  );
}
