import type { OrdersDesk, OrdersState } from "./useOrders";
import type { OrderDesk, OrderRead } from "./useOrder";

/**
 * Finding an order, with no network.
 *
 * The superseded pair is the fixture worth having: D44's cancel-and-reraise is
 * the case a lookup would hide, and it takes an amended order to produce.
 */
const noop = async () => {};

export function fixtureOrders(state: OrdersState, reference = "S260041"): OrdersDesk {
  return { state, reference, type: () => {}, search: noop, list: noop };
}

const commitment = (over: Partial<import("@domain/types").FulfilmentSummary> = {}) => ({
  fulfilment_id: "f01f0000-0000-0000-0000-000000000004",
  reference: "IF260101",
  state: "open",
  site_id: "a5170000-0000-0000-0000-000000000001",
  site_code: "MEL",
  line_count: 3,
  committed_quantity: 24,
  picked_quantity: 24,
  packed_quantity: 24,
  despatched_quantity: 0,
  fully_picked: true,
  as_at: "2026-08-18T00:00:00Z",
  ...over,
});

export const FOUND: OrdersState = {
  kind: "found",
  reference: "S260041",
  orders: [
    {
      order_id: "0d1e0000-0000-0000-0000-000000000001",
      confirmation_number: "S260041",
      external_ref: null,
      customer_name: "Marden Trade Supplies",
      state: "open",
      placed_at: "2026-08-14T02:00:00Z",
      promised_to: "2026-08-19T02:00:00Z",
      supersedes_order_id: null,
      fulfilments: [commitment(), commitment({
        fulfilment_id: "f01f0000-0000-0000-0000-000000000005",
        reference: "IF260102",
        picked_quantity: 6,
        packed_quantity: 0,
        fully_picked: false,
        as_at: null,
      })],
    },
  ],
};

/** D44's cancel-and-reraise: one reference, two orders, and the older one is
 *  cancelled. A lookup that showed the newest would hide the thing somebody
 *  ringing about a changed order needs to see. */
export const SUPERSEDED: OrdersState = {
  kind: "found",
  reference: "S260041",
  orders: [
    {
      ...FOUND.kind === "found" ? FOUND.orders[0]! : ({} as never),
      order_id: "0d1e0000-0000-0000-0000-000000000002",
      supersedes_order_id: "0d1e0000-0000-0000-0000-000000000001",
    },
    {
      ...FOUND.kind === "found" ? FOUND.orders[0]! : ({} as never),
      state: "cancelled",
      fulfilments: [],
    },
  ],
};

/** The number is right in the other system and missing here. */
/** What the screen opens on: the latest, nobody having asked for them. */
export const LATEST: OrdersState = {
  kind: "listed",
  orders: FOUND.kind === "found" ? FOUND.orders : [],
};

export const NOTHING: OrdersState = { kind: "found", reference: "S999999", orders: [] };

export const IDLE: OrdersState = { kind: "idle" };

const line = (over: Partial<import("@domain/types").OrderLineView>): import("@domain/types").OrderLineView => ({
  order_line_id: "0d1e1000-0000-0000-0000-000000000001",
  line_number: 1,
  item_id: "17e00000-0000-0000-0000-000000000001",
  item_code: "GLOVE-M",
  description: "Nitrile gloves, medium, box of 100",
  ordered_quantity: 12,
  committed_quantity: 12,
  picked_quantity: 12,
  external_picked_quantity: 0,
  packed_quantity: 12,
  despatched_quantity: 0,
  ...over,
});

/** An order's own page, with the lines worth drawing: one done, one picked on
 *  the handheld (D172), and one committed short of what was ordered. */
export function fixtureOrder(read: OrderRead): OrderDesk {
  return { read, refresh: noop };
}

export const ORDER: OrderRead = {
  kind: "ready",
  order: {
    ...(FOUND.kind === "found" ? FOUND.orders[0]! : ({} as never)),
    lines: [
      line({}),
      line({
        order_line_id: "0d1e1000-0000-0000-0000-000000000002",
        line_number: 2,
        item_id: "17e00000-0000-0000-0000-000000000002",
        item_code: "TRAY-FOIL-L",
        description: "Foil tray, large, sleeve of 50, with lids",
        ordered_quantity: 6,
        committed_quantity: 6,
        picked_quantity: 0,
        external_picked_quantity: 6,
        packed_quantity: 0,
      }),
      line({
        order_line_id: "0d1e1000-0000-0000-0000-000000000003",
        line_number: 3,
        item_id: "17e00000-0000-0000-0000-000000000003",
        item_code: "BAG-KRAFT-S",
        description: "Kraft bag, small",
        ordered_quantity: 10,
        committed_quantity: 6,
        picked_quantity: 0,
        packed_quantity: 0,
      }),
    ],
  },
};

export const ORDER_MISSING: OrderRead = { kind: "failed", message: "Nothing was found at that address." };
