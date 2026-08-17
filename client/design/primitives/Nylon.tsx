import type { ReactNode } from "react";
import { cx } from "../cx";
import styles from "../materials/nylon.module.css";

/**
 * L1b — ripstop nylon.
 *
 * Metal encloses; nylon protects and labels (D122). It appears where the
 * system is padded, carried or tagged, and it never carries a number — a
 * textile behind a figure read at arm's length is texture competing with
 * data. `check-laws.mjs` enforces the half of that it can see.
 */
export function Nylon({
  dyed = false,
  elevation = "flush",
  as: Tag = "div",
  children,
}: {
  dyed?: boolean;
  elevation?: "flush" | "raised";
  as?: "div" | "section" | "span";
  children?: ReactNode;
}) {
  return (
    <Tag
      data-layer="nylon"
      data-material="nylon"
      className={cx(styles.nylon, dyed && styles.dyed, styles[elevation])}
    >
      {children}
    </Tag>
  );
}

/** The moulded case a handheld actually lives in, and the webbing loop a
 *  strap threads through. */
export function Boot({ children }: { children: ReactNode }) {
  return (
    <div
      data-layer="nylon"
      data-material="nylon"
      className={cx(styles.nylon, styles.raised, styles.boot)}
    >
      <span className={cx(styles.nylon, styles.dyed, styles.loop)} aria-hidden="true" />
      {children}
    </div>
  );
}

/**
 * A woven care label: stitched edge, mono, nothing else. It carries what a
 * thing *is* — a site, a device, a tenant — never what it currently reads.
 */
export function Tag({ dyed = false, children }: { dyed?: boolean; children: ReactNode }) {
  return (
    <span
      data-layer="nylon"
      data-material="nylon"
      className={cx(styles.nylon, dyed && styles.dyed, styles.tag)}
    >
      {children}
    </span>
  );
}
