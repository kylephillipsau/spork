import type { BinRow, BinView, BinsList, CellBin, DraftReport, LayoutPlace, LayoutView, PlaceView, PlanShape } from "@domain/types";

import type { Chosen, DraftState, WarehouseDesk } from "./useWarehouse";
import type { BinDesk, PlaceDesk } from "./usePlace";

/**
 * Places with no network (D173).
 *
 * One building, **Main**: two racks on the odd side of their aisles, a rack
 * with a face on each side, a dock strip along the front and a reserved area.
 * Rack C runs bays 01 to 09 by twos over four levels, with two cells nobody
 * has a bin in, so the face shows what an empty cell looks like beside a full
 * one. Rack E is six columns long with bins on both faces, numbered round it:
 * E-01 to E-06 along the front and E-07 to E-12 back along the other side.
 */

const MAIN = "01990000-0000-7000-8000-00000000a001";
const RACK_C = "01990000-0000-7000-8000-00000000a0c0";
const RACK_D = "01990000-0000-7000-8000-00000000a0d0";
const DOCK = "01990000-0000-7000-8000-00000000a0e0";
const RETURNS = "01990000-0000-7000-8000-00000000a0f0";
const RACK_E = "01990000-0000-7000-8000-00000000a0e1";

const rect = (x: number, y: number, l: number, d: number): [number, number][] => [
  [x, y],
  [x + l, y],
  [x + l, y + d],
  [x, y + d],
];

/** A place as the list carries it, before its box is read off the plan. */
type Bare = Omit<LayoutPlace, "x" | "y" | "z" | "length" | "depth" | "height" | "turn" | "outlined">;

/**
 * Its box, read off its rectangle on the plan, relative to its parent's
 * corner: every fixture place is square to the site, so that is a subtraction.
 */
function boxed(places: Bare[], plan: PlanShape[]): LayoutPlace[] {
  const shapes = new Map(plan.map((sh) => [sh.place_id, sh]));
  return places.map((p) => {
    const sh = shapes.get(p.place_id);
    const parent = p.parent_id ? shapes.get(p.parent_id) : undefined;
    const [[x0, y0] = [0, 0], [x1] = [0, 0], , [, y3] = [0, 0]] = sh?.corners ?? [];
    const [px, py] = parent?.corners[0] ?? [0, 0];
    return { ...p, x: x0 - px, y: y0 - py, z: sh?.z ?? 0, length: x1 - x0 || 1, depth: y3 - y0 || 1, height: sh?.height || 1, turn: 0, outlined: false };
  });
}

const PLAN: PlanShape[] = [
  { place_id: MAIN, name: "Main", solid: false, nesting: 0, corners: rect(0, 0, 14, 12), z: 0, height: 6, frame: { x: 0, y: 0, z: 0, turn: 0 } },
  { place_id: RACK_C, name: "Rack C", solid: true, nesting: 1, corners: rect(1, 1, 5, 1), z: 0, height: 4, frame: { x: 1, y: 1, z: 0, turn: 0 } },
  { place_id: RACK_D, name: "Rack D", solid: true, nesting: 1, corners: rect(1, 4, 5, 1), z: 0, height: 3, frame: { x: 1, y: 4, z: 0, turn: 0 } },
  { place_id: RETURNS, name: "Returns (reserved)", solid: false, nesting: 1, corners: rect(9, 4, 4, 3), z: 0, height: 1, frame: { x: 9, y: 4, z: 0, turn: 0 } },
  { place_id: DOCK, name: "Dock", solid: false, nesting: 1, corners: rect(1, 10, 12, 1), z: 0, height: 1, frame: { x: 1, y: 10, z: 0, turn: 0 } },
  { place_id: RACK_E, name: "Rack E", solid: true, nesting: 1, corners: rect(1, 7, 6, 2), z: 0, height: 3, frame: { x: 1, y: 7, z: 0, turn: 0 } },
];

const TRAIL = [{ place_id: MAIN, name: "Main" }];

let n = 0;
const bin = (code: string, bay: number, level: number, position = 1, side = 1): CellBin => ({
  location_id: `01990000-0000-7000-8000-${String(++n).padStart(12, "0")}`,
  code,
  kind: "pick_face",
  cell: { bay, level, row: 1, position, side },
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
  sides: 1,
  reach_levels: 1,
  bay_labels: C_LABELS,
  back_labels: [],
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
  contents: [],
  contents_total: 0,
};

/**
 * Rack E, numbered round: column 1 is E-01 on the front and E-12 behind it on
 * the back. Two cells are empty, one a side.
 */
const E_LABELS = ["01", "02", "03", "04", "05", "06"];
const E_BACK = ["07", "08", "09", "10", "11", "12"];
const E_BINS: CellBin[] = [1, 2].flatMap((side) =>
  [1, 2, 3, 4, 5, 6].flatMap((bay) =>
    [1, 2, 3]
      .filter((level) => !(side === 1 && bay === 4 && level === 3) && !(side === 2 && bay === 6 && level === 1))
      .map((level) => bin(`E-${side === 1 ? E_LABELS[bay - 1] : E_BACK[6 - bay]}-${level}`, bay, level, 1, side)),
  ),
);

const RACK_E_VIEW: PlaceView = {
  place_id: RACK_E,
  name: "Rack E",
  solid: true,
  bays: 6,
  levels: 3,
  rows: 1,
  positions: [1, 1, 1],
  pattern: "E-{bay:02}-{level}",
  sides: 2,
  reach_levels: 1,
  bay_labels: E_LABELS,
  back_labels: E_BACK,
  level_labels: ["1", "2", "3"],
  trail: TRAIL,
  children: [],
  bins: E_BINS,
  plan: PLAN,
};

const behind = E_BINS.find((b) => b.code === "E-11-2")!;

/** E-11-2: on the back of the second column, behind E-02. */
export const BIN_BACK: BinView = {
  location_id: behind.location_id,
  code: behind.code,
  kind: "pick_face",
  active: true,
  cell: behind.cell,
  place: RACK_E_VIEW,
  contents: [],
  contents_total: 0,
};

export const BIN_UNPLACED: BinView = {
  location_id: "01990000-0000-7000-8000-0000000000ff",
  code: "C-FLOOR",
  kind: "bulk",
  active: true,
  cell: null,
  place: null,
  contents: [],
  contents_total: 0,
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
  sides: 1,
  reach_levels: 1,
  bay_labels: ["01", "03", "05", "07"],
  back_labels: [],
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
  sides: 1,
  reach_levels: 1,
  bay_labels: ["1"],
  back_labels: [],
  level_labels: ["1"],
  trail: [],
  children: [
    { place_id: RACK_C, name: "Rack C", solid: true, bins: C_BINS.length },
    { place_id: RACK_D, name: "Rack D", solid: true, bins: 20 },
    { place_id: RETURNS, name: "Returns (reserved)", solid: false, bins: 0 },
    { place_id: DOCK, name: "Dock", solid: false, bins: 6 },
    { place_id: RACK_E, name: "Rack E", solid: true, bins: 34 },
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
  version: "none",
  cell_mm: null,
};

export const DRAFTED: DraftReport = {
  inside: "Building",
  inside_created: true,
  places: [
    { name: "Rack A", solid: true, pattern: "A-{bay:02}-{level}", bays: 30, levels: 5, bins: 300, sides: 1, from_right: false, split: ["01–15", "16–30"] },
    { name: "Rack B", solid: true, pattern: "B-{bay:02}-{level}", bays: 30, levels: 5, bins: 296, sides: 1, from_right: false, split: ["01–15", "16–30"] },
    { name: "Rack C", solid: true, pattern: "C-{bay:02}-{level}", bays: 20, levels: 4, bins: 160, sides: 1, from_right: false, split: ["01–10", "11–20"] },
    { name: "Rack D", solid: true, pattern: "D-{bay:02}-{level}-{position}", bays: 24, levels: 3, bins: 480, sides: 1, from_right: false, split: ["01–12", "13–24"] },
    { name: "DOCK", solid: false, pattern: "DOCK-{bay}", bays: 6, levels: 1, bins: 6, sides: 1, from_right: false, split: null },
    { name: "Rack X", solid: true, pattern: "X-{bay}-{level:02}", bays: 1, levels: 1, bins: 1, sides: 1, from_right: false, split: null },
  ],
  bins_filled: 0,
  bins_placed: 1243,
  unplaced: 42,
  unplaced_sample: ["3PL", "ASSEMBLY-BIN", "C-FLOOR", "QUARANTINE"],
  left_out: [],
  applied: false,
};

export const LAID_OUT: LayoutView = {
  site_code: "NORTH",
  places: boxed([
    { place_id: MAIN, parent_id: null, name: "Main", solid: false, bays: 1, levels: 1, rows: 1, positions: [1], sides: 1, from_right: false, reach_levels: 1, pattern: null, bins: 0 },
    { place_id: RACK_C, parent_id: MAIN, name: "Rack C", solid: true, bays: 5, levels: 4, rows: 1, positions: [1, 1, 1, 1], sides: 1, from_right: false, reach_levels: 1, pattern: "C-{bay:02}-{level}", bins: 18 },
    { place_id: RACK_D, parent_id: MAIN, name: "Rack D", solid: true, bays: 4, levels: 3, rows: 1, positions: [1, 1, 1], sides: 1, from_right: false, reach_levels: 1, pattern: "D-{bay:02}-{level}-{position}", bins: 20 },
    { place_id: RETURNS, parent_id: MAIN, name: "Returns (reserved)", solid: false, bays: 1, levels: 1, rows: 1, positions: [1], sides: 1, from_right: false, reach_levels: 1, pattern: null, bins: 0 },
    { place_id: DOCK, parent_id: MAIN, name: "Dock", solid: false, bays: 6, levels: 1, rows: 1, positions: [1], sides: 1, from_right: false, reach_levels: 1, pattern: "DOCK-{bay}", bins: 6 },
    { place_id: RACK_E, parent_id: MAIN, name: "Rack E", solid: true, bays: 6, levels: 3, rows: 1, positions: [1, 1, 1], sides: 2, from_right: false, reach_levels: 1, pattern: "E-{bay:02}-{level}", bins: 34 },
  ], PLAN),
  bins: 81,
  unplaced: 3,
  unplaced_sample: ["3PL", "ASSEMBLY-BIN", "C-FLOOR"],
  plan: PLAN,
  version: "laid-out",
  cell_mm: null,
};

const ITEM = (k: number) => `01990000-0000-7000-8000-0000000b${String(k).padStart(4, "0")}`;

/**
 * A site just after its first draft was applied: nineteen families of bins,
 * laid out as the draft lays them. Each is a row of its own with an aisle of
 * two cells to the next, inside the building the draft made. It is what a real
 * bin list looks like before anyone has arranged it, and the 3D view is how
 * its shape is first seen.
 */
const FAMILIES: [name: string, bays: number, levels: number, solid: boolean][] = [
  ["Rack A", 42, 5, true],
  ["Rack B", 42, 5, true],
  ["Rack C", 38, 5, true],
  ["Rack D", 38, 5, true],
  ["Rack E", 30, 4, true],
  ["Rack F", 30, 4, true],
  ["Rack G", 24, 6, true],
  ["Rack H", 24, 6, true],
  ["Rack J", 18, 3, true],
  ["Rack K", 18, 3, true],
  ["Rack L", 12, 4, true],
  ["Rack M", 12, 4, true],
  ["Rack N", 9, 2, true],
  ["Shelf P", 16, 5, true],
  ["Shelf Q", 16, 5, true],
  ["Shelf R", 6, 5, true],
  ["BULK", 20, 1, false],
  ["STAGE", 8, 1, false],
  ["DOCK", 6, 1, false],
];

const DRAFT_ID = (i: number) => `01990000-0000-7000-8000-0000000c${String(i).padStart(4, "0")}`;
const BUILDING_ID = DRAFT_ID(0);

/**
 * The racks have a face on each side, numbered round them: each is one place,
 * half as many columns as it has bays and two cells deep, a side to each
 * aisle. Shelves have one side.
 */
const DRAFT_PLACES: { place: Bare; shape: PlanShape }[] = (() => {
  let y = 1;
  return FAMILIES.map(([name, bays, levels, solid], i) => {
    const id = DRAFT_ID(i + 1);
    const pattern = solid ? `${name.slice(-1)}-{bay:02}-{level}` : `${name}-{bay}`;
    const sides = name.startsWith("Rack") && bays > 1 ? 2 : 1;
    const columns = Math.ceil(bays / sides);
    const at = y;
    y += sides + 2;
    return {
      place: { place_id: id, parent_id: BUILDING_ID, name, solid, bays: columns, levels, rows: 1, positions: Array<number>(levels).fill(1), sides, from_right: false, reach_levels: 1, pattern, bins: bays * levels - (i % 4) },
      shape: { place_id: id, name, solid, nesting: 1, corners: rect(1, at, columns, sides), z: 0, height: solid ? levels : 1, frame: { x: 1, y: at, z: 0, turn: 0 } },
    };
  });
})();

const DRAFT_WIDE = Math.max(10, ...FAMILIES.map(([, bays]) => bays + 2));
const DRAFT_DEEP = Math.max(...DRAFT_PLACES.flatMap((p) => p.shape.corners.map(([, y]) => y))) + 1;

const DRAFTED_PLAN: PlanShape[] = [
  { place_id: BUILDING_ID, name: "Building", solid: false, nesting: 0, corners: rect(0, 0, DRAFT_WIDE, DRAFT_DEEP), z: 0, height: 8, frame: { x: 0, y: 0, z: 0, turn: 0 } },
  ...DRAFT_PLACES.map((p) => p.shape),
];

export const DRAFTED_SITE: LayoutView = {
  site_code: "NORTH",
  places: boxed(
    [
      { place_id: BUILDING_ID, parent_id: null, name: "Building", solid: false, bays: 1, levels: 1, rows: 1, positions: [1], sides: 1, from_right: false, reach_levels: 1, pattern: null, bins: 0 },
      ...DRAFT_PLACES.map((p) => p.place),
    ],
    DRAFTED_PLAN,
  ),
  bins: 1640,
  unplaced: 42,
  unplaced_sample: ["3PL", "ASSEMBLY-BIN", "C-FLOOR", "QUARANTINE"],
  plan: DRAFTED_PLAN,
  version: "drafted",
  cell_mm: null,
};

/** Rack G, chosen: the first dozen of its bins, on its front. */
export const DRAFTED_CHOSEN = DRAFT_PLACES.find((p) => p.place.name === "Rack G")!.place.place_id;
export const DRAFTED_BINS: BinsList = {
  bins: Array.from({ length: 12 }, (_, i) => {
    const bay = Math.floor(i / 6) + 1;
    const level = (i % 6) + 1;
    return {
      location_id: `01990000-0000-7000-8000-0000000d${String(i).padStart(4, "0")}`,
      code: `G-${String(bay).padStart(2, "0")}-${level}`,
      kind: "pick_face",
      place_id: DRAFTED_CHOSEN,
      place_name: "Rack G",
      cell: { bay, level, row: 1, position: 1, side: 1 },
      whereabouts: `front, bay ${String(bay).padStart(2, "0")}, level ${level}`,
      pick_sequence: i + 1,
      within_reach: true,
      reported: i % 3 === 2 ? [] : [{ item_id: ITEM(400 + i), item_code: `SKU-${6200 + i}`, on_hand: String(12 * (1 + (i % 4))) }],
      reported_items: i % 3 === 2 ? 0 : 1,
      held: 0,
    };
  }),
  total: 141,
};


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
    within_reach: true,
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
    within_reach: false,
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
  opts: { draft?: DraftState; chosen?: Chosen; bins?: BinsList; asked?: string; leftOut?: string[]; twoSided?: string[]; fromRight?: string[] } = {},
): WarehouseDesk {
  const asked = opts.asked ?? "";
  return {
    read: { kind: "ready", value },
    draft: opts.draft ?? { kind: "idle" },
    preview: async () => {},
    apply: async () => {},
    dismiss: () => {},
    leftOut: new Set(opts.leftOut ?? []),
    leaveOut: () => {},
    twoSided: new Set(opts.twoSided ?? []),
    setTwoSided: () => {},
    fromRight: new Set(opts.fromRight ?? []),
    setFromRight: () => {},
    chosen: opts.chosen ?? null,
    choose: () => {},
    bins: opts.bins ? { kind: "ready", value: opts.bins } : { kind: "idle" },
    asked,
    typed: asked,
    type: () => {},
    search: () => {},
  };
}
