import { grams } from "../../common/format.ts";

import type { Dims } from "./arrange";

/**
 * What an order leaves as, as a carrier sees it, and what makes one way of
 * sending it better than another (D224). No React and no three.js here.
 *
 * **One description of the parcels, read three ways.** The suggestion scores
 * the ways it could box an order by it, the bench totals it, and the summary
 * for the carrier's booking is it, so the three can't disagree about what is
 * being sent.
 *
 * **Better is a setting, not the algorithm.** An objective is a cost over the
 * parcels, lower being better. The suggestion makes a few candidate plans and
 * keeps the one the objective prices lowest, so fewer parcels and cheaper
 * freight are two settings of one search, and a carrier's own rates can be a
 * third without touching how a box is filled.
 */

/** So many parcels alike: their outside size, when known, and what one weighs. */
export interface Freight {
  count: number;
  size: Dims | null;
  /** What one weighs: on the scale when it has been, else by the record. */
  weight_g: number | null;
}

export interface Objective {
  id: ObjectiveId;
  label: string;
  /** What sending these costs, in the objective's own units: lower is better. */
  cost: (parcels: readonly Freight[]) => number;
}

export type ObjectiveId = "parcels" | "chargeable";

/**
 * Kilograms charged per cubic metre where a parcel is bulkier than it is heavy:
 * what Australian road freight commonly uses. A carrier's own factor replaces
 * it when the workspace says.
 */
export const CUBIC_KG_PER_M3 = 250;

function volume_m3([l, w, h]: Dims): number {
  return (l * w * h) / 1e9;
}

/** What a carrier charges one parcel by: its weight or its cubic weight, whichever is more. */
export function chargeable_g(f: Freight, cubicKgPerM3 = CUBIC_KG_PER_M3): number {
  const cubic = f.size ? volume_m3(f.size) * cubicKgPerM3 * 1000 : 0;
  return Math.max(f.weight_g ?? 0, cubic);
}

export const OBJECTIVES: Record<ObjectiveId, Objective> = {
  // Fewest parcels, and of plans with as many, the least air sent.
  parcels: {
    id: "parcels",
    label: "Fewest parcels",
    cost: (parcels) =>
      parcels.reduce((t, p) => t + p.count, 0) * 1e6 + parcels.reduce((t, p) => t + (p.size ? volume_m3(p.size) : 0) * p.count, 0),
  },
  // Cheapest freight, until a carrier's rates say otherwise: the weight it charges.
  chargeable: {
    id: "chargeable",
    label: "Least chargeable weight",
    cost: (parcels) => parcels.reduce((t, p) => t + chargeable_g(p) * p.count, 0),
  },
};

/** The default until the workspace chooses (D224). */
export const DEFAULT_OBJECTIVE: Objective = OBJECTIVES.parcels;

/** Parcels alike gathered: the same size and weight, counted once with how many. */
export function gathered(parcels: readonly Freight[]): Freight[] {
  const out: Freight[] = [];
  for (const p of parcels) {
    const same = out.find((o) => o.weight_g === p.weight_g && (o.size?.join("x") ?? "") === (p.size?.join("x") ?? ""));
    if (same) same.count += p.count;
    else out.push({ ...p, size: p.size ? ([...p.size] as Dims) : null });
  }
  return out;
}

/** A length as a booking takes it: whole centimetres, rounded up, as carriers measure. */
export function bookingCm(mm: number): number {
  return Math.ceil(mm / 10);
}

/**
 * The parcels as a booking takes them, a line each: how many, the outside
 * size in whole centimetres and the weight of one in kilograms, separated by
 * tabs so a paste lands in columns.
 */
export function bookingText(parcels: readonly Freight[]): string {
  return gathered(parcels)
    .map((p) =>
      [
        String(p.count),
        ...(p.size ? p.size.map((mm) => String(bookingCm(mm))) : ["", "", ""]),
        p.weight_g === null ? "" : grams(p.weight_g),
      ].join("\t"),
    )
    .join("\n");
}
