import type { ReactNode } from "react";
import { CircleCheck, Info, TriangleAlert } from "lucide-react";

import { Count } from "./Badge";
import { cx } from "./cx";
import s from "./layout.module.css";

/**
 * Page structure. Every screen is a Page: its header, then its sections, at
 * one rhythm. `narrow` caps the width for forms that do not need the page.
 */
export function Page({ narrow = false, children }: { narrow?: boolean | undefined; children: ReactNode }) {
  return <div className={cx(s.page, narrow && s.narrow)}>{children}</div>;
}

/**
 * A strip across a card: tabs, search and filters at the top, or the actions
 * at the bottom. The divider is on the side that faces the card's content.
 */
export function Toolbar({ placement = "top", children }: { placement?: "top" | "bottom" | undefined; children: ReactNode }) {
  return <div className={cx(s.toolbar, placement === "bottom" && s.toolbarBottom)}>{children}</div>;
}

/** Pushes what follows to the far end of a Toolbar or a row. */
export function Spacer() {
  return <span className={s.spacer} aria-hidden />;
}

/**
 * A titled block inside a drawer or a card body, lighter than a Card: a small
 * heading, an optional count and actions, then the content.
 */
export function Section({
  title,
  count,
  actions,
  children,
}: {
  title: ReactNode;
  count?: number | undefined;
  actions?: ReactNode | undefined;
  children: ReactNode;
}) {
  return (
    <section className={s.section}>
      <header className={s.sectionHead}>
        <h3 className={s.sectionTitle}>
          {title}
          {count !== undefined && <Count value={count} showZero />}
        </h3>
        {actions}
      </header>
      {children}
    </section>
  );
}

export type AlertTone = "danger" | "warning" | "success" | "info";

/**
 * An inline message beside what it is about: a failure, a finding-grade
 * warning, or a confirmation. For transient confirmations use a toast.
 */
export function Alert({
  tone,
  onDismiss,
  children,
}: {
  tone: AlertTone;
  onDismiss?: (() => void) | undefined;
  children: ReactNode;
}) {
  const Icon = tone === "success" ? CircleCheck : tone === "info" ? Info : TriangleAlert;
  return (
    <div className={cx(s.alert, s[tone])} role={tone === "danger" || tone === "warning" ? "alert" : "status"}>
      <Icon className={s.alertIcon} aria-hidden />
      <div className={s.alertText}>{children}</div>
      {onDismiss && (
        <button type="button" className={s.alertDismiss} onClick={onDismiss}>
          Dismiss
        </button>
      )}
    </div>
  );
}

/** A row of labelled figures, divided by hairlines. */
export function StatGrid({ children }: { children: ReactNode }) {
  return <div className={s.stats}>{children}</div>;
}

export function Stat({
  label,
  value,
  tone,
  size = "lg",
}: {
  label: ReactNode;
  value: ReactNode;
  tone?: "muted" | "warning" | undefined;
  /** "lg" for a headline figure, "md" for a dense row of facts. */
  size?: "md" | "lg" | undefined;
}) {
  return (
    <div className={s.stat}>
      <span className={s.statLabel}>{label}</span>
      <span className={cx(s.statValue, size === "md" && s.statMd, tone && s[`stat_${tone}`])}>{value}</span>
    </div>
  );
}

/** Label/value pairs, as a definition list laid out in columns. */
export function Facts({ columns = 2, children }: { columns?: 1 | 2 | 3 | 4 | undefined; children: ReactNode }) {
  return <dl className={cx(s.facts, s[`cols${columns}`])}>{children}</dl>;
}

/** One pair in Facts. Renders nothing when there is no value, unless `always`. */
export function Fact({
  label,
  children,
  mono = false,
  wide = false,
  always = false,
}: {
  label: ReactNode;
  children: ReactNode;
  mono?: boolean | undefined;
  wide?: boolean | undefined;
  always?: boolean | undefined;
}) {
  if (!always && (children === null || children === undefined || children === "")) return null;
  return (
    <div className={cx(s.fact, wide && s.wide)}>
      <dt>{label}</dt>
      <dd className={cx(mono && s.mono)}>{children}</dd>
    </div>
  );
}
