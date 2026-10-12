import type { BenchLine, CartonSummary, ExpectedWeight, LineCheck, PackedRow } from "@domain/types";
import { kg } from "../../common/format.ts";
import { plural, unitWord } from "../../items/subjects.ts";

/**
 * Checking the order at the bench (D244), as words and verdicts.
 *
 * The packer is shown what should be there for a line, in the units they will
 * see on the bench, and confirms it or says what they see. A carton is not
 * sealed while a line in it is unchecked. A typed weight is the second check:
 * whether it agrees is the site's tolerance, sent with it; what would explain
 * it when it doesn't is said here, as a guess to count again by.
 */

/**
 * What should be on the bench for a line, in the units on the box: "2 cartons
 * of 12 and 5 each", "1 carton of 10 boxes". Its packs above what the line
 * counts, largest first, then what is left in the line's own unit.
 */
export function expectedIn(line: Pick<BenchLine, "committed" | "packs" | "unit">): string {
  const word = unitWord({ unit: line.unit });
  // "of 10 boxes", but "of 12" where the word names nothing.
  const named = word !== "Each" && word !== "Unit";
  let left = line.committed;
  const parts: string[] = [];
  const bigger = line.packs
    .filter((p) => (p.level === "inner" || p.level === "carton") && p.units > 1)
    .sort((a, b) => b.units - a.units);
  for (const p of bigger) {
    const k = Math.floor(left / p.units);
    if (k === 0) continue;
    const what = p.level === "carton" ? "carton" : "pack";
    parts.push(`${k} ${plural(what, k)} of ${p.units}${named ? ` ${plural(word, p.units)}` : ""}`);
    left -= k * p.units;
  }
  if (left > 0 || parts.length === 0) parts.push(`${left} ${plural(word, left)}`);
  return parts.length === 1 ? parts[0]! : `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}`;
}

/** Where a line's check stands. */
export type CheckState =
  | { kind: "unchecked"; check: null }
  /** Checked against another count than the line now commits: the order changed. */
  | { kind: "stale"; check: LineCheck }
  | { kind: "agrees"; check: LineCheck }
  | { kind: "short"; by: number; check: LineCheck }
  | { kind: "over"; by: number; check: LineCheck };

export function checkOf(line: Pick<BenchLine, "check" | "committed">): CheckState {
  const c = line.check;
  if (!c) return { kind: "unchecked", check: null };
  if (c.expected !== line.committed) return { kind: "stale", check: c };
  if (c.seen < c.expected) return { kind: "short", by: c.expected - c.seen, check: c };
  if (c.seen > c.expected) return { kind: "over", by: c.seen - c.expected, check: c };
  return { kind: "agrees", check: c };
}

/** How many fewer were seen than the line commits: what the plan leaves out. */
export function shortOf(line: Pick<BenchLine, "check" | "committed">): number {
  const state = checkOf(line);
  return state.kind === "short" ? state.by : 0;
}

/** The lines as they are on the bench: a short line's missing units are not planned for. */
export function onTheBench<L extends Pick<BenchLine, "check" | "committed" | "remaining">>(lines: readonly L[]): L[] {
  return lines.map((l) => {
    const short = shortOf(l);
    return short > 0 ? { ...l, remaining: Math.max(0, l.remaining - short) } : l;
  });
}

/** Whether a line still needs checking before a carton of it is sealed. */
export function needsCheck(line: Pick<BenchLine, "check" | "committed">): boolean {
  const k = checkOf(line).kind;
  return k === "unchecked" || k === "stale";
}

/** The codes of what is in a carton that nobody has checked: sealing waits for them. */
export function uncheckedIn(carton: Pick<CartonSummary, "contents">, lines: readonly BenchLine[]): string[] {
  const codes = new Set<string>();
  for (const row of carton.contents) {
    if (lines.some((l) => l.item_id === row.item_id && needsCheck(l))) codes.add(row.item_code);
  }
  return [...codes];
}

/** "1.400 kg heavy", "0.300 kg light": the scale against the expectation, once weighed. */
export function offBy(expected: Pick<ExpectedWeight, "delta_g">): string | null {
  if (expected.delta_g === null || expected.delta_g === 0) return null;
  return `${kg(Math.abs(expected.delta_g))} ${expected.delta_g > 0 ? "heavy" : "light"}`;
}

/** Most of one thing a weight is explained by: past this, it is not a miscount. */
const MOST = 12;

/** So many more or fewer of one thing than the carton is said to hold. */
export interface Miscount {
  code: string;
  /** More in it when positive, fewer when negative. */
  by: number;
  each_g: number;
}

/**
 * What would explain a weight that does not agree: so many more or fewer of one
 * thing in the carton, to within the tolerance; the fewest miscounted first,
 * and any others as few. Empty when it agrees, isn't weighed, or nothing in it
 * explains it. A guess to count again by, never a correction.
 */
export function explain(expected: ExpectedWeight, contents: readonly PackedRow[]): Miscount[] {
  const delta = expected.delta_g;
  if (delta === null || expected.agrees !== false) return [];
  const within = (expected.grams * expected.tolerance_per_mille) / 1000;
  const fits = contents
    .flatMap((row) => {
      const each = row.each_g;
      if (!each || each <= 0) return [];
      const k = Math.round(delta / each);
      if (k === 0 || Math.abs(k) > MOST || -k > row.quantity) return [];
      const residual = Math.abs(delta - k * each);
      return residual <= within ? [{ code: row.item_code, by: k, each_g: each, residual }] : [];
    })
    // The fewest miscounted is the likeliest: two wipes, not nine gloves.
    .sort((a, b) => Math.abs(a.by) - Math.abs(b.by) || a.residual - b.residual);
  const fewest = fits[0];
  if (!fewest) return [];
  return fits.filter((f) => Math.abs(f.by) === Math.abs(fewest.by)).map(({ code, by, each_g }) => ({ code, by, each_g }));
}
