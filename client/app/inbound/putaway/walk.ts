import type { Home, PutawayCell, PutawayScreen } from "@domain/types";

/**
 * The arithmetic of a put-away, with no React and no network in it.
 *
 * The same split `picking/walk.ts` and `capture/figures.ts` make: the parts with
 * a wrong answer that reaches the ledger are separated so a test can ask them
 * directly.
 */

/** What the screen offers to move: the whole cell, which is the usual case. */
export function offered(cell: PutawayCell): number {
  return cell.quantity;
}

/**
 * The nearest bin that already holds this item, if any.
 *
 * **Offered, never chosen.** Consolidating with what is already there is the
 * obvious thing to do and the operator is the one who can see whether it fits.
 * The list arrives in walk order, so the first is the nearest.
 */
export function nearestHome(cell: PutawayCell): Home | undefined {
  return cell.homes[0];
}

/**
 * Put a moved cell's new figure back, and drop it when the dock is clear of it.
 *
 * **Patched, not re-read**, for the reason the pick walk gives: refetching
 * renumbers the list under somebody standing at the dock. A cell moved in full
 * is off the dock and leaves; a cell moved in part stays with what is left,
 * because the rest still needs a home.
 */
export function fold(
  screen: PutawayScreen,
  stockId: string,
  moved: number,
): PutawayScreen {
  const cells = screen.cells.flatMap((cell) => {
    if (cell.stock_id !== stockId) return [cell];
    const quantity = cell.quantity - moved;
    if (quantity <= 0) return [];
    // `available` cannot go below zero and cannot exceed what is left: moving
    // goods does not release a claim, so the free part shrinks first.
    const available = Math.max(0, Math.min(quantity, cell.available - moved));
    return [{ ...cell, quantity, available }];
  });
  return { ...screen, cells };
}

/**
 * The name of a put-away.
 *
 * Cell, destination and amount. A second press behind a lost response is the
 * same act and folds (D5); the same cell sent to a different bin is a different
 * act and rightly.
 */
export function awayKey(
  cell: PutawayCell | null,
  to: { id: string } | null,
  quantity: string,
): string {
  if (!cell || !to) return "putaway:none";
  return `putaway:${cell.stock_id}:${to.id}:${quantity.trim()}`;
}
