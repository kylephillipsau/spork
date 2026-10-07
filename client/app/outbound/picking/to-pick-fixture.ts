import type { AskedOrder, ToPick, ToPickLine } from "@domain/types";
import { MAP_SITE, MAP_WALK } from "@app/layout/map-fixture";

import type { ToPickDesk, TripWalk } from "./useToPick";

/** A batch of four pasted orders, invented: two waiting, one picked, one nobody knows. */

const line = (key: string, code: string, description: string, takes: [string, number, boolean][], extra: Partial<ToPickLine> = {}): ToPickLine => ({
  line_key: key,
  line_no: Number(key),
  item_id: null,
  code,
  description,
  art_no: null,
  picture: null,
  ordered: takes.reduce((n, t) => n + t[1], 0),
  committed: null,
  to_pick: takes.reduce((n, t) => n + t[1], 0),
  kit: false,
  part_of: null,
  takes: takes.map(([bin, quantity, reach], i) => ({ location_id: `10c00000-0000-0000-0000-00000000000${i}`, bin, quantity, on_hand: quantity * 4, within_reach: reach })),
  short: 0,
  ...extra,
});

const stop = (bin: string, code: string, order: string, quantity: number) => ({
  location_id: null,
  bin,
  within_reach: true,
  code,
  description: null,
  takes: [{ order, quantity, gathered: false }],
  off_route: false,
});

const order = (number: string, customer: string, lines: ToPickLine[], extra: Partial<AskedOrder> = {}): AskedOrder => ({
  asked: number,
  number,
  state: "waiting",
  ordered_on: "2026-10-07",
  customer,
  ship_to: `${customer}\n12 Example Street\nBRUNSWICK VIC 3056`,
  picking_instructions: null,
  customer_notes: null,
  ship_via: "Courier",
  po_ref: null,
  status: "Pending Fulfillment",
  lines,
  ...extra,
});

export const BATCH: ToPick = {
  orders_as_at: new Date(Date.now() - 3 * 60_000).toISOString(),
  balance_as_at: new Date(Date.now() - 4 * 60_000).toISOString(),
  orders: [
    order(
      "S100231",
      "Hillcrest Cafe",
      [line("1", "GLV-NIT-M", "Nitrile gloves, medium, box 100", [["K-12-01", 10, true]]), line("2", "BIN-LNR-80", "Bin liners 80 L, carton 250", [["M-02-04", 1, false]])],
      { picking_instructions: "Rear door. Leave with the manager.", customer_notes: "No snakes, please." },
    ),
    order("S100245", "Riverside Deli", [
      line("1", "WIP-ROLL-B", "Wiper roll, blue", [["D-04-02", 6, true]]),
      line("2", "DUST-PAN", "Hygiene dustpan", [], { short: 2, to_pick: 2, ordered: 2 }),
    ]),
    order("S100252", "Northgate Bakery", [], { state: "picked", ship_to: null }),
    order("S1009999", "", [], { asked: "1009999", number: null, customer: null, state: "unknown", ship_to: null, ship_via: null, ordered_on: null }),
  ],
  plan: {
    from: "pack",
    cell_mm: 1000,
    pickers: [
      [{ orders: ["S100231"], walked: 40, minutes: 3.4, path: [], stops: [stop("K-12-01", "GLV-NIT-M", "S100231", 10), stop("M-02-04", "BIN-LNR-80", "S100231", 1)] }],
      [{ orders: ["S100245"], walked: 22, minutes: 2.1, path: [], stops: [stop("D-04-02", "WIP-ROLL-B", "S100245", 6)] }],
    ],
  },
};

/** The screen's desk over a batch, doing nothing when pressed. */
export function fixtureToPick(batch: ToPick | null = BATCH, pasted = "S100231\tS100245\tS100252\t1009999"): ToPickDesk {
  return {
    state: batch ? { kind: "ready", batch } : { kind: "loading" },
    pasted,
    paste: () => undefined,
    asked: pasted ? pasted.split("\t") : [],
    lookUp: () => undefined,
    everything: () => undefined,
    pickers: 2,
    setPickers: () => undefined,
    perTrip: null,
    setPerTrip: () => undefined,
    gather: true,
    setGather: () => undefined,
    printed: () => null,
    walking: null,
    map: { kind: "loading" },
    walkTrip: () => undefined,
    goTo: () => undefined,
    got: () => undefined,
    notGot: () => undefined,
    leaveWalk: () => undefined,
  };
}

/** Group A's trip, two stops in: three of the drafted site's bins on the map's sample route. */
export function fixtureWalk(): ToPickDesk {
  const [a, b, c] = MAP_SITE.bins.bins.filter((x) => x.reported_items > 0);
  const at = (bin: typeof a, code: string, takes: [string, number, boolean][]) => ({
    location_id: bin!.location_id,
    bin: bin!.code,
    within_reach: bin!.within_reach,
    code,
    description: null,
    takes: takes.map(([order, quantity, gathered]) => ({ order, quantity, gathered })),
    off_route: false,
  });
  const walking: TripWalk = {
    label: "Group A · trip 1",
    trip: {
      orders: ["S100231", "S100245"],
      walked: MAP_WALK.walked,
      minutes: 6,
      path: MAP_WALK.path,
      stops: [
        at(a, "WIP-ROLL-B", [["S100245", 6, false]]),
        at(b, "GLV-NIT-M", [["S100231", 10, false], ["S100245", 4, false], ["S100260", 4, true]]),
        at(c, "BIN-LNR-80", [["S100231", 1, false]]),
      ],
    },
    pictures: {},
    customers: { S100231: "Hillcrest Cafe", S100245: "Riverside Deli" },
    at: 1,
    done: [0],
  };
  return { ...fixtureToPick(), walking, map: { kind: "ready", site: MAP_SITE } };
}
