import type { BenchLine, BenchScreen, CartonSummary, Footprint, PlannedAsIs, PlannedParcel, Preset, RuleSet, StatedSize, Uuid } from "@domain/types";

import { keepOf } from "../../items/box.ts";
import { arrange, boxFreight, fitInto, piecesOf, type AsIs, type Aside, type BoxPlan, type Goods, type Layer } from "./arrange.ts";
import { carrierOf, heavierThan, palletRules, palletTare, palletWeight } from "./carrying.ts";
import { shortOf } from "./check.ts";
import { LEVEL_WORDS, account, boxNote, cartonDetail, cartonWeight, dimsOf, lookOf, type OrderView, type Parcel, type ParcelLine } from "./order.ts";

/**
 * The board (D245): the whole order as parcels to drag goods and cartons
 * between, and what each drag does.
 *
 * A parcel is made (a carton or a pallet, from the server) or planned (the
 * kept draft, migration 142). What goes into a planned parcel changes the plan
 * only; what goes into a made, open carton is the act itself (D246). Things
 * that ship as they are (D196) are parcels of their own, worked out from what
 * is left, and made by Ship.
 *
 * Things are named by key, as the kit's drag names them: `goods:<line>` is a
 * line's units still to place, `goods:<line>@<parcel>` its units in or planned
 * into a parcel, `parcel:<id>` a parcel. A drop lands on `parcel:<id>`,
 * `new-box` (the smallest that takes it), `new-box:<preset>` (that box),
 * `new-pallet` or `bench` (To pack).
 *
 * No React here, so a test runner can read it.
 */

export type Box = Preset & { size: StatedSize };

/** A line on the board: what it still needs, and what of that nothing holds yet. */
export interface BoardLine {
  line: BenchLine;
  /** Committed and in no carton, less what the check found short (D244). */
  toPack: number;
  /** Planned into a parcel not yet made. */
  planned: number;
  /** In nothing and planned into nothing: what To pack offers to drag. */
  left: number;
}

export interface BoardParcel extends Parcel {
  /** The carton's id, or the one it will be made with. */
  id: Uuid;
  made: boolean;
  kind: "box" | "pallet" | "as_is";
  /** A box's preset, sized: what fits are worked out against. */
  preset: Box | null;
  /** A pallet's deck. */
  deck: Footprint | null;
  /** The pallet it is on, made or planned. */
  on: Uuid | null;
  /** Planned by the packer rather than by Suggest. */
  byHand: boolean;
  /** What is in it, or planned into it, line by line. */
  goods: Goods[];
  /** As arranged: how full, and what its goods weigh. */
  arranged: BoxPlan | null;
  limit_g: number | null;
  carton: CartonSummary | null;
  planned: PlannedParcel | null;
}

export interface Board {
  lines: BoardLine[];
  /** Made parcels by number, then planned in the plan's order, then those that ship as they are. */
  parcels: BoardParcel[];
  /** The tally and the booking (D202, D224), counted as the board has it. */
  view: OrderView;
}

export const goodsKey = (line: Uuid, parcel: Uuid | null = null) => (parcel ? `goods:${line}@${parcel}` : `goods:${line}`);
export const parcelKey = (id: Uuid) => `parcel:${id}`;

/** What a key names. */
export type Thing = { kind: "goods"; line: Uuid; parcel: Uuid | null } | { kind: "parcel"; id: Uuid };

export function thingOf(key: string): Thing | null {
  if (key.startsWith("parcel:")) return { kind: "parcel", id: key.slice(7) };
  if (!key.startsWith("goods:")) return null;
  const [line, parcel] = key.slice(6).split("@");
  return { kind: "goods", line: line!, parcel: parcel ?? null };
}

/** A carton's preset, which the bench names it by (D196). */
export function presetOf(screen: BenchScreen, c: CartonSummary): Preset | null {
  return c.package_type ? (screen.presets.find((p) => p.name === c.package_type) ?? null) : null;
}

const sized = (p: Preset | null | undefined): p is Box => !!p && p.size !== null;

/** Units of each line, summed. */
export function byLine(goods: Iterable<{ line: Uuid; units: number }>): Map<Uuid, number> {
  const out = new Map<Uuid, number>();
  for (const g of goods) out.set(g.line, (out.get(g.line) ?? 0) + g.units);
  return out;
}

/** What the placements of an arrangement hold of each line. */
function placedOf(layers: Layer[]): Map<Uuid, number> {
  return byLine(layers.flatMap((l) => l.placements.map((p) => ({ line: p.kind.line, units: p.kind.units }))));
}

/**
 * An arrangement with what was there before marked in and the rest to go
 * in, as the 3D draws a carton being filled (D198).
 */
function marked(plan: BoxPlan, before: Map<Uuid, number>): BoxPlan {
  const left = new Map(before);
  return {
    ...plan,
    layers: plan.layers.map((layer) => ({
      ...layer,
      placements: layer.placements.map((p) => {
        const there = left.get(p.kind.line) ?? 0;
        if (there >= p.kind.units) left.set(p.kind.line, there - p.kind.units);
        return { ...p, packed: there >= p.kind.units };
      }),
    })),
  };
}

function parcelLines(goods: Goods[], state: ParcelLine["state"]): ParcelLine[] {
  return goods.map((g) => ({ line: g.line.line_id, item_id: g.line.item_id, item_code: g.line.item_code, units: g.units, state }));
}

export function boardOf(screen: BenchScreen): Board {
  const lineOf = new Map(screen.lines.map((l) => [l.line_id, l]));
  const goodsOf = (rows: { line: Uuid; units: number }[]): Goods[] =>
    [...byLine(rows)].flatMap(([id, units]) => (lineOf.has(id) && units > 0 ? [{ line: lineOf.get(id)!, units }] : []));

  /** One of a line's item as it is (D196): to ship, or planned onto a pallet first (migration 144). */
  const asIsParcel = (id: Uuid, a: AsIs, pp: PlannedParcel | null): BoardParcel => {
    const line = lineOf.get(a.line)!;
    return {
      key: parcelKey(id),
      id,
      made: false,
      kind: "as_is",
      title: `${a.item_code} ${LEVEL_WORDS[a.level]}`,
      detail: null,
      state: pp ? "planned" : "as-is",
      size: a.size,
      box: null,
      asIs: a,
      count: a.count,
      lines: [{ line: a.line, item_id: a.item_id, item_code: a.item_code, units: a.units, state: "planned" }],
      loose: [],
      layers: null,
      index: a.index,
      faces: a.faces,
      round: a.round,
      keep: 1,
      weight_g: a.weight_g === null ? null : a.weight_g / a.count,
      weighed: false,
      weightNote: a.weight_g === null ? "not weighed" : null,
      preset: null,
      deck: null,
      on: pp ? (pp.on_parcel_id ?? pp.on_package_id) : null,
      byHand: pp?.placed_by_hand ?? false,
      goods: [{ line, units: a.units }],
      arranged: null,
      limit_g: null,
      carton: null,
      planned: pp,
    };
  };

  // ── made ───────────────────────────────────────────────────────────────
  const made: BoardParcel[] = screen.cartons.map((c) => {
    const preset = presetOf(screen, c);
    const kind = c.kind === "as_is" ? "as_is" : c.kind === "pallet" ? "pallet" : "box";
    const goods = goodsOf(c.contents.map((r) => ({ line: r.line_id, units: r.quantity })));
    const box: Box | null = kind === "box" && c.stated_size ? { ...(preset ?? DEFAULT_BOX), size: c.stated_size } : null;
    const arranged = box ? fitInto(box, goods).plan : null;
    return {
      key: parcelKey(c.id),
      id: c.id,
      made: true,
      kind,
      title: `${kind === "pallet" ? "Pallet" : "Carton"} ${c.sequence}`,
      detail: cartonDetail(c),
      state: c.sealed ? "sealed" : "open",
      size: dimsOf(c.stated_size),
      box: null,
      asIs: null,
      count: 1,
      lines: parcelLines(goods, "in"),
      loose: [],
      layers: arranged ? marked(arranged, byLine(goods.map((g) => ({ line: g.line.line_id, units: g.units })))).layers : null,
      index: null,
      ...lookOf(screen, c),
      ...cartonWeight(c, null),
      preset: box,
      deck: preset?.deck ?? null,
      on: c.on?.id ?? null,
      byHand: true,
      goods,
      arranged,
      limit_g: preset?.max_payload_g ?? null,
      carton: c,
      planned: null,
    };
  });
  const inCartons = byLine(made.flatMap((p) => p.goods.map((g) => ({ line: g.line.line_id, units: g.units }))));

  // ── planned ────────────────────────────────────────────────────────────
  const planned: BoardParcel[] = screen.plan.map((pp) => {
    // A thing as it is, unless its line has gone: then it shows as nothing on file.
    if (pp.as_is && lineOf.has(pp.lines[0]?.line_id ?? "")) return asIsParcel(pp.id, plannedAsIs(pp, pp.as_is, lineOf), pp);
    const preset = screen.presets.find((p) => p.id === pp.package_type_id) ?? null;
    const goods = goodsOf(pp.lines.map((l) => ({ line: l.line_id, units: l.quantity })));
    const box = sized(preset) ? preset : null;
    const arranged = box ? fitInto(box, goods).plan : null;
    return {
      key: parcelKey(pp.id),
      id: pp.id,
      made: false,
      kind: preset?.deck ? "pallet" : "box",
      title: preset?.name ?? "A box no longer on file",
      detail: null,
      state: "planned",
      size: dimsOf(box?.size ?? null),
      box: null,
      asIs: null,
      count: 1,
      lines: parcelLines(goods, "planned"),
      loose: [],
      layers: arranged ? marked(arranged, new Map()).layers : null,
      index: null,
      faces: preset?.faces ?? {},
      round: null,
      keep: keepOf(preset?.cut ?? null, preset?.size ?? null),
      weight_g: arranged ? boxFreight(arranged).weight_g : null,
      weighed: false,
      weightNote: arranged ? boxNote(arranged) : null,
      preset: box,
      deck: preset?.deck ?? null,
      on: pp.on_parcel_id ?? pp.on_package_id,
      byHand: pp.placed_by_hand,
      goods,
      arranged,
      limit_g: preset?.max_payload_g ?? null,
      carton: null,
      planned: pp,
    };
  });
  const plannedIn = byLine(planned.flatMap((p) => p.goods.map((g) => ({ line: g.line.line_id, units: g.units }))));

  // ── what is left, and what of it ships as it is ────────────────────────
  const toPack = (l: BenchLine) => Math.max(0, l.committed - (inCartons.get(l.line_id) ?? 0) - shortOf(l));
  const unplanned = screen.lines.map((l) => ({ ...l, remaining: Math.max(0, toPack(l) - (plannedIn.get(l.line_id) ?? 0)) }));
  const { asIs } = piecesOf(unplanned);
  const asIsOf = byLine(asIs.map((a) => ({ line: a.line, units: a.units })));
  const shipping: BoardParcel[] = asIs.map((a) => asIsParcel(`as-is:${a.line}:${a.level}`, a, null));

  const lines: BoardLine[] = screen.lines.map((l) => {
    const pack = toPack(l);
    const plannedUnits = plannedIn.get(l.line_id) ?? 0;
    return { line: l, toPack: pack, planned: plannedUnits, left: Math.max(0, pack - plannedUnits - (asIsOf.get(l.line_id) ?? 0)) };
  });
  const parcels = [...made, ...planned, ...shipping];
  const unplaced: Aside[] = lines
    .filter((b) => b.left > 0)
    .map((b) => ({ line: b.line.line_id, item_id: b.line.item_id, item_code: b.line.item_code, units: b.left, weight_g: null }));
  // What the carrier is handed (D247): a parcel on a pallet that goes as one
  // goes with it, and a pallet broken down at the depot is not handed over.
  const breaks = (id: Uuid | null) => !!id && !!parcels.find((p) => p.id === id)?.carton?.breaks_down;
  const delivered = parcels.filter((p) => (p.on === null || breaks(p.on)) && !(p.kind === "pallet" && p.carton?.breaks_down));
  return { lines, parcels, view: account(screen, parcels, { unplaced, placedLoose: [], tooMany: false }, delivered) };
}

/** A box with no preset on file still has a size from the carton; it has no limit. */
const DEFAULT_BOX: Preset = { id: "", name: "", size: null, suggested: false, max_payload_g: null, tare_weight_g: null, deck: null, cut: null, faces: {} };

/** A change the server records: done by the bench's acts, undone by their opposites. */
export type BoardAct =
  /** Units still to place into a made carton: from the bench first, then a bin, then handed over. */
  | { kind: "put"; line: Uuid; units: number; carton: Uuid }
  /**
   * A put undone straight after (settled 2): what went into the carton
   * reversed, newest first, so units picked go back to their bin. Later, a
   * take out puts them on the bench instead.
   */
  | { kind: "unput"; line: Uuid; units: number; carton: Uuid }
  /** From one made carton to another, or to the bench when `to` is null (settled 2). */
  | { kind: "repack"; line: Uuid; units: number; from: Uuid; to: Uuid | null }
  /** A planned parcel made into a carton, and its goods put in. */
  | { kind: "make"; parcel: Uuid }
  | { kind: "contain"; carton: Uuid; pallet: Uuid }
  /** Off its pallet, back on the bench. */
  | { kind: "takeOff"; carton: Uuid }
  /**
   * A carton made by the step before, taken apart to undo it (settled 5):
   * opened, its puts undone, off its pallet and discarded, with the pallet
   * that step made for it. Worked out against the bench as it is when undone.
   */
  | { kind: "unmake"; carton: Uuid; pallet: Uuid | null };

/** What a drop does, or would, and how to go back. */
export interface Outcome {
  ok: boolean;
  /** Said while dragging and after: "Into Carton 2 · 58% full", "8 of 12 fit · 4 stay", "Carton 1 is sealed". */
  verdict: string;
  /** The plan as it would be, when the drop changes it. */
  plan: PlannedParcel[] | null;
  acts: BoardAct[];
  undo: { plan: PlannedParcel[] | null; acts: BoardAct[] } | null;
  /** The parcel it lands in, arranged with it, to draw. */
  shown: { parcel: Uuid; arranged: BoxPlan } | null;
  /** Nothing about it says which box: ask the packer, then drop on `new-box:<preset>`. */
  chooseBox: boolean;
}

const refused = (verdict: string, chooseBox = false): Outcome => ({ ok: false, verdict, plan: null, acts: [], undo: null, shown: null, chooseBox });

/** Whether a line has no size at any level: a box for it is the packer's to choose, and its fit theirs to judge. */
const unmeasured = (line: BenchLine) => !line.packs.some((p) => p.size);

/** How many: as many as there are, one with Shift, or the count asked for with Alt (settled 5). */
export interface HowMany {
  one: boolean;
  count?: number | undefined;
}

const percent = (n: number) => `${Math.round(n * 100)}%`;

/**
 * The plan from before parcels were made, to plan them again: under new ids,
 * since the old ones are cartons now, discarded but on record.
 */
function afresh(plan: readonly PlannedParcel[], made: readonly Uuid[], newId: () => Uuid): PlannedParcel[] {
  const ids = new Map(made.map((id) => [id, newId()]));
  return plan.map((pp) => ({ ...pp, id: ids.get(pp.id) ?? pp.id, on_parcel_id: pp.on_parcel_id && (ids.get(pp.on_parcel_id) ?? pp.on_parcel_id) }));
}

/**
 * Make a planned parcel (D245), and how to go back: taken apart, and planned
 * again as it was. A planned pallet it goes on is made with it, and undone
 * with it.
 */
export function making(screen: BenchScreen, parcel: Uuid, newId: () => Uuid): Pick<Outcome, "plan" | "acts" | "undo"> {
  const pp = screen.plan.find((p) => p.id === parcel);
  const pallet = pp?.on_parcel_id ?? null;
  return {
    // Making uses the plan's row up on the server; nothing to write to it here.
    plan: null,
    acts: [{ kind: "make", parcel }],
    undo: pp ? { plan: afresh(screen.plan, pallet ? [parcel, pallet] : [parcel], newId), acts: [{ kind: "unmake", carton: parcel, pallet }] } : null,
  };
}

/** A thing as it is, planned: the piece its units make, as Ship would make it. Its line is on the screen. */
function plannedAsIs(pp: PlannedParcel, as: PlannedAsIs, lineOf: Map<Uuid, BenchLine>): AsIs {
  const { line_id, quantity: units } = pp.lines[0]!;
  const line = lineOf.get(line_id)!;
  const piece = piecesOf([{ ...line, remaining: units }]).asIs.find((a) => a.level === as.level);
  if (piece) return piece;
  // Its item no longer ships as it is at that level: shown unsized.
  return { line: line_id, item_id: as.item_id, item_code: line.item_code, units, weight_g: null, level: as.level, count: 1, per: units, size: null, faces: {}, round: null, index: 0 };
}

/** The plan as the server keeps it, from the board's planned parcels. */
function planOf(board: Board): PlannedParcel[] {
  return board.parcels.flatMap((p) => (p.planned ? [p.planned] : []));
}

function withLine(pp: PlannedParcel, line: Uuid, by: number): PlannedParcel {
  const there = pp.lines.find((l) => l.line_id === line);
  const lines = there
    ? pp.lines.map((l) => (l.line_id === line ? { ...l, quantity: l.quantity + by } : l)).filter((l) => l.quantity > 0)
    : by > 0
      ? [...pp.lines, { line_id: line, quantity: by }]
      : pp.lines;
  return { ...pp, lines };
}

/**
 * What dropping these things on `target` does (D245, settled 5). Goods go in
 * as many as fit and the rest stay; a carton goes on a pallet; dropped on To
 * pack, goods come out (onto the bench, settled 2) and a carton comes off its
 * pallet. Into a planned parcel it changes the plan only; into a made, open
 * carton it is the act. Several things go one after another, each round what
 * the last put in.
 */
export function dropOn(screen: BenchScreen, board: Board, keys: readonly string[], target: string, how: HowMany, newId: () => Uuid): Outcome {
  let plan = planOf(board);
  const before = plan;
  const acts: BoardAct[] = [];
  const undo: BoardAct[] = [];
  const said: string[] = [];
  let shown: Outcome["shown"] = null;
  let changed = false;
  // Planned pallets this drop makes: made once, and undone by taking them apart.
  const made: Uuid[] = [];
  // What this drop has put on each pallet so far, for its weight rule (settled 10).
  const added = new Map<Uuid, number>();
  const parcel = (id: Uuid) => board.parcels.find((p) => p.id === id) ?? null;
  const lineOf = (id: Uuid) => screen.lines.find((l) => l.line_id === id) ?? null;
  // What is in or planned into a parcel now, with the drops so far.
  const inside = new Map<Uuid, Goods[]>(board.parcels.map((p) => [p.id, [...p.goods]]));
  const add = (id: Uuid, line: BenchLine, units: number) => inside.set(id, [...(inside.get(id) ?? []), { line, units }]);

  // A new planned parcel, or the first new one this drop made.
  let fresh: PlannedParcel | null = null;
  const newPallet = (): PlannedParcel | null => {
    const pallet = screen.presets.find((p) => p.deck && p.name.toUpperCase() === "PALLET") ?? screen.presets.find((p) => p.deck);
    if (!pallet) return null;
    const pp: PlannedParcel = { id: newId(), package_type_id: pallet.id, as_is: null, on_parcel_id: null, on_package_id: null, at: null, placed_by_hand: true, carrier_id: null, lines: [] };
    plan = [...plan, pp];
    changed = true;
    return pp;
  };

  for (const key of keys) {
    const thing = thingOf(key);
    if (!thing) continue;

    if (thing.kind === "goods") {
      const line = lineOf(thing.line);
      const from = thing.parcel ? parcel(thing.parcel) : null;
      if (!line || (thing.parcel && !from)) continue;
      const have = from
        ? (from.goods.find((g) => g.line.line_id === line.line_id)?.units ?? 0)
        : (board.lines.find((b) => b.line.line_id === line.line_id)?.left ?? 0);
      const want = Math.min(have, how.count ?? (how.one ? 1 : have));
      if (want <= 0) {
        said.push(`No ${line.item_code} left to move`);
        continue;
      }

      if (target === "bench") {
        if (!from) {
          said.push(`${line.item_code} is on the bench already`);
          continue;
        }
        if (from.made) {
          if (from.state === "sealed") return refused(`${from.title} is sealed: open it to take anything out`);
          acts.push({ kind: "repack", line: line.line_id, units: want, from: from.id, to: null });
          undo.unshift({ kind: "put", line: line.line_id, units: want, carton: from.id });
          said.push(`${want} ${line.item_code} out of ${from.title}, onto the bench`);
        } else if (from.planned) {
          plan = plan.map((pp) => (pp.id === from.id ? withLine(pp, line.line_id, -want) : pp));
          changed = true;
          said.push(`${want} ${line.item_code} back to To pack`);
        }
        continue;
      }

      let into: BoardParcel | null = null;
      let box: Box | null = null;
      if (target === "new-box" || target.startsWith("new-box:")) {
        // The box named, or the smallest that takes them, as the suggestion
        // would choose it. Nothing measured, there is nothing to choose by.
        const named = target.startsWith("new-box:") ? screen.presets.find((p) => p.id === target.slice(8)) : undefined;
        const chosen: Box | undefined = named && sized(named) ? named : arrange([{ ...line, remaining: want }], screen.presets).boxes[0]?.preset;
        if (!chosen) {
          if (unmeasured(line)) return refused(`${line.item_code} isn't measured, so choose its box`, true);
          said.push(`No box takes ${line.item_code}`);
          continue;
        }
        if (!fresh || fresh.package_type_id !== chosen.id) {
          fresh = { id: newId(), package_type_id: chosen.id, as_is: null, on_parcel_id: null, on_package_id: null, at: null, placed_by_hand: true, carrier_id: null, lines: [] };
          plan = [...plan, fresh];
          changed = true;
        }
        box = chosen;
      } else {
        const t = thingOf(target);
        into = t?.kind === "parcel" ? parcel(t.id) : null;
        if (!into) return refused("Drop it on a parcel, or below for a new box");
        if (into.id === from?.id) return refused(`Already in ${into.title}`);
        if (into.kind === "pallet") return refused(`Goods go into a box: drop them on a box on ${into.title}, or below for a new box`);
        if (into.kind === "as_is") return refused(`${into.title} ships as it is`);
        if (into.state === "sealed") return refused(`${into.title} is sealed: open it to add to it`);
        if (from?.made && !into.made) return refused(`Put them into a carton, or out on the bench, then plan them`);
        box = into.preset;
        if (!box) return refused(`${into.title} has no size to fit things in`);
      }

      const at = into?.id ?? fresh!.id;
      const fit = fitInto(box, inside.get(at) ?? [], { line, units: want });
      if (fit.fits === 0) {
        said.push(`${line.item_code} won't fit ${into?.title ?? box.name}`);
        continue;
      }
      shown = { parcel: at, arranged: marked(fit.plan, byLine((inside.get(at) ?? []).map((g) => ({ line: g.line.line_id, units: g.units })))) };
      add(at, line, fit.fits);
      if (into?.made) {
        if (from?.made) {
          acts.push({ kind: "repack", line: line.line_id, units: fit.fits, from: from.id, to: into.id });
          undo.unshift({ kind: "repack", line: line.line_id, units: fit.fits, from: into.id, to: from.id });
        } else {
          acts.push({ kind: "put", line: line.line_id, units: fit.fits, carton: into.id });
          undo.unshift({ kind: "unput", line: line.line_id, units: fit.fits, carton: into.id });
          if (from?.planned) {
            plan = plan.map((pp) => (pp.id === from.id ? withLine(pp, line.line_id, -fit.fits) : pp));
            changed = true;
          }
        }
      } else {
        plan = plan.map((pp) => {
          if (pp.id === at) return { ...withLine(pp, line.line_id, fit.fits), placed_by_hand: true };
          if (pp.id === from?.id) return withLine(pp, line.line_id, -fit.fits);
          return pp;
        });
        changed = true;
      }
      const name = into?.title ?? `a new ${box.name}`;
      said.push(
        unmeasured(line)
          ? `${want} ${line.item_code} into ${name} · not measured, so check it fits`
          : fit.fits < want
            ? `${fit.fits} of ${want} ${line.item_code} fit ${name} · ${want - fit.fits} stay`
            : `${want} ${line.item_code} into ${name} · ${percent(fit.plan.fill)} full`,
      );
      continue;
    }

    // ── a parcel ───────────────────────────────────────────────────────
    const moving = parcel(thing.id);
    if (!moving) continue;
    if (moving.kind === "pallet") return refused(`A pallet goes on nothing; drop cartons on it`);
    if (target === "bench") {
      if (!moving.on) {
        said.push(`${moving.title} is on no pallet`);
        continue;
      }
      if (moving.made) {
        acts.push({ kind: "takeOff", carton: moving.id });
        undo.unshift({ kind: "contain", carton: moving.id, pallet: moving.on });
      } else {
        plan = plan.map((pp) => (pp.id === moving.id ? { ...pp, on_parcel_id: null, on_package_id: null, at: null } : pp));
        changed = true;
      }
      said.push(`${moving.title} off its pallet`);
      continue;
    }
    let pallet: { id: Uuid; made: boolean; title: string; grams: number; rules: RuleSet | null } | null = null;
    if (target === "new-pallet") {
      const pp = newPallet();
      if (!pp) return refused("There is no pallet preset to start one with");
      const rules = carrierOf(screen, null)?.as_one ?? null;
      pallet = { id: pp.id, made: false, title: "a new pallet", grams: palletTare(screen, { planned: pp, carton: null }), rules };
    } else {
      const t = thingOf(target);
      const onto = t?.kind === "parcel" ? parcel(t.id) : null;
      if (!onto || onto.kind !== "pallet") return refused(`${moving.title} goes on a pallet`);
      if (onto.id === moving.on) return refused(`${moving.title} is on ${onto.title} already`);
      pallet = { id: onto.id, made: onto.made, title: onto.title, grams: palletWeight(screen, board, onto).grams, rules: palletRules(screen, onto) };
    }
    // Its weight with this on it, against its rules (settled 10).
    const going = moving.asIs ? Math.min(moving.count, how.count ?? (how.one ? 1 : moving.count)) : 1;
    const adding = (moving.weight_g ?? 0) * going;
    const heavier = heavierThan(pallet.rules, pallet.title, pallet.grams + (added.get(pallet.id) ?? 0) + adding);
    if (heavier.refused) return refused(heavier.refused);
    added.set(pallet.id, (added.get(pallet.id) ?? 0) + adding);
    if (moving.made) {
      // A made carton goes on a made pallet: one only planned is made first.
      if (!pallet.made && !made.includes(pallet.id)) {
        acts.push({ kind: "make", parcel: pallet.id });
        made.push(pallet.id);
        undo.unshift({ kind: "unmake", carton: pallet.id, pallet: null });
      }
      acts.push({ kind: "contain", carton: moving.id, pallet: pallet.id });
      undo.unshift(moving.on ? { kind: "contain", carton: moving.id, pallet: moving.on } : { kind: "takeOff", carton: moving.id });
    } else if (moving.planned) {
      plan = plan.map((pp) =>
        pp.id === moving.id
          ? { ...pp, on_parcel_id: pallet!.made ? null : pallet!.id, on_package_id: pallet!.made ? pallet!.id : null, at: null, placed_by_hand: true }
          : pp,
      );
      changed = true;
    } else if (moving.asIs) {
      // Things as they are, planned onto it one parcel each, as Ship would make them.
      const a = moving.asIs;
      const config = a.level === "each" ? null : (moving.goods[0]?.line.own_carton?.item_packing_config_id ?? null);
      for (let i = 0; i < going; i++) {
        plan = [
          ...plan,
          {
            id: newId(),
            package_type_id: null,
            as_is: { item_id: a.item_id, level: a.level, config_id: config },
            on_parcel_id: pallet.made ? null : pallet.id,
            on_package_id: pallet.made ? pallet.id : null,
            at: null,
            placed_by_hand: true,
            carrier_id: null,
            lines: [{ line_id: a.line, quantity: a.per }],
          },
        ];
      }
      changed = true;
    }
    said.push(`${moving.title} onto ${pallet.title}`);
    if (heavier.said) said.push(heavier.said);
  }

  if (!changed && acts.length === 0) return refused(said.join(" · ") || "Nothing to move");
  return {
    ok: true,
    verdict: said.join(" · "),
    plan: changed ? plan : null,
    acts,
    undo: { plan: changed || made.length > 0 ? afresh(before, made, newId) : null, acts: undo },
    shown,
    chooseBox: false,
  };
}

/**
 * A pallet not yet made, built for another carrier (settled 9): the plan
 * changed, and undone by changing it back. A made pallet keeps the carrier
 * and rules it was started under.
 */
export function buildFor(board: Board, pallet: Uuid, carrier: { id: Uuid; name: string }): Outcome {
  const before = planOf(board);
  const title = board.parcels.find((p) => p.id === pallet)?.title ?? "The pallet";
  return {
    ok: true,
    verdict: `${title} built for ${carrier.name}`,
    plan: before.map((pp) => (pp.id === pallet ? { ...pp, carrier_id: carrier.id } : pp)),
    acts: [],
    undo: { plan: before, acts: [] },
    shown: null,
    chooseBox: false,
  };
}

/**
 * Suggest (D245): everything not placed by hand planned again round what was.
 * The packer's planned parcels stay as they are; Suggest's own are planned
 * afresh, so pressing it twice changes nothing.
 */
export function suggest(screen: BenchScreen, board: Board, newId: () => Uuid): PlannedParcel[] {
  const kept = board.parcels.flatMap((p) => (p.planned?.placed_by_hand ? [p.planned] : []));
  const byHand = byLine(kept.flatMap((pp) => pp.lines.map((l) => ({ line: l.line_id, units: l.quantity }))));
  const lines = board.lines.map((b) => ({ ...b.line, remaining: Math.max(0, b.toPack - (byHand.get(b.line.line_id) ?? 0)) }));
  const fresh = arrange(lines, screen.presets).boxes.map((b): PlannedParcel => {
    const placed = placedOf(b.layers);
    for (const a of b.loose) placed.set(a.line, (placed.get(a.line) ?? 0) + a.units);
    return {
      id: newId(),
      package_type_id: b.preset.id,
      as_is: null,
      on_parcel_id: null,
      on_package_id: null,
      at: null,
      placed_by_hand: false,
      carrier_id: null,
      lines: [...placed].filter(([, q]) => q > 0).map(([line_id, quantity]) => ({ line_id, quantity })),
    };
  });
  return [...kept, ...fresh];
}

/** Where units put into a carton come from: the bench first, then a bin, then handed over (D172, settled 2). */
export type Source = { kind: "bench"; units: number } | { kind: "cell"; stock: Uuid; units: number } | { kind: "handover"; units: number };

export function sourcesFor(line: BenchLine, units: number): { sources: Source[]; short: number } {
  const sources: Source[] = [];
  let left = units;
  const take = (n: number) => {
    const t = Math.min(left, Math.max(0, n));
    left -= t;
    return t;
  };
  const bench = take(line.on_bench);
  if (bench > 0) sources.push({ kind: "bench", units: bench });
  for (const c of line.cells) {
    const n = take(c.available);
    if (n > 0) sources.push({ kind: "cell", stock: c.stock_id, units: n });
  }
  if (line.elsewhere) {
    const n = take(line.elsewhere.reported - line.elsewhere.handed);
    if (n > 0) sources.push({ kind: "handover", units: n });
  }
  return { sources, short: left };
}

/** What the ghost under the pointer says of what is dragged. */
export function labelOf(board: Board, keys: readonly string[]): string {
  const names = keys.flatMap((key) => {
    const t = thingOf(key);
    if (!t) return [];
    if (t.kind === "parcel") return [board.parcels.find((p) => p.id === t.id)?.title ?? "A parcel"];
    const b = board.lines.find((x) => x.line.line_id === t.line);
    if (!b) return [];
    const units = t.parcel ? (board.parcels.find((p) => p.id === t.parcel)?.goods.find((g) => g.line.line_id === t.line)?.units ?? 0) : b.left;
    return [`${b.line.item_code} × ${units}`];
  });
  return names.length <= 2 ? names.join(", ") : `${names[0]} and ${names.length - 1} more`;
}
