import type { ExpectedLine, PackLevel } from "@domain/types";

/**
 * The arithmetic of a delivery, with no React and no network in it.
 *
 * The same split the pick and put-away walks make. Two of these have a wrong
 * answer that reaches the ledger as a quantity.
 */

/** The level with this name, or `each`, which every item can be counted in. */
export function levelNamed(line: ExpectedLine, name: string): PackLevel | undefined {
  return line.levels.find((l) => l.level === name) ?? line.levels[0];
}

/**
 * What a count in this packaging comes to in base units.
 *
 * **Shown before the press, never sent.** `POST /receipts` does the conversion
 * that counts, from the config it loads itself — this is the same sum done twice
 * so the receiver can see "6 cartons is 72" before committing to it. A screen
 * that showed nothing would be asking somebody to trust an arithmetic they
 * cannot check.
 */
export function baseUnits(entered: string, level: PackLevel | undefined): number | null {
  const n = Number.parseInt(entered.trim(), 10);
  if (!Number.isFinite(n) || n <= 0 || !level) return null;
  return n * level.units;
}

/**
 * Whether this count would take the promise past what was promised.
 *
 * Over-delivery is **not** refused — the pallet is on the dock and D5 will not
 * have a receiver blocked to protect a number. What it may become is a finding,
 * and whether it does is the tolerance's decision, which is the server's. So
 * this says *more than expected*, which is a fact, and never *too much*, which
 * would be this screen guessing at a policy it cannot read.
 */
export function overDelivery(line: ExpectedLine, base: number | null): number {
  if (base === null) return 0;
  return Math.max(0, base - line.outstanding);
}

/**
 * Whether the line can be sent at all, and what is missing when it cannot.
 *
 * Returns the sentence to show, or null when it is ready. The three things the
 * write path refuses on, said before the press rather than after: a required lot
 * (`disposition` refuses outright), an owner nobody named, and a count that is
 * not a count.
 */
export function whatIsMissing(
  line: ExpectedLine,
  entered: string,
  lotCode: string,
  owner: string | null,
): string | null {
  const counted = Number.parseInt(entered.trim(), 10);
  if (!Number.isFinite(counted) || counted <= 0) return "Say how many arrived.";
  if (line.requires_lot && lotCode.trim() === "") {
    return "This item is received by lot. Scan or type the lot on the carton.";
  }
  if (!line.owner_id && !owner) {
    return "Say whose the goods are.";
  }
  return null;
}

/**
 * Put a received line's new figures back, and drop it when the promise is met.
 *
 * **Patched, not re-read**, as the two walks are: a delivery is worked down a
 * list and refetching would renumber it under somebody at the dock. A line with
 * nothing outstanding is off the list, because the read's own
 * `quantity_outstanding > 0` says it is not expected any more.
 *
 * `received` comes from what the server wrote, not from what was asked for: the
 * two differ whenever packaging conversion or a clamp changed the number, and
 * the ledger is the one that is right.
 */
export function fold(
  lines: ExpectedLine[],
  supplyId: string,
  base: number,
): ExpectedLine[] {
  return lines.flatMap((line) => {
    if (line.expected_supply_id !== supplyId) return [line];
    const received = line.received + base;
    const outstanding = line.outstanding - base;
    if (outstanding <= 0) return [];
    return [{ ...line, received, outstanding }];
  });
}

/**
 * The name of a receipt.
 *
 * The promise, the delivery it belongs to, and what was counted. A second press
 * behind a lost response is the same act and folds (D5); the same line counted
 * again after a recount is a different act and rightly.
 */
export function receiptKey(
  line: ExpectedLine | null,
  delivery: string | null,
  entered: string,
  level: string,
  lotCode: string,
): string {
  if (!line || !delivery) return "receipt:none";
  return `receipt:${delivery}:${line.expected_supply_id}:${entered.trim()}:${level}:${lotCode.trim()}`;
}
