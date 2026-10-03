import type { PickLine, PickListScreen, Uuid } from "@domain/types";

/**
 * The arithmetic of a walk, with no React and no network in it.
 *
 * Three small functions, and each one has a wrong answer that reaches the
 * ledger. They live here rather than in the hook so a test can ask them
 * directly — the same split `capture/figures.ts` makes, for the same reason.
 */

/**
 * **The claim to make before picking `q` from this line.**
 *
 * `covered` counts every claim in a covering state (J31), `picked` is what the
 * ledger has folded, and J56 raises a finding when `picked` exceeds `covered`.
 * The shortfall is exactly what would breach it — usually nothing, because
 * planning covered the line when the order arrived.
 *
 * **Both easy answers are wrong.** Always claiming is refused outright with
 * `OverCovers` on any line planning had already covered. Never claiming raises
 * J56 against a warehouse that did nothing wrong — which is what
 * `api.pickInto`'s comment means by *"the allocation is not a formality, it is
 * what stops the bench seeding findings for everyone else"*. Both were
 * reachable from this screen, which is why `PickLine` gained `picked` and
 * `covered`: a bool about one bin cannot answer this.
 */
export function claimFor(line: PickLine, quantity: number): number {
  return Math.max(0, line.picked + quantity - line.covered);
}

/**
 * What the screen offers to take: what is wanted, or what is there.
 *
 * A bin holding less than the line wants is a short pick, and the offer is the
 * smaller figure so the common case is one press. The picker can type over it —
 * they are the one holding the goods.
 */
export function offered(line: PickLine): number {
  return line.available === null ? line.remaining : Math.min(line.remaining, line.available);
}

/**
 * Put a served line's new figures back, and drop it when it is done.
 *
 * **Patched, not re-read.** The pick's response carries the live fold for the
 * line it served, so the row is corrected from what happened rather than from a
 * second query — and the walk keeps its order under somebody standing in an
 * aisle. Weigh makes the same argument in its own words: refetching renumbers
 * the queue between one box and the next.
 *
 * A line with nothing left to pick leaves the list, because the read's own
 * `WHERE fl.picked_quantity < fl.quantity` says it is not on the walk. Keeping
 * a satisfied row on screen would be the client disagreeing with the list it
 * was given.
 */
export function fold(
  screen: PickListScreen,
  lineId: Uuid,
  picked: number,
  covered: number,
): PickListScreen {
  const lines = screen.lines.flatMap((line) => {
    if (line.fulfilment_line_id !== lineId) return [line];
    // From the server's figure, not from what was asked for: a pick that landed
    // as something other than the amount requested is the case where the two
    // differ, and the ledger is the one that is right.
    const remaining = line.remaining - (picked - line.picked);
    if (remaining <= 0) return [];
    return [{ ...line, picked, covered, remaining }];
  });
  return { ...screen, lines };
}

/**
 * A fresh read laid over the walk somebody is standing in (D206).
 *
 * When somebody else picks, the device hears it and reads the list again. Laid
 * down as read, that would be the refetch `fold` exists to avoid, so it is laid
 * over what is on screen instead. A row still on the walk keeps its place and
 * takes the fresh figures. A row somebody else finished leaves. A new row, an
 * order that has just arrived, goes in after the row it follows in the fresh
 * read, which is where the walk would have put it.
 */
export function merge(current: PickListScreen, fresh: PickListScreen): PickListScreen {
  const now = new Map(fresh.lines.map((line) => [line.fulfilment_line_id, line]));
  const lines = current.lines.flatMap((line) => {
    const next = now.get(line.fulfilment_line_id);
    return next ? [next] : [];
  });
  const shown = new Set(lines.map((line) => line.fulfilment_line_id));
  fresh.lines.forEach((line, at) => {
    if (shown.has(line.fulfilment_line_id)) return;
    let after = -1;
    for (let back = at - 1; back >= 0 && after < 0; back -= 1) {
      const before = fresh.lines[back];
      if (before) after = lines.findIndex((l) => l.fulfilment_line_id === before.fulfilment_line_id);
    }
    lines.splice(after + 1, 0, line);
    shown.add(line.fulfilment_line_id);
  });
  return { ...fresh, lines };
}

/** A pick kept on the device and not yet sent (D207), as the walk counts it. */
export interface Waiting {
  line: Uuid;
  quantity: number;
  claim: number;
}

/**
 * The walk as the picker has left it: what the server said, less what this
 * device has picked and not yet been able to send (D207).
 *
 * **A view over the read, never written into it.** The goods are in the tote,
 * so the row shows them taken, and a picker isn't sent back to the shelf for
 * them. But the server hasn't heard yet, and the next read says so; laying
 * this over each read, rather than folding it in once, is what stops that read
 * putting the row back.
 */
export function overlay(screen: PickListScreen, waiting: readonly Waiting[]): PickListScreen {
  if (waiting.length === 0) return screen;
  const by = new Map<Uuid, { quantity: number; claim: number }>();
  for (const w of waiting) {
    const sum = by.get(w.line) ?? { quantity: 0, claim: 0 };
    by.set(w.line, { quantity: sum.quantity + w.quantity, claim: sum.claim + w.claim });
  }
  const lines = screen.lines.flatMap((line) => {
    const held = by.get(line.fulfilment_line_id);
    if (!held) return [line];
    const remaining = line.remaining - held.quantity;
    if (remaining <= 0) return [];
    return [
      {
        ...line,
        picked: line.picked + held.quantity,
        covered: line.covered + held.claim,
        remaining,
        available: line.available === null ? null : Math.max(0, line.available - held.quantity),
      },
    ];
  });
  return { ...screen, lines };
}
