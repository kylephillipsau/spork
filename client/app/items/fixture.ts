import type { ItemView } from "@domain/types";

import type { ItemDesk } from "./useItem";

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
