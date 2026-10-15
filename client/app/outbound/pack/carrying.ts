import type { BenchScreen, BuildRule, RuleKind, RuleSet, Uuid } from "@domain/types";

import { kgLimit } from "../../common/format.ts";
import type { Board, BoardParcel } from "./board.ts";

/**
 * What a parcel carries and what its carrier allows, as the board judges it
 * without knowing where on a pallet each carton sits (D249, D250, settled 10).
 *
 * A pallet is judged by the rules it keeps: a made one by the version it was
 * started under, one only planned by its carrier's rules now. A carton is
 * judged by its pallet's, and on none by the usual carrier's for breakdown
 * pallets, which is where a loose carton goes (settled 1). The board never
 * reads a carrier, only the rules.
 *
 * No React here, so a test runner can read it.
 */

/** Full enough, of things that all take weight, to carry on its lid what its walls do. */
export const SOLID_FILL = 0.85;

/**
 * Whether a carton is hollow, as the bench proposes it at sealing (D249):
 * solid when its goods fill it and every one of them takes weight, hollow
 * otherwise. The packer says the other with one press.
 */
export function proposedHollow(p: BoardParcel): boolean {
  const plan = p.arranged;
  if (!plan || plan.loose.length > 0 || plan.fill < SOLID_FILL) return true;
  const takes = (line: Uuid, level: string) =>
    p.goods.find((g) => g.line.line_id === line)?.line.packs.find((u) => u.level === level)?.on_top_g === null;
  return !plan.layers.every((l) => l.placements.every((x) => takes(x.kind.line, x.kind.level)));
}

/** A rule of a set, when the set has it on. */
export function ruleOf(set: RuleSet | null, kind: RuleKind): BuildRule | null {
  const r = set?.rules.find((x) => x.kind === kind) ?? null;
  return r && r.strength !== "off" ? r : null;
}

/** The carrier a pallet not yet made is built for: the one the board was told, or the usual one. */
export function carrierOf(screen: BenchScreen, said: Uuid | null) {
  return screen.carriers.find((c) => (said ? c.id === said : c.usual)) ?? null;
}

/** The rules a pallet is built to: kept when made, its carrier's now when planned. */
export function palletRules(screen: BenchScreen, p: BoardParcel): RuleSet | null {
  if (p.carton) return p.carton.rules;
  // A pallet the board plans goes with its order, as one.
  return carrierOf(screen, p.planned?.carrier_id ?? null)?.as_one ?? null;
}

/** The rules a parcel is judged by: its pallet's, or a loose carton's (settled 1). */
export function rulesFor(screen: BenchScreen, board: Board, p: BoardParcel): RuleSet | null {
  if (p.kind === "pallet") return palletRules(screen, p);
  const pallet = p.on ? board.parcels.find((x) => x.id === p.on) : undefined;
  if (pallet) return palletRules(screen, pallet);
  return screen.carriers.find((c) => c.usual)?.breakdown ?? null;
}

/** Whether a parcel is delivered on its own: on no pallet, or on one broken down at the depot (D247). */
function deliveredAlone(board: Board, p: BoardParcel): boolean {
  if (p.kind === "pallet") return false;
  if (!p.on) return true;
  return !!board.parcels.find((x) => x.id === p.on)?.carton?.breaks_down;
}

/** What the board says of a parcel by its weight (settled 10). */
export interface Marks {
  /** Over the heavy limit: mark it heavy. */
  heavy: string | null;
  /** Over the loose limit, delivered on its own: it goes on a pallet. */
  pallet: string | null;
  /** A pallet heavier than its rules allow, with what is on it. */
  over: string | null;
}

export function marksOf(screen: BenchScreen, board: Board, p: BoardParcel): Marks {
  const none = { heavy: null, pallet: null, over: null };
  if (p.kind === "pallet") {
    const said = heavierThan(palletRules(screen, p), p.title, palletWeight(screen, board, p).grams);
    return { ...none, over: said.refused ?? said.said };
  }
  if (p.weight_g === null) return none;
  const rules = rulesFor(screen, board, p);
  const heavy = ruleOf(rules, "heavy");
  const loose = ruleOf(rules, "loose");
  const over = (r: BuildRule | null) => r?.amount != null && p.weight_g! > r.amount;
  return {
    heavy: over(heavy) ? `Over ${kgLimit(heavy!.amount)}: mark it heavy` : null,
    pallet: over(loose) && deliveredAlone(board, p) ? `Over ${kgLimit(loose!.amount)}: send it on a pallet` : null,
    over: null,
  };
}

/** What a pallet weighs empty, by its preset, when the workspace says. */
export function palletTare(screen: BenchScreen, pallet: Pick<BoardParcel, "planned" | "carton">): number {
  const preset = pallet.planned
    ? screen.presets.find((x) => x.id === pallet.planned!.package_type_id)
    : screen.presets.find((x) => x.name === pallet.carton?.package_type);
  return preset?.tare_weight_g ?? 0;
}

/** What a pallet weighs with what is on it, and how many on it nothing weighs. */
export function palletWeight(screen: BenchScreen, board: Board, pallet: BoardParcel): { grams: number; unweighed: number } {
  const own = pallet.carton?.gross_weight_g;
  if (own != null) return { grams: own, unweighed: 0 };
  let grams = palletTare(screen, pallet);
  let unweighed = 0;
  for (const p of board.parcels.filter((x) => x.on === pallet.id)) {
    if (p.weight_g === null) unweighed += 1;
    else grams += p.weight_g * p.count;
  }
  return { grams, unweighed };
}

/**
 * Whether a pallet weighing so much keeps within its weight rule: past a
 * must, a drop is refused, as one that won't fit is; past a preference, it is
 * said.
 */
export function heavierThan(rules: RuleSet | null, title: string, grams: number): { refused: string | null; said: string | null } {
  const rule = ruleOf(rules, "weight");
  if (rule?.amount == null || grams <= rule.amount) return { refused: null, said: null };
  const at = `${title} at ${kgLimit(grams)}`;
  return rule.strength === "must"
    ? { refused: `${at} is over the ${kgLimit(rule.amount)} its rules allow`, said: null }
    : { refused: null, said: `${at} is heavier than its rules prefer` };
}

/** "Built for Road · up to 1.8 m and 1,000 kg": a pallet's carrier and limits. */
export function builtFor(screen: BenchScreen, p: BoardParcel): string | null {
  const carrier = p.carton ? screen.carriers.find((c) => c.id === p.carton!.carrier_id) : carrierOf(screen, p.planned?.carrier_id ?? null);
  const rules = palletRules(screen, p);
  if (!carrier && !rules) return null;
  const height = ruleOf(rules, "height")?.amount;
  const weight = ruleOf(rules, "weight")?.amount;
  const limits = [height != null ? `${height / 1000} m` : null, weight != null ? kgLimit(weight) : null].filter(Boolean).join(" and ");
  return [carrier ? `Built for ${carrier.name}` : null, limits ? `up to ${limits}` : rules ? null : "no rules"].filter(Boolean).join(" · ");
}
