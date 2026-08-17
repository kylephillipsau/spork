import type { CSSProperties, ReactNode } from "react";
import styles from "./layout.module.css";

/**
 * SPACING BELONGS TO THE PARENT (D124).
 *
 * Components set no outer margin; separation is the parent's `gap`. A
 * component that can push its neighbours around cannot be reused, and this
 * one rule is most of what keeps a design system from rotting.
 *
 * `gap` indexes the density-scaled token scale, so the same value is the
 * same *idea* on the bench and on a handheld and a different number of
 * pixels. Nothing here accepts a raw length.
 */
export type Gap = 0 | 1 | 2 | 3 | 4 | 5 | 6;

function gapVar(gap: Gap): CSSProperties {
  return { gap: `var(--gap-${gap})` };
}

export function Stack({
  gap = 3,
  children,
}: {
  gap?: Gap;
  children?: ReactNode;
}) {
  return (
    <div className={styles.stack} style={gapVar(gap)}>
      {children}
    </div>
  );
}

export function Row({
  gap = 3,
  align = "center",
  wrap = false,
  children,
}: {
  gap?: Gap;
  /** `end` bottom-aligns: a field carries its label above the input, so a
   *  field and a readout beside it only line up along their bottoms. */
  align?: "start" | "center" | "end" | "baseline" | "stretch";
  wrap?: boolean;
  children?: ReactNode;
}) {
  return (
    <div
      className={styles.row}
      data-align={align}
      data-wrap={wrap ? "" : undefined}
      style={gapVar(gap)}
    >
      {children}
    </div>
  );
}

export function Grid({
  columns,
  gap = 4,
  children,
}: {
  columns: 2 | 3 | 4;
  gap?: Gap;
  children?: ReactNode;
}) {
  return (
    <div className={styles.grid} data-columns={columns} style={gapVar(gap)}>
      {children}
    </div>
  );
}

/** Pushes what follows it to the far end of a Row. The only alignment
 *  affordance, because `margin-left: auto` scattered through components is
 *  exactly the rule above being broken one call site at a time. */
export function Spacer() {
  return <span className={styles.spacer} aria-hidden="true" />;
}

/**
 * What trails a row: the key, or the two keys, at the far right.
 *
 * Put the trailing controls in one of these instead of after a [`Spacer`].
 * `Spacer` pushes to the end of the *line*, and once the row wraps that is the
 * middle of the row — the failure D167 names, and the reason `Record` and
 * `Notice` do not use one either.
 *
 * Those two keep their own copies because the box is a column in their grid
 * rather than a child of a flex row. If a fourth caller appears they should all
 * come here.
 */
export function Trailing({ children }: { children?: ReactNode }) {
  return <div className={styles.trailing}>{children}</div>;
}
