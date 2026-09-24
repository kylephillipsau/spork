import type { ReceivingScreen } from "@domain/types";
import type { Bay, ReceivingBench } from "./useReceiving";

/**
 * A truck at the dock, reachable with no network.
 *
 * Three lines, and each is a different thing the screen has to say: one received
 * by lot with a case pack to count in, one that has already been part-delivered,
 * and one whose promise names no owner.
 */
const LINES: ReceivingScreen["lines"] = [
  {
    // **Received by lot.** The disposition refuses this line without one, so the
    // row says so before the pallet is opened rather than after.
    expected_supply_id: "01a05b03-0eeb-7423-aac4-fe6dac905e8a",
    item_id: "17e10000-0000-0000-0000-000000000001",
    item_code: "GLOVE-M",
    description: "Nitrile glove, medium",
    order_number: "PO-2026-0031",
    supplier: "Gloveco",
    expected_from: "2026-09-01T07:00:00Z",
    expected: 130,
    received: 0,
    outstanding: 130,
    requires_lot: true,
    levels: [
      { level: "each", units: 1, item_packing_config_id: null, baseline: null },
      {
        level: "carton",
        units: 12,
        item_packing_config_id: "9ac00000-0000-0000-0000-000000000001",
        // **The state the third witness exists for.** A carton of these has
        // been on a scale fourteen times, so a dock weight can be divided by
        // it. The `each` level above has no baseline and that is not an
        // oversight: nobody weighs one glove.
        baseline: { grams: 400, n: 14, borrowed: false, established: true },
      },
    ],
    owner_id: "9a247000-0000-0000-0000-000000000001",
    owners: [],
    picture: null,
  },
  {
    // Part delivered already: 97 of 130 in, 33 still owed. The state the
    // fixture's own promise is in, and the one a second truck arrives against.
    expected_supply_id: "01a05b03-0eeb-7423-aac4-fe6dac905e8b",
    item_id: "17e10000-0000-0000-0000-000000000002",
    item_code: "STY-7720-08",
    description: "Ridgeway StepSure Gumboot - Steel Toe - Pair - Green - AU8 EU42",
    order_number: "PO-2026-0031",
    supplier: "Gloveco",
    expected_from: "2026-09-01T07:00:00Z",
    expected: 130,
    received: 97,
    outstanding: 33,
    requires_lot: false,
    // A sized code whose only weighing belongs to its style, and only one of
    // them. Drawn so the borrowed-and-thin wording is visible somewhere.
    levels: [
      {
        level: "each",
        units: 1,
        item_packing_config_id: null,
        baseline: { grams: 1_850, n: 1, borrowed: true, established: false },
      },
    ],
    owner_id: "9a247000-0000-0000-0000-000000000001",
    owners: [],
    picture: {
      digest: "ebf4f635a17d10d6eb46ba680b70142419aa3220f228001a036d311a22ee9d2a",
      source: "own",
    },
  },
  {
    // **The promise names no owner**, which the fixture's real one does not
    // either — so the receipt is refused until somebody says whose the goods
    // are, and the parties already holding this item here are what is offered.
    expected_supply_id: "01a05b03-0eeb-7423-aac4-fe6dac905e8c",
    item_id: "17e10000-0000-0000-0000-000000000003",
    item_code: "PN165918",
    description: "Oates Commercial Lobby Pan Set",
    order_number: "PO-2026-0044",
    supplier: "Kalgoorlie Mine Supplies",
    expected_from: null,
    expected: 24,
    received: 0,
    outstanding: 24,
    requires_lot: false,
    levels: [{ level: "each", units: 1, item_packing_config_id: null, baseline: null }],
    owner_id: null,
    owners: [
      { owner_id: "9a247000-0000-0000-0000-000000000001", name: "Alpha Foods" },
      { owner_id: "9a247000-0000-0000-0000-000000000003", name: "Spork Operating" },
    ],
    picture: null,
  },
];

export const RECEIVING_FIXTURE: ReceivingScreen = { site: "MEL", lines: LINES };

/** Nothing expected. Everything promised has arrived, which is the good state. */
export const RECEIVING_CLEAR: ReceivingScreen = { site: "MEL", lines: [] };

export const THE_DOCK: Bay = {
  id: "10c00000-0000-0000-0000-000000000003",
  code: "DOCK-1",
};

export function fixtureReceiving(
  screen: ReceivingScreen = RECEIVING_FIXTURE,
  over: Partial<ReceivingBench> = {},
): ReceivingBench {
  return {
    status: { kind: "ready", screen },
    busy: false,
    problem: null,
    dismiss: () => {},
    delivery: null,
    bay: null,
    closeDelivery: () => {},
    scan: { typed: "", refocus: 0 },
    typeScan: () => {},
    read: async () => {},
    counting: null,
    choose: () => {},
    release: () => {},
    entered: "",
    typeEntered: () => {},
    level: "each",
    chooseLevel: () => {},
    lotCode: "",
    typeLot: () => {},
    lotExpiry: "",
    typeExpiry: () => {},
    owner: null,
    chooseOwner: () => {},
    base: null,
    weighed: "",
    typeWeighed: () => {},
    scale: null,
    missing: null,
    receive: async () => {},
    closeShort: async () => {},
    landed: null,
    refresh: async () => {},
    ...over,
  };
}
