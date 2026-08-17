import type { PutawayScreen } from "@domain/types";
import type { Bin, PutawayBench } from "./usePutaway";

/**
 * The dock, reachable with no network.
 *
 * Three cells, and each one is a different thing the screen has to say: goods
 * that already have a home to consolidate into, goods that have never been
 * stored here, and a cell partly claimed by an order that has not shipped.
 */
const CELLS: PutawayScreen["cells"] = [
  {
    // The common case, and the one `homes` exists for: this item already lives
    // in A-01-1, so the obvious answer is on the row rather than in somebody's
    // memory of the warehouse.
    stock_id: "01a05ac2-0bcd-7c85-a7cc-cd8a9877ea17",
    item_id: "17e10000-0000-0000-0000-000000000001",
    item_code: "GLOVE-M",
    description: "Nitrile glove, medium",
    location_code: "DOCK-1",
    lot_code: "L2026-021",
    quantity: 120,
    available: 120,
    homes: [
      { location_id: "10c00000-0000-0000-0000-000000000001", location_code: "A-01-1", quantity: 40, pick_sequence: 1 },
      { location_id: "10c00000-0000-0000-0000-000000000002", location_code: "B-01-1", quantity: 18, pick_sequence: 12 },
    ],
    picture: null,
  },
  {
    // **Never stored here**, which is the row a directed put-away would have the
    // least to say about and the one an operator most needs to think about.
    // "nowhere yet" is an answer; a confident wrong bin would not be.
    stock_id: "01a05ac2-19e9-7496-a3ce-151ccb624fdb",
    item_id: "17e10000-0000-0000-0000-000000000002",
    item_code: "STY-7720-08",
    description: "Ridgeway StepSure Gumboot - Steel Toe - Pair - Green - AU8 EU42",
    location_code: "DOCK-1",
    lot_code: null,
    quantity: 24,
    available: 24,
    homes: [],
    picture: {
      digest: "ebf4f635a17d10d6eb46ba680b70142419aa3220f228001a036d311a22ee9d2a",
      source: "own",
    },
  },
  {
    // Part of it is already claimed against an order. It does not stop the
    // put-away — the goods need a shelf either way — and it is worth saying
    // before somebody wonders where the free units went.
    stock_id: "01a05ac3-4a1b-7d20-9f31-2c9a7e410b55",
    item_id: "17e10000-0000-0000-0000-000000000003",
    item_code: "PN165918",
    description: "Oates Commercial Lobby Pan Set",
    location_code: "DOCK-1",
    lot_code: null,
    quantity: 30,
    available: 12,
    homes: [
      { location_id: "10c00000-0000-0000-0000-000000000002", location_code: "B-01-1", quantity: 6, pick_sequence: 12 },
    ],
    picture: null,
  },
];

export const PUTAWAY_FIXTURE: PutawayScreen = { site: "MEL", cells: CELLS };

/** A clear dock: everything that arrived has a home. The good state. */
export const PUTAWAY_CLEAR: PutawayScreen = { site: "MEL", cells: [] };

/** The bin somebody scanned, in the state after the scan. */
export const A_BIN: Bin = {
  id: "10c00000-0000-0000-0000-000000000001",
  code: "A-01-1",
};

export function fixturePutaway(
  screen: PutawayScreen = PUTAWAY_FIXTURE,
  over: Partial<PutawayBench> = {},
): PutawayBench {
  return {
    status: { kind: "ready", screen },
    busy: false,
    problem: null,
    dismiss: () => {},
    scan: { typed: "", refocus: 0 },
    typeScan: () => {},
    read: async () => {},
    holding: null,
    bin: null,
    quantity: "",
    typeQuantity: () => {},
    choose: () => {},
    chooseBin: () => {},
    release: () => {},
    away: async () => {},
    stowed: null,
    refresh: async () => {},
    ...over,
  };
}
