import { useEffect, useRef, useState } from "react";
import { Check, RotateCcw } from "lucide-react";

import { Badge, Button, Card, TextField } from "@ui/index";
import type { BenchLine } from "@domain/types";
import { ItemLine } from "@app/items/ItemProperties";

import { checkOf, expectedIn, needsCheck, type CheckState } from "./check";
import type { PackBench } from "./usePackBench";
import s from "./pack-bench.module.css";

/**
 * Checking what came to the bench, a line at a time (D244). Each line says
 * what should be there in the units on the box; Enter confirms it, and a
 * number typed first says what was seen instead. Then on to the next.
 */
export function Checks({ lines, bench, look }: { lines: BenchLine[]; bench: PackBench; look: (itemId: string) => void }) {
  const [typed, setTyped] = useState<Record<string, string>>({});
  // Lines somebody asked to count again, though they have a check.
  const [again, setAgain] = useState<ReadonlySet<string>>(new Set());
  const list = useRef<HTMLUListElement>(null);
  const done = lines.filter((l) => !needsCheck(l)).length;

  // A line checked: on to the next one still to check.
  const before = useRef(done);
  useEffect(() => {
    if (done > before.current) list.current?.querySelector<HTMLInputElement>("input")?.focus();
    before.current = done;
  }, [done]);

  if (lines.length === 0) return null;
  const short = lines.filter((l) => checkOf(l).kind === "short").length;

  const confirm = (l: BenchLine) => {
    const said = (typed[l.line_id] ?? "").trim();
    const seen = said === "" ? l.committed : Number.parseInt(said, 10);
    setTyped((t) => ({ ...t, [l.line_id]: "" }));
    setAgain((a) => new Set([...a].filter((id) => id !== l.line_id)));
    void bench.check({ line: l.line_id, expected: l.committed, seen });
  };

  return (
    <Card
      title="Check what came to the bench"
      description={`${done} of ${lines.length} checked${short ? ` · ${short} short` : ""}. Enter confirms; type a number first to say what you see.`}
      padded={false}
    >
      <ul className={s.checks} ref={list}>
        {lines.map((l) => {
          const state = checkOf(l);
          const open = needsCheck(l) || again.has(l.line_id);
          const phrase = expectedIn(l);
          return (
            <li key={l.line_id} className={s.checkRow} data-state={open ? "open" : state.kind}>
              <ItemLine
                code={l.item_code}
                description={l.description ?? undefined}
                tags={l.tags}
                picture={l.picture}
                onOpen={() => look(l.item_id)}
                note={state.kind === "stale" ? "The order changed since it was checked" : undefined}
              />
              <span className={s.checkExpect}>
                <span className={s.expect}>{phrase}</span>
                {phrase.includes(" of ") && <span className={s.note}>{l.committed} in all</span>}
              </span>
              {open ? (
                <form
                  className={s.add}
                  onSubmit={(e) => {
                    e.preventDefault();
                    confirm(l);
                  }}
                >
                  <TextField
                    aria-label={`How many of ${l.item_code} you see`}
                    inputMode="numeric"
                    className={s.qty}
                    placeholder={String(l.committed)}
                    value={typed[l.line_id] ?? ""}
                    onChange={(e) => setTyped((t) => ({ ...t, [l.line_id]: e.target.value }))}
                  />
                  <Button type="submit" size="sm" icon={<Check />} disabled={bench.busy}>
                    Confirm
                  </Button>
                </form>
              ) : (
                <span className={s.checked}>
                  <CheckBadge state={state} />
                  <span className={s.note}>{whoAndWhen(state)}</span>
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={<RotateCcw />}
                    onClick={() => setAgain((a) => new Set([...a, l.line_id]))}
                  >
                    Count again
                  </Button>
                </span>
              )}
            </li>
          );
        })}
      </ul>
    </Card>
  );
}

function CheckBadge({ state }: { state: CheckState }) {
  if (state.kind === "short") return <Badge tone="warning">Short {state.by}</Badge>;
  if (state.kind === "over") return <Badge tone="warning">Over {state.by}</Badge>;
  return (
    <Badge tone="success" dot>
      Checked
    </Badge>
  );
}

/** "Saw 10 of 12 · Alex, 10:41", or who and when for a confirmation. */
function whoAndWhen(state: CheckState): string {
  const c = state.check;
  if (!c) return "";
  const when = new Date(c.at).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  const who = [c.by, when].filter(Boolean).join(", ");
  return state.kind === "agrees" ? who : `Saw ${c.seen} of ${c.expected} · ${who}`;
}
