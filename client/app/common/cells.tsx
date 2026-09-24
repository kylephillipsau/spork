/**
 * Small pieces several screens draw the same way: progress, stage and state
 * badges, and the client-side formatting D114 assigns to the client.
 */

import type { ReactNode } from "react";

import { Badge, cx, type Tone } from "@ui/index";
import type { Stage } from "@domain/types";

import s from "./cells.module.css";

/** A thin bar and "done/of", e.g. units picked against committed. */
export function Progress({ done, of, label = "picked" }: { done: number; of: number; label?: string | undefined }) {
  const pct = of > 0 ? Math.min(100, Math.round((done / of) * 100)) : 0;
  return (
    <span className={s.progress} title={`${done} of ${of} ${label}`}>
      <span className={s.bar} aria-hidden>
        <span className={cx(s.fill, of > 0 && done >= of && s.full)} style={{ width: `${pct}%` }} />
      </span>
      <span className={s.text}>
        {done}/{of}
      </span>
    </span>
  );
}

export const STAGE_BADGE: Record<Stage, { label: string; tone: Tone }> = {
  ready: { label: "Ready to pack", tone: "accent" },
  on_the_bench: { label: "On the bench", tone: "info" },
  packed: { label: "Packed", tone: "success" },
  nothing_committed: { label: "Nothing committed", tone: "neutral" },
};

export function StageBadge({ stage }: { stage: Stage }) {
  return (
    <Badge tone={STAGE_BADGE[stage].tone} dot>
      {STAGE_BADGE[stage].label}
    </Badge>
  );
}

/** "overdue" / "due tomorrow", phrased by the server (D114). */
export function DueBadge({ due }: { due: string | null }) {
  if (!due) return <span className={s.faint}>—</span>;
  return <Badge tone={due === "overdue" ? "danger" : "neutral"}>{sentence(due)}</Badge>;
}

const ORDER_TONE: Record<string, Tone> = {
  placed: "info",
  open: "info",
  fulfilled: "success",
  despatched: "success",
  cancelled: "neutral",
  superseded: "neutral",
};

export function StateBadge({ state, tones = ORDER_TONE }: { state: string; tones?: Record<string, Tone> | undefined }) {
  return (
    <Badge tone={tones[state] ?? "neutral"} dot>
      {sentence(state)}
    </Badge>
  );
}

/** `short_pick` -> "Short pick". */
export function sentence(v: string): string {
  const t = v.replace(/_/g, " ").trim();
  return t.charAt(0).toUpperCase() + t.slice(1);
}

/** "-4" stays, "3" becomes "+3". */
export function signed(v: string): string {
  const n = Number(v);
  return Number.isFinite(n) && n > 0 ? `+${v}` : v;
}

export function shortDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

export function dateTime(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** "24 min", "3 h", "2 d". */
export function ago(iso: string): string {
  const mins = Math.max(0, Math.round((Date.now() - new Date(iso).getTime()) / 60000));
  if (mins < 60) return `${mins} min`;
  const h = Math.round(mins / 60);
  if (h < 48) return `${h} h`;
  return `${Math.round(h / 24)} d`;
}

export function Faint({ children }: { children: ReactNode }) {
  return <span className={s.faint}>{children}</span>;
}
