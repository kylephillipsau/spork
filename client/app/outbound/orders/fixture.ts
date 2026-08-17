import type { OrdersDesk, OrdersState } from "./useOrders";

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
