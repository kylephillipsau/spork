import type { ItemRow, ItemView } from "@domain/types";

import type { ItemDesk } from "./useItem";
import type { Asked, ItemsDesk, ItemsState } from "./useItems";

/**
 * Items with no network.
 *
 * **SKU-5120B**, the blue one of a brush family in four colours. Its carton
 * was copied from a prepack list against the family, it is on two shelves by
 * NetSuite's last report plus a quantity at the warehouse with no shelf, and
 * Spork has one of those shelves in its own ledger. **SKU-8837**, which nobody
 * has measured, photographed or reported anywhere.
 */

const AS_AT = "2026-09-29T23:10:00Z";

export const ITEM: ItemView = {
  item_id: "01990000-0000-7000-8000-0000000b5120",
  code: "SKU-5120B",
  description: "Floor brush, 450 mm, blue",
  active: true,
  style: { code: "SKU-5120", description: "Floor brush, 450 mm", variants: 4 },
  picture: null,
  measurements: [
    {
      packaging_level: "carton",
      source: "style",
      style_code: "SKU-5120",
      item_packing_config_id: "01990000-0000-7000-8000-0000000c5120",
      length_mm: 520,
      width_mm: 310,
      height_mm: 240,
      gross_weight_g: 4600,
      net_weight_g: null,
      tare_weight_g: null,
      method: "transcribed",
      observed_at: "2026-09-30T00:20:00Z",
    },
  ],
  packing: { units_per_inner: 1, inners_per_carton: 8, effective_from: "2026-09-30" },
  held: [
    {
      site_code: "NTH",
      location_id: "01990000-0000-7000-8000-000000000001",
      bin_code: "C-01-1",
      quantity: 4,
      allocated_quantity: 1,
    },
  ],
  reported: [
    {
      site_code: "NTH",
      location_id: "01990000-0000-7000-8000-000000000001",
      bin_code: "C-01-1",
      on_hand: "4",
      available: "4",
      status: "Good",
      as_at: AS_AT,
      source: "netsuite-inventory-balance",
    },
    {
      site_code: "NTH",
      location_id: "01990000-0000-7000-8000-000000000007",
      bin_code: "D-03-2",
      on_hand: "18",
      available: "12",
      status: "Good",
      as_at: AS_AT,
      source: "netsuite-inventory-balance",
    },
    {
      site_code: "NTH",
      location_id: null,
      bin_code: null,
      on_hand: "2",
      available: "2",
      status: "Good",
      as_at: AS_AT,
      source: "netsuite-inventory-balance",
    },
  ],
};

export const ITEM_UNKNOWN: ItemView = {
  item_id: "01990000-0000-7000-8000-0000000b8837",
  code: "SKU-8837",
  description: "Tape gun, 50 mm",
  active: true,
  style: null,
  picture: null,
  measurements: [],
  packing: null,
  held: [],
  reported: [],
};

export const fixtureItem = (item: ItemView): ItemDesk => ({ read: { kind: "ready", item } });

let row = 0;
const ROW = (over: Partial<ItemRow> & Pick<ItemRow, "code" | "description">): ItemRow => ({
  item_id: `01990000-0000-7000-8000-00000000c${String(++row).padStart(3, "0")}`,
  active: true,
  style_code: null,
  picture: null,
  figures: "none",
  reported_on_hand: null,
  reported_bins: 0,
  held: 0,
  ...over,
});

/** A page of the list: in stock and not, measured and not, a family's and its own. */
export const ITEMS: ItemRow[] = [
  ROW({ code: "SKU-3928", description: "Label roll, 100 × 150 mm", reported_on_hand: "140", reported_bins: 3 }),
  ROW({ code: "SKU-5120B", description: "Floor brush, 450 mm, blue", style_code: "SKU-5120", figures: "listed", reported_on_hand: "24", reported_bins: 2, held: 4 }),
  ROW({ code: "SKU-5120G", description: "Floor brush, 450 mm, green", style_code: "SKU-5120", figures: "listed", reported_on_hand: "12", reported_bins: 1 }),
  ROW({ code: "SKU-7461", description: "Wet floor sign, folding", figures: "measured", reported_on_hand: "7.5", reported_bins: 1 }),
  ROW({ code: "SKU-8837", description: "Tape gun, 50 mm" }),
];

const noop = () => {};

export function fixtureItems(state: ItemsState, asked: Partial<Asked> = {}): ItemsDesk {
  const a = { q: "", stock: "", needs: "", ...asked } as Asked;
  return { state, asked: a, typed: a.q, type: noop, search: noop, narrow: noop, more: async () => {} };
}

export const ITEMS_PAGE: ItemsState = { kind: "ready", items: ITEMS, total: 9181, next: "SKU-8837", more: false };
export const ITEMS_NONE: ItemsState = { kind: "ready", items: [], total: 0, next: null, more: false };
