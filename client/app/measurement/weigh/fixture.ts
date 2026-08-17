import type { ToWeigh } from "@domain/types";
import type { WeighBench } from "./useWeigh";

/**
 * The weighing worklist, reachable with no network.
 *
 * The two lists the read produces, in the order it produces them: **never
 * measured first**, whatever their apparent age, then overdue, and within each
 * by how often the thing is actually ordered. That ordering is the server's and
 * this fixture keeps it rather than re-sorting, because a fixture that shows an
 * order the endpoint would not produce is a fixture that reviews a screen
 * nobody will see.
 */

const ago = (days: number) => new Date(Date.now() - days * 86_400_000).toISOString();

/** Transcribed off a supplier's sheet and never confirmed — which is 115 of the
 *  116 weights on file, and the finding that made this two lists. */
export const NEVER_MEASURED: ToWeigh = {
  item_id: "17e10000-0000-0000-0000-000000000001",
  item_style_id: null,
  code: "GLOVE-M",
  description: "Nitrile glove, medium",
  packaging_level: "each",
  held_g: 500,
  held_method: "transcribed",
  held_at: ago(12),
  demand: 3,
  because: "never",
};

const STYLE_CARTON: ToWeigh = {
  item_id: null,
  item_style_id: "57110000-0000-0000-0000-000000000001",
  code: "STY-7720",
  description: "Ridgeway StepSure Gumboot - Steel Toe - Green",
  packaging_level: "carton",
  held_g: 11400,
  held_method: "transcribed",
  held_at: ago(12),
  demand: 1,
  because: "never",
};

/** Somebody did put this on a scale, a long time ago. */
const OVERDUE: ToWeigh = {
  item_id: "17e10000-0000-0000-0000-000000000002",
  item_style_id: null,
  code: "STY-7720-08",
  description: "Ridgeway StepSure Gumboot - Steel Toe - Pair - Green - AU8 EU42",
  packaging_level: "each",
  held_g: 1900,
  held_method: "instrument",
  held_at: ago(500),
  demand: 1,
  because: "overdue",
};

export const WEIGH_FIXTURE: ToWeigh[] = [NEVER_MEASURED, STYLE_CARTON, OVERDUE];

/** Nothing waiting — every weight measured and in date. Unlikely on real data
 *  today, and the state the screen has to draw. */
export const WEIGH_CLEAR: ToWeigh[] = [];

const noop = async () => {};

export function fixtureBench(over: Partial<WeighBench> = {}): WeighBench {
  return {
    status: { kind: "ready", queue: WEIGH_FIXTURE },
    at: 0,
    reading: "",
    unit: "kg",
    busy: false,
    problem: null,
    recorded: null,
    type: () => {},
    setUnit: () => {},
    skip: () => {},
    dismiss: () => {},
    record: noop,
    ...over,
  };
}

/** A reading that materially disagrees with what was held: 500 g on file, 640 g
 *  on the scale. Both stay on file and a finding carries the pair — which is
 *  the comparison this side of the system exists to make possible. */
export const DISAGREED: NonNullable<WeighBench["recorded"]> = {
  code: "GLOVE-M",
  observation_id: "0b5e0000-0000-0000-0000-0000000000ff",
  recorded_g: 640,
  previous_g: 500,
  previous_method: "transcribed",
  disagreed: true,
  discrepancy_id: "d15c0000-0000-0000-0000-0000000000ff",
};
