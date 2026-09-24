/**
 * The API's shapes.
 *
 * **These are hand-mirrored, and that is a known debt.** D113 says types are
 * generated from the API rather than kept by hand, because a hand-kept copy of
 * a Rust struct is a copy that drifts — which is the failure this repository
 * has a register for. Until the generator exists, `bench.contract.test` reads
 * `crates/server/src/bench.rs` and fails when a field here has no counterpart
 * there, which buys the same guarantee more cheaply than a schema pipeline and
 * is honest about being a stopgap.
 */

export type Uuid = string;

/** A place the item actually is, with what is free to claim. The operator picks
 *  the cell because question 26 — who allocates, and when — is deferred, so
 *  `/allocations` is directed only and the screen has to offer the choice. */
export interface Cell {
  stock_id: Uuid;
  location: string;
  lot: string | null;
  available: number;
}

export interface BenchLine {
  line_id: Uuid;
  item_code: string;
  description: string | null;
  remaining: number;
  cells: Cell[];
}

export interface Preset {
  id: Uuid;
  name: string;
}

/** A box states its size; a pallet's height is the stack and only the
 *  measurement knows it. Present only when the preset claims a fixed size and
 *  states all three. */
export interface StatedSize {
  length_mm: number;
  width_mm: number;
  height_mm: number;
}

export interface PackedRow {
  item_code: string;
  description: string | null;
  lot_code: string | null;
  quantity: number;
  /** `[movement, units left]`, newest first. Taking units back out reverses
   *  these in order — a row is not one movement, and pretending it is fails on
   *  the second pick of the same item into the same carton. */
  picks: [Uuid, number][];
}

/**
 * What something has weighed before, and how much that is worth knowing.
 *
 * **Never render `grams` on its own.** `n` is how many weighings are behind the
 * figure — the fewest behind any line, where it is a sum — and `borrowed` says
 * whether they belong to an item's style rather than to the code itself. A
 * figure resting on one weighing of another size looks exactly like one resting
 * on forty of this one until those two are read. `established` is the server's
 * answer to "is that enough", and it is the server's so that the dock and the
 * bench cannot come to different ones.
 */
export interface WeightBaseline {
  grams: number;
  n: number;
  borrowed: boolean;
  established: boolean;
}

/** What a carton should weigh, beside what the scale says.
 *
 *  `WeightBaseline` flattened on the wire, plus the gap. */
export interface ExpectedWeight extends WeightBaseline {
  /** Weighed minus expected. Null until somebody puts the carton on a scale. */
  delta_g: number | null;
  delta_per_mille: number | null;
}

export interface CartonSummary {
  id: Uuid;
  sequence: string;
  package_type: string | null;
  sealed: boolean;
  gross_weight_g: number | null;
  height_mm: number | null;
  stated_size: StatedSize | null;
  expected: ExpectedWeight | null;
  contents: PackedRow[];
}

/** `GET /fulfilments/{id}/bench` — the whole screen, in one read. */
export interface BenchScreen {
  reference: string;
  order_reference: string;
  customer: string;
  site: string;
  dock_id: Uuid | null;
  lines: BenchLine[];
  cartons: CartonSummary[];
  presets: Preset[];
  wrong_box_reason_id: Uuid | null;
}

// ── despatch ──────────────────────────────────────────────────────────────
// Stages 6 to 9, which had no screen in any form.

export interface WaitingCarton {
  id: Uuid;
  sequence: string;
  package_type: string | null;
  gross_weight_g: number | null;
  /** A carton nobody weighed is a carton the carrier will weigh for you, and
   *  their figure arrives attached to their invoice. */
  weighed: boolean;
}

export interface WaitingJob {
  fulfilment_id: Uuid;
  reference: string;
  order_reference: string;
  customer: string;
  cartons: WaitingCarton[];
  /** Null when any carton in the job was never weighed: unknown rather than
   *  light, because a total that treats a missing figure as zero is the number
   *  a carrier's invoice later disagrees with. */
  gross_weight_g: number | null;
}

export interface CartonLine {
  fulfilment_line_id: Uuid;
  quantity: number;
}

export interface BookedCarton {
  id: Uuid;
  sequence: string;
  reference: string;
  despatched: boolean;
  /** Despatching is a movement of stock out, and stock moves by line — so the
   *  read carries what is in the carton rather than making the screen fetch
   *  each one before it can act. */
  lines: CartonLine[];
}

export interface BookedConsignment {
  consignment_id: Uuid;
  carrier_name: string | null;
  carrier_service_name: string | null;
  /** The carrier's word, null until one has been asked. */
  status: string | null;
  despatch_at: string | null;
  package_count: number;
  awaiting_despatch: number;
  gross_weight_g: number | null;
  packages: BookedCarton[];
}

export interface GoneConsignment {
  consignment_id: Uuid;
  carrier_name: string | null;
  package_count: number;
  last_despatched_at: string | null;
}

export interface CarrierService {
  id: Uuid;
  name: string;
  code: string;
}

export interface Carrier {
  id: Uuid;
  name: string;
  code: string;
  services: CarrierService[];
}

/** How the carrier is reached, which D1 keeps separate from who they are. */
export interface Provider {
  id: Uuid;
  name: string;
  kind: string;
}

/** `GET /sites/{id}/despatch` */
export interface DespatchScreen {
  site: string;
  waiting: WaitingJob[];
  booked: BookedConsignment[];
  gone_today: GoneConsignment[];
  carriers: Carrier[];
  providers: Provider[];
}

/** One line of what a carrier is actually handed — the walkthrough's "quantity
 *  gap", answered as a fold rather than re-entered by hand. */
export interface CarrierLine {
  package_type: string | null;
  carrier_package_code: string | null;
  package_count: number;
  gross_weight_g: number | null;
  length_mm: number | null;
  width_mm: number | null;
  height_mm: number | null;
  uniform: boolean;
}

/** What `POST /consignments` answers with. */
export interface ConsignmentResponse {
  consignment_id: Uuid;
  carrier_name: string | null;
  carrier_service_name: string | null;
  status: string | null;
  despatch_at: string | null;
  package_count: number;
  total_gross_weight_g: number | null;
  carrier_lines: CarrierLine[];
  replayed: boolean;
  warnings: string[];
}

// ── capture ───────────────────────────────────────────────────────────────
//
// `GET /capture`. Three lists, and a subject is on one of them or on none —
// which is the property `crates/server/tests/capture_worklist.rs` asserts and
// the reason the screen can draw them as three sections without deduplicating.

/** One thing to walk to, and everything the screen draws about it.
 *
 *  The identity is `(item_id | item_style_id, packaging_level)`, which is
 *  exactly what `POST /observations` takes as a subject. */
export interface CaptureSubject {
  item_id: Uuid | null;
  item_style_id: Uuid | null;
  /** D139. A pan and its handle, measured on their own because the set they
   *  make has no box of its own. A subject carrying this posts `item_part_id`
   *  and no packaging level. */
  item_part_id: Uuid | null;
  part_label: string | null;
  code: string;
  description: string | null;
  /** `each` or `carton`, and null for a part — a part has no packaging level,
   *  and sending one would send a field the writer refuses. */
  packaging_level: string | null;
  /** Parts this subject is made of. Non-zero says the box to measure is not
   *  this one: measure the parts, and answer *not applicable* here. D139. */
  parts: number;
  /** Canonical: grams and millimetres. What was typed lives on the observation. */
  gross_weight_g: number | null;
  length_mm: number | null;
  width_mm: number | null;
  height_mm: number | null;
  /** Declared to have none, rather than not yet weighed. D138. */
  weight_absent: boolean;
  /** All three lengths declared absent. Two of three is not an answer. */
  dimensions_absent: boolean;
  /** `own`, `style` or `mixed` — D108. A screen that cannot tell them apart
   *  reports a number nobody took against this code as though somebody had. */
  source: string | null;
  style_code: string | null;
  method: string | null;
  observed_at: string | null;
  /** Faces photographed against this subject, of the seven. */
  faces: string[];
  /** `weight`, `dimensions`, `photographs` — what a session here would add. */
  wants: string[];
  demand: number;
  /** `nothing`, `incomplete`, `never-measured`, `overdue`. */
  because: string;
  /** The bin holding the most of it at this site. Null where the stock
   *  projection has none — a catalogue row nobody has put anywhere. */
  location_code: string | null;
  /** How much is here, over every bin at this site. */
  soh: number;
}

/**
 * A barcode bound to an item at a packaging level (D164).
 *
 * **`packaging_level` is what a binding written before D164 does not say.** The
 * list read spells those `unrecorded` rather than sending a null, because the
 * screen draws the word and *the level nobody recorded* is a different sentence
 * from an empty pill.
 */
export interface BoundBarcode {
  id: Uuid;
  item_id: Uuid;
  item_code: string;
  barcode: string;
  /** `gtin` or `internal`. `supplier_reference` needs an issuer party and the
   *  floor has no way to name one. */
  scheme: string;
  packaging_level: string;
  /** Base units per scan. Null is a variable measure, and only a GTIN may be
   *  one — the weight rides in the barcode rather than in a table. */
  quantity: number | null;
  /** Who said this barcode means this item. Null is a feed, an importer or the
   *  seed, which is a real and different answer from an unnamed person (D11). */
  bound_by_name: string | null;
  /** This binding already existed exactly as proposed. A trigger under a glove
   *  fires twice, so a repeat answers the row rather than refusing it. */
  already: boolean;
  warnings: string[];
}

/** What `POST /items/{id}/barcodes` takes. */
export interface BindBarcodeRequest {
  scan: string;
  packaging_level: string;
  quantity: number | null;
}

/** `GET /capture` */
export interface CaptureScreen {
  site: string;
  /** **One list, in the order somebody would walk it.** This was three —
   *  unrecorded, partial, unconfirmed — which is right for triage at a desk and
   *  wrong for a lap of the warehouse: three lists in bin order walk past each
   *  shelf three times. `wants` and `because` carry what the list used to say. */
  walk: CaptureSubject[];
}

/** What `POST /observations` answers with.
 *
 *  `observation_event_id` is the whole reason the client reads this response:
 *  D133 makes the capture session one act, and the photographs that follow are
 *  attached to the event it returns. */
export interface RecordObservationResponse {
  observable_id: Uuid;
  observation_event_id: Uuid;
  observation_ids: Uuid[];
  /** Package dimension columns the act set, so J12 keeps holding. Empty for a
   *  catalogue subject, which caches nothing. */
  package_columns_written: string[];
  warnings: string[];
}

// ── the locator ───────────────────────────────────────────────────────────
//
// `GET /resolve`. One input resolves any scannable identifier against the three
// surfaces (D111, D34), and answers with one of D24's four outcome words.

/** One thing an identifier resolved to. */
export interface Subject {
  /** `item`, `package` or `location`. */
  kind: string;
  id: Uuid;
  code: string;
  description: string | null;
  /** Which surface answered: `item_barcode`, `item_code`, `package_barcode`,
   *  `package_sscc` or `location_code`. */
  via: string;
  /** The capture subjects this item offers — the worklist's own enumeration,
   *  not a second one, so a scan cannot open a session the worklist would not
   *  list. Absent for a package or a location. */
  capture: CaptureSubject[];
}

/** `GET /resolve?scan=…` */
export interface Resolution {
  /** `resolved`, `identifier_unknown`, `identifier_unrecognised`,
   *  `identifier_ambiguous`. Four states, not two: a well-formed GTIN nobody
   *  stocks is a different report from a smudge, and an operator can act on
   *  the first. */
  outcome: string;
  scanned: string;
  /** Normalised to fourteen characters when the string carried one. */
  gtin: string | null;
  sscc: string | null;
  /** A GS1-128 carton label carries these beside the GTIN, so one scan answers
   *  three questions. */
  lot: string | null;
  expiry: string | null;
  subjects: Subject[];
}

// ── findings ──────────────────────────────────────────────────────────────
//
// `GET /discrepancies`. The queue D8 raises into, and the thing architecture.md
// calls the most valuable output the system produces.

/** One finding: where the model and the world disagree. */
export interface DiscrepancyRow {
  id: Uuid;
  /** `count_variance`, `scan_mismatch`, `containment_conflict`, … */
  kind: string;
  /** `open`, `investigating`, `resolved`, `accepted`. */
  state: string;
  item_id: Uuid | null;
  item_code: string | null;
  holder_location_id: Uuid | null;
  location_code: string | null;
  holder_package_id: Uuid | null;
  package_barcode: string | null;
  /** **Decimal strings, not numbers.** A quantity here is `numeric` and a JSON
   *  number is an IEEE double; the server sends the digits rather than round
   *  the evidence on the way out. Render as received. */
  expected_quantity: string | null;
  observed_quantity: string | null;
  variance: string | null;
  detail: string | null;
  stock_count_id: Uuid | null;
  stock_movement_id: Uuid | null;
  detected_at: string;
  detected_by_id: Uuid | null;
  resolving_movement_id: Uuid | null;
  resolved_at: string | null;
  resolved_by_id: Uuid | null;
  resolution_reason: string | null;
  /** Who found it. Null is a scheduled check, not an unnamed person. */
  detected_by_name: string | null;
  resolved_by_name: string | null;
  /** Content addresses of the photographs offered in support of this finding.
   *  D140. Empty is the ordinary case: evidence is possible, never required. */
  evidence: string[];
}

/** What `POST /discrepancies/{id}/investigate` and `…/accept` answer with.
 *
 *  Both flatten the finding over `warnings` on the wire, so one interface
 *  mirrors two Rust structs — the same shape `BenchScreen` already has. */
export interface FindingActionResponse extends DiscrepancyRow {
  warnings: string[];
}

// ── weighing ──────────────────────────────────────────────────────────────
//
// `GET /revalidation` and `POST /weighings`. The act that makes an interval mean
// something: until a weight has been measured once, its age is the age of the
// import that carried it, and no schedule can be built on that.

/** One thing to put on the scale. */
export interface ToWeigh {
  item_id: Uuid | null;
  item_style_id: Uuid | null;
  code: string;
  description: string | null;
  packaging_level: string;
  /** What is held now, in grams, and how it was come by. */
  held_g: number | null;
  held_method: string | null;
  held_at: string | null;
  /** Open order lines naming it. The reason to walk there first. */
  demand: number;
  /** `never` — nobody has measured it. `overdue` — somebody did, long ago.
   *  Two lists rather than one, because an imported figure carries the date of
   *  the import and its age is unknown rather than small. */
  because: string;
}

/** What `POST /weighings` answers with. */
export interface WeighingRecorded {
  observation_id: Uuid;
  recorded_g: number;
  previous_g: number | null;
  previous_method: string | null;
  /** True when the reading and what it replaced are far enough apart to be
   *  worth somebody's attention — a tenth, with a five-gram floor. */
  disagreed: boolean;
  /** The finding it raised, when it disagreed. This screen and Findings are
   *  two ends of one act. */
  discrepancy_id: Uuid | null;
}

// ── the pick list ─────────────────────────────────────────────────────────
//
// `GET /sites/{id}/picking`. What to pick, where it is, and what it looks
// like, in the order somebody walks the floor.

/** A photograph offered for recognition, and whose it is. D141. */
export interface Picture {
  /** SHA-256. `GET /images/{digest}` serves the bytes inside the tenant scope. */
  digest: string;
  /** `own` or `style`. An inherited picture drawn unlabelled claims to be a
   *  photograph of this code when it is a photograph of another one. */
  source: string;
}

export interface PickLine {
  fulfilment_line_id: Uuid;
  item_id: Uuid;
  item_code: string;
  description: string | null;
  reference: string | null;
  /** The cell to take from. Null when nothing at this site can serve the line,
   *  which is the row a picker most needs to see. */
  stock_id: Uuid | null;
  location_code: string | null;
  pick_sequence: number | null;
  lot_code: string | null;
  /** Still to pick, in base units. */
  remaining: number;
  /** Already picked, folded. */
  picked: number;
  /** **How much of this line is spoken for**, over every covering state and
   *  every cell. The screen needs it because `POST /allocations` refuses an
   *  over-claim: to pick `q` more, the claim to make first is
   *  `max(0, picked + q - covered)`, which is often nothing. */
  covered: number;
  /** Free in the chosen cell. Below `remaining` is a short pick coming. */
  available: number | null;
  /** The cell was claimed for this line. D12 makes that advisory. */
  allocated: boolean;
  picture: Picture | null;
}

/** What `POST /evidence` answers with. D140.
 *
 *  `observation_event_id` is the whole reason the client reads this: the
 *  photograph hangs off the look, exactly as it does after a capture session. */
export interface RecordEvidenceResponse {
  evidence_id: Uuid;
  observation_event_id: Uuid;
  observable_id: Uuid;
}

/** `GET /sites/{id}/picking` */
export interface PickListScreen {
  site: string;
  lines: PickLine[];
}


/**
 * Line progress, folded from the ledger.
 *
 * Not the projection: `POST /picks` answers with the live fold and separately
 * with the cache, because D95 says a number read from a cache has to say it is
 * one. The screen patches its row from this.
 */
export interface Progress {
  covered_quantity: number;
  picked_quantity: number;
  packed_quantity: number;
  despatched_quantity: number;
  uncovered_quantity: number;
}

/** The same fold, read from the cache, with how old the cache is (D95). */
export interface ProjectionProgress extends Progress {
  /** When the fulfilment rebuild last ran for this tenant. Null = never. */
  as_at: string | null;
}

/** What `POST /picks` answers with. */
export interface RecordPickResponse {
  movement_id: Uuid;
  /** Soft problems that did not block the write — over-available, and such. */
  warnings: string[];
  /** Live progress for the line just served, folded from the ledger rather
   *  than read from the cache. This is what the row is patched from, so a walk
   *  is not renumbered under the operator between one bin and the next. */
  ledger: Progress;
  /** The cached figures as stored, which may lag until the maintainers run.
   *  Carried because D95 refuses to let a cached number pass for a live one. */
  projection: ProjectionProgress;
}

// ── receiving ─────────────────────────────────────────────────────────────
//
// `GET /sites/{id}/receiving`. What is expected here and has not all arrived —
// the read `POST /receipts` had been waiting for since migration 21.

/** A packaging level this item can be counted in, and what it converts by. */
export interface PackLevel {
  /** `each` | `inner` | `carton` | `layer` | `pallet`. */
  level: string;
  /** Base units per one of these. The screen multiplies to show a total before
   *  the press; the server does the conversion that counts. */
  units: number;
  /** `POST /receipts` needs this for any level above `each` (J57). */
  item_packing_config_id: Uuid | null;
  /** What one of these has weighed before. Null where nobody has put one on an
   *  instrument — which is most codes, most of the time, today. */
  baseline: WeightBaseline | null;
}

/** A party that already owns this item here. */
export interface Owner {
  owner_id: Uuid;
  name: string;
}

/** One promise with something still to come. */
export interface ExpectedLine {
  expected_supply_id: Uuid;
  item_id: Uuid;
  item_code: string;
  description: string | null;
  order_number: string | null;
  supplier: string | null;
  expected_from: string | null;
  expected: number;
  received: number;
  outstanding: number;
  /** **The policy refuses a line without a lot when this is true.** Asked for
   *  up front rather than discovered from a refusal at the dock. */
  requires_lot: boolean;
  /** `each` always first. */
  levels: PackLevel[];
  /** Whose the goods are, when the promise says. Often null. */
  owner_id: Uuid | null;
  /** Parties already owning this item here, offered when the promise names
   *  none. A fact rather than an invented default. */
  owners: Owner[];
  picture: Picture | null;
}

/** `GET /sites/{id}/receiving` */
export interface ReceivingScreen {
  site: string;
  lines: ExpectedLine[];
}

/** What `POST /receipts` answers with. */
export interface RecordReceiptResponse {
  goods_receipt_id: Uuid;
  goods_receipt_line_id: Uuid;
  /** True when this act opened the delivery. */
  header_created: boolean;
  /** Canonical base units written, after packaging conversion. */
  quantity: number;
  entered_quantity: number;
  entered_packaging_level: string;
  item_packing_config_id: Uuid | null;
  /** Null when the line was refused — `lot_missing` is the case. */
  movement_id: Uuid | null;
  stock_id: Uuid | null;
  repointed_allocation_ids: Uuid[];
  /** The version that governed the disposition (J63). */
  receiving_policy_id: Uuid | null;
  /** **False is a refusal, not an error.** The goods are on the dock either
   *  way and the screen has to say which happened. */
  accepted: boolean;
  /** The finding the disposition raised: `over_receipt`, `lot_missing`, … */
  discrepancy_id: Uuid | null;
  warnings: string[];
}

// ── put-away ──────────────────────────────────────────────────────────────
//
// `GET /sites/{id}/putaway`. The step between a receipt and a shelf, on a floor
// that checks goods in at the dock and puts them away afterwards.

/** A storage bin that already holds this item. Information, never a direction. */
export interface Home {
  location_id: Uuid;
  location_code: string;
  /** How much of this item is already there. */
  quantity: number;
  pick_sequence: number | null;
}

/** One thing on the dock with nowhere to live. */
export interface PutawayCell {
  /** The cell to move from, which is what `POST /moves` needs. */
  stock_id: Uuid;
  item_id: Uuid;
  item_code: string;
  description: string | null;
  /** Where it is now. A dock, by definition of this list. */
  location_code: string;
  lot_code: string | null;
  quantity: number;
  /** What is free to move. Below `quantity` means some of it is claimed, which
   *  does not stop a put-away and is worth saying before it happens. */
  available: number;
  /** Bins already holding this item, nearest on the walk first. */
  homes: Home[];
  picture: Picture | null;
}

/** `GET /sites/{id}/putaway` */
export interface PutawayScreen {
  site: string;
  cells: PutawayCell[];
}

// ── setting a deployment up ────────────────────────────────────────────────
//
// `GET /setup` and `POST /setup` (D142). A deployment with nobody in it has no
// way to sign in, so it offers this instead — once, gated by a token printed in
// the server's log.

/** `GET /setup` */
export interface SetupStatus {
  /** True when this deployment has nobody in it. */
  required: boolean;
  /** True when a usable token exists on the server. Lets the screen say
   *  "check the log" rather than ask for something that cannot exist. */
  token_ready: boolean;
}

/** `POST /setup` */
export interface SetupRequest {
  token: string;
  organisation: string;
  site_name: string;
  site_code: string;
  timezone: string;
  display_name: string;
  email: string;
  password: string;
}

/** What `POST /setup` answers with. */
export interface SetupDone {
  tenant_id: Uuid;
  site_id: Uuid;
  person_id: Uuid;
}

// ── signing in ────────────────────────────────────────────────────────────
//
// `POST /sessions`. The one call that answers before a tenant is known, because
// which tenant a person acts for is a property of the session rather than of
// the request.

/** `POST /sessions` */
export interface SignOnRequest {
  email: string;
  password: string;
  /** Required when the person belongs to more than one tenant (D19). */
  tenant_id?: Uuid;
  /** Where they are working. Every act they record names it. */
  site_id?: Uuid;
  /** D27's recording device, when the client knows it. */
  device_id?: Uuid;
  /** "Keep me signed in on this device": a week idle and thirty days in all,
   *  rather than half an hour and a shift. Absent means no. */
  remember?: boolean;
}

/** What `POST /sessions` answers with. */
export interface SignOnResponse {
  person_id: Uuid;
  display_name: string;
  tenant_id: Uuid;
  site_id: Uuid | null;
  expires_at: string;
  /** Returned once, for clients that cannot hold a cookie — D5's handhelds.
   *  A browser ignores this and uses the cookie. */
  token: string;
}

/** One of the companies a person works for, offered when they work for more
 *  than one and named none. The 409 body carries these. */
export interface TenantChoice {
  tenant_id: Uuid;
  name: string;
}

// ── who is signed in ───────────────────────────────────────────────────────

/** `GET /sessions/current`. What a screen calls to know whose session this is. */
// ── the pack queue ────────────────────────────────────────────────────────
//
// `GET /packing?q=`. The four groups are computed from quantities and never
// stored (S44): any label a column held would be a function of coverage
// numbers that can disagree with it.

/** Where a commitment has got to. Named by the server, because progress is a
 *  judgement rather than a value to format (D114). */
export type Stage = "ready" | "on_the_bench" | "packed" | "nothing_committed";

/** `GET /packing` */
export interface PackJob {
  fulfilment_id: Uuid;
  reference: string | null;
  order_reference: string | null;
  customer: string;
  lines: number;
  committed: number;
  picked: number;
  cartons: number;
  /** Server-phrased: "overdue", "due tomorrow". Null when nothing was promised. */
  due: string | null;
  stage: Stage;
}

// ── finding an order ──────────────────────────────────────────────────────
//
// `GET /orders?reference=`. Stages 1 and 2 of the recorded process in one
// request, where today they are four screens across two systems.

/** One commitment against an order, with how far the floor has got. */
export interface FulfilmentSummary {
  fulfilment_id: Uuid;
  state: string;
  site_id: Uuid | null;
  site_code: string | null;
  line_count: number;
  committed_quantity: number;
  picked_quantity: number;
  packed_quantity: number;
  despatched_quantity: number;
  /** **Computed, never stored** (S44): no table carries a rollup status, because
   *  any label it held would be a function of four coverage quantities that can
   *  disagree with it. */
  fully_picked: boolean;
  /** When the projection behind those numbers last ran; null means never (D95).
   *  A gate answered from a stale cache should say so. */
  as_at: string | null;
}

/** `GET /orders` */
export interface OrderMatch {
  order_id: Uuid;
  confirmation_number: string | null;
  external_ref: string | null;
  customer_name: string | null;
  state: string;
  placed_at: string | null;
  promised_to: string | null;
  /** D44: an externally-authoritative order is amended by cancel-and-reraise,
   *  so one reference can name a cancelled order and the one that replaced it. */
  supersedes_order_id: Uuid | null;
  fulfilments: FulfilmentSummary[];
}

/**
 * `GET /work` — what is waiting for you, at this site, now (D112).
 *
 * The landing screen and the rail's badges read the same object, so they cannot
 * disagree about one number. There is no `weigh` count: what is due for
 * weighing is decided in Rust from staleness rather than by a SQL predicate,
 * and a second definition of "stale" is worse than a missing badge.
 */
export interface WorkWaiting {
  pack: number;
  pick: number;
  despatch: number;
  findings: number;
  /** No site chosen. Everything above is zero and means nothing. */
  no_site: boolean;
}

/** `GET /sites` — where this caller could be working. */
export interface SiteRow {
  id: Uuid;
  code: string;
  name: string;
  /** True for the one this session is already working at. */
  current: boolean;
}

/** `POST /sessions/site` */
export interface ChooseSiteRequest {
  site_id: Uuid;
}

export interface CurrentSession {
  person_id: Uuid;
  display_name: string;
  tenant_id: Uuid;
  tenant_name: string;
  site_id: Uuid | null;
  site_code: string | null;
}

// ── changing your own password ─────────────────────────────────────────────
//
// `POST /credentials/password`. Under `/credentials` rather than `/sessions`
// because the thing being changed belongs to the person and outlives the
// session; the session only says whose it is.

/** `POST /credentials/password` */
export interface ChangePasswordRequest {
  current_password: string;
  new_password: string;
}

/** What `POST /credentials/password` answers with. */
export interface PasswordChanged {
  /** How many other sessions were signed out. Reported rather than assumed:
   *  somebody changing a password because they think one was stolen wants to
   *  know whether there was anything to end. */
  other_sessions_ended: number;
}

/**
 * A machine credential, as the listing shows it (D158).
 *
 * **No secret anywhere in this shape**, which is the point rather than an
 * omission: the row stores a SHA-256, so a lost token is re-minted rather than
 * recovered, and a listing that could show one would undo that.
 */
export interface ApiToken {
  readonly id: string;
  readonly label: string;
  /** Who is answerable for it existing, which is not who uses it. */
  readonly created_by_id: string;
  readonly created_at: string;
  readonly expires_at: string;
  /** Null until something has used it — which is how a token nobody wired up
   *  is told apart from one doing its job. */
  readonly last_used_at: string | null;
  readonly revoked_at: string | null;
}

export interface MintTokenRequest {
  readonly label: string;
  readonly days?: number;
}

/** The one and only answer that carries the secret. */
export interface MintedApiToken {
  readonly id: string;
  readonly label: string;
  readonly token: string;
  readonly expires_at: string;
}

/** A key that can become you. Migration 75 holds it. */
export interface Passkey {
  readonly id: string;
  readonly label: string | null;
  readonly created_at: string;
  readonly last_used_at: string | null;
  readonly sign_count: number;
  /** True when the authenticator says the key is backed up to a keychain, so
   *  losing the device does not lose the key. Null when it did not say. */
  readonly backup_state: boolean | null;
}

export interface CeremonyBegun<Options> {
  readonly ceremony_id: string;
  readonly options: Options;
}

/** The organisation this deployment belongs to. */
export interface Organisation {
  id: Uuid;
  name: string;
  /** URL-safe, fixed at setup. */
  slug: string;
  active: boolean;
  created_at: string;
}

/** A warehouse, with enough on it to tell a loaded one from an empty one. */
export interface WorkspaceSite {
  id: Uuid;
  code: string;
  name: string;
  timezone: string;
  active: boolean;
  /** Bins on file here. */
  locations: number;
  /** Of those, how many carry a walking position. */
  sequenced: number;
  current: boolean;
}

export interface Workspace {
  organisation: Organisation;
  sites: WorkspaceSite[];
}

/** One warehouse, as the file describes it (D158). */
export interface SiteSurvey {
  warehouse: string;
  bins: number;
  typed: number;
  untyped: number;
  /** The clock, or why this warehouse is being left alone. */
  note: string;
  skipped: boolean;
}

/** What the file says, before any database is consulted. */
export interface Survey {
  bins: number;
  sites: SiteSurvey[];
  sequenced: number;
  zeroed: number;
  disagree: number;
  untyped: number;
}

/** What the database did, or would have done. */
export interface Loaded {
  sites_created: number;
  sites_matched: number;
  bins_created: number;
  bins_corrected: number;
  bins_left_out: number;
  /** False when the writes were rolled back. */
  applied: boolean;
}

/**
 * The arrival, once the file is on file (D21).
 *
 * Absent on a dry run, which keeps nothing. `replay` is true when these exact
 * bytes had already arrived, and then `party_message_id` names the arrival on
 * file rather than a new one — including on a dry run, which answers the
 * question without writing.
 */
export interface FileArrival {
  party_message_id: string;
  replay: boolean;
}

export interface ImportReport {
  survey: Survey;
  loaded: Loaded;
  arrival: FileArrival | null;
}

/** The item master, as the file describes it (D158). */
export interface ItemSurvey {
  items: number;
  /** Rows whose description was blank and fell back to the code. */
  unnamed: number;
  /** Codes appearing more than once. The first wins. */
  duplicated: number;
}

export interface ItemsLoaded {
  items_created: number;
  /** Already on file, left exactly as they were. */
  items_present: number;
  applied: boolean;
}

/** One warehouse's share of an inventory balance export (D158). */
export interface WarehouseRows {
  warehouse: string;
  rows: number;
  positioned: number;
}

/** What the inventory balance export contains, before any database is asked. */
export interface StockSurvey {
  rows: number;
  /** Rows naming a shelf. The rest name a quantity at a warehouse and no more. */
  positioned: number;
  /** Rows reporting nothing on hand, kept because "empty" is a statement. */
  empty: number;
  warehouses: WarehouseRows[];
}

export interface StockLoaded {
  rows_written: number;
  /** Rows the previous load of this source left behind, now cleared. */
  rows_replaced: number;
  items_unknown: number;
  bins_unknown: number;
  warehouses_unknown: number;
  applied: boolean;
}

export interface StockImportReport {
  survey: StockSurvey;
  loaded: StockLoaded;
  arrival: FileArrival | null;
  /**
   * Set when the row count disagreed with what the export was said to have.
   * Nothing was loaded and the stored arrival is marked `partial`, because a
   * truncated inventory report reads as empty shelves rather than as missing.
   */
  refused: string | null;
}

export interface ItemImportReport {
  survey: ItemSurvey;
  loaded: ItemsLoaded;
  arrival: FileArrival | null;
}
