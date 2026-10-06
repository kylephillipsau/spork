import type { BenchScreen, CartonSummary, Picture, Uuid } from "@domain/types";

import { boxFreight, type Arrangement, type AsIs, type Aside, type BoxPlan, type Dims, type Layer } from "./arrange.ts";
import type { Freight } from "./freight";

/**
 * The whole order as it will leave (D202): every parcel, packed or planned,
 * and every line's units accounted for, so the bench can be checked off and
 * nothing goes missing.
 *
 * A line is ticked when all it commits is in a carton, sealed or open. Until
 * then its units are planned (a suggested box, round one, or a parcel as it
 * is), listed as not placeable (not measured, too big), or picked and not in a
 * carton yet. What is none of those is missing, and said to be.
 *
 * Every parcel says its size and what one weighs (D224): on the scale once it
 * has been, else by the record, goods and box. The order's parcels as freight
 * are what a booking is made from.
 *
 * No React here, so a test runner can read it.
 */

export type ParcelState = "sealed" | "open" | "planned" | "as-is";

export interface ParcelLine {
  line: Uuid | null;
  item_id: Uuid;
  item_code: string;
  units: number;
  /** In it already, or to go in. */
  state: "in" | "planned";
}

export interface Parcel {
  key: string;
  /** "Carton 1", "Small Box", "JWR-1002R as it is". */
  title: string;
  /** Its box, when the title is not: "small box", "SLV-PE-BLU carton". */
  detail: string | null;
  state: ParcelState;
  size: Dims | null;
  /** The suggestion's box it is, to show its layers; null when there is none. */
  box: number | null;
  /** For parcels that ship as they are: the press that ships them. */
  asIs: AsIs | null;
  /** Parcels alike: so many rolls each as it is; one otherwise. */
  count: number;
  lines: ParcelLine[];
  /** Things with no size, going in round the rest. */
  loose: Aside[];
  /** The arrangement, when there is one to draw. */
  layers: Layer[] | null;
  /** Its line's place, for its colour, when it is one of a product. */
  index: number | null;
  faces: AsIs["faces"];
  /** What one weighs: on the scale when `weighed`, else by the record; null when nothing says (D224). */
  weight_g: number | null;
  weighed: boolean;
  /** What the record's weight leaves out, when anything: "2 not weighed, box not weighed". */
  weightNote: string | null;
}

export interface OrderLine {
  line: Uuid;
  item_id: Uuid;
  item_code: string;
  description: string | null;
  picture: Picture | null;
  committed: number;
  /** In a carton already, sealed or open. */
  packed: number;
  /** In the suggestion: a box, round one, or a parcel as it is. */
  planned: number;
  /** Listed as not placeable: not measured, too big, or no box for it. */
  unplaced: number;
  /** Picked, or handed over, and not in a carton yet. */
  notBoxed: number;
  /** None of those: the units the order would leave without. */
  missing: number;
  /** All of it is in a carton. */
  done: boolean;
  where: { parcel: string; units: number; state: "in" | "planned" }[];
}

export interface OrderView {
  parcels: Parcel[];
  /** Every parcel as a carrier sees it: what a booking is made from (D224). */
  freight: Freight[];
  lines: OrderLine[];
  committed: number;
  packed: number;
  planned: number;
  unplaced: number;
  notBoxed: number;
  missing: number;
}

const LEVEL_WORDS = { each: "as it is", inner: "inner pack", carton: "carton" } as const;

/**
 * A carton that is one of a product as it is (D196) looks like the product at
 * that level: its sides, as photographed and cut. A box type has none.
 */
function facesOf(screen: BenchScreen, c: CartonSummary): Parcel["faces"] {
  if (!c.own_carton_of) return {};
  const line = screen.lines.find((l) => l.item_code === c.own_carton_of);
  return line?.packs.find((p) => p.level === (c.own_level ?? "carton"))?.faces ?? {};
}

/** What a box's weight by the record leaves out: pieces nobody weighed, and the box itself. */
function boxNote(b: BoxPlan): string | null {
  const parts = [];
  if (b.unweighed > 0) parts.push(`${b.unweighed} not weighed`);
  if (b.tare_g === null) parts.push("box not weighed");
  return parts.length > 0 ? parts.join(", ") : null;
}

/** A carton's weight: on the scale, else what it is filling up to, else what its record expects. */
function cartonWeight(c: CartonSummary, filling: BoxPlan | null): Pick<Parcel, "weight_g" | "weighed" | "weightNote"> {
  if (c.gross_weight_g !== null) return { weight_g: c.gross_weight_g, weighed: true, weightNote: null };
  if (filling) return { weight_g: boxFreight(filling).weight_g, weighed: false, weightNote: boxNote(filling) };
  const recorded = c.listed_weight_g ?? c.expected?.grams ?? null;
  return { weight_g: recorded, weighed: false, weightNote: recorded === null ? "not weighed" : null };
}

function cartonDetail(c: CartonSummary): string | null {
  if (c.own_carton_of) return `${c.own_carton_of} ${LEVEL_WORDS[c.own_level ?? "carton"]}`;
  return c.package_type;
}

function dimsOf(s: { length_mm: number; width_mm: number; height_mm: number } | null): Dims | null {
  return s ? [s.length_mm, s.width_mm, s.height_mm] : null;
}

/** Add `units` of a line to a parcel's lines, one row per line and state. */
function put(lines: ParcelLine[], row: ParcelLine): void {
  const there = lines.find((l) => l.item_id === row.item_id && l.line === row.line && l.state === row.state);
  if (there) there.units += row.units;
  else lines.push({ ...row });
}

/** The units of each line a box plans to put in: what is not in it already. */
function plannedIn(layers: Layer[]): ParcelLine[] {
  const out: ParcelLine[] = [];
  for (const layer of layers) {
    for (const p of layer.placements) {
      if (p.packed) continue;
      put(out, { line: p.kind.line, item_id: p.kind.item_id, item_code: p.kind.item_code, units: p.kind.units, state: "planned" });
    }
  }
  return out;
}

export function wholeOrder(screen: BenchScreen, plan: Arrangement): OrderView {
  const byItem = new Map<Uuid, Uuid>();
  for (const l of screen.lines) if (!byItem.has(l.item_id)) byItem.set(l.item_id, l.line_id);
  const parcels: Parcel[] = [];

  // ── what is in cartons already ─────────────────────────────────────────
  for (const c of screen.cartons) {
    const box = plan.boxes.findIndex((b) => b.carton?.id === c.id);
    const lines: ParcelLine[] = [];
    for (const r of c.contents) {
      put(lines, { line: byItem.get(r.item_id) ?? null, item_id: r.item_id, item_code: r.item_code, units: r.quantity, state: "in" });
    }
    const filling = box >= 0 ? plan.boxes[box]! : null;
    if (filling) for (const row of plannedIn(filling.layers)) put(lines, row);
    parcels.push({
      key: `carton:${c.id}`,
      title: `Carton ${c.sequence}`,
      detail: cartonDetail(c),
      state: c.sealed ? "sealed" : "open",
      size: dimsOf(c.stated_size),
      box: box >= 0 ? box : null,
      asIs: null,
      count: 1,
      lines,
      loose: filling && plan.looseBox === box ? plan.placedLoose : [],
      layers: filling ? filling.layers : null,
      index: null,
      faces: facesOf(screen, c),
      ...cartonWeight(c, filling),
    });
  }

  // ── what the suggestion plans ──────────────────────────────────────────
  plan.boxes.forEach((b, i) => {
    if (b.carton) return;
    parcels.push({
      key: `box:${i}`,
      title: b.preset.name,
      detail: null,
      state: "planned",
      size: dimsOf(b.preset.size),
      box: i,
      asIs: null,
      count: 1,
      lines: plannedIn(b.layers),
      loose: plan.looseBox === i ? plan.placedLoose : [],
      layers: b.layers,
      index: null,
      faces: {},
      weight_g: boxFreight(b).weight_g,
      weighed: false,
      weightNote: boxNote(b),
    });
  });
  for (const a of plan.asIs) {
    parcels.push({
      key: `as-is:${a.line}:${a.level}`,
      title: `${a.item_code} ${LEVEL_WORDS[a.level]}`,
      detail: null,
      state: "as-is",
      size: a.size,
      box: null,
      asIs: a,
      count: a.count,
      lines: [{ line: a.line, item_id: a.item_id, item_code: a.item_code, units: a.units, state: "planned" }],
      loose: [],
      layers: null,
      index: a.index,
      faces: a.faces,
      weight_g: a.weight_g === null ? null : a.weight_g / a.count,
      weighed: false,
      weightNote: a.weight_g === null ? "not weighed" : null,
    });
  }

  // ── every line, accounted for ──────────────────────────────────────────
  const unplacedOf = new Map<Uuid, number>();
  const listed = [...plan.unmeasured, ...plan.oversize, ...plan.loose];
  for (const a of listed) unplacedOf.set(a.line, (unplacedOf.get(a.line) ?? 0) + a.units);
  const looseOf = new Map<Uuid, number>();
  for (const a of plan.placedLoose) looseOf.set(a.line, (looseOf.get(a.line) ?? 0) + a.units);

  const lines: OrderLine[] = screen.lines.map((l) => {
    const where: OrderLine["where"] = [];
    let packed = 0;
    let planned = looseOf.get(l.line_id) ?? 0;
    for (const p of parcels) {
      for (const row of p.lines) {
        if (row.line !== l.line_id) continue;
        if (row.state === "in") packed += row.units;
        else planned += row.units;
        const name = p.state === "as-is" ? (p.count === 1 ? p.title : `${p.count} parcels as they are`) : p.title;
        const same = where.find((w) => w.parcel === name && w.state === row.state);
        if (same) same.units += row.units;
        else where.push({ parcel: name, units: row.units, state: row.state });
      }
      if (p.loose.some((a) => a.line === l.line_id)) {
        const units = p.loose.filter((a) => a.line === l.line_id).reduce((t, a) => t + a.units, 0);
        where.push({ parcel: `${p.title}, round the rest`, units, state: "planned" });
      }
    }
    // Everything still to pack, when it was too much to arrange piece by piece.
    const unplaced = plan.tooMany ? Math.max(0, l.remaining) : (unplacedOf.get(l.line_id) ?? 0);
    if (plan.tooMany) planned = 0;
    // Picked or handed over and in no carton: what the bench counts as done
    // that no carton holds.
    const notBoxed = Math.max(0, l.committed - l.remaining - packed);
    const missing = Math.max(0, l.committed - packed - planned - unplaced - notBoxed);
    return {
      line: l.line_id,
      item_id: l.item_id,
      item_code: l.item_code,
      description: l.description,
      picture: l.picture,
      committed: l.committed,
      packed,
      planned,
      unplaced,
      notBoxed,
      missing,
      done: l.committed > 0 && packed >= l.committed,
      where,
    };
  });

  const sum = (f: (l: OrderLine) => number) => lines.reduce((t, l) => t + f(l), 0);
  return {
    parcels,
    freight: parcels.map((p) => ({ count: p.count, size: p.size, weight_g: p.weight_g })),
    lines,
    committed: sum((l) => l.committed),
    packed: sum((l) => l.packed),
    planned: sum((l) => l.planned),
    unplaced: sum((l) => l.unplaced),
    notBoxed: sum((l) => l.notBoxed),
    missing: sum((l) => l.missing),
  };
}
