import type { ReactNode } from "react";

import { cx } from "./cx";
import s from "./misc.module.css";

export type Tone = "neutral" | "accent" | "success" | "warning" | "danger" | "info";

/** A short status or category label. `dot` adds a coloured status dot. */
export function Badge({
  tone = "neutral",
  dot = false,
  children,
}: {
  tone?: Tone | undefined;
  dot?: boolean | undefined;
  children: ReactNode;
}) {
  return (
    <span className={cx(s.badge, s[tone])}>
      {dot && <span className={s.dot} aria-hidden />}
      {children}
    </span>
  );
}

/**
 * A small count, e.g. beside a tab or a section title. Renders nothing at
 * zero (D112: zero is absent) unless `showZero`, for a list that is empty.
 */
export function Count({
  value,
  tone = "neutral",
  showZero = false,
}: {
  value: number | undefined;
  tone?: Tone | undefined;
  showZero?: boolean | undefined;
}) {
  if (value === undefined || (!value && !showZero)) return null;
  return <span className={cx(s.count, s[tone])}>{value > 999 ? "999+" : value}</span>;
}
