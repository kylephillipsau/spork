import type { ReactNode } from "react";
import { cx } from "../cx";
import styles from "../materials/face.module.css";

/**
 * L3 — the instrument face.
 *
 * The chassis is metal and the data is paper (D118). Every number and every
 * identifier lives here, and nothing translucent is ever laid over it
 * (D119) — the cover lens is confined to a masked corner region inside the
 * stylesheet, where no call site can extend it.
 */
export function Face({
  pad = true,
  as: Tag = "div",
  children,
}: {
  pad?: boolean;
  as?: "div" | "section" | "article";
  children?: ReactNode;
}) {
  return (
    <Tag data-layer="face" className={cx(styles.face, pad && styles.pad)}>
      <span className={styles.grain} aria-hidden="true" />
      {children}
    </Tag>
  );
}

/** A sub-panel on a face. Recessed on the day face, lifted on the night
 *  face — a recess in an already-dark field is invisible, so the direction
 *  reverses in the tokens rather than in a second component. */
export function FaceWell({ children }: { children?: ReactNode }) {
  return <div className={styles.well}>{children}</div>;
}

export function Band({
  children,
  count,
}: {
  children: ReactNode;
  count?: number;
}) {
  return (
    <div className={styles.band}>
      {/* A div, not a span: both screens pass a Row in here, and phrasing
          content cannot hold flow content. Browsers cope and the markup is
          still wrong. */}
      <div className={styles.bandLabel}>{children}</div>
      {count !== undefined && (
        <span className={styles.bandCount}>({count})</span>
      )}
    </div>
  );
}

/** An identifier: anything read off a label or said down a phone. */
export function Code({ children }: { children: ReactNode }) {
  return <span className={styles.code}>{children}</span>;
}

export function Soft({ children }: { children: ReactNode }) {
  return <span className={styles.soft}>{children}</span>;
}

export function Faint({ children }: { children: ReactNode }) {
  return <span className={styles.faint}>{children}</span>;
}

/**
 * `good` confirms something that just happened. `state` reports what a thing
 * currently is — sealed, open, how much is free — and is the channel that did
 * not exist, which is why everything was either violet or nothing.
 *
 * There is deliberately no amber tone. Amber marks findings (D115).
 */
export function Pill({
  tone = "good",
  children,
}: {
  tone?: "good" | "state" | "quiet";
  children: ReactNode;
}) {
  return (
    <span
      className={cx(
        styles.pill,
        tone === "quiet" && styles.pillQuiet,
        tone === "state" && styles.pillSteel,
      )}
    >
      {children}
    </span>
  );
}

/** A figure carrying state rather than quantity: how much is free, and how
 *  close to none that is. */
export function Steel({ children }: { children: ReactNode }) {
  return <span className={styles.steel}>{children}</span>;
}


/**
 * The only amber in the interface. A finding carries its evidence on the
 * row rather than behind it — architecture.md: *"an owner, the evidence
 * behind it and a resolution"*.
 */
export function Finding({
  kind,
  children,
}: {
  kind: string;
  children?: ReactNode;
}) {
  return (
    <div className={styles.finding}>
      <span className={styles.findingKind}>{kind}</span>
      {children}
    </div>
  );
}

/** Expected against observed. The pair is the finding. */
export function EvidencePair({
  expected,
  observed,
  expectedLabel = "Expected",
  observedLabel = "Observed",
}: {
  expected: string;
  observed: string;
  expectedLabel?: string;
  observedLabel?: string;
}) {
  return (
    <div className={styles.pair}>
      <div className={styles.pairCell}>
        <div className={styles.pairKey}>{expectedLabel}</div>
        <div className={styles.pairValue}>{expected}</div>
      </div>
      <div className={styles.pairCell}>
        <div className={styles.pairKey}>{observedLabel}</div>
        <div className={styles.pairValue}>{observed}</div>
      </div>
    </div>
  );
}

/**
 * Stage 4's whole input surface. Two of these — weight and height — are the
 * only genuinely variable figures in the recorded process; everything else on
 * the pack screen is a consequence of who signed on and what they did.
 *
 * `inputMode` rather than `type="number"`: a spinner is a control nobody uses
 * on a bench and it swallows the scanner's Enter.
 */
export function Field({
  label,
  value,
  onChange,
  width = "auto",
  quiet = false,
  numeric = true,
  secret = false,
  suffix,
  disabled,
  onSubmit,
}: {
  label: string;
  value: string;
  onChange: (next: string) => void;
  /** A role, not a length. `inline` sits in a table cell, `measure` holds a
   *  weight or a height, `auto` fills what it is given. */
  width?: "auto" | "inline" | "measure";
  /** Hide the label visually where a column heading already names the field.
   *  It stays in the accessibility tree. */
  quiet?: boolean;
  numeric?: boolean;
  /** A password. Renders `type="password"`, so it is not readable over a
   *  shoulder or in a screenshot — which is how the setup screen shipped its
   *  first draft, with the operator's new password set in mono at 1.15rem. */
  secret?: boolean;
  suffix?: string;
  disabled?: boolean;
  /** Enter commits. A bench is driven from the keyboard and a scanner, and
   *  reaching for a button after typing a quantity is the reach the whole
   *  screen exists to remove. */
  onSubmit?: () => void;
}) {
  return (
    <label
      className={cx(
        styles.field,
        quiet && styles.fieldQuiet,
        width === "inline" && styles.fieldInline,
        width === "measure" && styles.fieldMeasure,
      )}
    >
      <span className={styles.fieldLabel}>
        {label}
        {suffix ? ` (${suffix})` : ""}
      </span>
      <input
        className={cx(styles.fieldInput, numeric && styles.fieldNumeric)}
        value={value}
        type={secret ? "password" : "text"}
        autoComplete={secret ? "new-password" : undefined}
        inputMode={numeric ? "decimal" : "text"}
        aria-label={quiet ? label : undefined}
        disabled={disabled ?? false}
        onChange={(e) => onChange(e.currentTarget.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && onSubmit) {
            e.preventDefault();
            onSubmit();
          }
        }}
      />
    </label>
  );
}

/** A ruled table, scrolling inside its own box so the page never does. */
export function Table({
  head,
  min = "auto",
  children,
}: {
  head: ReactNode;
  /**
   * `wide` keeps a floor under the table and lets the scroller do its job.
   *
   * The wrapper has had `overflow-x: auto` since it was written and it never
   * fired, because `width: 100%` meant the table always fitted. What it did
   * instead was squeeze.
   */
  min?: "auto" | "wide";
  children: ReactNode;
}) {
  return (
    <div className={styles.scroller}>
      <table className={cx(styles.table, min === "wide" && styles.tableWide)}>
        <thead>{head}</thead>
        <tbody>{children}</tbody>
      </table>
    </div>
  );
}

/** A cell holding a figure compared down its column rather than read across. */
export function Num({ children }: { children: ReactNode }) {
  return <td className={styles.numeric}>{children}</td>;
}

export function NumHead({ children }: { children: ReactNode }) {
  return <th className={styles.numeric}>{children}</th>;
}

/** A cell holding a control rather than a figure. */
export function Action({ children }: { children: ReactNode }) {
  return <td className={styles.action}>{children}</td>;
}

/**
 * A select, dressed as a field.
 *
 * The pack screen picks a stock cell from this because question 26 — who
 * allocates, and when — is deferred against building the allocator, so
 * `/allocations` is directed only and the operator makes the choice. When 26 is
 * answered the list becomes a default and this becomes a confirmation.
 */
export function Chooser<T extends string>({
  label,
  value,
  options,
  onChange,
  quiet = false,
  disabled,
}: {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (next: T) => void;
  quiet?: boolean;
  disabled?: boolean;
}) {
  return (
    <label className={cx(styles.chooser, quiet && styles.fieldQuiet)}>
      <span className={styles.fieldLabel}>{label}</span>
      <span className={styles.chooserControl}>
        <select
          className={styles.chooserSelect}
          value={value}
          aria-label={quiet ? label : undefined}
          disabled={disabled ?? false}
          onChange={(e) => onChange(e.currentTarget.value as T)}
        >
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        <span className={styles.chooserCaret} aria-hidden="true" />
      </span>
    </label>
  );
}
