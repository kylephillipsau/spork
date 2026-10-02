import type { CaptureSubject, ItemListRow, ItemRow, ItemView } from "@domain/types";

import { NO_FIGURES } from "./subjects";
import type { PropertiesDesk } from "./useItemProperties";
import type { Asked, ItemsDesk, ItemsState } from "./useItems";
import type { QueueDesk, Queued } from "./usePhotoQueue";

/**
 * Items with no network.
 *
 * **SKU-5120B**, the blue one of a brush family in four colours. Its family's
 * carton was copied from a prepack list, its own each has been weighed and
 * photographed from the front, it is on two shelves by NetSuite's last report
 * plus a quantity at the warehouse with no shelf, and Spork has one of those
 * shelves in its own ledger. **SKU-8837**, which nobody has measured,
 * photographed or reported anywhere.
 */

const AS_AT = "2026-09-29T23:10:00Z";
const BRUSH = "01990000-0000-7000-8000-0000000b5120";
const BRUSH_FAMILY = "01990000-0000-7000-8000-0000000a5120";
const TAPE_GUN = "01990000-0000-7000-8000-0000000b8837";

const subject = (over: Partial<CaptureSubject> & Pick<CaptureSubject, "code" | "packaging_level">): CaptureSubject => ({
  item_id: null,
  item_style_id: null,
  item_part_id: null,
  part_label: null,
  lot_id: null,
  lot_code: null,
  description: null,
  parts: 0,
  gross_weight_g: null,
  length_mm: null,
  width_mm: null,
  height_mm: null,
  weight_absent: false,
  dimensions_absent: false,
  source: null,
  style_code: null,
  method: null,
  observed_at: null,
  faces: [],
  wants: ["weight", "dimensions", "photographs"],
  demand: 0,
  because: "nothing",
  location_code: null,
  soh: 0,
  ...over,
});

/** The family's carton, copied from a prepack list; nobody has measured it here. */
const FAMILY_CARTON = subject({
  item_style_id: BRUSH_FAMILY,
  code: "SKU-5120",
  description: "Floor brush, 450 mm",
  packaging_level: "carton",
  gross_weight_g: 4600,
  length_mm: 520,
  width_mm: 310,
  height_mm: 240,
  source: "own",
  method: "transcribed",
  observed_at: "2026-09-30T00:20:00Z",
  wants: ["weight", "dimensions", "photographs"],
  because: "never-measured",
  demand: 14,
});

/** Its own each: weighed on a scale, not yet measured, photographed from the front. */
const BRUSH_EACH = subject({
  item_id: BRUSH,
  code: "SKU-5120B",
  description: "Floor brush, 450 mm, blue",
  packaging_level: "each",
  gross_weight_g: 520,
  source: "own",
  method: "instrument",
  observed_at: "2026-09-30T04:12:00Z",
  faces: ["front"],
  wants: ["dimensions", "photographs"],
  because: "incomplete",
  demand: 6,
});

const PHOTO = "a3f1e0c2b4d6a8f0e2c4b6d8f0a2c4e6b8d0f2a4c6e8b0d2f4a6c8e0b2d4f6a8";
/** The front of the family's carton, cut out and straightened (D176). */
const CUT = "c7d1e0c2b4d6a8f0e2c4b6d8f0a2c4e6b8d0f2a4c6e8b0d2f4a6c8e0b2d4f6c7";
const image = (n: number) => `01990000-0000-7000-8000-0000000e${String(n).padStart(4, "0")}`;

export const ITEM: ItemView = {
  item_id: BRUSH,
  code: "SKU-5120B",
  description: "Floor brush, 450 mm, blue",
  active: true,
  style: { code: "SKU-5120", description: "Floor brush, 450 mm", variants: 4 },
  picture: { digest: PHOTO, source: "own" },
  measurements: [],
  packing: { units_per_inner: 1, inners_per_carton: 8, effective_from: "2026-09-30" },
  held: [
    {
      site_code: "NTH",
      location_id: "01990000-0000-7000-8000-000000000001",
      bin_code: "C-01-1",
      within_reach: true,
      quantity: 4,
      allocated_quantity: 1,
    },
  ],
  reported: [
    {
      site_code: "NTH",
      location_id: "01990000-0000-7000-8000-000000000001",
      bin_code: "C-01-1",
      within_reach: true,
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
      within_reach: true,
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
      within_reach: null,
      on_hand: "2",
      available: "2",
      status: "Good",
      as_at: AS_AT,
      source: "netsuite-inventory-balance",
    },
  ],
  subjects: [FAMILY_CARTON, BRUSH_EACH],
  photos: [
    {
      item_id: BRUSH,
      item_style_id: null,
      item_part_id: null,
      lot_id: null,
      packaging_level: "each",
      face: "front",
      image_id: image(1),
      digest: PHOTO,
      captured_at: "2026-09-30T04:13:00Z",
      cut: null,
      same_as: null,
    },
    // The family's carton photographed on four of its sides and its label:
    // drawn as a box, the two sides nobody took show their names.
    // Its front has been cut to the face; the rest are as taken.
    ...(["front", "right", "top", "back", "label"] as const).map((face, i) => ({
      item_id: null,
      item_style_id: BRUSH_FAMILY,
      item_part_id: null,
      lot_id: null,
      packaging_level: "carton",
      face,
      image_id: image(10 + i),
      digest: PHOTO,
      captured_at: "2026-09-30T04:20:00Z",
      cut: face === "front" ? { digest: CUT, corners: [0.12, 0.2, 0.9, 0.16, 0.94, 0.86, 0.08, 0.9] } : null,
      same_as: null,
    })),
  ],
};

export const ITEM_UNKNOWN: ItemView = {
  item_id: TAPE_GUN,
  code: "SKU-8837",
  description: "Tape gun, 50 mm",
  active: true,
  style: null,
  picture: null,
  measurements: [],
  packing: null,
  held: [],
  reported: [],
  // Its own carton offered before anybody has said it has one (D178).
  subjects: [
    subject({ item_id: TAPE_GUN, code: "SKU-8837", description: "Tape gun, 50 mm", packaging_level: "carton", wants: [] }),
    subject({ item_id: TAPE_GUN, code: "SKU-8837", description: "Tape gun, 50 mm", packaging_level: "each" }),
  ],
  photos: [],
};

const noop = () => {};
const later = async () => {};

/** An item's properties with nothing behind them: the screen as it would be. */
export function fixtureProperties(item: ItemView, over: Partial<PropertiesDesk> = {}): PropertiesDesk {
  return {
    read: { kind: "ready", item },
    open: null,
    show: noop,
    close: noop,
    reading: "",
    unit: "kg",
    typeReading: noop,
    setUnit: noop,
    weigh: later,
    holds: "",
    typeHolds: noop,
    figures: NO_FIGURES,
    type: noop,
    choosePresentation: noop,
    toggleNoDimensions: noop,
    measure: later,
    addVariant: async () => false,
    taken: [],
    sending: {},
    attach: noop,
    resend: noop,
    same: noop,
    cropping: null,
    crop: noop,
    uncrop: noop,
    cut: later,
    // The face where a model would put it, without the model.
    findFace: async () => [0.18, 0.2, 0.84, 0.17, 0.88, 0.83, 0.14, 0.86],
    barcodes: [],
    binding: "",
    typeBinding: noop,
    count: "",
    typeCount: noop,
    bind: later,
    busy: false,
    problem: null,
    said: null,
    dismiss: noop,
    ...over,
  };
}

/** The each being measured: an each says how it was arranged, and may say it has no box. */
export const MEASURING = fixtureProperties(ITEM, {
  open: { key: `${BRUSH}:each`, action: "measure" },
  figures: { ...NO_FIGURES, weight: "0.52", length: "45", width: "", height: "" },
});

/**
 * An item's own carton measured before anybody said it had one (D178): how
 * many it holds is typed with its figures, and saying it makes the carton.
 */
export const CARTON_MEASURING = fixtureProperties(ITEM_UNKNOWN, {
  open: { key: `${TAPE_GUN}:carton`, action: "measure" },
  holds: "12",
  figures: { ...NO_FIGURES, weight: "6.4", length: "41", width: "31", height: "" },
});

/** The family's carton put on the scale, and the reading far from the list's figure. */
export const WEIGHED_APART = fixtureProperties(ITEM, {
  said: {
    tone: "warning",
    text: "Weighed 6.1 kg. That is a long way from the 4.6 kg on record, so a finding was raised to look into it.",
    finding: "01990000-0000-7000-8000-0000000f0001",
  },
});

/** A thing with no box shape: one photo and its label, not six sides. */
export const NO_BOX: ItemView = {
  ...ITEM_UNKNOWN,
  code: "SKU-4410",
  description: "Mop head, cotton, 400 g",
  subjects: [
    subject({
      item_id: TAPE_GUN,
      code: "SKU-4410",
      packaging_level: "each",
      gross_weight_g: 400,
      dimensions_absent: true,
      method: "instrument",
      source: "own",
      observed_at: "2026-09-30T05:00:00Z",
      wants: ["photographs"],
      because: "incomplete",
    }),
  ],
};
export const PHOTOGRAPHING_NO_BOX = fixtureProperties(NO_BOX, {
  open: { key: `${TAPE_GUN}:each`, action: "photos" },
});

/** The camera open on the each, its front and then its top taken in this look. */
export const PHOTOGRAPHING = fixtureProperties(
  { ...ITEM, photos: [...ITEM.photos, { ...ITEM.photos[0]!, face: "top", image_id: image(2), captured_at: "2026-09-30T04:14:00Z" }] },
  { open: { key: `${BRUSH}:each`, action: "photos" }, taken: ["front", "top"] },
);

/** Straight after photographing the each's front: its corners to mark. */
export const CROPPING = fixtureProperties(ITEM, {
  open: { key: `${BRUSH}:each`, action: "photos" },
  taken: ["front"],
  cropping: { subject: BRUSH_EACH, face: "front", image_id: image(1), digest: PHOTO, corners: null },
});

/** The carton's front open again at a desk, where it was cut before. */
export const RECROPPING = fixtureProperties(ITEM, {
  open: { key: `${BRUSH_FAMILY}:carton`, action: "photos" },
  cropping: {
    subject: FAMILY_CARTON,
    face: "front",
    image_id: image(10),
    digest: PHOTO,
    corners: [0.12, 0.2, 0.9, 0.16, 0.94, 0.86, 0.08, 0.9],
  },
});

let row = 0;
const ROW = (over: Partial<ItemRow> & Pick<ItemRow, "code" | "description">): ItemRow => ({
  item_id: `01990000-0000-7000-8000-00000000c${String(++row).padStart(3, "0")}`,
  active: true,
  style_code: null,
  picture: null,
  weight: "none",
  size: "none",
  demand: 0,
  bin_code: null,
  bin_within_reach: null,
  reported_on_hand: null,
  reported_bins: 0,
  held: 0,
  list_position: null,
  ...over,
});

/** A page of the list: in stock and not, measured and not, a family's and its own. */
export const ITEMS: ItemRow[] = [
  ROW({ code: "SKU-3928", description: "Label roll, 100 × 150 mm", reported_on_hand: "140", reported_bins: 3, bin_code: "A-02-1", demand: 22 }),
  ROW({
    item_id: BRUSH,
    code: "SKU-5120B",
    description: "Floor brush, 450 mm, blue",
    style_code: "SKU-5120",
    picture: { digest: PHOTO, source: "own" },
    weight: "measured",
    size: "listed",
    reported_on_hand: "24",
    reported_bins: 2,
    held: 4,
    bin_code: "C-01-1",
    demand: 6,
  }),
  ROW({ code: "SKU-5120G", description: "Floor brush, 450 mm, green", style_code: "SKU-5120", size: "listed", weight: "listed", reported_on_hand: "12", reported_bins: 1, bin_code: "C-01-2" }),
  ROW({ code: "SKU-7461", description: "Wet floor sign, folding", weight: "measured", size: "measured", reported_on_hand: "7.5", reported_bins: 1, bin_code: "D-01-3", demand: 3 }),
  ROW({ code: "SKU-8837", description: "Tape gun, 50 mm" }),
];

/** Two sheets made into lists (D179). */
export const LISTS: ItemListRow[] = [
  { item_list_id: "01990000-0000-7000-8000-0000000a0002", name: "Weights and sizes, 1 Oct", items: 30, recorded_at: "2026-10-02T00:10:00Z", recorded_by_name: "Sam Lee" },
  { item_list_id: "01990000-0000-7000-8000-0000000a0001", name: "Weights and sizes, 24 Sep", items: 25, recorded_at: "2026-10-02T00:05:00Z", recorded_by_name: "Sam Lee" },
];

export function fixtureItems(
  state: ItemsState,
  asked: Partial<Asked> = {},
  chosen: string | null = null,
  over: Partial<ItemsDesk> = {},
): ItemsDesk {
  const a = { q: "", stock: "", needs: "", list: "", order: "", ...asked } as Asked;
  return {
    state,
    asked: a,
    typed: a.q,
    type: noop,
    search: noop,
    narrow: noop,
    more: later,
    chosen,
    choose: noop,
    lists: LISTS,
    pick: noop,
    makeList: async () => false,
    making: { busy: false, problem: null, dismiss: noop },
    ...over,
  };
}

/** The 1 Oct sheet, in its order: its place on the paper beside each row. */
export const ITEMS_LISTED: ItemsState = {
  kind: "ready",
  items: ITEMS.slice(0, 4).map((r, i) => ({ ...r, list_position: i + 1 })),
  total: 4,
  next: null,
  more: false,
};

export const ITEMS_PAGE: ItemsState = { kind: "ready", items: ITEMS, total: 9181, next: "SKU-8837", more: false };
export const ITEMS_NONE: ItemsState = { kind: "ready", items: [], total: 0, next: null, more: false };
export { BRUSH };

/** One photograph in the queue to cut (D181). */
const QUEUED = (n: number, over: Partial<Queued> & Pick<Queued, "state">): Queued => ({
  photo: {
    image_id: image(40 + n),
    digest: PHOTO,
    face: (["front", "right", "back", "left", "top", "bottom"] as const)[n % 6]!,
    captured_at: "2026-10-02T00:08:00Z",
    item_id: BRUSH,
    code: "SKU-5120B",
    description: "Floor brush, 450 mm, blue",
  },
  subject: BRUSH_EACH,
  name: (["Front", "Right", "Back", "Left", "Top", "Bottom"] as const)[n % 6]!,
  aspect: null,
  corners: null,
  ticked: false,
  ...over,
});

const FOUND = [0.16, 0.2, 0.86, 0.18, 0.9, 0.84, 0.12, 0.86];

/** A morning's photographs part-way through: found and ticked, one missed, the rest still to look at. */
export function fixturePhotoQueue(over: Partial<QueueDesk> = {}): QueueDesk {
  return {
    read: { kind: "ready" },
    queued: [
      QUEUED(0, { state: "found", corners: FOUND, ticked: true }),
      QUEUED(1, { state: "found", corners: [0.2, 0.24, 0.8, 0.22, 0.84, 0.8, 0.18, 0.82], ticked: true }),
      QUEUED(2, { state: "missed" }),
      QUEUED(3, { state: "finding" }),
      QUEUED(4, { state: "waiting" }),
      QUEUED(5, { state: "waiting" }),
    ],
    phone: false,
    tick: noop,
    save: later,
    saving: null,
    adjusting: null,
    adjust: noop,
    crop: { findFace: async () => FOUND, cut: later, uncrop: noop, busy: false, problem: null, dismiss: noop },
    ...over,
  };
}
