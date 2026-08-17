import type { BoundBarcode, CaptureScreen, CaptureSubject, Resolution } from "@domain/types";
import type { CaptureBench, Figures } from "./useCapture";

/**
 * The capture screen, reachable with no network.
 *
 * **Every state the screen draws is reached from one of these**, which is the
 * rule D131's gate enforces and the reason it exists: a drawn empty-container
 * state once shipped and rendered on zero screens for a commit, because no
 * fixture had an empty carton. So every list here carries a row it could
 * actually be on, `CAPTURE_CLEAR` carries none at all, the two session fixtures
 * cover the stages a worklist never shows, and the three `SCAN_*` values at the
 * foot cover what the locator can answer.
 *
 * The figures are the seed's own: STY-7720's carton is four transcribed
 * numbers and STY-7720-08's each is an instrument weight with no dimensions,
 * which is what makes the two lists here the two lists the read produces.
 */

const GUMBOOT_CARTON: CaptureSubject = {
  item_id: null,
  item_style_id: "57110000-0000-0000-0000-000000000001",
  item_part_id: null,
  part_label: null,
  parts: 0,
  code: "STY-7720",
  description: "Ridgeway StepSure Gumboot - Steel Toe - Green",
  packaging_level: "carton",
  gross_weight_g: 11400,
  length_mm: 450,
  width_mm: 340,
  height_mm: 410,
  weight_absent: false,
  dimensions_absent: false,
  source: "own",
  style_code: null,
  method: "transcribed",
  observed_at: "2026-08-04T05:00:00Z",
  faces: [],
  wants: ["photographs"],
  demand: 1,
  because: "incomplete",
  location_code: "A-01-1",
  soh: 14,
};

const GUMBOOT_EACH: CaptureSubject = {
  item_id: "17e10000-0000-0000-0000-000000000002",
  item_style_id: null,
  item_part_id: null,
  part_label: null,
  parts: 0,
  code: "STY-7720-08",
  description: "Ridgeway StepSure Gumboot - Steel Toe - Pair - Green - AU8 EU42",
  packaging_level: "each",
  gross_weight_g: 1900,
  length_mm: null,
  width_mm: null,
  height_mm: null,
  weight_absent: false,
  dimensions_absent: false,
  source: "own",
  style_code: null,
  method: "instrument",
  observed_at: "2026-08-04T05:00:00Z",
  faces: [],
  wants: ["dimensions", "photographs"],
  demand: 1,
  because: "incomplete",
  location_code: "A-01-1",
  soh: 14,
};

/** The inherited case, which is the one D108 exists for: every figure here was
 *  taken against the style and none against this code. */
const GUMBOOT_VARIANT_CARTON: CaptureSubject = {
  item_id: "17e10000-0000-0000-0000-000000000002",
  item_style_id: null,
  item_part_id: null,
  part_label: null,
  parts: 0,
  code: "STY-7720-08",
  description: "Ridgeway StepSure Gumboot - Steel Toe - Pair - Green - AU8 EU42",
  packaging_level: "carton",
  gross_weight_g: 11400,
  length_mm: 450,
  width_mm: 340,
  height_mm: 410,
  weight_absent: false,
  dimensions_absent: false,
  source: "style",
  style_code: "STY-7720",
  method: "transcribed",
  observed_at: "2026-08-04T05:00:00Z",
  faces: ["front"],
  wants: [],
  demand: 1,
  because: "never-measured",
  location_code: "A-04-2",
  soh: 6,
};

/** Nothing on file at all — the subject the `Held` panel draws as absence. */
const GLOVE_CARTON: CaptureSubject = {
  item_id: "17e10000-0000-0000-0000-000000000001",
  item_style_id: null,
  item_part_id: null,
  part_label: null,
  parts: 0,
  code: "GLOVE-M",
  description: "Nitrile glove, medium",
  packaging_level: "carton",
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
  demand: 3,
  because: "nothing",
  location_code: "B-02-1",
  soh: 40,
};

/**
 * D139's case, drawn: one sellable code and two things that go in a box.
 *
 * The set's each carries a **declared absence** rather than three empty
 * fields — somebody looked at a pan and a 1200mm handle and recorded that they
 * make no bounding box between them. Without this row the screen has no way to
 * draw the difference between *there is none* and *nobody has looked*, which is
 * the whole of D138.
 */
const PAN_SET_EACH: CaptureSubject = {
  item_id: "17e10000-0000-0000-0000-000000000003",
  item_style_id: null,
  item_part_id: null,
  part_label: null,
  parts: 2,
  code: "PN165918",
  description: "Oates Commercial Lobby Pan Set",
  packaging_level: "each",
  gross_weight_g: 2100,
  length_mm: null,
  width_mm: null,
  height_mm: null,
  weight_absent: false,
  dimensions_absent: true,
  source: "own",
  style_code: null,
  method: "instrument",
  observed_at: "2026-08-17T01:00:00Z",
  faces: [],
  wants: ["photographs"],
  demand: 2,
  because: "incomplete",
  location_code: "C-11-3",
  soh: 3,
};

/** The half with figures against it, off the supplier's sheet rather than off a
 *  scale — which is what puts a complete part on `unconfirmed` rather than on
 *  no list at all. A part has no packaging level, which is what the null says
 *  and what stops a screen posting one. */
const PAN_SET_PAN: CaptureSubject = {
  item_id: null,
  item_style_id: null,
  item_part_id: "9a710000-0000-0000-0000-000000000001",
  part_label: "Pan",
  parts: 0,
  code: "PN165918",
  description: "Pan",
  packaging_level: null,
  gross_weight_g: 900,
  length_mm: 330,
  width_mm: 290,
  height_mm: 300,
  weight_absent: false,
  dimensions_absent: false,
  source: "own",
  style_code: null,
  method: "transcribed",
  observed_at: "2026-08-17T01:00:00Z",
  faces: ["front"],
  wants: [],
  demand: 2,
  because: "never-measured",
  location_code: "C-11-3",
  soh: 3,
};

/** The half nobody has walked to yet. */
const PAN_SET_HANDLE: CaptureSubject = {
  item_id: null,
  item_style_id: null,
  item_part_id: "9a710000-0000-0000-0000-000000000002",
  part_label: "Handle",
  parts: 0,
  code: "PN165918",
  description: "Handle",
  packaging_level: null,
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
  demand: 2,
  because: "nothing",
  location_code: "C-11-3",
  soh: 3,
};

/**
 * **Every row says what the server would say about it.**
 *
 * The first draft had the glove — three wants, every figure null — under
 * `partial`, which `capture_worklist.rs` asserts the read never produces: a
 * three-wants subject takes the classifier's first branch and is `unrecorded`,
 * whose word here is `because: "nothing"`. So the fixture was drawing a state
 * the endpoint cannot emit, in the half of the system no test reads. That is
 * the same "two renderings of one state disagreeing" the HTTP test exists to
 * catch, and the fixture is the reviewable surface — a fixture that lies is
 * worse than no fixture, because it is what gets looked at.
 */
export const CAPTURE_FIXTURE: CaptureScreen = {
  site: "MEL",
  // **Bin order, and deliberately not code order.** A fixture sorted the same
  // way both would be a fixture that cannot tell the two apart, and the sort is
  // the thing this list is for.
  walk: [
    GUMBOOT_CARTON,
    GUMBOOT_EACH,
    GUMBOOT_VARIANT_CARTON,
    GLOVE_CARTON,
    PAN_SET_EACH,
    PAN_SET_PAN,
    PAN_SET_HANDLE,
  ],
};

/**
 * Nothing to capture, which is the screen's best state and its emptiest.
 *
 * Its own fixture rather than an empty list inside the one above, now that the
 * walk carries every row it should. The drawn absence and the "Nothing waiting"
 * pill, neither of which any other fixture reaches.
 */
export const CAPTURE_CLEAR: CaptureScreen = {
  site: "MEL",
  walk: [],
};

/** The subject a session fixture is opened against. A carton: rigid, one
 *  arrangement, so the figures stage draws four fields and nothing else. */
export const CAPTURE_SUBJECT = GLOVE_CARTON;

/** **The other figures stage**, and the one D138 and D139 both land on: a
 *  single loose thing, which has to say how it was arranged, and a set with
 *  parts, which can say it has no box at all. A carton reaches neither
 *  control, so without this route both render nowhere. */
export const CAPTURE_SUBJECT_EACH = PAN_SET_EACH;

const noop = async () => {};

/** A bench with no network behind it, at whichever stage is asked for. */
export function fixtureBench(
  stage: CaptureBench["stage"],
  over: Partial<CaptureBench> = {},
  screen: CaptureScreen = CAPTURE_FIXTURE,
): CaptureBench {
  const figures: Figures = {
    weight: "",
    length: "",
    width: "",
    height: "",
    presentation: "",
    noDimensions: false,
  };
  return {
    status: { kind: "ready", screen },
    stage,
    figures,
    scan: { typed: "", found: null, refocus: 0 },
    typeScan: () => {},
    lookUp: noop,
    clearScan: () => {},
    taken: [],
    busy: false,
    problem: null,
    recorded: null,
    dismiss: () => {},
    choose: () => {},
    leave: () => {},
    type: () => {},
    choosePresentation: () => {},
    toggleNoDimensions: () => {},
    record: noop,
    attach: noop,
    finish: noop,
    barcodes: [],
    binding: "",
    typeBinding: () => {},
    count: "",
    typeCount: () => {},
    bind: noop,
    ...over,
  };
}

/**
 * What a box already answers to (D164).
 *
 * Three bindings and each is a different sentence: a GTIN on the carton with a
 * case pack, an internal code on the box, and one written before D164 that does
 * not say which of the two it is on — which is the state the column exists to
 * stop being created and the one that will be on screen for a while yet. The
 * unbound person on that row is a feed rather than an unnamed operator, and the
 * screen says so.
 */
export const BOUND_BARCODES: BoundBarcode[] = [
  {
    id: "b47c0000-0000-0000-0000-000000000001",
    item_id: "17e10000-0000-0000-0000-000000000001",
    item_code: "GLOVE-M",
    barcode: "09312345678907",
    scheme: "gtin",
    packaging_level: "carton",
    quantity: 100,
    bound_by_name: "d.stooke",
    already: false,
    warnings: [],
  },
  {
    id: "b47c0000-0000-0000-0000-000000000002",
    item_id: "17e10000-0000-0000-0000-000000000001",
    item_code: "GLOVE-M",
    barcode: "GLV-M-1",
    scheme: "internal",
    packaging_level: "each",
    quantity: 1,
    bound_by_name: "k.phillips",
    already: false,
    warnings: [],
  },
  {
    id: "b47c0000-0000-0000-0000-000000000003",
    item_id: "17e10000-0000-0000-0000-000000000001",
    item_code: "GLOVE-M",
    barcode: "LEGACY-9",
    scheme: "internal",
    packaging_level: "unrecorded",
    quantity: 1,
    bound_by_name: null,
    already: false,
    warnings: [],
  },
];

// ── what a scan turned out to be ──────────────────────────────────────────
//
// Three of the four outcomes, because each draws something different and a
// state no fixture reaches is a state nobody has seen. `identifier_unrecognised`
// draws the same shape as `identifier_unknown` with one different sentence, so
// it is the one this does not spend a route on.

/** The good case, and the one the style rule makes interesting: the gumboot
 *  resolves to its own `each` and to its **style's** carton, which is the
 *  subject the worklist would offer. A screen that opened the variant's carton
 *  instead would send somebody to weigh one box four times. */
export const SCAN_RESOLVED: Resolution = {
  outcome: "resolved",
  scanned: "9312345678907",
  gtin: "09312345678907",
  sscc: null,
  lot: "1ABC42",
  expiry: "260101",
  subjects: [
    {
      kind: "item",
      id: "17e10000-0000-0000-0000-000000000002",
      code: "STY-7720-08",
      description: "Ridgeway StepSure Gumboot - Steel Toe - Pair - Green - AU8 EU42",
      via: "item_barcode",
      capture: [GUMBOOT_VARIANT_CARTON, GUMBOOT_EACH],
    },
  ],
};

/** A supplier code collision — the thing `issuer_party_id` exists to scope, and
 *  the outcome that must never be answered by picking one. */
export const SCAN_AMBIGUOUS: Resolution = {
  outcome: "identifier_ambiguous",
  scanned: "CTN-99",
  gtin: null,
  sscc: null,
  lot: null,
  expiry: null,
  subjects: [
    {
      kind: "item",
      id: "17e10000-0000-0000-0000-000000000001",
      code: "GLOVE-M",
      description: "Nitrile glove, medium",
      via: "item_barcode",
      capture: [],
    },
    {
      kind: "item",
      id: "17e10000-0000-0000-0000-000000000002",
      code: "STY-7720-08",
      description: "Ridgeway StepSure Gumboot - Steel Toe - Pair - Green - AU8 EU42",
      via: "item_code",
      capture: [],
    },
  ],
};

/** A real GTIN for a product nobody here stocks. Well-formed, and not ours —
 *  which is a different thing to tell an operator than "that did not read". */
export const SCAN_UNKNOWN: Resolution = {
  outcome: "identifier_unknown",
  scanned: "12345670",
  gtin: "00000012345670",
  sscc: null,
  lot: null,
  expiry: null,
  subjects: [],
};
