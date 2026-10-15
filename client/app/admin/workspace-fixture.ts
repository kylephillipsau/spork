import type { BoxPhoto, BuildRule, CarrierRow, Carries, Conditions, PackageTypeRow, RuleSetRow, Workspace, ReportedField, Side } from "@domain/types";
import type { WorkspaceBench, WorkspaceState } from "./useWorkspace";

/** The workspace as a deployment looks after a bin import. */

/** The conditions to start (D249): about an eighth of a box's strength empty is left. */
const CONDITIONS: Conditions = { humidity_pct: 70, days_under_load: 1, stacking: "interlocked", deck_gaps: true, handling: "some_rough", road_factor: 3 };

const WORKSPACE: Workspace = {
  conditions: CONDITIONS,
  conditions_share: 0.1224,
  organisation: {
    id: "01920000-0000-7000-8000-000000000001",
    name: "Harbourline",
    slug: "harbourline",
    active: true,
    created_at: "2026-01-06T09:20:00Z",
  },
  sites: [
    // Perth is the state worth drawing: the import creates a site as soon as
    // a warehouse appears in the export, and leaves its bins out when they
    // state no type. A site with none is expected, not broken.
    { id: "5171e000-0000-0000-0000-000000000001", code: "BRI", name: "Brisbane",
      timezone: "Australia/Brisbane", active: true, locations: 2140, sequenced: 2035, current: false,
      pack_location: "DESP-1", owner: "Harbourline", weight_tolerance_per_mille: 30 },
    { id: "5171e000-0000-0000-0000-000000000002", code: "PER", name: "Perth",
      timezone: "Australia/Perth", active: true, locations: 0, sequenced: 0, current: false,
      pack_location: null, owner: null, weight_tolerance_per_mille: 30 },
    { id: "5171e000-0000-0000-0000-000000000003", code: "MEL", name: "Melbourne",
      timezone: "Australia/Melbourne", active: true, locations: 1620, sequenced: 1620, current: true,
      pack_location: null, owner: null, weight_tolerance_per_mille: 30 },
    { id: "5171e000-0000-0000-0000-000000000004", code: "SYD", name: "Sydney",
      timezone: "Australia/Sydney", active: true, locations: 2160, sequenced: 2150, current: false,
      pack_location: "PACK", owner: "Harbourline", weight_tolerance_per_mille: 30 },
  ],
};

export const READY: WorkspaceState = { kind: "ready", workspace: WORKSPACE };
export const EMPTY: WorkspaceState = {
  kind: "ready",
  workspace: { ...WORKSPACE, sites: [] },
};
export const FAILED: WorkspaceState = {
  kind: "failed",
  message: "The server could not be reached.",
};

const noop = async () => {};

/** Boxes as a prepack import leaves them: a shovel box is one nobody wants suggested (D196). */
const box = (name: string, code: string | null, size: [number, number, number] | null, suggested = true): PackageTypeRow => ({
  id: `9a7e0000-0000-0000-0000-${name.replace(/[^a-z]/gi, "").padEnd(12, "0").slice(0, 12).toLowerCase().replace(/[^0-9a-f]/g, "0")}`,
  name,
  carrier_package_code: code,
  dimensions_fixed: size !== null,
  length_mm: size?.[0] ?? null,
  width_mm: size?.[1] ?? null,
  height_mm: size?.[2] ?? null,
  tare_weight_g: null,
  reusable: false,
  max_payload_g: null,
  tenant_owned: true,
  suggested,
  cut: null,
  packed: 0,
  photos: [],
  carries: size && code !== "PAL" ? estimated(size) : null,
});

/** What a box carries by the estimate, single-wall 32 ECT under the conditions to start, as the server works it out (D249). */
function estimated([l, w]: [number, number, number]): Carries {
  const crush_g = Math.round(((5.87 * ((32 * 4.448222) / 25.4) * Math.sqrt(4 * 2 * (l + w))) / 9.80665) * 1000);
  return { walls_g: Math.round((crush_g * 0.1224) / 100) * 100, walls_estimated: true, crush_g, lid_g: 10000, lid_said: false, board: { walls: 1, ect: 32 }, board_said: false };
}

const MEDIUM = box("Medium Box", "CTN", [450, 340, 410]);

/** A height of a box, cut from it (D252): its own name for the booking. */
const cut = (from: PackageTypeRow, n: number, height: number, packed = 0): PackageTypeRow => ({
  ...from,
  id: `9a7e0000-0000-0000-0000-0000000c${String(n).padStart(4, "0")}`,
  name: `${from.name}, cut to ${height / 10}`,
  height_mm: height,
  cut: { from_id: from.id, from_name: from.name, full_height_mm: from.height_mm! },
  packed,
});

/** The medium box photographed round, its back not yet cropped. */
const shot = (face: Side, n: number, cropped = true): BoxPhoto => ({
  face,
  image_id: `01990000-0000-7000-8000-0000000b${String(n).padStart(4, "0")}`,
  digest: `b0c5${String(n).padStart(4, "0")}a1f2e3d4c5b6a7980f1e2d3c4b5a69788796a5b4c3d2e1f0011`,
  captured_at: "2026-10-14T10:00:00Z",
  cut: cropped ? { digest: `c7d1${String(n).padStart(4, "0")}a1f2e3d4c5b6a7980f1e2d3c4b5a69788796a5b4c3d2e1f0011`, corners: [0.1, 0.1, 0.9, 0.1, 0.9, 0.9, 0.1, 0.9] } : null,
});

/** The medium box, opened to its heights and photographs (D252). */
export const OPEN_BOX = MEDIUM.id;

export const BOXES: PackageTypeRow[] = [
  box("Extra Small Box", "CTN", [320, 230, 160]),
  box("Small Box", "CTN", [390, 310, 300]),
  { ...MEDIUM, packed: 40, photos: [shot("front", 1), shot("right", 2), shot("back", 3, false), shot("top", 4)] },
  cut(MEDIUM, 1, 345, 3),
  cut(MEDIUM, 2, 280, 12),
  cut(MEDIUM, 3, 210),
  // Its walls from a test on the floor, its board read off the stamp, and a lid said to take less.
  {
    ...box("Large Box", "CTN", [660, 440, 460]),
    max_payload_g: 25000,
    carries: { walls_g: 40000, walls_estimated: false, crush_g: null, lid_g: 5000, lid_said: true, board: { walls: 2, ect: 48 }, board_said: true },
  },
  box("Shovel Box", "CTN", [1400, 340, 400], false),
  box("Satchel", null, null),
  box("Pallet", "PAL", [1200, 1200, 1300]),
];

export function fixtureWorkspace(state: WorkspaceState): WorkspaceBench {
  return {
    state,
    busy: false,
    problem: null,
    dismiss: () => {},
    setPackLocation: noop,
    setOwner: noop,
    setWeightTolerance: noop,
    boxes: state.kind === "ready" ? BOXES : null,
    suggest: noop,
    boxWeight: noop,
    boxEmptyWeight: noop,
    addHeight: async () => false,
    removeHeight: noop,
    photographBox: noop,
    cropping: null,
    crop: () => {},
    uncrop: () => {},
    cut: noop,
    findFace: async () => null,
    fields: state.kind === "ready" ? FIELDS : null,
    sayField: noop,
    setCarries: noop,
    setConditions: noop,
    carriers: state.kind === "ready" ? CARRIERS : null,
    addCarrier: async () => false,
    saveCarrier: noop,
    addService: async () => false,
    setUsualCarrier: noop,
    rules: state.kind === "ready" ? RULES : null,
    newRuleSet: async () => null,
    saveRuleSet: async () => false,
  };
}

/** NetSuite's fields as the item details carried them (D238): read by their defaults, but one said here. */
export const FIELDS: ReportedField[] = [
  { source: "netsuite-item-details", field: "Alert", items: 412, example: "Charge bulky freight", role: "warning", unit: null, unit_field: null, level: null, said: false },
  { source: "netsuite-item-details", field: "APN (Carton)", items: 41, example: "19300000000016", role: "barcode", unit: null, unit_field: null, level: "carton", said: false },
  { source: "netsuite-item-details", field: "Colour", items: 520, example: "Blue", role: "shown", unit: null, unit_field: null, level: null, said: false },
  { source: "netsuite-item-details", field: "Cubic", items: 2398, example: "0.004", role: "kept", unit: null, unit_field: null, level: null, said: false },
  { source: "netsuite-item-details", field: "Each/Carton", items: 1201, example: "100", role: "per_carton", unit: null, unit_field: null, level: null, said: false },
  { source: "netsuite-item-details", field: "Item Weight", items: 2290, example: "0.4", role: "weight", unit: null, unit_field: "Weight Unit", level: null, said: false },
  { source: "netsuite-item-details", field: "Length (cm)", items: 1480, example: "37", role: "length", unit: "cm", unit_field: null, level: null, said: false },
  { source: "netsuite-item-details", field: "Melbourne WH Bin", items: 240, example: "C-01-2", role: "note", unit: null, unit_field: null, level: null, said: true },
  { source: "netsuite-item-details", field: "Weight Unit", items: 2290, example: "kg", role: "kept", unit: null, unit_field: null, level: null, said: false },
];

/** A set's rules, the canvas's to start, changed where a carrier asks otherwise (D250). */
function rulesWith(over: Partial<Record<BuildRule["kind"], Partial<BuildRule>>>): BuildRule[] {
  const start: BuildRule[] = [
    { kind: "height", strength: "must", weight: null, amount: 1800, with_base: true, top_counts: null },
    { kind: "weight", strength: "must", weight: null, amount: 1000000, with_base: null, top_counts: null },
    { kind: "overhang_sides", strength: "prefer", weight: 2, amount: 0, with_base: null, top_counts: null },
    { kind: "overhang_ends", strength: "prefer", weight: 2, amount: 0, with_base: null, top_counts: null },
    { kind: "labels", strength: "prefer", weight: 3, amount: null, with_base: null, top_counts: false },
    { kind: "support", strength: "must", weight: null, amount: 750, with_base: null, top_counts: null },
    { kind: "heavy", strength: "must", weight: null, amount: 20000, with_base: null, top_counts: null },
    { kind: "loose", strength: "must", weight: null, amount: 30000, with_base: null, top_counts: null },
    { kind: "together", strength: "prefer", weight: 3, amount: null, with_base: null, top_counts: null },
    { kind: "columns", strength: "prefer", weight: 2, amount: null, with_base: null, top_counts: null },
    { kind: "low_and_centred", strength: "prefer", weight: 2, amount: null, with_base: null, top_counts: null },
    { kind: "flat_top", strength: "prefer", weight: 1, amount: null, with_base: null, top_counts: null },
  ];
  return start.map((r) => ({ ...r, ...over[r.kind] }));
}

const SET = (n: number) => `b0e10000-0000-0000-0000-00000000000${n}`;

/** The two carriers' sets as the user put them (D250): the strict one's labels out and nothing over the sides. */
export const RULES: RuleSetRow[] = [
  {
    id: SET(1),
    version_id: "b0e1a000-0000-0000-0000-000000000001",
    name: "Order pallet",
    rules: rulesWith({}),
    saved_at: "2026-10-16T09:00:00Z",
    saved_by: "Alex Packer",
    used_by: ["Road Freight's pallets that go as one", "Second Freight's pallets that go as one"],
  },
  {
    id: SET(2),
    version_id: "b0e1a000-0000-0000-0000-000000000002",
    name: "Second carrier, breakdown",
    rules: rulesWith({
      overhang_sides: { strength: "must", weight: null },
      labels: { strength: "must", weight: null },
      columns: { weight: 3 },
    }),
    saved_at: "2026-10-16T09:05:00Z",
    saved_by: "Alex Packer",
    used_by: ["Second Freight's breakdown pallets"],
  },
  {
    id: SET(3),
    version_id: "b0e1a000-0000-0000-0000-000000000003",
    name: "Usual carrier, breakdown",
    rules: rulesWith({ overhang_sides: { weight: 1 }, overhang_ends: { weight: 1 } }),
    saved_at: "2026-10-16T09:10:00Z",
    saved_by: "Alex Packer",
    used_by: ["Road Freight's breakdown pallets"],
  },
];

export const CARRIERS: CarrierRow[] = [
  {
    id: "ca220000-0000-0000-0000-000000000001",
    name: "Road Freight",
    code: "ROADFREIGHT",
    own: true,
    usual: true,
    breakdown_rules_id: SET(3),
    as_one_rules_id: SET(1),
    services: [
      { id: "ca225000-0000-0000-0000-000000000001", name: "General", code: "GENERAL" },
      { id: "ca225000-0000-0000-0000-000000000002", name: "Express", code: "EXPRESS" },
    ],
  },
  {
    id: "ca220000-0000-0000-0000-000000000002",
    name: "Second Freight",
    code: "SECONDFREIGHT",
    own: true,
    usual: false,
    breakdown_rules_id: SET(2),
    as_one_rules_id: SET(1),
    services: [{ id: "ca225000-0000-0000-0000-000000000003", name: "Road", code: "ROAD" }],
  },
];
