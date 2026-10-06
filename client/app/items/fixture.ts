import type { CaptureSubject, FamilyMember, ItemListRow, ItemRow, ItemView, PackagingType } from "@domain/types";

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

/** A few of GS1's packaging types, as the server lists them: the common first. */
export const PACKAGING_TYPES: PackagingType[] = [
  { code: "CS", name: "Case", definition: "A container designed to hold its content while protecting it.", six_sided: true, round: false, common: 1 },
  { code: "BX", name: "Box", definition: "A rigid container with closed faces.", six_sided: true, round: false, common: 2 },
  { code: "BG", name: "Bag", definition: "A preformed, flexible container.", six_sided: false, round: false, common: 3 },
  { code: "SW", name: "Shrinkwrapped", definition: "A film heated to shrink around an item.", six_sided: false, round: false, common: 4 },
  { code: "NE", name: "Not packed", definition: "The item is provided without packaging.", six_sided: false, round: false, common: 11 },
  { code: "TU", name: "Tube", definition: "A cylindrical container sealed on one end.", six_sided: false, round: false, common: null },
  { code: "BJ", name: "Bucket", definition: "A container, usually cylindrical, can be equipped with a lid and a handle.", six_sided: false, round: true, common: null },
];
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
  variant_lot_id: null,
  variant_code: null,
  description: null,
  parts: 0,
  gross_weight_g: null,
  length_mm: null,
  width_mm: null,
  height_mm: null,
  diameter_mm: null,
  base_diameter_mm: null,
  top_height_mm: null,
  weight_absent: false,
  dimensions_absent: false,
  packed_in: null,
  packed_in_source: null,
  ships_as_is: false,
  ships_as_is_source: "default",
  upright: false,
  upright_source: "default",
  box_shaped: true,
  round: false,
  is_unit: false,
  offered: false,
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
  is_unit: true,
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

/**
 * The blue brush's family (D228): the green measured and photographed, the
 * yellow weighed, the red with nothing yet.
 */
const BRUSH_FAMILY_MEMBERS: FamilyMember[] = [
  {
    item_id: "01990000-0000-7000-8000-0000000b5121",
    code: "SKU-5120G",
    description: "Floor brush, 450 mm, green",
    active: true,
    picture: { digest: PHOTO, source: "own" },
    cards: [
      { level: "each", weighed: true, measured: true, faces: 1 },
      { level: "carton", weighed: true, measured: true, faces: 7 },
    ],
  },
  {
    item_id: "01990000-0000-7000-8000-0000000b5122",
    code: "SKU-5120R",
    description: "Floor brush, 450 mm, red",
    active: true,
    picture: null,
    cards: [],
  },
  {
    item_id: "01990000-0000-7000-8000-0000000b5123",
    code: "SKU-5120Y",
    description: "Floor brush, 450 mm, yellow",
    active: false,
    picture: null,
    cards: [{ level: "carton", weighed: true, measured: false, faces: 0 }],
  },
];

export const ITEM: ItemView = {
  item_id: BRUSH,
  code: "SKU-5120B",
  description: "Floor brush, 450 mm, blue",
  active: true,
  style: { code: "SKU-5120", description: "Floor brush, 450 mm", variants: 4, picture_item_id: null },
  family: BRUSH_FAMILY_MEMBERS,
  picture: { digest: PHOTO, source: "own" },
  measurements: [],
  packing: { units_per_inner: 1, inners_per_carton: 8, effective_from: "2026-09-30" },
  unit: { level: "each", said: false, netsuite_unit: "Each" },
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
  flags: [],
  box_picture: null,
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
      cut: face === "front" ? { digest: CUT, corners: [0.08, 0.3, 0.92, 0.28, 0.94, 0.68, 0.06, 0.7] } : null,
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
  family: [],
  picture: null,
  measurements: [],
  packing: null,
  unit: { level: "each", said: false, netsuite_unit: null },
  held: [],
  reported: [],
  flags: [],
  // Its own carton offered before anybody has said it has one (D178, D218).
  box_picture: null,
  subjects: [
    subject({ item_id: TAPE_GUN, code: "SKU-8837", description: "Tape gun, 50 mm", packaging_level: "each", is_unit: true }),
    subject({
      item_id: TAPE_GUN,
      code: "SKU-8837",
      description: "Tape gun, 50 mm",
      packaging_level: "carton",
      wants: [],
      offered: true,
    }),
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
    per: "",
    typePer: noop,
    figures: NO_FIGURES,
    type: noop,
    choosePresentation: noop,
    toggleNoDimensions: noop,
    measure: later,
    addVariant: async () => false,
    chooseVariant: later,
    pictureFamily: later,
    packagingTypes: PACKAGING_TYPES,
    packIn: later,
    shipAsIs: later,
    keepUpright: later,
    flagBin: async () => null,
    sayUnit: async () => false,
    refile: async () => false,
    matchFamily: async () => false,
    findItem: async () => null,
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

/** Fixed ids for the D218 items, which share nothing else. */
const CATALOGUE = "01990000-0000-7000-8000-0000000c0a70";
const GLOVES = "01990000-0000-7000-8000-0000000c0a71";
const PLUGS = "01990000-0000-7000-8000-0000000c0a72";
const RESPIRATOR = "01990000-0000-7000-8000-0000000c0a73";

/**
 * Sold by the each, and nothing said of a carton (D218): one card, and a
 * carton only offered.
 */
export const SOLD_SINGLY: ItemView = {
  ...ITEM_UNKNOWN,
  item_id: CATALOGUE,
  code: "Catalogue",
  description: "Foodcare Industry Catalogue",
  unit: { level: "each", said: false, netsuite_unit: "Each" },
  subjects: [
    subject({
      item_id: CATALOGUE,
      code: "Catalogue",
      packaging_level: "each",
      is_unit: true,
      gross_weight_g: 940,
      length_mm: 240,
      width_mm: 172,
      height_mm: 20,
      method: "instrument",
      source: "own",
      observed_at: "2026-10-05T00:30:00Z",
      faces: ["front", "back", "label"],
      wants: [],
      because: "settled",
    }),
    subject({ item_id: CATALOGUE, code: "Catalogue", packaging_level: "carton", wants: [], offered: true }),
  ],
};

/** Sold by the carton of 1,000 (D218): the carton is the item; a single glove is offered. */
export const SOLD_BY_CARTON: ItemView = {
  ...ITEM_UNKNOWN,
  item_id: GLOVES,
  code: "DGN-4110-XL",
  description: "Disposable Nitrile Powder Free Gloves - ctn 1000 - Black - XL",
  unit: { level: "carton", said: false, netsuite_unit: "CTN" },
  packing: { units_per_inner: 1, inners_per_carton: 1000, effective_from: "2026-09-30" },
  subjects: [
    subject({
      item_id: GLOVES,
      code: "DGN-4110-XL",
      packaging_level: "carton",
      is_unit: true,
      gross_weight_g: 6600,
      length_mm: 330,
      width_mm: 400,
      height_mm: 485,
      method: "transcribed",
      source: "own",
      observed_at: "2026-09-30T08:59:00Z",
      ships_as_is: true,
      ships_as_is_source: "default",
      wants: ["photographs"],
      because: "incomplete",
    }),
    subject({ item_id: GLOVES, code: "DGN-4110-XL", packaging_level: "each", wants: [], offered: true }),
  ],
};

/**
 * Sold by the box of 100 (D218), in cartons of ten boxes: the box is the item,
 * the carton is ten of it, and a single pair is offered.
 */
export const SOLD_BY_BOX: ItemView = {
  ...ITEM_UNKNOWN,
  item_id: PLUGS,
  code: "DEJ-8040",
  description: "CS40 Soft Corded Metal Detectable Earplugs Non-Touch TPR Box 100",
  unit: { level: "inner", said: false, netsuite_unit: "Box" },
  packing: { units_per_inner: 100, inners_per_carton: 10, effective_from: "2026-10-02" },
  subjects: [
    subject({
      item_id: PLUGS,
      code: "DEJ-8040",
      packaging_level: "inner",
      is_unit: true,
      gross_weight_g: 520,
      length_mm: 205,
      width_mm: 105,
      height_mm: 95,
      method: "instrument",
      source: "own",
      observed_at: "2026-10-02T03:00:00Z",
      faces: ["front", "right", "top"],
      wants: [],
      because: "settled",
    }),
    subject({
      item_id: PLUGS,
      code: "DEJ-8040",
      packaging_level: "carton",
      gross_weight_g: 5400,
      length_mm: 430,
      width_mm: 220,
      height_mm: 410,
      method: "instrument",
      source: "own",
      observed_at: "2026-10-02T03:10:00Z",
      ships_as_is: true,
      wants: ["photographs"],
      because: "incomplete",
    }),
    subject({ item_id: PLUGS, code: "DEJ-8040", packaging_level: "each", wants: [], offered: true }),
  ],
};

/**
 * A kit measured as though it were its part (D222): a sprayer kit is no
 * physical thing, and the head measured on its card belongs on the head's
 * own item. Sold by the each with no case pack, so it has no other card.
 */
export const MEASURED_AS_KIT: ItemView = {
  ...ITEM_UNKNOWN,
  item_id: "17e10000-0000-0000-0000-0000000000a7",
  code: "SPR-1000",
  description: "Trigger sprayer with 1L bottle (kit)",
  unit: { level: "each", said: false, netsuite_unit: "Each" },
  packing: null,
  subjects: [
    subject({
      item_id: "17e10000-0000-0000-0000-0000000000a7",
      code: "SPR-1000",
      packaging_level: "each",
      is_unit: true,
      gross_weight_g: 95,
      length_mm: 225,
      width_mm: 60,
      height_mm: 110,
      method: "instrument",
      source: "own",
      observed_at: "2026-10-02T01:10:00Z",
      wants: ["photographs"],
      because: "incomplete",
    }),
  ],
};

/**
 * A box of ten P2 respirators weighed, measured and photographed on its
 * carton card before the item said it is sold by the box (D219): the box is
 * the unit with nothing on it, and the carton holds the box's figures.
 */
export const MISFILED: ItemView = {
  ...ITEM_UNKNOWN,
  item_id: RESPIRATOR,
  code: "P2R-0010",
  description: "Portwest P2 Respirator With Valve 10/box",
  unit: { level: "inner", said: true, netsuite_unit: "Box" },
  packing: { units_per_inner: 10, inners_per_carton: 10, effective_from: "2026-10-05" },
  subjects: [
    subject({
      item_id: RESPIRATOR,
      code: "P2R-0010",
      packaging_level: "inner",
      is_unit: true,
      because: "never-measured",
    }),
    subject({
      item_id: RESPIRATOR,
      code: "P2R-0010",
      packaging_level: "carton",
      gross_weight_g: 230,
      length_mm: 140,
      width_mm: 130,
      height_mm: 165,
      method: "instrument",
      source: "own",
      observed_at: "2026-10-01T02:40:00Z",
      faces: ["front"],
      wants: ["photographs"],
      because: "incomplete",
    }),
    subject({ item_id: RESPIRATOR, code: "P2R-0010", packaging_level: "each", wants: [], offered: true }),
  ],
};

/**
 * Said on the floor against NetSuite's bins (D215): none in D-03-2, where
 * NetSuite lists 18, and six found in E-02-4, where it lists none.
 */
export const ITEM_FLAGGED: ItemView = {
  ...ITEM,
  flags: [
    {
      discrepancy_id: "01990000-0000-7000-8000-0000000f0021",
      kind: "found_in_unlisted_bin",
      location_id: "01990000-0000-7000-8000-000000000009",
      bin_code: "E-02-4",
      found: "6",
      detected_at: "2026-09-30T00:02:00Z",
      detected_by: "D. Stooke",
    },
    {
      discrepancy_id: "01990000-0000-7000-8000-0000000f0020",
      kind: "not_in_listed_bin",
      location_id: "01990000-0000-7000-8000-000000000007",
      bin_code: "D-03-2",
      found: "0",
      detected_at: "2026-09-30T00:01:00Z",
      detected_by: "D. Stooke",
    },
  ],
};

/** The each being measured: an each says how it was arranged, and may say it has no box. */
export const MEASURING = fixtureProperties(ITEM, {
  open: { key: `${BRUSH}:each`, action: "measure" },
  figures: { ...NO_FIGURES, weight: "0.52", length: "45", width: "", height: "" },
});

/**
 * An item's own carton measured before anybody said it had one (D178): how
 * many it holds is typed with its figures, and saying it makes the carton.
 * A thousand in packs of 50: twenty packs (D185).
 */
export const CARTON_MEASURING = fixtureProperties(ITEM_UNKNOWN, {
  open: { key: `${TAPE_GUN}:carton`, action: "measure" },
  holds: "1000",
  per: "50",
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

/** A thing in a bag (D191): its photo and what else is worth taking, not six sides. */
export const NO_BOX: ItemView = {
  ...ITEM_UNKNOWN,
  code: "SKU-4410",
  description: "Mop head, cotton, 400 g",
  box_picture: null,
  subjects: [
    subject({
      item_id: TAPE_GUN,
      code: "SKU-4410",
      packaging_level: "each",
      gross_weight_g: 400,
      is_unit: true,
      packed_in: "BG",
      packed_in_source: "own",
      box_shaped: false,
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

/** A bucket (D213): measured across its top and base, its height and the straight band under its rim. */
export const BUCKET: ItemView = {
  ...ITEM_UNKNOWN,
  code: "SKU-2040",
  description: "Floor sealer, 20 L",
  box_picture: null,
  subjects: [
    subject({
      item_id: TAPE_GUN,
      code: "SKU-2040",
      packaging_level: "each",
      gross_weight_g: 21400,
      length_mm: 300,
      width_mm: 300,
      height_mm: 380,
      diameter_mm: 300,
      base_diameter_mm: 265,
      top_height_mm: 75,
      packed_in: "BJ",
      packed_in_source: "own",
      box_shaped: false,
      round: true,
      is_unit: true,
      method: "instrument",
      source: "own",
      observed_at: "2026-10-05T01:00:00Z",
      wants: ["photographs"],
      because: "incomplete",
    }),
  ],
};
export const MEASURING_BUCKET = fixtureProperties(BUCKET, {
  open: { key: `${TAPE_GUN}:each`, action: "measure" },
  figures: { ...NO_FIGURES, weight: "21.4", top: "30", base: "26.5", height: "38", topHeight: "", presentation: "as_supplied" },
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
    // Its front, 52 × 24 cm, marked the right way round (D214).
    corners: [0.08, 0.3, 0.92, 0.28, 0.94, 0.68, 0.06, 0.7],
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
  to_pack: 0,
  to_pack_lines: 0,
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
  ROW({ code: "SKU-3928", description: "Label roll, 100 × 150 mm", reported_on_hand: "140", reported_bins: 3, bin_code: "A-02-1", demand: 22, to_pack: 48, to_pack_lines: 3 }),
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
  const a = { q: "", stock: "", needs: "", has: "", list: "", order: "", ...asked } as Asked;
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
    exportUrl: (format) => `/api/items/export?format=${format}`,
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
    look_id: "01990000-0000-7000-8000-0000000e0099",
    level: "each",
    family: null,
    variant: null,
  },
  subject: BRUSH_EACH,
  name: (["Front", "Right", "Back", "Left", "Top", "Bottom"] as const)[n % 6]!,
  aspect: null,
  corners: null,
  lie: null,
  ...over,
});

const FOUND = [0.16, 0.2, 0.86, 0.18, 0.9, 0.84, 0.12, 0.86];

/**
 * The family carton's front, 52 × 24 cm, its corners found from its side
 * (D214): marked taller than wide, a quarter turn out.
 */
export const TURNED: Queued = QUEUED(6, {
  state: "turned",
  subject: FAMILY_CARTON,
  aspect: 520 / 240,
  corners: [0.3, 0.1, 0.7, 0.1, 0.7, 0.9, 0.3, 0.9],
  lie: "turned",
});

/** A morning's photographs part-way through: one saving, one found, one missed, the rest still to look at. */
export function fixturePhotoQueue(over: Partial<QueueDesk> = {}): QueueDesk {
  return {
    read: { kind: "ready" },
    queued: [
      QUEUED(0, { state: "saving", corners: FOUND }),
      QUEUED(1, {
        state: "found",
        subject: FAMILY_CARTON,
        aspect: 310 / 240,
        corners: [0.2, 0.24, 0.8, 0.22, 0.84, 0.8, 0.18, 0.82],
        lie: "matches",
      }),
      TURNED,
      QUEUED(2, { state: "missed" }),
      QUEUED(3, { state: "finding" }),
      QUEUED(4, { state: "waiting" }),
      QUEUED(5, { state: "waiting" }),
    ],
    phone: false,
    save: later,
    adjusting: null,
    adjust: noop,
    again: noop,
    keep: later,
    move: async () => false,
    crop: { findFace: async () => FOUND, cut: later, uncrop: noop, busy: false, problem: null, dismiss: noop },
    ...over,
  };
}
