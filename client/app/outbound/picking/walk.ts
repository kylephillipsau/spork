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
