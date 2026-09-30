import type { BinRow, BinView, BinsList, CellBin, DraftReport, LayoutView, PlaceView, PlanShape } from "@domain/types";

import type { Chosen, DraftState, WarehouseDesk } from "./useWarehouse";
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

// ── the warehouse ────────────────────────────────────────────────────────

export const NO_LAYOUT: LayoutView = {
  site_code: "NORTH",
  places: [],
  bins: 1284,
  unplaced: 1284,
  unplaced_sample: [],
  plan: [],
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
  plan: PLAN,
};

const ITEM = (k: number) => `01990000-0000-7000-8000-0000000b${String(k).padStart(4, "0")}`;

/** Rack C's bins, as the list reads them: most hold one item, a few several. */
const RACK_C_ROWS: BinRow[] = C_BINS.map((b, i) => {
  const items = i % 7 === 3 ? 0 : i % 5 === 2 ? 3 : 1;
  const reported = Array.from({ length: Math.min(items, 3) }, (_, j) => ({
    item_id: ITEM(i * 3 + j),
    item_code: `SKU-${5100 + i * 3 + j}${j === 0 ? "B" : ""}`,
    on_hand: String([24, 6, 2][j]! * (1 + (i % 3))),
  }));
  return {
    location_id: b.location_id,
    code: b.code,
    kind: b.kind,
    place_id: RACK_C,
    place_name: "Rack C",
    cell: b.cell,
    whereabouts: `bay ${C_LABELS[b.cell.bay - 1]}, level ${b.cell.level}`,
    pick_sequence: i + 1,
    reported,
    reported_items: items,
    held: i === 0 ? 12 : i === 4 ? 3 : 0,
  };
});

export const RACK_C_BINS: BinsList = { bins: RACK_C_ROWS, total: RACK_C_ROWS.length };

/** The tray: bins whose codes the draft could not read. */
export const UNPLACED_BINS: BinsList = {
  bins: ["3PL", "ASSEMBLY-BIN", "C-FLOOR"].map((code, i) => ({
    location_id: `01990000-0000-7000-8000-0000000000${(0xf0 + i).toString(16)}`,
    code,
    kind: "bulk",
    place_id: null,
    place_name: null,
    cell: null,
    whereabouts: null,
    pick_sequence: null,
    reported: i === 2 ? [{ item_id: ITEM(900), item_code: "SKU-5120B", on_hand: "40" }] : [],
    reported_items: i === 2 ? 1 : 0,
    held: 0,
  })),
  total: 3,
};

/** A search across the site for "01": bay 01 of two racks, and the dock's first door. */
export const FOUND_BINS: BinsList = {
  bins: [
    RACK_C_ROWS[0]!,
    RACK_C_ROWS[1]!,
    {
      ...RACK_C_ROWS[2]!,
      location_id: "01990000-0000-7000-8000-0000000000d1",
      code: "D-01-1-1",
      place_id: RACK_D,
      place_name: "Rack D",
      whereabouts: "bay 01, level 1",
      held: 0,
    },
    {
      ...UNPLACED_BINS.bins[0]!,
      location_id: "01990000-0000-7000-8000-0000000000e1",
      code: "DOCK-01",
      kind: "dock",
      place_id: DOCK,
      place_name: "Dock",
      whereabouts: "bay 1",
    },
  ],
  total: 4,
};

export function fixtureWarehouse(
  value: LayoutView,
  opts: { draft?: DraftState; chosen?: Chosen; bins?: BinsList; asked?: string } = {},
): WarehouseDesk {
  const asked = opts.asked ?? "";
  return {
    read: { kind: "ready", value },
    draft: opts.draft ?? { kind: "idle" },
    preview: async () => {},
    apply: async () => {},
    dismiss: () => {},
    chosen: opts.chosen ?? null,
    choose: () => {},
    bins: opts.bins ? { kind: "ready", value: opts.bins } : { kind: "idle" },
    asked,
    typed: asked,
    type: () => {},
    search: () => {},
  };
}
