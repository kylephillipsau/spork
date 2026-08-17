import type { DiscrepancyRow } from "@domain/types";
import type { FindingsDesk } from "./useFindings";
import type { ViewKey } from "./views";

/**
 * The findings queue, reachable with no network.
 *
 * Every state the screen draws is reached from one of these, and the kinds are
 * real ones off `discrepancy_kind` rather than invented: `short_pick` is what
 * the seed actually raises, and `identity_mismatch` is the case that matters
 * most to cover — **a finding with no quantities at all**. Half of the twelve
 * kinds have no expected-against-observed pair, so a row that assumed one would
 * draw `— / —` on every containment conflict in the building.
 */

const now = Date.now();
const ago = (hours: number) => new Date(now - hours * 3_600_000).toISOString();

const EMPTY = {
  item_id: null,
  item_code: null,
  holder_location_id: null,
  location_code: null,
  holder_package_id: null,
  package_barcode: null,
  expected_quantity: null,
  observed_quantity: null,
  variance: null,
  detail: null,
  stock_count_id: null,
  stock_movement_id: null,
  detected_by_id: null,
  resolving_movement_id: null,
  resolved_at: null,
  resolved_by_id: null,
  resolution_reason: null,
  detected_by_name: null,
  resolved_by_name: null,
  evidence: [
    // D140. A photograph offered in support, which the rail draws beside the
    // numbers rather than behind a link.
    "ebf4f635a17d10d6eb46ba680b70142419aa3220f228001a036d311a22ee9d2a",
  ],
};

/** The seed's own finding: a pick that came up four short. */
export const SHORT_PICK: DiscrepancyRow = {
  ...EMPTY,
  id: "d15c0000-0000-0000-0000-000000000001",
  kind: "short_pick",
  state: "open",
  item_id: "17e10000-0000-0000-0000-000000000001",
  item_code: "GLOVE-M",
  location_code: "A-01-1",
  expected_quantity: "40",
  observed_quantity: "36",
  variance: "-4",
  detail: "Picked 36 against a commitment of 40",
  detected_at: ago(3),
  detected_by_name: "d.stooke",
};

const COUNT_VARIANCE: DiscrepancyRow = {
  ...EMPTY,
  id: "d15c0000-0000-0000-0000-000000000002",
  kind: "count_variance",
  state: "investigating",
  item_id: "17e10000-0000-0000-0000-000000000002",
  item_code: "STY-7720-08",
  location_code: "B-04-2",
  expected_quantity: "120",
  observed_quantity: "117",
  variance: "-3",
  detected_at: ago(52),
  detected_by_name: "k.phillips",
};

/** **No quantities.** A containment conflict is a disagreement about where a
 *  thing is, not about how much of it there is, so the pair has nothing to
 *  show and the row must not draw an empty one. */
const CONTAINMENT: DiscrepancyRow = {
  ...EMPTY,
  id: "d15c0000-0000-0000-0000-000000000003",
  kind: "containment_conflict",
  state: "open",
  holder_package_id: "9a7e0000-0000-0000-0000-0000000000b1",
  package_barcode: "CTN-0041",
  detail: "Carton scanned into A-01-1 while the ledger holds it on the dock",
  detected_at: ago(0.4),
  // Nobody: a scheduled check raised it, which the row says in as many words.
};

/** Closed, so the panel draws who and why rather than the two acts. */
const ACCEPTED: DiscrepancyRow = {
  ...EMPTY,
  id: "d15c0000-0000-0000-0000-000000000004",
  kind: "damage",
  state: "accepted",
  item_code: "GLOVE-M",
  location_code: "A-01-1",
  expected_quantity: "12",
  observed_quantity: "9",
  variance: "-3",
  detail: "Three boxes crushed in transit",
  detected_at: ago(200),
  resolved_at: ago(190),
  resolved_by_name: "k.phillips",
  resolution_reason: "Written off against the carrier claim",
  detected_by_name: "d.stooke",
};

export const FINDINGS_FIXTURE: DiscrepancyRow[] = [
  CONTAINMENT,
  SHORT_PICK,
  COUNT_VARIANCE,
  ACCEPTED,
];

/** The good state, and the one the screen must not draw as an error. */
export const FINDINGS_CLEAR: DiscrepancyRow[] = [];

const noop = async () => {};

export function fixtureDesk(over: Partial<FindingsDesk> = {}): FindingsDesk {
  return {
    status: { kind: "ready", findings: FINDINGS_FIXTURE },
    view: "live" as ViewKey,
    selected: null,
    reason: "",
    busy: false,
    problem: null,
    said: null,
    look: () => {},
    select: () => {},
    typeReason: () => {},
    dismiss: () => {},
    investigate: noop,
    attach: noop,
    accept: noop,
    ...over,
  };
}
