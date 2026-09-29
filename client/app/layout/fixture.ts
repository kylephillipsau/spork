import type { BinView, CellBin, DraftReport, LayoutView, PlaceView, PlanShape } from "@domain/types";

import type { DraftState, LayoutDesk } from "./useLayout";
import type { BinDesk, PlaceDesk } from "./usePlace";

/**
 * Places with no network (D173).
 *
 * One building, **Main**: two racks on the odd side of their aisles, a dock
 * strip along the front and a reserved area. Rack C runs bays 01 to 09 by twos
 * over four levels, with two cells nobody has a bin in, so the face shows what
 * an empty cell looks like beside a full one.
 */

const MAIN = "01990000-0000-7000-8000-00000000a001";
const RACK_C = "01990000-0000-7000-8000-00000000a0c0";
const RACK_D = "01990000-0000-7000-8000-00000000a0d0";
const DOCK = "01990000-0000-7000-8000-00000000a0e0";
const RETURNS = "01990000-0000-7000-8000-00000000a0f0";

const rect = (x: number, y: number, l: number, d: number): [number, number][] => [
  [x, y],
  [x + l, y],
  [x + l, y + d],
  [x, y + d],
];

const PLAN: PlanShape[] = [
  { place_id: MAIN, name: "Main", solid: false, nesting: 0, corners: rect(0, 0, 14, 12), z: 0, height: 6 },
  { place_id: RACK_C, name: "Rack C", solid: true, nesting: 1, corners: rect(1, 1, 5, 1), z: 0, height: 4 },
  { place_id: RACK_D, name: "Rack D", solid: true, nesting: 1, corners: rect(1, 4, 5, 1), z: 0, height: 3 },
  { place_id: RETURNS, name: "Returns (reserved)", solid: false, nesting: 1, corners: rect(9, 4, 4, 3), z: 0, height: 1 },
  { place_id: DOCK, name: "Dock", solid: false, nesting: 1, corners: rect(1, 10, 12, 1), z: 0, height: 1 },
];

const TRAIL = [{ place_id: MAIN, name: "Main" }];

let n = 0;
const bin = (code: string, bay: number, level: number, position = 1): CellBin => ({
  location_id: `01990000-0000-7000-8000-${String(++n).padStart(12, "0")}`,
  code,
  kind: "pick_face",
  cell: { bay, level, row: 1, position },
});

const C_LABELS = ["01", "03", "05", "07", "09"];
const C_BINS: CellBin[] = C_LABELS.flatMap((label, i) =>
  [1, 2, 3, 4]
    // Bay 07 has no top-level bin, and bay 09 none at the floor.
    .filter((level) => !(label === "07" && level === 4) && !(label === "09" && level === 1))
    .map((level) => bin(`C-${label}-${level}`, i + 1, level)),
);

const RACK_C_VIEW: PlaceView = {
  place_id: RACK_C,
  name: "Rack C",
  solid: true,
  bays: 5,
  levels: 4,
  rows: 1,
  positions: [1, 1, 1, 1],
  pattern: "C-{bay:02}-{level}",
  bay_labels: C_LABELS,
  level_labels: ["1", "2", "3", "4"],
  trail: TRAIL,
  children: [],
  bins: C_BINS,
  plan: PLAN,
};

const target = C_BINS.find((b) => b.code === "C-05-3")!;

export const BIN: BinView = {
  location_id: target.location_id,
  code: target.code,
  kind: "pick_face",
  active: true,
  cell: target.cell,
  place: RACK_C_VIEW,
};

export const BIN_UNPLACED: BinView = {
  location_id: "01990000-0000-7000-8000-0000000000ff",
  code: "C-FLOOR",
  kind: "bulk",
  active: true,
  cell: null,
  place: null,
};

/** A shelf whose bottom level holds three small bins a bay. */
export const SHELF: PlaceView = {
  place_id: RACK_D,
  name: "Rack D",
  solid: true,
  bays: 4,
  levels: 3,
  rows: 1,
  positions: [3, 1, 1],
  pattern: "D-{bay:02}-{level}-{position}",
  bay_labels: ["01", "03", "05", "07"],
  level_labels: ["1", "2", "3"],
  trail: TRAIL,
  children: [],
  bins: [1, 2, 3, 4].flatMap((bay) => [
    ...[1, 2, 3].map((p) => bin(`D-${["01", "03", "05", "07"][bay - 1]}-1-${p}`, bay, 1, p)),
    bin(`D-${["01", "03", "05", "07"][bay - 1]}-2-1`, bay, 2),
    bin(`D-${["01", "03", "05", "07"][bay - 1]}-3-1`, bay, 3),
  ]),
  plan: PLAN,
};

export const BUILDING: PlaceView = {
  place_id: MAIN,
  name: "Main",
  solid: false,
  bays: 1,
  levels: 1,
  rows: 1,
  positions: [1],
  pattern: null,
  bay_labels: ["1"],
  level_labels: ["1"],
  trail: [],
  children: [
    { place_id: RACK_C, name: "Rack C", solid: true, bins: C_BINS.length },
    { place_id: RACK_D, name: "Rack D", solid: true, bins: 20 },
    { place_id: RETURNS, name: "Returns (reserved)", solid: false, bins: 0 },
    { place_id: DOCK, name: "Dock", solid: false, bins: 6 },
  ],
  bins: [],
  plan: PLAN,
};

export const fixtureBin = (value: BinView): BinDesk => ({ read: { kind: "ready", value } });
export const fixturePlace = (value: PlaceView): PlaceDesk => ({ read: { kind: "ready", value } });

// ── the layout page ──────────────────────────────────────────────────────

export const NO_LAYOUT: LayoutView = {
  site_code: "NORTH",
  places: [],
  bins: 1284,
  unplaced: 1284,
  unplaced_sample: [],
};

export const DRAFTED: DraftReport = {
  inside: "Building",
  inside_created: true,
  places: [
    { name: "Rack A", solid: true, pattern: "A-{bay:02}-{level}", bays: 30, levels: 5, bins: 300 },
    { name: "Rack B", solid: true, pattern: "B-{bay:02}-{level}", bays: 30, levels: 5, bins: 296 },
    { name: "Rack C", solid: true, pattern: "C-{bay:02}-{level}", bays: 20, levels: 4, bins: 160 },
    { name: "Rack D", solid: true, pattern: "D-{bay:02}-{level}-{position}", bays: 24, levels: 3, bins: 480 },
    { name: "DOCK", solid: false, pattern: "DOCK-{bay}", bays: 6, levels: 1, bins: 6 },
  ],
  bins_filled: 0,
  bins_placed: 1242,
  unplaced: 42,
  unplaced_sample: ["3PL", "ASSEMBLY-BIN", "C-FLOOR", "QUARANTINE"],
  applied: false,
};

export const LAID_OUT: LayoutView = {
  site_code: "NORTH",
  places: [
    { place_id: MAIN, parent_id: null, name: "Main", solid: false, bays: 1, levels: 1, rows: 1, pattern: null, bins: 0 },
    { place_id: RACK_C, parent_id: MAIN, name: "Rack C", solid: true, bays: 5, levels: 4, rows: 1, pattern: "C-{bay:02}-{level}", bins: 18 },
    { place_id: RACK_D, parent_id: MAIN, name: "Rack D", solid: true, bays: 4, levels: 3, rows: 1, pattern: "D-{bay:02}-{level}-{position}", bins: 20 },
    { place_id: RETURNS, parent_id: MAIN, name: "Returns (reserved)", solid: false, bays: 1, levels: 1, rows: 1, pattern: null, bins: 0 },
    { place_id: DOCK, parent_id: MAIN, name: "Dock", solid: false, bays: 6, levels: 1, rows: 1, pattern: "DOCK-{bay}", bins: 6 },
  ],
  bins: 47,
  unplaced: 3,
  unplaced_sample: ["3PL", "ASSEMBLY-BIN", "C-FLOOR"],
};

export function fixtureLayout(value: LayoutView, draft: DraftState = { kind: "idle" }): LayoutDesk {
  return {
    read: { kind: "ready", value },
    draft,
    preview: async () => {},
    apply: async () => {},
    dismiss: () => {},
  };
}
