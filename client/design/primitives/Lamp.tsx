import { cx } from "../cx";
import styles from "../materials/lamp.module.css";

/**
 * L5 — emitted light, never paint.
 *
 * `finding` is the only amber in the interface (D115). It is not exposed as
 * a colour, it is exposed as a meaning, so there is no way to reach amber
 * from a call site that is not describing a finding.
 *
 * `recorded` confirms an action that just completed. It never reports a
 * steady state — a lit lamp on every healthy row trains people to stop
 * reading the channel the finding rule depends on.
 */
export type LampKind = "finding" | "active" | "recorded" | "off";

const KIND_CLASS: Record<LampKind, string> = {
  finding: styles.amber!,
  active: styles.nitrile!,
  recorded: styles.good!,
  off: styles.off!,
};

const KIND_LABEL: Record<LampKind, string> = {
  finding: "Finding",
  active: "Active",
  recorded: "Recorded",
  off: "Off",
};

export function Lamp({ kind, label }: { kind: LampKind; label?: string }) {
  return (
    <span
      data-layer="lamp"
      className={cx(styles.lamp, KIND_CLASS[kind])}
      role="img"
      aria-label={label ?? KIND_LABEL[kind]}
    />
  );
}
