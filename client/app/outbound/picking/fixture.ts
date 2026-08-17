import type { PickListScreen } from "@domain/types";
import type { Destination, PickBench } from "./usePicking";

/**
 * The pick walk, reachable with no network.
 *
 * **Every state the screen draws is reached from one of these**, which is the
 * rule D131's gate enforces. Four rows, and each one is a different thing the
 * screen has to say: a picture of this code, a picture borrowed from the
 * style, no picture at all, and a line with nowhere to pick it from.
 */
const LINES: PickListScreen["lines"] = [
  {
    fulfilment_line_id: "f11e0000-0000-0000-0000-000000000004",
    item_id: "17e10000-0000-0000-0000-000000000002",
    item_code: "STY-7720-08",
    description: "Ridgeway StepSure Gumboot - Steel Toe - Pair - Green - AU8 EU42",
    reference: "IF400187",
    stock_id: "01a00d1f-45e8-7b3d-9179-bc3da0d05d25",
    location_code: "A-01-1",
    pick_sequence: 1,
    lot_code: null,
    picked: 0,
    // Covered by planning, so picking these claims nothing.
    covered: 24,
    remaining: 24,
    available: 60,
    allocated: true,
    picture: {
      digest: "ebf4f635a17d10d6eb46ba680b70142419aa3220f228001a036d311a22ee9d2a",
      source: "own",
    },
  },
  {
    // **The inherited picture, which is the case D141 exists for.** Nobody has
    // photographed size 12; the style's carton is what the picker gets, and the
    // label on it is what stops that being a claim about this code.
    fulfilment_line_id: "f11e0000-0000-0000-0000-000000000005",
    item_id: "17e10000-0000-0000-0000-000000000009",
    item_code: "STY-7720-12",
    description: "Ridgeway StepSure Gumboot - Steel Toe - Pair - Green - AU12 EU46",
    reference: "IF265596",
    stock_id: "01a00d1f-45e8-7b3d-9179-bc3da0d05d26",
    location_code: "A-02-3",
    pick_sequence: 7,
    lot_code: null,
    picked: 0,
    // Nothing covered: the claim is the whole pick.
    covered: 0,
    remaining: 2,
    available: 2,
    allocated: false,
    picture: {
      digest: "ebf4f635a17d10d6eb46ba680b70142419aa3220f228001a036d311a22ee9d2a",
      source: "style",
    },
  },
  {
    // Nothing photographed, and a short pick coming: the bin holds less than
    // the line wants and the screen says so before the walk rather than after.
    fulfilment_line_id: "f11e0000-0000-0000-0000-000000000001",
    item_id: "17e10000-0000-0000-0000-000000000001",
    item_code: "GLOVE-M",
    description: "Nitrile glove, medium",
    reference: "IF265591",
    stock_id: "01a00d1f-45dc-7bd0-876b-e8d74d909f09",
    location_code: "B-04-2",
    pick_sequence: 22,
    lot_code: "L2026-021",
    // **Half picked and covered for less than that.** The line was for 60,
    // twenty are picked, and only thirty were ever claimed — so picking ten
    // more claims exactly what is missing rather than all ten or none.
    picked: 20,
    covered: 30,
    remaining: 40,
    available: 12,
    allocated: false,
    picture: null,
  },
  {
    // **Nowhere to pick it from**, which is the row a filtered list would hide
    // and the one a picker most needs to see.
    fulfilment_line_id: "f11e0000-0000-0000-0000-000000000003",
    item_id: "17e10000-0000-0000-0000-000000000003",
    item_code: "PN165918",
    description: "Oates Commercial Lobby Pan Set",
    reference: "IF265593",
    stock_id: null,
    location_code: null,
    pick_sequence: null,
    lot_code: null,
    picked: 0,
    covered: 0,
    remaining: 6,
    available: null,
    allocated: false,
    picture: null,
  },
];

export const PICKING_FIXTURE: PickListScreen = { site: "MEL", lines: LINES };

/** Nothing to pick, which is the screen's best state and its emptiest. */
export const PICKING_CLEAR: PickListScreen = { site: "MEL", lines: [] };

export function fixturePicking(
  screen: PickListScreen = PICKING_FIXTURE,
  over: Partial<PickBench> = {},
): PickBench {
  return {
    status: { kind: "ready", screen },
    busy: false,
    problem: null,
    dismiss: () => {},
    destination: null,
    scan: { typed: "", refocus: 0 },
    typeScan: () => {},
    read: async () => {},
    clearDestination: () => {},
    confirmed: null,
    quantity: "",
    typeQuantity: () => {},
    choose: () => {},
    release: () => {},
    take: async () => {},
    took: null,
    refresh: async () => {},
    ...over,
  };
}

/** The pallet the forklift is loading. A package, which is the arm that has
 *  always worked — `PALLET-A` has been in the fixture since the beginning. */
export const ON_A_PALLET: Destination = {
  kind: "package",
  id: "9a7e0000-0000-0000-0000-0000000000a1",
  code: "PALLET-A",
};

/** The packing station a trolley is wheeled to. A location, which is what D166
 *  added and what a trolley makes necessary. */
export const AT_THE_STATION: Destination = {
  kind: "location",
  id: "10c00000-0000-0000-0000-000000000004",
  code: "PACK-1",
};
