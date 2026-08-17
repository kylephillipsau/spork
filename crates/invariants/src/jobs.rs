//! The job-asserted invariants.
//!
//! These differ from the structural class in contract, not just in mechanism.
//! `docs/invariants.md`:
//!
//! > **Job-asserted (J)** is checked by the rebuild-and-assert cycle against
//! > real data. A failure raises a `discrepancy`, **never an error**, so the
//! > model's self-consistency lands in the same queue as every other finding and
//! > never stops the floor.
//!
//! So this module produces [`Finding`] values and returns them. It does not
//! panic, it does not abort, and it has no opinion about what the caller does
//! next. In CI a finding fails the build. In production it becomes a
//! `discrepancy` row with an owner and a timestamp, which is D8's whole thesis
//! applied to the model's own consistency: the system's disagreement with itself
//! is a finding like any other, and it is more useful in a queue than in a stack
//! trace.
//!
//! That is also why a J check never rejects. If `stock` disagrees with its
//! ledger, the ledger is right and the projection is stale or wrong. Refusing to
//! serve the floor until someone fixes it would be the exact trade D5 exists to
//! refuse.

use crate::Blocker;
use postgres::Client;
use std::fmt;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[allow(clippy::upper_case_acronyms)]
pub enum Id {
    J1, J2, J3, J4, J5, J6, J7, J8, J9, J10,
    J11, J12, J13, J14, J15, J16, J17, J18, J19, J20,
    J21, J22, J23, J24, J25, J26, J27, J28, J29, J30,
    J31, J32, J33, J34, J35, J36, J37, J38, J39, J40,
    J41, J42, J43, J44, J45, J46, J47, J48, J49, J50,
    J51, J52, J53, J54, J55, J56, J57, J58, J59, J60, J61, J62, J63, J64, J65, J66,
    J67, J68, J69, J70, J71, J72, J73,
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub const ALL: &[Id] = &[
    Id::J1, Id::J2, Id::J3, Id::J4, Id::J5, Id::J6, Id::J7, Id::J8, Id::J9, Id::J10,
    Id::J11, Id::J12, Id::J13, Id::J14, Id::J15, Id::J16, Id::J17, Id::J18, Id::J19, Id::J20,
    Id::J21, Id::J22, Id::J23, Id::J24, Id::J25, Id::J26, Id::J27, Id::J28, Id::J29, Id::J30,
    Id::J31, Id::J32, Id::J33, Id::J34, Id::J35, Id::J36, Id::J37, Id::J38, Id::J39, Id::J40,
    Id::J41, Id::J42, Id::J43, Id::J44, Id::J45, Id::J46, Id::J47, Id::J48, Id::J49,
    Id::J50, Id::J51, Id::J52, Id::J53, Id::J54, Id::J55, Id::J56, Id::J57,
    Id::J58, Id::J59, Id::J60, Id::J61, Id::J62, Id::J63, Id::J64, Id::J65, Id::J66,
    Id::J67, Id::J68, Id::J69, Id::J70, Id::J71, Id::J72, Id::J73,
];

/// What a failing job-asserted invariant produces.
///
/// Shaped like the `discrepancy` row it becomes rather than like an assertion
/// message, because that is where it is going. `kind` is the discrepancy kind
/// the register names where it names one.
pub struct Finding {
    pub invariant: Id,
    pub kind: &'static str,
    pub detail: String,
}

pub struct Invariant {
    pub id: Id,
    pub statement: &'static str,
    pub owners: &'static str,
    /// The register's vacuity column: does this assert an absence?
    pub asserts_absence: bool,
    pub check: Check,
}

/// What a job-asserted check returns: how many objects it examined, and what it
/// found. The count is the half that decides `PASS` from `VACUOUS` — the register
/// takes the verdict from how much a check looked at rather than from how its
/// entry is phrased.
pub type CheckResult = Result<(usize, Vec<Finding>), postgres::Error>;

pub enum Check {
    /// Structured, so S42 can tell whether the waiting is still true. See
    /// [`crate::Blocker`] for why this stopped being a sentence.
    Pending(&'static [Blocker]),
    Run(fn(&mut Client) -> CheckResult),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Vacuous,
    Findings,
    Pending,
}

pub fn spec(id: Id) -> Invariant {
    use Id::*;
    match id {
        J1 => Invariant { id, owners: "D5, D12, D24", asserts_absence: false,
            statement: "stock.quantity = the signed two-sided fold of stock_movement over the cell key",
            check: Check::Run(checks::j1_stock_quantity_is_the_fold) },
        J2 => Invariant { id, owners: "D20", asserts_absence: false,
            statement: "stock.weight_g = the same fold over catch_weight_g",
            check: Check::Run(checks::j2_stock_weight_is_the_fold) },
        J3 => Invariant { id, owners: "D12 narrowed, Q91", asserts_absence: false,
            statement: "stock.allocated_quantity = active cell-bound allocations only; never a reference test",
            check: Check::Run(checks::j3_allocated_quantity_is_the_fold) },
        J4 => Invariant { id, owners: "D24", asserts_absence: false,
            statement: "expected_supply.quantity_allocated = active allocations against it. J3's fold on the supply side of the same allocation",
            check: Check::Run(checks::j4_supply_allocation_is_the_fold) },
        J5 => Invariant { id, owners: "D24", asserts_absence: false,
            statement: "stock.resolved_location_id = holder location, or the holder package's",
            check: Check::Run(checks::j5_resolved_location_follows_the_holder) },
        J6 => Invariant { id, owners: "D24, Q90", asserts_absence: false,
            statement: "package placement, identity and depth = the fold of package_event in (occurred_at, recorded_at, id) order; replay in any arrival order is identical",
            check: Check::Run(checks::j6_package_projection_is_the_fold) },
        J7 => Invariant { id, owners: "D24", asserts_absence: true,
            statement: "stock.quantity > 0 implies resolved_location_id IS NOT NULL",
            check: Check::Run(checks::j7_positive_stock_has_a_location) },
        J8 => Invariant { id, owners: "D24", asserts_absence: false,
            statement: "expected_supply.quantity_refined = the sum of refining rows, the double-count guard",
            check: Check::Pending(&[Blocker::Absent("expected_supply.refines_expected_supply_id")]) },
        J9 => Invariant { id, owners: "D24", asserts_absence: true,
            statement: "No active allocation references a closed expected_supply. A finding rather than a release: a counterparty's retraction must not silently un-promise a customer order",
            check: Check::Run(checks::j9_no_allocation_against_closed_supply) },
        J10 => Invariant { id, owners: "D23", asserts_absence: false,
            statement: "observation_current is exactly rebuildable from observation plus the recorded precedence policy",
            check: Check::Run(checks::j10_current_is_exactly_the_rebuild) },
        J11 => Invariant { id, owners: "D23, D25", asserts_absence: true,
            statement: "A counterparty-asserted observation enters observation_current only with an acceptance",
            check: Check::Run(checks::j11_their_number_needs_our_acceptance) },
        J12 => Invariant { id, owners: "D23", asserts_absence: false,
            statement: "package dimensions equal observation_current for unsealed packages only",
            check: Check::Run(checks::j12_unsealed_package_dimensions_agree) },
        J13 => Invariant { id, owners: "D22", asserts_absence: false,
            statement: "Every tenant policy value version has a policy_change at the moment it came into force, anti-joined both ways. Platform-shipped versions are exempt and that exemption is question 138",
            check: Check::Run(checks::j13_every_value_version_has_its_change) },
        J14 => Invariant { id, owners: "D22, D18", asserts_absence: true,
            statement: "Every scope FK resolves within the binding's tenant, and a platform-shipped binding references only shared rows",
            check: Check::Run(checks::j14_a_binding_stays_inside_its_tenant) },
        J15 => Invariant { id, owners: "D22", asserts_absence: true,
            statement: "No binding names a node inconsistent with a coarser node on the same dimension",
            check: Check::Run(checks::j15_scope_axes_do_not_contradict) },
        J16 => Invariant { id, owners: "D22", asserts_absence: true,
            statement: "Zero discrepancy rows of kind policy_ambiguous after the suite",
            check: Check::Run(checks::j16_no_ambiguous_resolution) },
        J17 => Invariant { id, owners: "D21", asserts_absence: true,
            statement: "assertion.supersedes is acyclic; at most one in-force assertion per (tenant, author, kind, reference)",
            check: Check::Run(checks::j17_supersession_is_acyclic_and_singular) },
        J18 => Invariant { id, owners: "D21, D23", asserts_absence: false,
            statement: "A counterparty observation's promoted value still equals its assertion's",
            check: Check::Run(checks::j18_a_promoted_value_is_still_theirs) },
        J19 => Invariant { id, owners: "D21 rule 3, Q90, D24 supply side", asserts_absence: false,
            statement: "Truncate every assertion table, rebuild stock, assert byte-identical, and assert no stock key column is reachable from an assertion table",
            check: Check::Run(checks::j19_stock_does_not_depend_on_a_claim) },
        J20 => Invariant { id, owners: "D18", asserts_absence: true,
            statement: "Every tenant_id on a fact agrees with its subject's and its event's",
            check: Check::Run(checks::j20_tenant_agreement_on_facts) },
        J21 => Invariant { id, owners: "D26, D36", asserts_absence: true,
            statement: "No tenant holds more claimed extension slots than issued",
            check: Check::Run(checks::j21_no_tenant_holds_more_than_it_was_issued) },
        J22 => Invariant { id, owners: "D22", asserts_absence: false,
            statement: "Golden snapshot: adding a scope dimension changes no existing resolution without a RESOLVER_VERSION bump",
            check: Check::Run(checks::j22_no_resolution_changed_without_a_version_bump) },
        J23 => Invariant { id, owners: "D23", asserts_absence: false,
            statement: "The five ingestion channels produce identical content columns",
            check: Check::Pending(&[Blocker::Elsewhere("the ingestion adapters")]) },
        J24 => Invariant { id, owners: "D25", asserts_absence: false,
            statement: "Projection cascades are at most two hops from the originating fact",
            check: Check::Pending(&[Blocker::Absent("projection_rebuild.cascade_of")]) },
        J25 => Invariant { id, owners: "D24 supply side", asserts_absence: true,
            statement: "refines_expected_supply_id is acyclic and of depth exactly 1",
            check: Check::Pending(&[Blocker::Absent("expected_supply.refines_expected_supply_id")]) },
        J26 => Invariant { id, owners: "D24 supply side, D45", asserts_absence: false,
            statement: "expected_supply.quantity_received = the fold of stock_movement rows whose goods_receipt_line_id names this supply row or any row refining it",
            check: Check::Run(checks::j26_received_is_the_ledger_fold) },
        J27 => Invariant { id, owners: "D24 supply side", asserts_absence: true,
            statement: "Transfer arm: no unit is simultaneously counted in origin available and destination promisable",
            check: Check::Pending(&[Blocker::Absent("expected_supply.transfer_order_line_id")]) },
        J28 => Invariant { id, owners: "D24 supply side", asserts_absence: true,
            statement: "No open row with quantity_outstanding > 0 past expected_to + grace",
            check: Check::Pending(&[Blocker::Elsewhere("the supply-side job that applies a resolved receiving policy to a promise -- D82 built the resolution, not the caller")]) },
        J29 => Invariant { id, owners: "D24 supply side", asserts_absence: true,
            statement: "No active allocation references a closed or overdue row; the allocation is not released by the job",
            check: Check::Pending(&[Blocker::Elsewhere("the supply-side job that applies a resolved receiving policy to a promise -- D82 built the resolution, not the caller")]) },
        J30 => Invariant { id, owners: "D24 supply side", asserts_absence: false,
            statement: "Rebuilding expected_supply preserves row identity. Truncate-and-regenerate is forbidden while any allocation holds an expected_supply_id",
            check: Check::Run(checks::j30_rebuild_preserves_identity) },
        J31 => Invariant { id, owners: "D24 supply side, D53, D99", asserts_absence: false,
            statement: "fulfilment_line.covered_quantity equals the fold of stock_allocation over that line, on the covering state set. Widened by D53 to four quantities and narrowed by D99 back to one: coverage is a question about intentions and an allocation is exactly that, while the three progress quantities assert what physically happened and moved to J68",
            check: Check::Run(checks::j31_line_coverage_is_the_fold) },
        J32 => Invariant { id, owners: "D24, D25, Q91, D35", asserts_absence: false,
            statement: "The reap predicate is the complement of the rebuild's existence predicate: reap, rebuild, assert produces zero drift",
            check: Check::Pending(&[Blocker::Elsewhere("the reaper phase of the rebuild")]) },
        J33 => Invariant { id, owners: "D24, Q91", asserts_absence: false,
            statement: "Rebuilding stock preserves row identity: every live cell resolves to the same id before and after",
            check: Check::Run(checks::j33_rebuild_preserves_stock_identity) },
        J34 => Invariant { id, owners: "D21, D24, Q90", asserts_absence: true,
            statement: "No package row's earliest package_event has source = 'asn'. Minting from an assertion is structurally absent",
            check: Check::Run(checks::j34_no_package_minted_from_an_assertion) },
        J35 => Invariant { id, owners: "D21, Q90", asserts_absence: true,
            statement: "Every sscc_allocation serial lies within its range's issued span, and no serial is reissued within the reuse window",
            check: Check::Pending(&[Blocker::Absent("number_range"), Blocker::Absent("sscc_allocation")]) },
        J36 => Invariant { id, owners: "D35", asserts_absence: true,
            statement: "No role other than the projection maintainer may UPDATE any column commented @projection, or DELETE from the table carrying one",
            check: Check::Run(checks::j36_no_login_role_writes_a_projection) },
        J37 => Invariant { id, owners: "D35", asserts_absence: true,
            statement: "Every SECURITY DEFINER function in the maintainer set has search_path in proconfig and no EXECUTE to PUBLIC",
            check: Check::Run(checks::j37_definer_functions_are_pinned_and_private) },
        J38 => Invariant { id, owners: "D35", asserts_absence: false,
            statement: "relforcerowsecurity is true on every table carrying an @projection column",
            check: Check::Run(checks::j38_projection_tables_force_rls) },
        J73 => Invariant { id, owners: "D19", asserts_absence: false,
            statement: "Every table with a tenant_id column that nylonite_app can reach forces row \
                    level security. Tables it holds no privilege on are closed by grant \
                    instead, and the exemption is asked of Postgres rather than named, so \
                    granting the application a column re-arms the check",
            check: Check::Run(checks::j73_tenant_tables_force_rls) },
        J39 => Invariant { id, owners: "D36", asserts_absence: false,
            statement: "Every record_scheme row has a claiming extension_slot, and no slot is claimed by two keys",
            check: Check::Run(checks::j39_a_scheme_holds_the_slot_it_names) },
        J40 => Invariant { id, owners: "D36", asserts_absence: true,
            statement: "No record_scheme whose slot is released has an unarchived materialised table",
            check: Check::Pending(&[Blocker::Absent("retention_floor")]) },
        J41 => Invariant { id, owners: "D37", asserts_absence: false,
            statement: "expected_supply.inbound_shipment_id equals the walk through the assertion chain, and is NULL for every other arm",
            check: Check::Pending(&[Blocker::Absent("expected_supply.inbound_shipment_id")]) },
        J42 => Invariant { id, owners: "D34", asserts_absence: true,
            statement: "Every GTIN in item_barcode is 14 characters and passes the mod-10 check digit",
            check: Check::Run(checks::j42_every_gtin_is_fourteen_and_checks_out) },
        J43 => Invariant { id, owners: "D34", asserts_absence: false,
            statement: "The item_barcode exclusion constraint's COALESCE sentinels are present",
            check: Check::Run(checks::j43_the_barcode_sentinels_are_present) },
        J44 => Invariant { id, owners: "D39, narrowed by D48, corrected by D54", asserts_absence: true,
            statement: "No externally-authoritative order carries a world_event amendment made by a person here. A record_error is permitted, and so is a world_event arriving through the channel: what D39 forbids is a place, not a class of change",
            check: Check::Run(checks::j44_an_external_order_is_not_amended_here) },
        J45 => Invariant { id, owners: "D41", asserts_absence: true,
            statement: "No query in the register reads across tenants except those on a declared exception list carrying a cohort floor",
            check: Check::Pending(&[Blocker::Elsewhere("the query register")]) },
        J46 => Invariant { id, owners: "D42, widened by D51", asserts_absence: false,
            statement: "Every covered order and order_line column an amendment sets equals the last writer over that subject in (occurred_at, recorded_at, id) order; replay in any arrival order is identical",
            check: Check::Run(checks::j46_the_amendment_fold_holds) },
        J47 => Invariant { id, owners: "D43, built by D96", asserts_absence: true,
            statement: "No physical asserted_unit subtree, and no package, resolves to content lines naming more than one purchase order. Scoped to physical roots, because a document node above two order nodes legitimately spans both",
            check: Check::Run(checks::j47_a_pallet_belongs_to_one_purchase_order) },
        J48 => Invariant { id, owners: "D44", asserts_absence: true,
            statement: "Every outbound party_message on a channel requiring acknowledgement has one, or a finding naming it",
            check: Check::Pending(&[Blocker::Absent("party_profile")]) },
        J49 => Invariant { id, owners: "D44", asserts_absence: true,
            statement: "No document_response assertion names a subject assertion of the same direction",
            check: Check::Run(checks::j49_a_disposition_answers_the_other_direction) },
        J50 => Invariant { id, owners: "D8, D47", asserts_absence: true,
            statement: "Every correction carries its target's occurred_at, mirrors its target's sides, names the same item and the same tenant, and gives a record_error reason",
            check: Check::Run(checks::j50_corrections_are_about_their_target) },
        J51 => Invariant { id, owners: "D47, widened by D103", asserts_absence: true,
            statement: "Corrections against a movement never exceed it in total quantity, measured on the chain: what is left of a movement after its whole correction subtree is never negative",
            check: Check::Run(checks::j51_corrections_do_not_overreach) },
        J52 => Invariant { id, owners: "D47, J17 shape", asserts_absence: true,
            statement: "reverses_movement_id is acyclic",
            check: Check::Run(checks::j52_correction_chains_do_not_loop) },
        J53 => Invariant { id, owners: "D48", asserts_absence: true,
            statement: "Every record_error amendment carries an occurred_at matching its order's placed_at or another amendment's occurred_at on the same order",
            check: Check::Run(checks::j53_corrections_name_a_moment_that_exists) },
        J54 => Invariant { id, owners: "D50, widened by D59", asserts_absence: true,
            statement: "No priced line on either order kind belongs to a header that names no currency. One rule over two tables, because a purchase order is an order we place",
            check: Check::Run(checks::j54_a_price_names_its_currency) },
        J55 => Invariant { id, owners: "D51", asserts_absence: true,
            statement: "No fulfilment that is not cancelled commits to a removed order line",
            check: Check::Run(checks::j55_no_commitment_against_a_removed_line) },
        J56 => Invariant { id, owners: "D53, load-bearing since D100", asserts_absence: true,
            statement: "No fulfilment line is covered beyond its own quantity, and despatched <= packed <= picked <= covered. The monotonicity held by construction while one fold produced all four from nested state sets; D100 folds three from the ledger and one from allocations, so a pick with no allocation or an over-pick breaks it, and this is what reports that",
            check: Check::Run(checks::j56_coverage_is_bounded_and_monotone) },
        J57 => Invariant { id, owners: "D23, D58, widened by D92 and D93", asserts_absence: true,
            statement: "Every row naming an item_packing_config -- stock_movement, goods_receipt_line and asserted_unit_content -- names one for its own item that was already effective when it happened, each against its own clock",
            check: Check::Run(checks::j57_a_conversion_names_a_config_that_applied) },
        J58 => Invariant { id, owners: "D24 supply side, D61", asserts_absence: true,
            statement: "No expected_supply row carries claims it can no longer satisfy: quantity_promisable is never negative. A fully received promise still holding allocations means the re-point to stock did not happen",
            check: Check::Run(checks::j58_promisable_is_never_negative) },
        J59 => Invariant { id, owners: "D22, D63", asserts_absence: true,
            statement: "item_class and party_class parentage is acyclic, and the closure covers every node. A cycle is insertable today and makes the subtree below it unreachable to every scoped policy",
            check: Check::Run(checks::j59_taxonomies_are_acyclic) },
        J60 => Invariant { id, owners: "D22, D73", asserts_absence: true,
            statement: "Every recorded re-parent carries the blast radius its approver was shown. The number cannot be recovered later -- it was measured against a taxonomy shape the move itself destroyed -- so a row without one is a decision nobody can audit",
            check: Check::Run(checks::j60_a_move_records_what_it_was_measured_to_cost) },
        J61 => Invariant { id, owners: "D74", asserts_absence: true,
            statement: "A retired class is not still being built on: no unretired child hangs under a retired parent, and no binding is created against a class already retired. Retirement does not change resolution, so neither of these breaks anything today -- they are the taxonomy being edited as though the retirement did not happen",
            check: Check::Run(checks::j61_a_retired_class_is_not_still_in_use) },
        J62 => Invariant { id, owners: "D22, D76", asserts_absence: true,
            statement: "Every class records where it started. Without a creation act the fold has no base, so the class's parentage before its first move is unrecoverable -- measured on the fixture before D76, where a class created at the root and moved once could not be placed at any instant before the move",
            check: Check::Run(checks::j62_a_class_records_where_it_started) },
        J63 => Invariant { id, owners: "D22, D76", asserts_absence: true,
            statement: "Every act a policy governed names the version that governed it, so an audit reads the decision rather than reconstructing it. The pattern is already here -- a movement names the packing config that converted it -- and the missing half is policy: nothing records which receiving_policy accepted a lot. Blocks on the resolver, and wakes the day the column exists",
            check: Check::Run(checks::j63_a_governed_act_names_its_version) },

        J64 => Invariant { id, owners: "D55, D79", asserts_absence: true,
            statement: "Every reference to a shared-reference row resolves to the referencing row's own tenant or to a platform row. S51 makes the strict half a key; this half cannot be one, because a platform row has no tenant to match, so it is checked as data across every such foreign key in the catalogue",
            check: Check::Run(checks::j64_a_shared_reference_is_ours_or_the_platform_s) },
        J65 => Invariant { id, owners: "Principle 5, D92", asserts_absence: true,
            statement: "Every stored canonical quantity equals its preserved entered quantity converted by the factor the row names. Principle 5 has the writer convert and the database store the result, which means nothing in the database has ever checked the arithmetic",
            check: Check::Run(checks::j65_the_canonical_agrees_with_what_was_entered) },
        J66 => Invariant { id, owners: "D25, D95", asserts_absence: true,
            statement: "No projection is older than the freshness bound its step declares, and no tenant is missing a run for a step entirely. A finding rather than a block, because a stale cache never stops the floor -- the ledger is still the truth",
            check: Check::Run(checks::j66_no_projection_is_staler_than_it_declared) },
        J67 => Invariant { id, owners: "D24, D96", asserts_absence: true,
            statement: "Every collapsed asserted_unit names a package whose SSCC agrees with the one declared, where both carry one. The function refuses a mismatch at the write; package.sscc is a fold of package_event and can move afterwards, which is what makes this a job",
            check: Check::Run(checks::j67_a_collapse_is_evidenced_by_its_licence_plate) },
        J68 => Invariant { id, owners: "D53, D99, D100", asserts_absence: false,
            statement: "fulfilment_line's picked, packed and despatched quantities each equal the fold of stock_movement over that line, on D99's shape rules: picked left a storage location, packed went into a carton whose winning status is sealed or despatched, despatched has no to side at all. A reversed movement and its reversal both leave the fold. It reported this gap before D100 moved the maintainer and asserts the agreement after, without the query changing",
            check: Check::Run(checks::j68_outbound_progress_against_the_ledger) },
        J69 => Invariant { id, owners: "D12, D99, D100", asserts_absence: true,
            statement: "No stock_movement naming a fulfilment line moves an item other than the one that line commits, reached through its order_line. A finding rather than a CHECK: it spans rows, and D12 makes a substituted pick a fact to record rather than a write to refuse",
            check: Check::Run(checks::j69_a_movement_serves_its_line_s_item) },
        J70 => Invariant { id, owners: "D99, D100", asserts_absence: true,
            statement: "No movement naming a fulfilment line leaves a package that never received stock for that same line. That is a pick out of package-held storage, which picked_quantity does not count, so the limit D99 named is reported rather than silently low",
            check: Check::Run(checks::j70_no_pick_leaves_package_held_storage_uncounted) },
        J71 => Invariant { id, owners: "D46", asserts_absence: true,
            statement: "No two active bins in one site claim the same pick_sequence. A finding rather than a unique index: two bins at one position is a disagreement about the floor, and refusing the write would refuse true data — the real bin list has two such pairs — while a unique constraint would also make reordering need a spare value nobody wants to store",
            check: Check::Run(checks::j71_one_bin_per_pick_position) },
        J72 => Invariant { id, owners: "D138", asserts_absence: true,
            statement: "Every length, width or height we recorded against a single thing names the presentation it was measured in. The writer refuses one without it, and a rule enforced only in the writer is a rule the second writer breaks — a loader, an import, a repair script. Scoped to observations that are ours: a counterparty's claim about an each is theirs to qualify and we cannot make them",
            check: Check::Run(checks::j72_a_single_things_size_names_its_arrangement) },
    }
}

pub fn run(client: &mut Client, id: Id) -> Result<(Verdict, usize, Vec<Finding>), postgres::Error> {
    match spec(id).check {
        Check::Pending(_) => Ok((Verdict::Pending, 0, vec![])),
        Check::Run(f) => {
            let (examined, findings) = f(client)?;
            let verdict = if !findings.is_empty() {
                Verdict::Findings
            } else if examined == 0 {
                Verdict::Vacuous
            } else {
                Verdict::Pass
            };
            Ok((verdict, examined, findings))
        }
    }
}

pub mod checks {
    use super::{Finding, Id};

    const APP_SCHEMA: &str = "public";
    use postgres::Client;

    fn finding(invariant: Id, kind: &'static str, detail: String) -> Finding {
        Finding { invariant, kind, detail }
    }

    /// The fold, recomputed independently of the maintainer.
    ///
    /// Written as its own SQL rather than by calling the rebuild function,
    /// because a check that asks the maintainer whether the maintainer is right
    /// is not a check.
    const LEDGER_FOLD: &str = "
        WITH ledger AS (
            SELECT tenant_id, item_id, to_location_id AS hl, to_package_id AS hp,
                   to_lot_id AS lot, to_status_id AS st, to_owner_id AS own,
                   quantity AS qty, catch_weight_g AS wt
              FROM stock_movement
             WHERE num_nonnulls(to_location_id, to_package_id) = 1
            UNION ALL
            SELECT tenant_id, item_id, from_location_id, from_package_id,
                   from_lot_id, from_status_id, from_owner_id,
                   -quantity, -catch_weight_g
              FROM stock_movement
             WHERE num_nonnulls(from_location_id, from_package_id) = 1
        )
        SELECT tenant_id, item_id, hl, hp, lot, st, own,
               sum(qty)::bigint AS quantity, sum(wt)::bigint AS weight_g
          FROM ledger GROUP BY 1,2,3,4,5,6,7";

    pub fn j1_stock_quantity_is_the_fold(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let sql = format!(
            "WITH fold AS ({LEDGER_FOLD})
             SELECT coalesce(s.id::text, 'missing'), coalesce(s.quantity, 0), f.quantity
               FROM fold f
               FULL OUTER JOIN stock s
                 ON s.tenant_id = f.tenant_id AND s.item_id = f.item_id
                AND s.holder_location_id IS NOT DISTINCT FROM f.hl
                AND s.holder_package_id IS NOT DISTINCT FROM f.hp
                AND s.lot_id IS NOT DISTINCT FROM f.lot
                AND s.status_id IS NOT DISTINCT FROM f.st
                AND s.owner_id IS NOT DISTINCT FROM f.own
              WHERE coalesce(s.quantity, 0) <> coalesce(f.quantity, 0)"
        );
        let rows = c.query(sql.as_str(), &[])?;
        let total = c.query_one("SELECT count(*) FROM stock", &[])?;
        let findings = rows
            .iter()
            .map(|r| {
                finding(
                    Id::J1,
                    "projection_drift",
                    format!(
                        "cell {} holds {} and the ledger folds to {}",
                        r.get::<_, String>(0),
                        r.get::<_, i64>(1),
                        r.get::<_, Option<i64>>(2).unwrap_or(0)
                    ),
                )
            })
            .collect();
        Ok((total.get::<_, i64>(0) as usize, findings))
    }

    pub fn j2_stock_weight_is_the_fold(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let sql = format!(
            "WITH fold AS ({LEDGER_FOLD})
             SELECT s.id::text, s.weight_g, f.weight_g
               FROM fold f JOIN stock s
                 ON s.tenant_id = f.tenant_id AND s.item_id = f.item_id
                AND s.holder_location_id IS NOT DISTINCT FROM f.hl
                AND s.holder_package_id IS NOT DISTINCT FROM f.hp
                AND s.lot_id IS NOT DISTINCT FROM f.lot
                AND s.status_id IS NOT DISTINCT FROM f.st
                AND s.owner_id IS NOT DISTINCT FROM f.own
              WHERE s.weight_g IS DISTINCT FROM f.weight_g"
        );
        let rows = c.query(sql.as_str(), &[])?;
        let total = c.query_one(
            "SELECT count(*) FROM stock WHERE weight_g IS NOT NULL",
            &[],
        )?;
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J2, "projection_drift",
                    format!("cell {} weight disagrees with the fold", r.get::<_, String>(0)))
            })
            .collect();
        Ok((total.get::<_, i64>(0) as usize, findings))
    }

    /// J3 carries a warning in the register worth repeating: it is a quantity
    /// fold and must never be used as a reference test, because terminal
    /// allocations still hold a stock_id and contribute nothing to it.
    pub fn j3_allocated_quantity_is_the_fold(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "SELECT s.id::text, s.allocated_quantity, coalesce(a.q, 0)
               FROM stock s
               LEFT JOIN (SELECT stock_id, sum(quantity)::bigint AS q
                            FROM stock_allocation
                           WHERE state IN ('allocated','picking','picked','packed')
                           GROUP BY stock_id) a ON a.stock_id = s.id
              WHERE s.allocated_quantity <> coalesce(a.q, 0)",
            &[],
        )?;
        let total = c.query_one("SELECT count(*) FROM stock", &[])?;
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J3, "projection_drift",
                    format!("cell {} holds allocated {} against active allocations of {}",
                        r.get::<_, String>(0), r.get::<_, i64>(1), r.get::<_, i64>(2)))
            })
            .collect();
        Ok((total.get::<_, i64>(0) as usize, findings))
    }

    pub fn j5_resolved_location_follows_the_holder(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "SELECT s.id::text
               FROM stock s LEFT JOIN package p ON p.id = s.holder_package_id
              WHERE s.resolved_location_id IS DISTINCT FROM
                    coalesce(s.holder_location_id, p.resolved_location_id)",
            &[],
        )?;
        let total = c.query_one("SELECT count(*) FROM stock", &[])?;
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J5, "projection_drift",
                    format!("cell {} resolves to a location its holder does not",
                        r.get::<_, String>(0)))
            })
            .collect();
        Ok((total.get::<_, i64>(0) as usize, findings))
    }

    /// J6, read-only. Recomputes the winning placement from the log and compares
    /// it against the projection. The replay-order property is a separate test,
    /// because demonstrating it requires perturbing arrival order and that is a
    /// mutation rather than an assertion.
    pub fn j6_package_projection_is_the_fold(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "WITH winner AS (
                 SELECT DISTINCT ON (package_id)
                        package_id, parent_package_id, location_id
                   FROM package_event
                  WHERE asserts_placement
                  ORDER BY package_id, occurred_at DESC, recorded_at DESC, id DESC)
             SELECT p.id::text, p.barcode
               FROM package p JOIN winner w ON w.package_id = p.id
              WHERE p.parent_package_id IS DISTINCT FROM w.parent_package_id
                 OR p.location_id IS DISTINCT FROM w.location_id",
            &[],
        )?;
        let total = c.query_one(
            "SELECT count(DISTINCT package_id) FROM package_event WHERE asserts_placement",
            &[],
        )?;
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J6, "containment_conflict",
                    format!("package {} disagrees with the winning placement in its log",
                        r.get::<_, Option<String>>(1).unwrap_or_else(|| r.get::<_, String>(0))))
            })
            .collect();
        Ok((total.get::<_, i64>(0) as usize, findings))
    }

    pub fn j7_positive_stock_has_a_location(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "SELECT id::text, quantity FROM stock
              WHERE quantity > 0 AND resolved_location_id IS NULL",
            &[],
        )?;
        let total = c.query_one("SELECT count(*) FROM stock WHERE quantity > 0", &[])?;
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J7, "stock_without_location",
                    format!("cell {} holds {} and resolves nowhere",
                        r.get::<_, String>(0), r.get::<_, i64>(1)))
            })
            .collect();
        Ok((total.get::<_, i64>(0) as usize, findings))
    }

    pub fn j20_tenant_agreement_on_facts(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let mut findings = vec![];
        let mut examined = 0usize;

        for (table, subject_join) in [
            ("stock_movement", "JOIN item i ON i.id = f.item_id
              WHERE i.tenant_id IS NOT NULL AND i.tenant_id <> f.tenant_id"),
            ("package_event", "JOIN package p ON p.id = f.package_id
              WHERE p.tenant_id <> f.tenant_id"),
        ] {
            let n = c.query_one(&format!("SELECT count(*) FROM {table}"), &[])?;
            examined += n.get::<_, i64>(0) as usize;
            let bad = c.query(
                &format!("SELECT f.id::text FROM {table} f {subject_join}"),
                &[],
            )?;
            for r in &bad {
                findings.push(finding(Id::J20, "tenant_mismatch",
                    format!("{table} {} disagrees with its subject's tenant", r.get::<_, String>(0))));
            }
        }
        Ok((examined, findings))
    }

    /// J33. Truncate-and-regenerate is forbidden while any allocation holds a
    /// stock_id, so a rebuild must preserve identity. This checks the property
    /// that makes that true rather than the rebuild itself: every live cell is
    /// reachable by its key, so an upsert finds it instead of inserting a twin.
    pub fn j33_rebuild_preserves_stock_identity(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "SELECT tenant_id::text, item_id::text, count(*)
               FROM stock
              GROUP BY tenant_id, item_id, holder_location_id, holder_package_id,
                       lot_id, status_id, owner_id
             HAVING count(*) > 1",
            &[],
        )?;
        let total = c.query_one("SELECT count(*) FROM stock", &[])?;
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J33, "projection_drift",
                    format!("tenant {} item {} has {} rows for one cell key: a rebuild would not find them",
                        r.get::<_, String>(0), r.get::<_, String>(1), r.get::<_, i64>(2)))
            })
            .collect();
        Ok((total.get::<_, i64>(0) as usize, findings))
    }

    /// J34. D29's structural guarantee: a package is minted when something *we
    /// observe* identifies it. A counterparty's claim lives in asserted_unit and
    /// becomes a package only when someone scans it or we build it.
    pub fn j34_no_package_minted_from_an_assertion(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "SELECT DISTINCT ON (package_id) package_id::text, source
               FROM package_event
              ORDER BY package_id, occurred_at, recorded_at, id",
            &[],
        )?;
        let findings = rows
            .iter()
            .filter(|r| r.get::<_, String>(1) == "asn")
            .map(|r| {
                finding(Id::J34, "minted_from_assertion",
                    format!("package {} was first identified by an ASN, not by an observation",
                        r.get::<_, String>(0)))
            })
            .collect();
        Ok((rows.len(), findings))
    }

    pub fn j36_no_login_role_writes_a_projection(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        // Rewritten twice, and the second time because the first rewrite still
        // could not fire.
        //
        // The original read information_schema.role_table_grants. That view is
        // wrong here for two independent reasons. It restricts to grants the
        // *current user* is grantor, grantee or a member of, so a superuser
        // running the suite sees none of the app role's grants. And it carries
        // table-level grants only, so `GRANT UPDATE (quantity) ON stock`, the
        // exact shape D25 prescribes everywhere else, never appears in it. What
        // the check actually asserted was "no table-level grant on a table that
        // happens to contain a projection column", which is why it caught the
        // package case and nothing since.
        //
        // has_column_privilege resolves the whole chain: table-level grants,
        // column-level grants, role membership and PUBLIC, from the catalog
        // rather than from a view with a visibility predicate.
        //
        // INSERT is deliberately not checked, and that is a narrowing rather
        // than an oversight. `projection_order_rebuild` coalesces the fold onto
        // the row's existing value, so an order's original promised window is
        // the base of the fold rather than an amendment, and the application has
        // to be able to write it once. UPDATE and DELETE are the verbs that
        // overwrite what the maintainer owns. A wholly maintained table such as
        // `stock` is protected more simply, by granting the application nothing
        // on it at all.
        let rows = c.query(
            "SELECT r.rolname, c.relname, a.attname, p.priv
               FROM pg_class c
               JOIN pg_namespace n ON n.oid = c.relnamespace
               JOIN pg_attribute a ON a.attrelid = c.oid
                                  AND a.attnum > 0 AND NOT a.attisdropped
               CROSS JOIN (VALUES ('UPDATE')) AS p(priv)
               JOIN pg_roles r ON NOT r.rolsuper AND r.rolname NOT LIKE 'pg\\_%'
              WHERE n.nspname = $1
                AND col_description(c.oid, a.attnum) LIKE '@projection%'
                -- Owning the projection is the maintainer's whole job.
                AND r.rolname <> 'nylonite_projection_owner'
                AND (has_column_privilege(r.oid, c.oid, a.attname, p.priv)
                     OR has_table_privilege(r.oid, c.oid, 'DELETE'))",
            &[&APP_SCHEMA],
        )?;

        // Every pair the check could have flagged, so a zero result is a real
        // zero rather than an empty catalog view.
        let examined: i64 = c
            .query_one(
                "SELECT count(*)
                   FROM pg_class c
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   JOIN pg_attribute a ON a.attrelid = c.oid
                                      AND a.attnum > 0 AND NOT a.attisdropped
                   CROSS JOIN (VALUES ('UPDATE')) AS p(priv)
                   JOIN pg_roles r ON NOT r.rolsuper AND r.rolname NOT LIKE 'pg\\_%'
                  WHERE n.nspname = $1
                    AND col_description(c.oid, a.attnum) LIKE '@projection%'
                    AND r.rolname <> 'nylonite_projection_owner'",
                &[&APP_SCHEMA],
            )?
            .get(0);

        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J36, "projection_writable",
                    format!("{} may {} {}.{}, which a rebuild function owns",
                        r.get::<_, String>(0), r.get::<_, String>(3),
                        r.get::<_, String>(1), r.get::<_, String>(2)))
            })
            .collect();
        Ok((examined as usize, findings))
    }

    pub fn j37_definer_functions_are_pinned_and_private(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "SELECT p.proname,
                    coalesce(array_to_string(p.proconfig, ','), '') AS cfg,
                    coalesce(array_to_string(p.proacl::text[], ','), '') AS acl
               FROM pg_proc p
              WHERE p.pronamespace = to_regnamespace('public') AND p.prosecdef",
            &[],
        )?;
        let mut findings = vec![];
        for r in &rows {
            let name: String = r.get(0);
            let cfg: String = r.get(1);
            let acl: String = r.get(2);
            if !cfg.contains("search_path") {
                findings.push(finding(Id::J37, "definer_unpinned",
                    format!("{name} is SECURITY DEFINER with no search_path: a mutable one is remote code execution as the owner")));
            }
            if acl.contains("=X/") && acl.split(',').any(|e| e.trim_start().starts_with("=X/")) {
                findings.push(finding(Id::J37, "definer_public",
                    format!("{name} still grants EXECUTE to PUBLIC")));
            }
        }
        Ok((rows.len(), findings))
    }

    pub fn j38_projection_tables_force_rls(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "SELECT DISTINCT c.relname, c.relforcerowsecurity
               FROM pg_class c JOIN pg_attribute a
                    ON a.attrelid = c.oid AND a.attnum > 0
              WHERE c.relkind = 'r'
                AND col_description(c.oid, a.attnum) LIKE '@projection%'",
            &[],
        )?;
        let findings = rows
            .iter()
            .filter(|r| !r.get::<_, bool>(1))
            .map(|r| {
                finding(Id::J38, "projection_unforced",
                    format!("{} carries a projection column without FORCE ROW LEVEL SECURITY",
                        r.get::<_, String>(0)))
            })
            .collect();
        Ok((rows.len(), findings))
    }
    /// D19. Every tenant-scoped table the application can reach is closed.
    ///
    /// **J38 checks the tables carrying an `@projection` column, and that is a
    /// narrower set than it looks.** `reported_stock` arrived carrying a
    /// `tenant_id`, no projection column, and row level security switched off —
    /// and the whole suite stayed green, because nothing was looking at the
    /// property that actually matters. A tenant column is the declaration that
    /// rows belong to somebody; RLS is the thing that makes it true.
    ///
    /// # Why the privilege clause, and why it is not an allow-list
    ///
    /// Row-level security is not the only way to close a table, and this schema
    /// uses the other one. `session` carries a `tenant_id` and has no policy at
    /// all, because `nylonite_app` holds no privilege on it whatsoever: the
    /// `SECURITY DEFINER` functions of migration 70 are its only interface. A
    /// check that demanded RLS there would be demanding a second lock on a door
    /// that is already welded shut.
    ///
    /// The tempting fix is to name the exceptions. This does not, because a
    /// named exception is a claim that ages: it stays true in the file long
    /// after it has stopped being true in the database. The exemption is
    /// instead the reason itself — *the application cannot reach this table* —
    /// asked of Postgres at check time. So the exemption audits itself. Grant
    /// `nylonite_app` a single column of `session` and this check fires on the
    /// next run, which is exactly when somebody should hear about it.
    ///
    /// Column privileges rather than table privileges: this schema grants by
    /// column in several places, and a table-level test would report no access
    /// while one column stood open.
    ///
    /// Counted over every ordinary table, so a new one cannot arrive open by
    /// being forgotten rather than by being decided.
    pub fn j73_tenant_tables_force_rls(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "SELECT c.relname, c.relrowsecurity, c.relforcerowsecurity
               FROM pg_class c
               JOIN pg_namespace n ON n.oid = c.relnamespace
               JOIN pg_attribute a ON a.attrelid = c.oid AND a.attname = 'tenant_id'
              WHERE c.relkind = 'r' AND n.nspname = 'public' AND NOT a.attisdropped
                AND (has_any_column_privilege('nylonite_app', c.oid, 'SELECT')
                  OR has_any_column_privilege('nylonite_app', c.oid, 'INSERT')
                  OR has_any_column_privilege('nylonite_app', c.oid, 'UPDATE')
                  OR has_table_privilege('nylonite_app', c.oid, 'DELETE'))
              ORDER BY c.relname",
            &[],
        )?;
        let findings = rows
            .iter()
            .filter(|r| !(r.get::<_, bool>(1) && r.get::<_, bool>(2)))
            .map(|r| {
                finding(Id::J73, "tenant_table_open",
                    format!("{} has a tenant_id and does not force row level security",
                        r.get::<_, String>(0)))
            })
            .collect();
        Ok((rows.len(), findings))
    }

    /// D47. The backstop behind the trigger.
    ///
    /// The guard runs BEFORE INSERT, so it cannot see rows written while it was
    /// dropped, rows loaded by a restore or a migration that disabled triggers,
    /// or rows written before migration 10 existed. S38 asserts the guard is
    /// armed; this asserts the data is what an armed guard would have produced.
    ///
    /// The `occurred_at` clause is the one that matters most and shows least. A
    /// correction stamped at the moment of discovery rather than the moment it
    /// is about makes every as-at query answer with the pre-correction number
    /// forever, and nothing anywhere reports an error.
    pub fn j50_corrections_are_about_their_target(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM stock_movement WHERE reverses_movement_id IS NOT NULL",
                &[],
            )?
            .get(0);

        let rows = c.query(
            "SELECT n.id::text,
                    n.occurred_at <> t.occurred_at        AS bad_time,
                    n.item_id <> t.item_id                AS bad_item,
                    n.tenant_id <> t.tenant_id            AS bad_tenant,
                    (ROW(n.from_location_id, n.from_package_id, n.from_lot_id,
                         n.from_status_id, n.from_owner_id)
                     IS DISTINCT FROM
                     ROW(t.to_location_id, t.to_package_id, t.to_lot_id,
                         t.to_status_id, t.to_owner_id))
                 OR (ROW(n.to_location_id, n.to_package_id, n.to_lot_id,
                         n.to_status_id, n.to_owner_id)
                     IS DISTINCT FROM
                     ROW(t.from_location_id, t.from_package_id, t.from_lot_id,
                         t.from_status_id, t.from_owner_id)) AS bad_sides,
                    (r.class IS DISTINCT FROM 'record_error'::revision_class) AS bad_reason
               FROM stock_movement n
               JOIN stock_movement t ON t.id = n.reverses_movement_id
               LEFT JOIN adjustment_reason r ON r.id = n.adjustment_reason_id
              WHERE n.reverses_movement_id IS NOT NULL",
            &[],
        )?;

        let mut findings = vec![];
        for r in &rows {
            let id: String = r.get(0);
            for (col, kind, detail) in [
                (1, "correction_time", "carries its own discovery time instead of its target's occurred_at"),
                (2, "correction_item", "corrects a movement of a different item"),
                (3, "correction_tenant", "corrects a movement belonging to another tenant"),
                (4, "correction_sides", "does not mirror the movement it reverses"),
                (5, "correction_reason", "does not give a record_error reason"),
            ] {
                if r.get::<_, bool>(col) {
                    findings.push(finding(Id::J50, kind, format!("correction {id} {detail}")));
                }
            }
        }
        Ok((examined as usize, findings))
    }

    /// D47. The aggregate the trigger cannot hold.
    ///
    /// The guard rejects a single correction larger than its target, but two
    /// concurrent corrections each see a pre-image without the other and both
    /// pass. Taking the lock that would prevent it would serialise every write
    /// to the ledger, which is the coordination D5 exists to refuse, so this
    /// stays a finding: over-reversal invents stock that never existed, and
    /// that is a discrepancy with an owner rather than an error at the till.
    ///
    /// **Widened by D103 to the chain.** The rule is unchanged and its arithmetic
    /// is not: a movement is over-reversed exactly when what is left of it goes
    /// negative, and what is left of it is now `stock_movement_effective` rather
    /// than a sum of face values one level down. The difference is a real case, not
    /// a refactor — a correction of twenty-five that was itself corrected by ten
    /// takes back fifteen, and against a target of twenty that is legitimate. The
    /// face-value form reported it as overreach, which is a check firing on its own
    /// arithmetic rather than on the data.
    pub fn j51_corrections_do_not_overreach(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            // sum(bigint) is numeric, so the cast is load-bearing rather than
            // decorative: without it this check deserialises fine on every
            // database where it finds nothing and panics on the first one where
            // it finds something.
            // D103. The comparison is against the correction's *effective* quantity,
            // not its face value: a correction of twenty-five that was itself
            // corrected by ten takes back fifteen, and against a target of twenty
            // that is legitimate. Reading face values would report the chain as
            // overreach, which is the check firing on the arithmetic rather than on
            // the data.
            "SELECT t.id::text, t.quantity, (t.quantity - v.effective_quantity)::bigint
               FROM stock_movement t
               JOIN stock_movement_effective v
                 ON v.movement_id = t.id AND v.tenant_id = t.tenant_id
              WHERE v.effective_quantity < 0",
            &[],
        )?;
        let examined: i64 = c
            .query_one(
                "SELECT count(DISTINCT reverses_movement_id) FROM stock_movement
                  WHERE reverses_movement_id IS NOT NULL",
                &[],
            )?
            .get(0);

        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J51, "correction_overreach",
                    format!("movement {} of {} units has {} units reversed against it",
                        r.get::<_, String>(0),
                        r.get::<_, i64>(1),
                        r.get::<_, i64>(2)))
            })
            .collect();
        Ok((examined as usize, findings))
    }

    /// D47, borrowing J17's shape.
    ///
    /// A correction is itself a fact, so it can be wrong, so it can be
    /// corrected. That has to stay legal, which means the graph is a chain
    /// rather than a single hop, which means it can loop.
    ///
    /// It cannot loop by ordinary writing. The FK is not deferrable, so a
    /// correction's target must already exist, and insertion order alone makes
    /// the graph acyclic. Creating a cycle takes an UPDATE, and D25 grants the
    /// application role none on this table. What is left is the maintainer role
    /// and a restore run with FK triggers disabled, which is a narrower
    /// population than it first appears and is stated here so nobody reads a
    /// pass as broader assurance than it is.
    ///
    /// Worth keeping because a cycle makes the fold non-terminating rather than
    /// merely wrong, and because a restore is exactly when nobody is looking.
    pub fn j52_correction_chains_do_not_loop(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM stock_movement WHERE reverses_movement_id IS NOT NULL",
                &[],
            )?
            .get(0);

        let rows = c.query(
            "WITH RECURSIVE walk(root, node, path, looped) AS (
                 SELECT id, reverses_movement_id, ARRAY[id], false
                   FROM stock_movement WHERE reverses_movement_id IS NOT NULL
                 UNION ALL
                 SELECT w.root, m.reverses_movement_id, w.path || m.id, m.id = ANY(w.path)
                   FROM walk w
                   JOIN stock_movement m ON m.id = w.node
                  WHERE NOT w.looped AND array_length(w.path, 1) < 64
             )
             SELECT DISTINCT root::text FROM walk WHERE looped",
            &[],
        )?;

        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J52, "correction_cycle",
                    format!("correction chain from {} returns to itself",
                        r.get::<_, String>(0)))
            })
            .collect();
        Ok((examined as usize, findings))
    }
    /// D48. The order side of D47's rule, and the reason it is a rule.
    ///
    /// An amendment correcting a typo has to carry the moment it is about, not
    /// the moment the typo was found. Stamped at discovery it sorts last in
    /// J46's `(occurred_at, recorded_at, id)` fold and wins, so a correction to
    /// a value the customer has since deliberately changed silently reverts
    /// their change. The order then promises a date nobody agreed to and every
    /// check passes.
    ///
    /// A movement correction names its target by foreign key, so the moment is
    /// unambiguous. An amendment has no target row, because the value it revises
    /// may be the order's own original, so the rule is expressed against the set
    /// of moments where a value was actually set: `placed_at`, or some other
    /// amendment on the same order.
    pub fn j53_corrections_name_a_moment_that_exists(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM intention_amendment WHERE revision_class = 'record_error'",
                &[],
            )?
            .get(0);

        let rows = c.query(
            "SELECT a.id::text, a.occurred_at::text, o.placed_at::text
               FROM intention_amendment a
               JOIN \"order\" o ON o.id = a.order_id
              WHERE a.revision_class = 'record_error'
                AND a.occurred_at <> o.placed_at
                AND NOT EXISTS (
                      SELECT 1 FROM intention_amendment p
                       WHERE p.order_id = a.order_id
                         AND p.id <> a.id
                         AND p.occurred_at = a.occurred_at)",
            &[],
        )?;

        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J53, "correction_moment",
                    format!("amendment {} corrects a moment {} at which nothing was set (the order was placed at {})",
                        r.get::<_, String>(0), r.get::<_, String>(1), r.get::<_, String>(2)))
            })
            .collect();
        Ok((examined as usize, findings))
    }
    /// D50. The pairing rule, expressed across a row boundary.
    ///
    /// `record_scheme_field` already states it for a single row: `currency` is
    /// NOT NULL exactly when the field is `money_minor`. Here the money is on
    /// the line and the currency is on the order, because one order has one
    /// currency and a per-line column would only ever agree with itself, so no
    /// CHECK can reach both and this is a job.
    ///
    /// An unpriced line on an order with no currency is fine and is most of
    /// them. A priced line without one is a number with no units.
    pub fn j54_a_price_names_its_currency(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        // Both order kinds. D59 makes a purchase order our own intention in the
        // same category as a customer order, so D50's pairing is one rule over
        // two tables rather than a second rule that happens to look alike.
        const SIDES: &[(&str, &str, &str, &str)] = &[
            ("order_line", "order", "order_id", "confirmation_number"),
            ("purchase_order_line", "purchase_order", "purchase_order_id", "order_number"),
        ];
        let mut findings = vec![];
        let mut examined = 0usize;

        for (line, header, fk, label) in SIDES {
            let n: i64 = c
                .query_one(
                    format!("SELECT count(*) FROM {line} WHERE unit_price_minor IS NOT NULL")
                        .as_str(),
                    &[],
                )?
                .get(0);
            examined += n as usize;

            let rows = c.query(
                format!(
                    "SELECT l.id::text, h.{label}
                       FROM {line} l JOIN \"{header}\" h ON h.id = l.{fk}
                      WHERE l.unit_price_minor IS NOT NULL AND h.currency IS NULL"
                )
                .as_str(),
                &[],
            )?;
            for r in &rows {
                findings.push(finding(Id::J54, "price_without_currency",
                    format!("{line} {} is priced but {header} {} names no currency",
                        r.get::<_, String>(0),
                        r.get::<_, Option<String>>(1).unwrap_or_else(|| "?".into()))));
            }
        }
        Ok((examined, findings))
    }
    /// The three `%_policy` tables. D22's S13 asserts this set equals the
    /// `policy_kind` enum, so a fourth kind arriving without being added here is
    /// already a structural failure rather than a silent gap in these checks.
    /// The `%_policy` value tables, read from the catalogue rather than listed.
    ///
    /// This was a hardcoded list of three, and D85 found it the same way it found
    /// the same mistake in `policy_candidate` and `policy_value`: a fourth kind
    /// arrived and J13 reported its perfectly good version as **missing**,
    /// because it only looked in three tables. Three places, one coupling, and
    /// the only fix that ends it is to stop writing the set down. S52 checks the
    /// two SQL functions; this is the Rust half.
    fn value_tables(c: &mut Client) -> Result<Vec<String>, postgres::Error> {
        Ok(c.query(
            "SELECT table_name FROM information_schema.tables
              WHERE table_schema = 'public' AND table_name LIKE '%\\_policy'
                AND table_name <> 'pg_policy'
              ORDER BY table_name",
            &[],
        )?
        .iter()
        .map(|r| r.get(0))
        .collect())
    }

    /// D22. A weight that changed with no reason recorded is how tuning becomes
    /// superstition, so a value version and the act that produced it are
    /// anti-joined in both directions.
    ///
    /// The match is on the moment rather than merely on the binding. A binding
    /// that was revalued four times and carries one `policy_change` would satisfy
    /// a binding-level join and is exactly the case worth catching, so a version
    /// is paired with a change at `lower(effective)`.
    ///
    /// # The exemption, which is a limit rather than a decision
    ///
    /// Platform-shipped bindings carry `tenant_id IS NULL`, and `policy_change`
    /// cannot describe one: it reaches `client_event` through
    /// `(tenant_scope_id, client_event_id)` and `client_event.tenant_id` is NOT
    /// NULL, so there is no client event for an act that belongs to no tenant.
    /// **A shipped default therefore cannot carry provenance in this schema at
    /// all**, which is not something a sweep should settle by picking a side. So
    /// platform versions are out of the population and question 138 carries it.
    pub fn j13_every_value_version_has_its_change(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let mut findings = vec![];
        let mut examined = 0usize;

        for t in &value_tables(c)? {
            // Direction one: a version with no act behind it.
            let orphan_versions = c.query(
                format!(
                    "SELECT v.id::text, lower(v.effective)::text
                       FROM {t} v
                       JOIN policy_binding b ON b.id = v.policy_binding_id
                      WHERE b.tenant_id IS NOT NULL
                        AND NOT EXISTS (SELECT 1 FROM policy_change pc
                                         WHERE pc.policy_binding_id = v.policy_binding_id
                                           AND pc.occurred_at = lower(v.effective))"
                )
                .as_str(),
                &[],
            )?;
            let n: i64 = c
                .query_one(
                    format!(
                        "SELECT count(*) FROM {t} v JOIN policy_binding b
                           ON b.id = v.policy_binding_id WHERE b.tenant_id IS NOT NULL"
                    )
                    .as_str(),
                    &[],
                )?
                .get(0);
            examined += n as usize;
            for r in &orphan_versions {
                findings.push(finding(Id::J13, "policy_value_without_change",
                    format!("{t} {} came into force at {} with no policy_change at that moment",
                        r.get::<_, String>(0), r.get::<_, String>(1))));
            }
        }

        // Direction two: an act that produced no version. `retired` closes a
        // range rather than opening one, so only the three opening kinds are
        // expected to have a version starting at their moment.
        let filter = value_tables(c)?
            .iter()
            .map(|t| format!(
                "EXISTS (SELECT 1 FROM {t} v WHERE v.policy_binding_id = pc.policy_binding_id
                          AND lower(v.effective) = pc.occurred_at)"))
            .collect::<Vec<_>>()
            .join(" OR ");
        let orphan_changes = c.query(
            format!(
                "SELECT pc.id::text, pc.change_kind::text, pc.occurred_at::text
                   FROM policy_change pc
                   JOIN policy_binding b ON b.id = pc.policy_binding_id
                  WHERE b.tenant_id IS NOT NULL
                    AND pc.change_kind <> 'retired'
                    AND NOT ({filter})"
            )
            .as_str(),
            &[],
        )?;
        let n: i64 = c
            .query_one(
                "SELECT count(*) FROM policy_change pc JOIN policy_binding b
                   ON b.id = pc.policy_binding_id
                  WHERE b.tenant_id IS NOT NULL AND pc.change_kind <> 'retired'",
                &[],
            )?
            .get(0);
        examined += n as usize;
        for r in &orphan_changes {
            findings.push(finding(Id::J13, "policy_change_without_value",
                format!("policy_change {} records a {} at {} that opened no value version",
                    r.get::<_, String>(0), r.get::<_, String>(1), r.get::<_, String>(2))));
        }

        Ok((examined, findings))
    }
    /// D22 and D18. A scope axis pointing outside its own tenant.
    ///
    /// The second clause is the one with teeth. `policy_binding` is under the
    /// shared-reference RLS shape, so a platform-shipped row with
    /// `tenant_id IS NULL` is readable by every tenant. Scope it to one tenant's
    /// item and the binding is visible to all of them while resolving for one,
    /// which is a shipped default that is not a default.
    ///
    /// Eight axes, each an independent nullable dimension, checked against the
    /// tenant of whatever they name. A shared row — `tenant_id IS NULL` on the
    /// target — is legitimate under either arm, because that is what a shared
    /// catalogue item is for.
    pub fn j14_a_binding_stays_inside_its_tenant(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        // (scope column, table it points at). zone and site carry a NOT NULL
        // tenant; item, party and metric may be shared.
        const AXES: &[(&str, &str)] = &[
            ("item_class_id", "item_class"),
            ("item_id", "item"),
            ("party_class_id", "party_class"),
            ("party_id", "party"),
            ("site_id", "site"),
            ("zone_id", "zone"),
            ("owner_party_id", "party"),
            ("metric_id", "metric"),
        ];
        let mut findings = vec![];

        for (col, table) in AXES {
            let rows = c.query(
                format!(
                    "SELECT b.id::text, b.tenant_id::text, t.tenant_id::text
                       FROM policy_binding b
                       JOIN {table} t ON t.id = b.{col}
                      WHERE b.{col} IS NOT NULL
                        AND t.tenant_id IS NOT NULL
                        AND t.tenant_id IS DISTINCT FROM b.tenant_id"
                )
                .as_str(),
                &[],
            )?;
            for r in &rows {
                let owner: Option<String> = r.get(1);
                findings.push(match owner {
                    None => finding(Id::J14, "platform_binding_names_tenant_row",
                        format!("platform-shipped binding {} scopes {col} to a row owned by tenant {}, so a default every tenant can read resolves for one of them",
                            r.get::<_, String>(0), r.get::<_, String>(2))),
                    Some(t) => finding(Id::J14, "binding_crosses_tenant",
                        format!("binding {} belongs to tenant {t} and scopes {col} to a row owned by {}",
                            r.get::<_, String>(0), r.get::<_, String>(2))),
                });
            }
        }

        // The population is every scope axis actually named, not every binding:
        // an all-NULL platform default has nothing to resolve and counting it
        // would inflate the denominator with rows this cannot fail on.
        let named: Vec<String> = AXES.iter().map(|(c, _)| format!("num_nonnulls({c})")).collect();
        let examined: i64 = c
            .query_one(
                format!("SELECT coalesce(sum({}), 0)::bigint FROM policy_binding", named.join(" + "))
                    .as_str(),
                &[],
            )?
            .get(0);

        Ok((examined as usize, findings))
    }
    /// D22. A scope is a conjunction, so two axes on one dimension must agree.
    ///
    /// Naming an item and an item class that does not contain it produces a
    /// binding that can never match anything: the resolver walks
    /// ancestor-or-self on both and no row satisfies both at once. It is not an
    /// error the database can catch, because each foreign key is individually
    /// valid, and the symptom is a policy somebody configured that silently never
    /// applies.
    ///
    /// Three dimensions have a coarse and a fine axis: Product, Counterparty and
    /// Space. Owner is a party with no class axis beside it, so it is not paired
    /// here.
    pub fn j15_scope_axes_do_not_contradict(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let mut findings = vec![];

        // Product: the item's class must be a descendant-or-self of the named
        // class, which is the closure lookup the whole design exists to make
        // cardinality one.
        for r in &c.query(
            "SELECT b.id::text
               FROM policy_binding b
              WHERE b.item_id IS NOT NULL AND b.item_class_id IS NOT NULL
                AND NOT EXISTS (
                      SELECT 1 FROM item_classification ic
                        JOIN item_class_closure cc ON cc.descendant_id = ic.item_class_id
                       WHERE ic.item_id = b.item_id
                         AND cc.ancestor_id = b.item_class_id)",
            &[],
        )? {
            findings.push(finding(Id::J15, "scope_axes_contradict",
                format!("binding {} names an item that is not under the item_class it also names, so it can never match",
                    r.get::<_, String>(0))));
        }

        // Counterparty, on both the customer axis and the owner axis.
        for (party_col, label) in [("party_id", "party"), ("owner_party_id", "owner party")] {
            for r in &c.query(
                format!(
                    "SELECT b.id::text
                       FROM policy_binding b
                       JOIN party p ON p.id = b.{party_col}
                      WHERE b.{party_col} IS NOT NULL AND b.party_class_id IS NOT NULL
                        AND NOT EXISTS (
                              SELECT 1 FROM party_class_closure cc
                               WHERE cc.descendant_id = p.party_class_id
                                 AND cc.ancestor_id = b.party_class_id)"
                )
                .as_str(),
                &[],
            )? {
                findings.push(finding(Id::J15, "scope_axes_contradict",
                    format!("binding {} names a {label} that is not under the party_class it also names",
                        r.get::<_, String>(0))));
            }
        }

        // Space: D46 made zone flat and hung it off site, so this one is a
        // column comparison rather than a closure walk.
        for r in &c.query(
            "SELECT b.id::text
               FROM policy_binding b JOIN zone z ON z.id = b.zone_id
              WHERE b.zone_id IS NOT NULL AND b.site_id IS NOT NULL
                AND z.site_id <> b.site_id",
            &[],
        )? {
            findings.push(finding(Id::J15, "scope_axes_contradict",
                format!("binding {} names a zone belonging to another site than the site it also names",
                    r.get::<_, String>(0))));
        }

        // The population is bindings that name both axes of some dimension,
        // because a binding naming one axis cannot contradict itself.
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM policy_binding
                  WHERE (item_id IS NOT NULL AND item_class_id IS NOT NULL)
                     OR (party_id IS NOT NULL AND party_class_id IS NOT NULL)
                     OR (owner_party_id IS NOT NULL AND party_class_id IS NOT NULL)
                     OR (zone_id IS NOT NULL AND site_id IS NOT NULL)",
                &[],
            )?
            .get(0);

        Ok((examined as usize, findings))
    }
    /// D22. The resolver's own findings, read back.
    ///
    /// Two bindings at equal specificity is the one outcome most-specific-wins
    /// cannot decide, and D22 chose to raise it rather than pick arbitrarily. So
    /// the resolver writes a `policy_ambiguous` discrepancy and this asserts the
    /// queue is empty of them after a run.
    ///
    /// The population is every discrepancy rather than every ambiguous one, so
    /// this reports what it inspected instead of going vacuous on a clean queue.
    pub fn j16_no_ambiguous_resolution(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c.query_one("SELECT count(*) FROM discrepancy", &[])?.get(0);
        let rows = c.query(
            "SELECT id::text, coalesce(detail, '(no detail)')
               FROM discrepancy WHERE kind = 'policy_ambiguous'",
            &[],
        )?;
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J16, "policy_ambiguous",
                    format!("discrepancy {} records an unresolvable policy scope: {}",
                        r.get::<_, String>(0), r.get::<_, String>(1)))
            })
            .collect();
        Ok((examined as usize, findings))
    }
    /// D24. J3's fold, on the supply side of the same allocation.
    ///
    /// An allocation names a stock cell or an expected supply, never both, so
    /// this and J3 partition the same table between them. The state set is
    /// identical because the question is: a despatched or released allocation has
    /// stopped claiming, whichever side it claimed from.
    pub fn j4_supply_allocation_is_the_fold(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "SELECT e.id::text, e.quantity_allocated, coalesce(a.q, 0)
               FROM expected_supply e
               LEFT JOIN (SELECT expected_supply_id, sum(quantity)::bigint AS q
                            FROM stock_allocation
                           WHERE state IN ('allocated','picking','picked','packed')
                             AND expected_supply_id IS NOT NULL
                           GROUP BY expected_supply_id) a ON a.expected_supply_id = e.id
              WHERE e.quantity_allocated <> coalesce(a.q, 0)",
            &[],
        )?;
        let examined: i64 = c.query_one("SELECT count(*) FROM expected_supply", &[])?.get(0);
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J4, "projection_drift",
                    format!("expected_supply {} holds allocated {} against active allocations of {}",
                        r.get::<_, String>(0), r.get::<_, i64>(1), r.get::<_, i64>(2)))
            })
            .collect();
        Ok((examined as usize, findings))
    }
    /// D24. A promise that has closed while something still counts on it.
    ///
    /// **The allocation is not released**, and D24 is explicit about why: *"A
    /// counterparty's retraction must not silently un-promise a customer order."*
    /// So this raises `supply_withdrawn` and a human decides, which is D8's whole
    /// thesis applied to supply rather than to stock.
    ///
    /// The population is every active allocation against future supply, so a run
    /// with none reports as examining nothing rather than as proving something.
    pub fn j9_no_allocation_against_closed_supply(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM stock_allocation
                  WHERE expected_supply_id IS NOT NULL
                    AND state IN ('allocated','picking','picked','packed')",
                &[],
            )?
            .get(0);
        let rows = c.query(
            "SELECT a.id::text, e.id::text, e.closed_reason::text, a.quantity
               FROM stock_allocation a
               JOIN expected_supply e ON e.id = a.expected_supply_id
              WHERE a.state IN ('allocated','picking','picked','packed')
                AND e.closed_at IS NOT NULL",
            &[],
        )?;
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J9, "supply_withdrawn",
                    format!("allocation {} still claims {} against expected_supply {}, which closed as {}",
                        r.get::<_, String>(0), r.get::<_, i64>(3),
                        r.get::<_, String>(1), r.get::<_, String>(2)))
            })
            .collect();
        Ok((examined as usize, findings))
    }
    /// D24 supply side. Identity survives a rebuild, which is what lets an
    /// allocation outlive one.
    ///
    /// `stock_allocation.expected_supply_id` is `ON DELETE RESTRICT`, so
    /// truncate-and-regenerate would either fail outright or orphan a commitment.
    /// The maintainer upserts on the arm's partial unique index for exactly this
    /// reason, and this is the assertion that it still does.
    ///
    /// Read-only: it compares the ids present against the source lines that
    /// should have produced them. The stronger claim — that running the rebuild
    /// twice changes no id — needs to run the rebuild and is a property test
    /// beside J6's and J46's.
    pub fn j30_rebuild_preserves_identity(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c.query_one("SELECT count(*) FROM expected_supply", &[])?.get(0);
        // A row whose source line no longer justifies it, and which therefore a
        // regenerating rebuild would have silently dropped rather than closed.
        let rows = c.query(
            "SELECT e.id::text
               FROM expected_supply e
               JOIN purchase_order_line l ON l.id = e.purchase_order_line_id
               JOIN purchase_order po ON po.id = l.purchase_order_id
              WHERE po.state <> 'issued' AND e.closed_at IS NULL",
            &[],
        )?;
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J30, "supply_withdrawn",
                    format!("expected_supply {} is open while its order is no longer issued, so a rebuild would have to either close it or drop it, and dropping it breaks any allocation holding it",
                        r.get::<_, String>(0)))
            })
            .collect();
        Ok((examined as usize, findings))
    }
    /// D45 as corrected, D61. Received is a fold, and the original folded a
    /// column that does not exist.
    ///
    /// J26 was written to fold `goods_receipt_line.quantity`. That table carries
    /// no quantity and never did — defining it in D45 is what found the defect,
    /// and the invariant had named the right relationship over the wrong table
    /// and could not have run. It folds the ledger instead, grouped by the typed
    /// cause arm D10 argued for.
    ///
    /// **Only arriving movements count.** A putaway afterwards names the same
    /// receipt line and would otherwise be counted as a second arrival, which is
    /// precisely the double-count the inbound analysis found shipped in a
    /// competitor and which a stored accumulator would have made permanent.
    ///
    /// **And an arrival counts for what is left of it.** D102: a correction may
    /// reverse part of a movement, so the fold subtracts what was taken back rather
    /// than dropping the arrival or ignoring the correction. This check read 100
    /// against a receipt where 96 arrived from the migration that built it until
    /// question 168 was answered, and it agreed with the maintainer the whole time —
    /// which is what a check that duplicates its maintainer's SQL is worth when both
    /// are wrong in the same way.
    pub fn j26_received_is_the_ledger_fold(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "SELECT e.id::text, e.quantity_received, coalesce(r.q, 0)
               FROM expected_supply e
               LEFT JOIN (SELECT grl.expected_supply_id,
                                 sum(v.effective_quantity)::bigint AS q
                            FROM stock_movement m
                            JOIN stock_movement_effective v
                              ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
                            JOIN goods_receipt_line grl ON grl.id = m.goods_receipt_line_id
                           WHERE grl.expected_supply_id IS NOT NULL
                             AND m.from_location_id IS NULL AND m.from_package_id IS NULL
                           GROUP BY grl.expected_supply_id) r ON r.expected_supply_id = e.id
              WHERE e.quantity_received <> coalesce(r.q, 0)",
            &[],
        )?;
        let examined: i64 = c.query_one("SELECT count(*) FROM expected_supply", &[])?.get(0);
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J26, "projection_drift",
                    format!("expected_supply {} holds received {} against a ledger folding to {}",
                        r.get::<_, String>(0), r.get::<_, i64>(1), r.get::<_, i64>(2)))
            })
            .collect();
        Ok((examined as usize, findings))
    }
    /// D22, D63. A taxonomy that eats itself, and a closure that quietly stops
    /// covering part of the tree.
    ///
    /// `item_class_not_own_parent_ck` forbids a node being its own parent and
    /// nothing forbids A → B → A, which was verified insertable before the
    /// closure maintainer was written. A recursive walk over that does not
    /// terminate, so the rebuild uses SQL's `CYCLE` clause and drops the cyclic
    /// branch rather than hanging.
    ///
    /// **That makes the maintainer safe and the data still wrong**, which is
    /// exactly the division this class exists for. The dropped branch is a
    /// subtree with no closure rows, so every policy scoped to a class inside it
    /// silently stops matching — not an error, not a warning, just a rule that
    /// never applies. So the check is two claims: no cycle, and every node
    /// reaches itself.
    ///
    /// The self-row is the one that matters for the second half. D22's matching
    /// language is ancestor-or-self as a single lookup, and a node missing its
    /// own depth-zero row is invisible to a binding naming it directly.
    pub fn j59_taxonomies_are_acyclic(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        const TAXONOMIES: &[(&str, &str)] = &[
            ("item_class", "item_class_closure"),
            ("party_class", "party_class_closure"),
        ];
        let mut findings = vec![];
        let mut examined = 0usize;

        for (node, closure) in TAXONOMIES {
            let n: i64 = c
                .query_one(format!("SELECT count(*) FROM {node}").as_str(), &[])?
                .get(0);
            examined += n as usize;

            // A cycle, found by walking with the same CYCLE clause the rebuild
            // uses and reporting what it refused to follow.
            for r in &c.query(
                format!(
                    "WITH RECURSIVE walk AS (
                         SELECT id AS root, id AS node, 0 AS depth FROM {node}
                         UNION ALL
                         SELECT w.root, n.id, w.depth + 1
                           FROM walk w JOIN {node} n ON n.parent_id = w.node
                     ) CYCLE node SET looped USING trail
                     SELECT DISTINCT root::text FROM walk WHERE looped"
                )
                .as_str(),
                &[],
            )? {
                findings.push(finding(Id::J59, "refinement_too_deep",
                    format!("{node} {} sits on a parentage cycle, so the closure drops that branch and every policy scoped inside it silently stops matching",
                        r.get::<_, String>(0))));
            }

            // Every node reaches itself. The depth-zero row is what makes
            // ancestor-or-self one lookup.
            for r in &c.query(
                format!(
                    "SELECT n.id::text FROM {node} n
                      WHERE NOT EXISTS (SELECT 1 FROM {closure} x
                                         WHERE x.ancestor_id = n.id
                                           AND x.descendant_id = n.id)"
                )
                .as_str(),
                &[],
            )? {
                findings.push(finding(Id::J59, "policy_ambiguous",
                    format!("{node} {} has no depth-zero closure row, so a binding naming it directly matches nothing",
                        r.get::<_, String>(0))));
            }
        }
        Ok((examined, findings))
    }
    /// D73. A move that never said what it would cost.
    ///
    /// `affected_binding_count` is frozen on the fact because it cannot be
    /// recomputed: it counts the bindings that *were* active against a taxonomy
    /// shape the move replaced. So a NULL is not a gap that can be backfilled
    /// later, and this check exists because nothing else can catch it — S7
    /// forbids the trigger, and a NOT NULL would have made migration 31
    /// unapplicable anywhere migration 30 had already recorded a move.
    ///
    /// Vacuous until something re-parents, which is the honest state: it examines
    /// re-parenting rows and there may be none.
    pub fn j60_a_move_records_what_it_was_measured_to_cost(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM policy_change WHERE change_kind = 'reparented'",
                &[],
            )?
            .get(0);

        let mut findings = vec![];
        for r in &c.query(
            "SELECT id::text, coalesce(reason, '(no reason)')
               FROM policy_change
              WHERE change_kind = 'reparented' AND affected_binding_count IS NULL",
            &[],
        )? {
            findings.push(finding(
                Id::J60,
                "policy_ambiguous",
                format!(
                    "re-parent {} records no blast radius, so what its approver was told is unrecoverable: {}",
                    r.get::<_, String>(0),
                    r.get::<_, String>(1)
                ),
            ));
        }
        Ok((examined as usize, findings))
    }

    /// D74. Building on a class that was retired.
    ///
    /// Neither finding is an outage. D74 makes retirement a statement to the
    /// editor rather than the resolver, so a live child under a retired parent
    /// still resolves and a binding created after retirement still matches. What
    /// they mean is that the retirement was not honoured — somebody carried on
    /// as though it had not happened, and the taxonomy now says two things.
    ///
    /// The third case — an item classified into a retired class — is not here and
    /// cannot be: `item_classification` has no timestamp, so there is no moment
    /// to compare against `retired_at`. Stated in migration 32 rather than left
    /// as an apparent oversight.
    pub fn j61_a_retired_class_is_not_still_in_use(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let mut findings = vec![];
        let mut examined = 0usize;

        for (node, class_col) in [
            ("item_class", "item_class_id"),
            ("party_class", "party_class_id"),
        ] {
            let n: i64 = c
                .query_one(
                    format!("SELECT count(*) FROM {node} WHERE retired_at IS NOT NULL").as_str(),
                    &[],
                )?
                .get(0);
            examined += n as usize;

            for r in &c.query(
                format!(
                    "SELECT ch.id::text, ch.code, p.code
                       FROM {node} ch JOIN {node} p ON p.id = ch.parent_id
                      WHERE p.retired_at IS NOT NULL AND ch.retired_at IS NULL"
                )
                .as_str(),
                &[],
            )? {
                findings.push(finding(
                    Id::J61,
                    "retired_parent_live_child",
                    format!(
                        "{node} {} ({}) is live under retired parent {}, so the branch was retired and then built on",
                        r.get::<_, String>(0),
                        r.get::<_, String>(1),
                        r.get::<_, String>(2)
                    ),
                ));
            }

            for r in &c.query(
                format!(
                    "SELECT b.id::text, n.code
                       FROM policy_binding b JOIN {node} n ON n.id = b.{class_col}
                      WHERE n.retired_at IS NOT NULL AND b.created_at > n.retired_at"
                )
                .as_str(),
                &[],
            )? {
                findings.push(finding(
                    Id::J61,
                    "binding_on_retired_class",
                    format!(
                        "binding {} was scoped to {} after it was retired, so the retirement is not being honoured",
                        r.get::<_, String>(0),
                        r.get::<_, String>(1)
                    ),
                ));
            }
        }
        Ok((examined, findings))
    }

    /// D76. A class whose beginning nobody wrote down.
    ///
    /// D72 made `parent_id` the fold of the acts, and there was no act for the
    /// one that placed the class to begin with — so the fold's base was the
    /// INSERT value, which the fold itself then overwrote. A class that has
    /// never moved looks fine and is not: it is one move away from having no
    /// recoverable history at all.
    ///
    /// A finding rather than a constraint, because classes created before D76
    /// legitimately have no origin and inventing one would manufacture the
    /// evidence D73 refused to manufacture. The set shrinks to nothing as they
    /// are recreated, and until then its size is a number in the report.
    pub fn j62_a_class_records_where_it_started(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let mut findings = vec![];
        let mut examined = 0usize;

        for (node, class_col) in [
            ("item_class", "item_class_id"),
            ("party_class", "party_class_id"),
        ] {
            let n: i64 = c
                .query_one(format!("SELECT count(*) FROM {node}").as_str(), &[])?
                .get(0);
            examined += n as usize;

            for r in &c.query(
                format!(
                    "SELECT n.id::text, n.code FROM {node} n
                      WHERE NOT EXISTS (SELECT 1 FROM policy_change p
                                         WHERE p.{class_col} = n.id
                                           AND p.change_kind = 'created')"
                )
                .as_str(),
                &[],
            )? {
                findings.push(finding(
                    Id::J62,
                    "origin_unrecorded",
                    format!(
                        "{node} {} ({}) records no creation, so its parentage before any move cannot be reconstructed",
                        r.get::<_, String>(0),
                        r.get::<_, String>(1)
                    ),
                ));
            }
        }
        Ok((examined, findings))
    }

    /// D21. Supersession that loops, and two claims both in force.
    ///
    /// A resend is legitimate and must be storable — D5, and D21 declines the
    /// unique key on `(author_reference, author_version)` for that reason. What
    /// is not legitimate is *both* versions being in force at once, because then
    /// the shipment has two declared contents and the receipt compares against
    /// whichever the query happened to reach first.
    pub fn j17_supersession_is_acyclic_and_singular(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one("SELECT count(*) FROM assertion", &[])?
            .get(0);
        let mut findings = vec![];

        for r in &c.query(
            "WITH RECURSIVE walk AS (
                 SELECT id AS root, supersedes_assertion_id AS node FROM assertion
                  WHERE supersedes_assertion_id IS NOT NULL
                 UNION ALL
                 SELECT w.root, a.supersedes_assertion_id
                   FROM walk w JOIN assertion a ON a.id = w.node
                  WHERE a.supersedes_assertion_id IS NOT NULL
             ) CYCLE node SET looped USING trail
             SELECT DISTINCT root::text FROM walk WHERE looped",
            &[],
        )? {
            findings.push(finding(
                Id::J17,
                "identity_mismatch",
                format!(
                    "assertion {} sits on a supersession cycle, so no version of the claim is the latest",
                    r.get::<_, String>(0)
                ),
            ));
        }

        // At most one in force per (tenant, author, kind, author_reference).
        for r in &c.query(
            "WITH latest AS (
                 SELECT DISTINCT ON (s.assertion_id) s.assertion_id, s.stance
                   FROM assertion_stance s
                  ORDER BY s.assertion_id, s.occurred_at DESC, s.recorded_at DESC, s.id DESC)
             SELECT a.tenant_id::text, a.author_party_id::text, a.kind::text,
                    coalesce(a.author_reference, '(none)'), count(*)::bigint
               FROM assertion a JOIN latest l ON l.assertion_id = a.id
              WHERE l.stance = 'in_force' AND a.author_reference IS NOT NULL
              GROUP BY 1, 2, 3, 4 HAVING count(*) > 1",
            &[],
        )? {
            findings.push(finding(
                Id::J17,
                "identity_mismatch",
                format!(
                    "{} claims from author {} are in force for {} reference {}, so the declared contents are ambiguous",
                    r.get::<_, i64>(4),
                    r.get::<_, String>(1),
                    r.get::<_, String>(2),
                    r.get::<_, String>(3)
                ),
            ));
        }
        Ok((examined as usize, findings))
    }

    /// D21 rule 3, and the reason the whole category exists.
    ///
    /// *"Never projects into `stock`, and never into a commitment that survives
    /// withdrawal of the claim."* A supplier's despatch advice must never be able
    /// to move our balance: if it could, a counterparty would be writing our
    /// inventory by sending a message.
    ///
    /// Checked as reachability in the catalogue rather than by truncating
    /// anything, because a job-asserted check runs against a live database and
    /// must not destroy it. `expected_supply` and `stock_allocation` are exempt
    /// and named — D24's supply side narrowed rule 3 deliberately, and an
    /// exemption that is listed is a decision while an exemption that is silent
    /// is a hole.
    pub fn j19_stock_does_not_depend_on_a_claim(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "WITH assertion_tables AS (
                 SELECT unnest(ARRAY['assertion','assertion_stance','assertion_check',
                                     'despatch_advice','document_response',
                                     'asserted_unit','asserted_unit_content']) AS t)
             SELECT con.conrelid::regclass::text, con.confrelid::regclass::text,
                    con.conname
               FROM pg_constraint con
               JOIN assertion_tables at ON at.t = con.confrelid::regclass::text
              WHERE con.contype = 'f'
                AND con.conrelid::regclass::text NOT IN (
                    SELECT t FROM assertion_tables)
              ORDER BY 1, 3",
            &[],
        )?;
        let examined = rows.len();
        let mut findings = vec![];
        for r in &rows {
            let referencing: String = r.get(0);
            // D24's supply side narrowed rule 3 in writing: a claim may reach a
            // promise, and demand may bind to it. Everything else that touches
            // the balance is the failure this exists to catch.
            let exempt = matches!(
                referencing.as_str(),
                "expected_supply" | "stock_allocation" | "inbound_shipment" | "discrepancy"
            );
            if !exempt && matches!(referencing.as_str(), "stock" | "stock_movement") {
                findings.push(finding(
                    Id::J19,
                    "projection_drift",
                    format!(
                        "{} references the assertion table {} via {}, so a counterparty's claim can reach our balance",
                        referencing,
                        r.get::<_, String>(1),
                        r.get::<_, String>(2)
                    ),
                ));
            }
        }
        Ok((examined, findings))
    }

    /// D44. A disposition of a claim travelling the same way.
    ///
    /// *"A counterparty's disposition is always of a claim travelling the other
    /// way, and the inverse is a mapping bug that would otherwise look like
    /// data."* Their rejection of our despatch advice is inbound about an
    /// outbound; ours of their order is outbound about an inbound.
    pub fn j49_a_disposition_answers_the_other_direction(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one("SELECT count(*) FROM document_response", &[])?
            .get(0);
        let mut findings = vec![];
        for r in &c.query(
            "SELECT d.assertion_id::text, a.direction::text
               FROM document_response d
               JOIN assertion a ON a.id = d.assertion_id
               JOIN assertion s ON s.id = d.subject_assertion_id
              WHERE a.direction = s.direction",
            &[],
        )? {
            findings.push(finding(
                Id::J49,
                "identity_mismatch",
                format!(
                    "document_response {} is {} and disposes of a claim travelling the same way, which is a mapping bug rather than data",
                    r.get::<_, String>(0),
                    r.get::<_, String>(1)
                ),
            ));
        }
        Ok((examined as usize, findings))
    }

    /// D23. The projection is exactly what a rebuild would produce.
    ///
    /// Recomputed here rather than by running the maintainer, because a
    /// job-asserted check reads a live database and must not write to one. The
    /// expression below is the maintainer's default precedence stated a second
    /// time — which is a duplication with a purpose: **if the two ever disagree,
    /// one of them is wrong, and that is the finding.**
    ///
    /// D23's general rule is why `observation_precedence_policy_id` exists at
    /// all: a projection maintained under a policy must record the policy that
    /// produced it, or a rebuild reports every policy change as drift. It is NULL
    /// throughout today, and this check is what that promise is worth until the
    /// policy exists.
    pub fn j10_current_is_exactly_the_rebuild(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one("SELECT count(*) FROM observation_current", &[])?
            .get(0);
        let mut findings = vec![];

        // D86. Recomputed **under the policy each row records**, which is why
        // D23 requires a projection maintained under a policy to record one:
        // otherwise a rebuild reports the policy working as drift, because the
        // projection was correct under the old rule and correct under the new
        // one and nothing here could tell the difference. A NULL policy is the
        // default rule, which is a fact rather than an absence.
        const EXPECTED: &str = "
            WITH applied AS (
                SELECT o.id, o.observable_id, o.metric_id, o.value_numeric,
                       o.observed_at, e.recorded_at, e.asserted_by_party_id,
                       coalesce(p.prefer_own, true) AS prefer_own,
                       coalesce(p.accept_counterparty, true) AS accept_counterparty
                  FROM observation o
                  JOIN observation_event e ON e.id = o.observation_event_id
                  LEFT JOIN observation_current oc
                         ON oc.observable_id = o.observable_id
                        AND oc.metric_id = o.metric_id
                  LEFT JOIN observation_precedence_policy p
                         ON p.id = oc.observation_precedence_policy_id),
            eligible AS (
                SELECT * FROM applied o
                 WHERE o.id NOT IN (SELECT id FROM applied WHERE false)
                   AND NOT EXISTS (SELECT 1 FROM observation r
                                    WHERE r.retracts_observation_id = o.id)
                   AND NOT EXISTS (SELECT 1 FROM observation cr
                                    WHERE cr.corrects_observation_id = o.id)
                   AND NOT EXISTS (SELECT 1 FROM observation me
                                    WHERE me.id = o.id
                                      AND me.retracts_observation_id IS NOT NULL)
                   AND (o.asserted_by_party_id IS NULL
                        OR (o.accept_counterparty
                            AND EXISTS (SELECT 1 FROM observation_acceptance a
                                         WHERE a.observation_id = o.id))))
            SELECT DISTINCT ON (observable_id, metric_id)
                   observable_id, metric_id, id AS observation_id, value_numeric
              FROM eligible
             ORDER BY observable_id, metric_id,
                      (prefer_own AND asserted_by_party_id IS NOT NULL),
                      observed_at DESC, recorded_at DESC, id DESC";

        // Rows the rebuild would produce that are missing or different.
        for r in &c.query(
            &format!(
                "WITH expected AS ({EXPECTED})
                 SELECT e.observable_id::text, e.metric_id::text,
                        coalesce(oc.observation_id::text, '(absent)'),
                        e.observation_id::text
                   FROM expected e
                   LEFT JOIN observation_current oc
                     ON oc.observable_id = e.observable_id AND oc.metric_id = e.metric_id
                  WHERE oc.observation_id IS DISTINCT FROM e.observation_id"
            ),
            &[],
        )? {
            findings.push(finding(
                Id::J10,
                "projection_drift",
                format!(
                    "observation_current for observable {} metric {} holds {} where a rebuild produces {}",
                    r.get::<_, String>(0),
                    r.get::<_, String>(1),
                    r.get::<_, String>(2),
                    r.get::<_, String>(3)
                ),
            ));
        }

        // Rows that survive in the projection and no rebuild would produce: the
        // reaper's half, and the one a stale projection fails silently on.
        for r in &c.query(
            &format!(
                "WITH expected AS ({EXPECTED})
                 SELECT oc.observable_id::text, oc.metric_id::text
                   FROM observation_current oc
                  WHERE NOT EXISTS (SELECT 1 FROM expected e
                                     WHERE e.observable_id = oc.observable_id
                                       AND e.metric_id = oc.metric_id)"
            ),
            &[],
        )? {
            findings.push(finding(
                Id::J10,
                "projection_drift",
                format!(
                    "observation_current holds a value for observable {} metric {} that no rebuild produces, so the reaper did not run",
                    r.get::<_, String>(0),
                    r.get::<_, String>(1)
                ),
            ));
        }
        Ok((examined as usize, findings))
    }

    /// D23 and D25. Their number does not become ours by arriving.
    ///
    /// A supplier's declared weight is theirs and unrevisable; adopting it as the
    /// value we compute freight against is a separate act of ours. Without the
    /// acceptance, a message would set the number an invoice is checked against —
    /// which is D21's control-not-authorship cut, applied to measurement.
    pub fn j11_their_number_needs_our_acceptance(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM observation_current WHERE asserted_by_party_id IS NOT NULL",
                &[],
            )?
            .get(0);
        let mut findings = vec![];
        for r in &c.query(
            "SELECT oc.observation_id::text, oc.asserted_by_party_id::text
               FROM observation_current oc
              WHERE oc.asserted_by_party_id IS NOT NULL
                AND NOT EXISTS (SELECT 1 FROM observation_acceptance a
                                 WHERE a.observation_id = oc.observation_id)",
            &[],
        )? {
            findings.push(finding(
                Id::J11,
                "assertion_unresolvable",
                format!(
                    "observation {} asserted by party {} is the current value with no acceptance, so their claim became our number by arriving",
                    r.get::<_, String>(0),
                    r.get::<_, String>(1)
                ),
            ));
        }
        Ok((examined as usize, findings))
    }

    /// D23. A package's dimensions against the current observation, for unsealed
    /// packages only.
    ///
    /// The restriction is the decision, not a convenience: *"a shipped package's
    /// dimensions are a historical fact about that consignment and must never
    /// change"*, because a retroactive correction would rewrite the number a
    /// freight invoice was computed against. Sealed packages are therefore
    /// expected to disagree with a later measurement, and checking them would
    /// report the design as a defect.
    pub fn j12_unsealed_package_dimensions_agree(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM package p
                   JOIN observable o ON o.package_id = p.id
                  WHERE p.sealed_at IS NULL",
                &[],
            )?
            .get(0);
        let mut findings = vec![];
        for r in &c.query(
            "SELECT p.id::text, m.code, oc.value_numeric::text,
                    CASE m.code WHEN 'length' THEN p.length_mm
                                WHEN 'width'  THEN p.width_mm
                                WHEN 'height' THEN p.height_mm
                                WHEN 'gross_weight' THEN p.gross_weight_g END::text
               FROM package p
               JOIN observable o ON o.package_id = p.id
               JOIN observation_current oc ON oc.observable_id = o.id
               JOIN metric m ON m.id = oc.metric_id
              WHERE p.sealed_at IS NULL
                AND m.code IN ('length','width','height','gross_weight')
                AND (CASE m.code WHEN 'length' THEN p.length_mm
                                 WHEN 'width'  THEN p.width_mm
                                 WHEN 'height' THEN p.height_mm
                                 WHEN 'gross_weight' THEN p.gross_weight_g END)
                    IS DISTINCT FROM oc.value_numeric",
            &[],
        )? {
            findings.push(finding(
                Id::J12,
                "identity_mismatch",
                format!(
                    "unsealed package {} carries {} of {} against a current observation of {}",
                    r.get::<_, String>(0),
                    r.get::<_, String>(1),
                    r.get::<_, Option<String>>(3).unwrap_or_else(|| "(none)".into()),
                    r.get::<_, Option<String>>(2).unwrap_or_else(|| "(none)".into())
                ),
            ));
        }
        Ok((examined as usize, findings))
    }

    /// D21 and D23. A promoted value is still the one they stated.
    ///
    /// Accepting a counterparty's observation adopts it; it does not license
    /// editing it. If the projection ever holds a different number from the
    /// observation it names, we are quoting a supplier as having said something
    /// they did not — which is the failure the whole assertion category exists to
    /// prevent, arriving through the projection instead of through an UPDATE.
    pub fn j18_a_promoted_value_is_still_theirs(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM observation_current WHERE asserted_by_party_id IS NOT NULL",
                &[],
            )?
            .get(0);
        let mut findings = vec![];
        for r in &c.query(
            "SELECT oc.observation_id::text, oc.value_numeric::text, o.value_numeric::text
               FROM observation_current oc
               JOIN observation o ON o.id = oc.observation_id
              WHERE oc.asserted_by_party_id IS NOT NULL
                AND (oc.value_numeric IS DISTINCT FROM o.value_numeric
                  OR oc.value_text    IS DISTINCT FROM o.value_text
                  OR oc.value_instant IS DISTINCT FROM o.value_instant
                  OR oc.value_boolean IS DISTINCT FROM o.value_boolean)",
            &[],
        )? {
            findings.push(finding(
                Id::J18,
                "identity_mismatch",
                format!(
                    "observation_current holds {} for observation {}, which states {} -- we are quoting them as saying something they did not",
                    r.get::<_, Option<String>>(1).unwrap_or_else(|| "(none)".into()),
                    r.get::<_, String>(0),
                    r.get::<_, Option<String>>(2).unwrap_or_else(|| "(none)".into())
                ),
            ));
        }
        Ok((examined as usize, findings))
    }

    /// D79. The half of the tenancy boundary that cannot be a key.
    ///
    /// A shared-reference table -- `tenant_id` nullable, D55's read and write
    /// policy pair -- holds platform rows belonging to nobody and tenant rows
    /// belonging to one. **A composite foreign key to one is impossible**: the
    /// child's tenant is real and a platform row's is NULL, so there is nothing
    /// to match. `item` is one of these, which is why twelve tables reference it
    /// by id alone and why S51 does not object.
    ///
    /// The property still holds, it just has to be checked rather than enforced:
    /// a row may name its own tenant's shared row, or the platform's, and nothing
    /// else. Walked from the catalogue so a new shared-reference table is covered
    /// the day it exists -- the same reason S50 reads the epoch's inputs from the
    /// catalogue rather than a list.
    pub fn j64_a_shared_reference_is_ours_or_the_platform_s(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let edges = c.query(
            "WITH shared AS (
                 SELECT c.oid, c.relname FROM pg_class c
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   JOIN pg_attribute a ON a.attrelid = c.oid
                        AND a.attname = 'tenant_id' AND NOT a.attnotnull
                  WHERE n.nspname = 'public' AND c.relkind = 'r'),
             src AS (
                 SELECT c.oid, c.relname FROM pg_class c
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   JOIN pg_attribute a ON a.attrelid = c.oid
                        AND a.attname = 'tenant_id' AND a.attnotnull
                  WHERE n.nspname = 'public' AND c.relkind = 'r')
             SELECT s.relname, t.relname, a.attname
               FROM pg_constraint k
               JOIN src s ON s.oid = k.conrelid
               JOIN shared t ON t.oid = k.confrelid
               JOIN unnest(k.conkey) x(attnum) ON true
               JOIN pg_attribute a ON a.attrelid = k.conrelid AND a.attnum = x.attnum
              WHERE k.contype = 'f' AND array_length(k.conkey, 1) = 1
              ORDER BY 1, 2, 3",
            &[],
        )?;

        let mut findings = vec![];
        let mut examined = 0usize;
        for e in &edges {
            let (child, parent, col): (String, String, String) =
                (e.get(0), e.get(1), e.get(2));
            let n: i64 = c
                .query_one(
                    &format!(
                        "SELECT count(*) FROM \"{child}\" f
                           JOIN \"{parent}\" p ON p.id = f.{col}
                          WHERE p.tenant_id IS NOT NULL AND p.tenant_id <> f.tenant_id"
                    ),
                    &[],
                )?
                .get(0);
            examined += 1;
            if n > 0 {
                findings.push(finding(
                    Id::J64,
                    "tenant_mismatch",
                    format!(
                        "{n} row(s) of {child} name a {parent} through {col} that belongs to another tenant"
                    ),
                ));
            }
        }
        Ok((examined, findings))
    }

    /// D26 and D36. The ceiling, checked against the rows that are the ceiling.
    ///
    /// Superseded as a job by D36: the limit is now claimed slots enforced at
    /// declaration, so most of this is structural and cannot be violated — a
    /// claim is an UPDATE of an issued row, so claimed can never exceed issued.
    /// **What is not structural is withdrawal.** D36 is explicit that lowering a
    /// ceiling must never destroy a schema, and a withdrawn slot still holding a
    /// key is that failure in progress: the platform has taken the slot out of
    /// issue while a tenant's table is still standing on it.
    pub fn j21_no_tenant_holds_more_than_it_was_issued(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one("SELECT count(*) FROM extension_slot", &[])?
            .get(0);
        let mut findings = vec![];

        for r in &c.query(
            "SELECT tenant_id::text, kind::text,
                    count(*) FILTER (WHERE claimed_key IS NOT NULL)::bigint,
                    count(*)::bigint
               FROM extension_slot
              GROUP BY 1, 2
             HAVING count(*) FILTER (WHERE claimed_key IS NOT NULL) > count(*)",
            &[],
        )? {
            findings.push(finding(
                Id::J21,
                "refinement_too_deep",
                format!(
                    "tenant {} holds {} claimed {} slots against {} issued",
                    r.get::<_, String>(0),
                    r.get::<_, i64>(2),
                    r.get::<_, String>(1),
                    r.get::<_, i64>(3)
                ),
            ));
        }

        for r in &c.query(
            "SELECT tenant_id::text, kind::text, ordinal, claimed_key
               FROM extension_slot
              WHERE withdrawn_at IS NOT NULL AND claimed_key IS NOT NULL",
            &[],
        )? {
            findings.push(finding(
                Id::J21,
                "refinement_too_deep",
                format!(
                    "{} slot {} for tenant {} is withdrawn while still claimed by {}, so a ceiling change is standing on a live schema",
                    r.get::<_, String>(1),
                    r.get::<_, i32>(2),
                    r.get::<_, String>(0),
                    r.get::<_, String>(3)
                ),
            ));
        }
        Ok((examined as usize, findings))
    }

    /// D36. A scheme holds the slot it names, and no slot holds two keys.
    ///
    /// The foreign key gets a scheme to *a* slot. It cannot say that the slot is
    /// claimed by *this* scheme's key, nor that two keys have not pointed at the
    /// same ordinal — and both would silently break the ceiling while every
    /// existence check passed, which is the shape D49's third failure mode takes
    /// in data rather than in prose.
    ///
    /// Shipped schemes are exempt and it is not an oversight: `tenant_id NULL`
    /// consumes no tenant's ceiling, and `record_scheme_slot_ck` already requires
    /// the two to be absent together.
    pub fn j39_a_scheme_holds_the_slot_it_names(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM record_scheme WHERE tenant_id IS NOT NULL",
                &[],
            )?
            .get(0);
        let mut findings = vec![];

        for r in &c.query(
            "SELECT rs.key, rs.slot_ordinal, coalesce(es.claimed_key, '(free)')
               FROM record_scheme rs
               JOIN extension_slot es
                 ON es.tenant_id = rs.tenant_id AND es.kind = rs.slot_kind
                AND es.ordinal = rs.slot_ordinal
              WHERE rs.tenant_id IS NOT NULL
                AND es.claimed_key IS DISTINCT FROM rs.key",
            &[],
        )? {
            findings.push(finding(
                Id::J39,
                "identity_mismatch",
                format!(
                    "scheme {} names slot {} which is held by {}, so the ceiling is counting the wrong thing",
                    r.get::<_, String>(0),
                    r.get::<_, i32>(1),
                    r.get::<_, String>(2)
                ),
            ));
        }

        for r in &c.query(
            "SELECT tenant_id::text, slot_ordinal, count(DISTINCT key)::bigint
               FROM record_scheme
              WHERE tenant_id IS NOT NULL
              GROUP BY 1, 2 HAVING count(DISTINCT key) > 1",
            &[],
        )? {
            findings.push(finding(
                Id::J39,
                "identity_mismatch",
                format!(
                    "slot {} for tenant {} is claimed by {} distinct keys, so the ceiling counts fewer schemes than exist",
                    r.get::<_, i32>(1),
                    r.get::<_, String>(0),
                    r.get::<_, i64>(2)
                ),
            ));
        }
        Ok((examined as usize, findings))
    }

    /// D22, D83. The golden snapshot: meaning, watched.
    ///
    /// Every other check in this suite asserts something about *structure* — a
    /// column exists, a grant is absent, a projection agrees with its fold. This
    /// one asserts that **the answers have not moved**, and it is the only guard
    /// that would notice a change to what ships.
    ///
    /// The inputs it protects are all invisible at the point of use: D81's
    /// eleven precedence orderings, D83's clamp directions, the specificity
    /// constants in `policy_candidate`, and the taxonomy the closures fold from —
    /// which D72 made an act precisely because moving a class changes which
    /// binding wins. A re-parent, a re-argued ordering or a flipped clamp all
    /// change shipped goods, and none of them changes a table definition.
    ///
    /// Two outcomes, and the difference between them is the point:
    ///
    /// - The snapshot was taken under the current `RESOLVER_VERSION` and an
    ///   answer differs: **something changed meaning without saying so.**
    /// - The snapshot was taken under a different version: it is stale, and the
    ///   finding asks for it to be regenerated. That is the explicit bump — it
    ///   does not make the check pass, it makes somebody read the diff.
    pub fn j22_no_resolution_changed_without_a_version_bump(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let recorded = match crate::golden::read_snapshot() {
            Some(s) => s,
            None => {
                return Ok((
                    0,
                    vec![finding(
                        Id::J22,
                        "projection_drift",
                        "no golden snapshot is recorded, so no resolution is being watched"
                            .to_string(),
                    )],
                ))
            }
        };
        let observed = crate::golden::observe(c)?;
        let mut findings = vec![];

        if recorded.version != nylonite_policy::RESOLVER_VERSION {
            findings.push(finding(
                Id::J22,
                "projection_drift",
                format!(
                    "the snapshot was taken under RESOLVER_VERSION {} and the code is {}: regenerate it and read the diff, which is what the bump is for",
                    recorded.version,
                    nylonite_policy::RESOLVER_VERSION
                ),
            ));
            return Ok((observed.len(), findings));
        }

        for (case, answer) in &observed {
            match recorded.answers.get(case) {
                None => findings.push(finding(
                    Id::J22,
                    "projection_drift",
                    format!("{case} resolves now and is not in the snapshot"),
                )),
                Some(was) if was != answer => findings.push(finding(
                    Id::J22,
                    "projection_drift",
                    format!(
                        "{case} answered '{was}' when the snapshot was taken and answers '{answer}' now, with RESOLVER_VERSION unchanged"
                    ),
                )),
                _ => {}
            }
        }
        for case in recorded.answers.keys() {
            if !observed.iter().any(|(c, _)| c == case) {
                findings.push(finding(
                    Id::J22,
                    "projection_drift",
                    format!("{case} is in the snapshot and no longer resolves at all"),
                ));
            }
        }
        Ok((observed.len(), findings))
    }

    /// D76, D87. The decision, recorded rather than reconstructed.
    ///
    /// This is what makes D76's answer to question 148 true rather than
    /// aspirational. 148 asked whether the closures should become temporal so a
    /// past resolution could be replayed; D76 answered that **an audit is served
    /// by recording the decision, not by replaying the world** -- and then
    /// `goods_receipt_line` went on recording who accepted a line and when, and
    /// nothing about which tolerances it was measured against.
    ///
    /// It cannot be reconstructed afterwards, and every reason is legitimate: the
    /// binding may have been superseded (D22 makes scope immutable and a change a
    /// new binding), its effective range may have closed with nothing written
    /// (D70's half no epoch can see), and the taxonomy it resolved through may
    /// have been re-parented -- D84 measured a re-parent silently removing a
    /// shelf-life floor.
    ///
    /// A finding rather than NOT NULL, for the reason D73 and D76 both gave: a
    /// line dispositioned before D87 legitimately has none, and backfilling one
    /// would invent evidence of a decision nobody recorded.
    pub fn j63_a_governed_act_names_its_version(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM goods_receipt_line
                  WHERE accepted_at IS NOT NULL OR rejected_at IS NOT NULL",
                &[],
            )?
            .get(0);
        let mut findings = vec![];
        for r in &c.query(
            "SELECT l.id::text,
                    CASE WHEN l.accepted_at IS NOT NULL THEN 'accepted' ELSE 'rejected' END
               FROM goods_receipt_line l
              WHERE (l.accepted_at IS NOT NULL OR l.rejected_at IS NOT NULL)
                AND l.receiving_policy_id IS NULL",
            &[],
        )? {
            findings.push(finding(
                Id::J63,
                "assertion_unresolvable",
                format!(
                    "receipt line {} was {} against no recorded policy, so why it was cannot be answered without replaying a world that has since moved",
                    r.get::<_, String>(0),
                    r.get::<_, String>(1)
                ),
            ));
        }
        Ok((examined as usize, findings))
    }

    /// D24 supply side. Claims that outlive the promise they were made against.
    ///
    /// D24 specifies the receipt handover precisely: *"At receipt, in the same
    /// transaction as the movements, allocations are re-pointed at the new
    /// `stock_id`, `bound_at` is stamped, and `origin_expected_supply_id` is
    /// retained."* Nothing performs that yet, because it is the receiving path
    /// rather than the schema.
    ///
    /// Until it does, a fully received promise goes on carrying the allocations
    /// made against it and `quantity_promisable` — expected less refined,
    /// received, closed-short and allocated — goes negative. **The fixture
    /// produced exactly that on its first run**, which is what this check was
    /// written from.
    ///
    /// Negative promisable has two causes and both are worth the same finding:
    /// the re-point did not happen, or more was promised against a supply row
    /// than the row ever had. Neither is expressible as a CHECK, because
    /// `quantity_promisable` is generated from five maintained columns that a
    /// rebuild sets independently.
    pub fn j58_promisable_is_never_negative(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c.query_one("SELECT count(*) FROM expected_supply", &[])?.get(0);
        let rows = c.query(
            "SELECT id::text, quantity_expected, quantity_received, quantity_allocated,
                    quantity_promisable
               FROM expected_supply WHERE quantity_promisable < 0",
            &[],
        )?;
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J58, "commitment_unbacked",
                    format!("expected_supply {} promised {}, received {} and still carries {} allocated, leaving {} promisable: either the receipt did not re-point its allocations to stock or more was promised than the row ever had",
                        r.get::<_, String>(0), r.get::<_, i64>(1), r.get::<_, i64>(2),
                        r.get::<_, i64>(3), r.get::<_, i64>(4)))
            })
            .collect();
        Ok((examined as usize, findings))
    }
    /// D58. The packing config a movement names has to be one that was in force.
    ///
    /// D23 versions `item_packing_config` by `effective_from` so that correcting a
    /// case pack cannot rewrite history, and D58 makes that versioning
    /// load-bearing by having the movement record which version converted it.
    /// Neither is worth anything if the recorded version is the wrong one.
    ///
    /// Two ways it goes wrong, and the foreign key catches neither. A config
    /// created next March is a perfectly valid row for a movement that happened
    /// last August — the FK is satisfied and the conversion it describes never
    /// applied. And a config for a different item is equally valid as a
    /// reference, while describing the case size of something else entirely.
    ///
    /// A finding rather than a constraint because it is a job's shape: the
    /// comparison is between two rows and one of them can be edited afterwards,
    /// so a CHECK would only cover the moment of writing.
    /// **Widened by D92 to `goods_receipt_line` and by D93 to
    /// `asserted_unit_content`**, each of which carries the identical triple and
    /// had the identical exposure. The rule was written about the ledger because
    /// the ledger was where D58 put the columns; nothing in it was ever about
    /// movements specifically, and a row converted by another product's case size
    /// is wrong in exactly the same way wherever it sits.
    ///
    /// **Each table's moment is a different column, and the differences are the
    /// point.** A movement has its own `occurred_at`. A receipt line does not —
    /// the receipt it belongs to carries `received_at`, and that is when the count
    /// was made. A claim line's moment is its assertion's `asserted_at`, the
    /// author's clock, because the case pack that applied is the one in force when
    /// *they* packed rather than when we read it; `received_at` stands in when
    /// they stated no time, which is D5's both-clocks rule.
    ///
    /// The claim arm tolerates an unresolved `resolved_item_id`. A GTIN we cannot
    /// place yet is D21's normal state and not evidence that the config is for the
    /// wrong product.
    pub fn j57_a_conversion_names_a_config_that_applied(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT (SELECT count(*) FROM stock_movement
                          WHERE item_packing_config_id IS NOT NULL)
                      + (SELECT count(*) FROM goods_receipt_line
                          WHERE item_packing_config_id IS NOT NULL)
                      + (SELECT count(*) FROM asserted_unit_content
                          WHERE item_packing_config_id IS NOT NULL)",
                &[],
            )?
            .get(0);

        let rows = c.query(
            "SELECT what, id::text, moment::text, effective_from::text, wrong_item
               FROM (
                 SELECT 'movement' AS what, m.id, m.occurred_at AS moment,
                        p.effective_from, p.item_id <> m.item_id AS wrong_item
                   FROM stock_movement m
                   JOIN item_packing_config p ON p.id = m.item_packing_config_id
                  WHERE p.effective_from > m.occurred_at::date
                     OR p.item_id <> m.item_id
                 UNION ALL
                 SELECT 'receipt line', l.id, r.received_at,
                        p.effective_from, p.item_id <> l.item_id
                   FROM goods_receipt_line l
                   JOIN goods_receipt r ON r.id = l.goods_receipt_id
                   JOIN item_packing_config p ON p.id = l.item_packing_config_id
                  WHERE p.effective_from > r.received_at::date
                     OR p.item_id <> l.item_id
                 UNION ALL
                 SELECT 'claim line', cc.id,
                        coalesce(a.asserted_at, a.received_at),
                        p.effective_from,
                        -- NULL when the GTIN is still unresolved, which is not a
                        -- claim that the item is wrong.
                        coalesce(p.item_id <> cc.resolved_item_id, false)
                   FROM asserted_unit_content cc
                   JOIN asserted_unit au ON au.id = cc.asserted_unit_id
                   JOIN assertion a ON a.id = au.assertion_id
                   JOIN item_packing_config p ON p.id = cc.item_packing_config_id
                  WHERE p.effective_from > coalesce(a.asserted_at, a.received_at)::date
                     OR p.item_id <> cc.resolved_item_id) v",
            &[],
        )?;

        let findings = rows
            .iter()
            .map(|r| {
                if r.get::<_, bool>(4) {
                    finding(Id::J57, "packing_config_wrong_item",
                        format!("{} {} names a packing config for a different item, so its entered quantity was converted by another product's case size",
                            r.get::<_, String>(0), r.get::<_, String>(1)))
                } else {
                    finding(Id::J57, "packing_config_not_yet_in_force",
                        format!("{} {} happened at {} and names a config effective from {}, which had not been decided yet",
                            r.get::<_, String>(0), r.get::<_, String>(1),
                            r.get::<_, String>(2), r.get::<_, String>(3)))
                }
            })
            .collect();
        Ok((examined as usize, findings))
    }

    /// J65. Principle 5, and the half of it nobody was checking.
    ///
    /// The rule is that the writer converts and the database stores the canonical
    /// value, with the entered form preserved beside it. J57 checks that a row
    /// names the *right* factor. **Nothing has ever checked that the stored number
    /// is what that factor produces**, which means a miscalibrated client writes a
    /// wrong quantity with a perfectly valid config beside it and every check in
    /// the suite passes.
    ///
    /// That is not hypothetical. D91 made this exact error in SQL — subtracting a
    /// carton count from a base quantity — and the fixture carried a claim
    /// asserting 400 base units against 40 entered in a unit whose factor is 1/1,
    /// for as long as the assertion tables have existed.
    ///
    /// Three arms, two factor sources. The packaging arm converts through
    /// `packing_factor`; the unit arm through `unit.factor_num/factor_den`, which
    /// is exact rational per Principle 5 and never a float.
    ///
    /// `factor_unavailable` is its own finding rather than a skip. A row naming a
    /// config that cannot convert the level it also names is a row whose quantity
    /// nobody can ever verify, which is worth saying out loud rather than
    /// quietly declining to examine.
    pub fn j65_the_canonical_agrees_with_what_was_entered(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT (SELECT count(*) FROM stock_movement
                          WHERE entered_quantity IS NOT NULL)
                      + (SELECT count(*) FROM goods_receipt_line
                          WHERE entered_quantity IS NOT NULL AND quantity IS NOT NULL)
                      + (SELECT count(*) FROM asserted_unit_content
                          WHERE entered_quantity IS NOT NULL AND quantity IS NOT NULL)",
                &[],
            )?
            .get(0);

        let rows = c.query(
            "SELECT what, id::text, stored::text, entered::text, expected::text
               FROM (
                 SELECT 'movement' AS what, m.id, m.quantity AS stored,
                        m.entered_quantity AS entered,
                        m.entered_quantity
                          * packing_factor(m.item_packing_config_id,
                                           m.entered_packaging_level) AS expected
                   FROM stock_movement m
                  WHERE m.entered_quantity IS NOT NULL
                 UNION ALL
                 SELECT 'receipt line', l.id, l.quantity, l.entered_quantity,
                        l.entered_quantity
                          * packing_factor(l.item_packing_config_id,
                                           l.entered_packaging_level)
                   FROM goods_receipt_line l
                  WHERE l.entered_quantity IS NOT NULL AND l.quantity IS NOT NULL
                 UNION ALL
                 SELECT 'claim line', cc.id, cc.quantity, cc.entered_quantity,
                        CASE
                          WHEN cc.resolved_unit_id IS NOT NULL
                            THEN cc.entered_quantity * u.factor_num / u.factor_den
                          ELSE cc.entered_quantity
                               * packing_factor(cc.item_packing_config_id,
                                                cc.resolved_packaging_level)
                        END
                   FROM asserted_unit_content cc
                   LEFT JOIN unit u ON u.id = cc.resolved_unit_id
                  WHERE cc.entered_quantity IS NOT NULL
                    AND cc.quantity IS NOT NULL) v
              WHERE expected IS NULL OR expected <> stored",
            &[],
        )?;

        let findings = rows
            .iter()
            .map(|r| {
                let expected: Option<String> = r.get(4);
                match expected {
                    None => finding(Id::J65, "factor_unavailable",
                        format!("{} {} stores {} against {} entered, and the config it names cannot convert the level it names, so the conversion can never be verified",
                            r.get::<_, String>(0), r.get::<_, String>(1),
                            r.get::<_, String>(2), r.get::<_, String>(3))),
                    Some(e) => finding(Id::J65, "entered_disagrees_with_canonical",
                        format!("{} {} stores {} base units against {} entered, which converts to {}",
                            r.get::<_, String>(0), r.get::<_, String>(1),
                            r.get::<_, String>(2), r.get::<_, String>(3), e)),
                }
            })
            .collect();
        Ok((examined as usize, findings))
    }
    /// D39, and the axis D48 narrowed it on turned out to be the wrong one.
    ///
    /// D39: *"an order whose record of authority is external is amended through
    /// that system, not here."* D48 read that as a class of change and narrowed
    /// J44 to forbid `world_event` amendments, permitting `record_error` because
    /// fixing our own mis-transcription is not amending their document.
    ///
    /// Implementing it shows the gap. A counterparty cancels their own order;
    /// their message arrives on the channel; recording it sets `order.state`,
    /// which is a covered column, so it is an amendment carrying `new_state`, and
    /// the world genuinely did change so its class is `world_event`. Under D48's
    /// statement that is forbidden, and nothing else can express something that
    /// unambiguously happened.
    ///
    /// **What D39 forbids is a place, not a class.** A counterparty's event
    /// arriving through the channel is their amendment reaching us; a person here
    /// typing the same thing is us editing their order, which is the
    /// bidirectional merge D39 refused outright.
    ///
    /// `intention_amendment_actor_ck` already carries that: exactly one of
    /// `recorded_by_id` and `automation_key`, so an amendment with a person on it
    /// was made here.
    ///
    /// # The proxy, and its number
    ///
    /// `automation_key` is undefined. D27 narrowed it by contrast — a device is
    /// *how*, a key is *who* — without saying what one is, which is question 105.
    /// So a local nightly job holding an automation key passes a check a person
    /// would fail. This is the strongest rule the schema can currently state, and
    /// 105 is now load-bearing rather than tidy.
    ///
    /// The first half of the register's statement — no order written by a path
    /// that does not set `source_channel` — became structural in migration 16:
    /// `source_channel_id` is NOT NULL with a foreign key, so no path can omit it
    /// and there is nothing left here to check.
    /// D34's normalisation, checked over the table rather than at the writer.
    ///
    /// **The arithmetic is done here in SQL and not by `barcodes::check_digit`,
    /// deliberately.** The register may not depend on the crate that writes the
    /// rows: if the check reused the writer's own function, a bug in it would
    /// make the writer and the checker agree and the invariant would pass while
    /// every barcode in the table was wrong. Two implementations of one rule is
    /// usually drift; here it is the point.
    ///
    /// D34's own reason for asserting it over the table is the same shape — the
    /// normalisation happens on write and *"a second write path will be
    /// added"*, so a call-site assertion would only ever cover the first one.
    ///
    /// The width is a CHECK constraint on the table, so this cannot fail on
    /// length while the constraint holds. It is still computed rather than
    /// assumed, because the constraint is a thing that can be dropped.
    pub fn j42_every_gtin_is_fourteen_and_checks_out(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM item_barcode WHERE scheme = 'gtin'",
                &[],
            )?
            .get(0);

        // Weights alternate 3 and 1 from the RIGHT of the thirteen-digit body.
        // `(13 - i) % 2 = 0` puts the 3 on the rightmost, which is the parity
        // that is easy to get backwards — written the other way it is correct
        // for even-length bodies only, so an EAN-13 would pass and this would
        // not.
        let rows = c.query(
            "WITH gtin AS (
                 SELECT id, barcode FROM item_barcode
                  WHERE scheme = 'gtin' AND barcode ~ '^[0-9]{14}$'
             ),
             digits AS (
                 SELECT g.id, g.barcode, s.i,
                        substr(g.barcode, s.i, 1)::int AS digit
                   FROM gtin g CROSS JOIN generate_series(1, 13) s(i)
             ),
             computed AS (
                 SELECT id, barcode,
                        -- Cast: `sum()` is bigint, and the union below has to
                        -- agree on a type. Reading an int8 as an i32 panics in
                        -- the driver rather than reporting anything, which is
                        -- how this was found.
                        ((10 - (sum(digit * CASE WHEN (13 - i) % 2 = 0 THEN 3 ELSE 1 END) % 10)) % 10)::int
                            AS want
                   FROM digits GROUP BY id, barcode
             )
             SELECT barcode, want, right(barcode, 1)::int AS given
               FROM computed WHERE want <> right(barcode, 1)::int
             UNION ALL
             -- The other half of the statement: anything claiming to be a GTIN
             -- and not fourteen digits. Zero while the CHECK holds, and a
             -- dropped CHECK is exactly when this needs to say so.
             SELECT barcode, -1, -1 FROM item_barcode
              WHERE scheme = 'gtin' AND barcode !~ '^[0-9]{14}$'",
            &[],
        )?;

        let findings = rows
            .iter()
            .map(|r| {
                let barcode: String = r.get(0);
                let want: i32 = r.get(1);
                if want < 0 {
                    finding(Id::J42, "gtin_not_normalised",
                        format!("barcode {barcode:?} is stored as a gtin and is not fourteen digits. A GTIN-13 and the same GTIN padded to 14 are one trade item, and holding both spellings makes the carton scan miss the retail row"))
                } else {
                    finding(Id::J42, "gtin_check_digit_wrong",
                        format!("barcode {barcode} fails its own check digit: the arithmetic wants {want} and the barcode carries {}. A scan of the printed label will not match this row, so the binding is unreachable rather than merely wrong", r.get::<_, i32>(2)))
                }
            })
            .collect();
        Ok((examined as usize, findings))
    }

    /// The sentinels, read from the catalogue rather than from the migration.
    ///
    /// **The failure mode of their removal is silence**, which is why this is a
    /// register entry and not a comment. `UNIQUE NULLS NOT DISTINCT` fixes the
    /// shared-catalogue hole for a unique index and has no equivalent for an
    /// exclusion constraint, so a reviewer who "simplifies" the `COALESCE` away
    /// leaves a constraint that still exists, still fires between tenant rows,
    /// and silently stops constraining the shared catalogue — where a duplicate
    /// barcode means one identifier resolving to two products.
    ///
    /// Read from `pg_constraint` because that is the deployed truth. A
    /// migration file says what was intended once.
    pub fn j43_the_barcode_sentinels_are_present(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let rows = c.query(
            "SELECT conname, pg_get_constraintdef(oid)
               FROM pg_constraint
              WHERE conrelid = 'item_barcode'::regclass AND contype = 'x'",
            &[],
        )?;

        let examined = rows.len();
        let mut findings = vec![];

        if rows.is_empty() {
            findings.push(finding(Id::J43, "barcode_exclusion_absent",
                "item_barcode carries no exclusion constraint at all, so one barcode may bind to any number of items at the same instant".to_string()));
        }

        for r in &rows {
            let name: String = r.get(0);
            let def: String = r.get(1);
            let tenant = def.contains("COALESCE(tenant_id");
            let issuer = def.contains("COALESCE(issuer_party_id");
            if !tenant || !issuer {
                let missing = match (tenant, issuer) {
                    (false, false) => "both sentinels",
                    (false, true) => "the tenant_id sentinel",
                    _ => "the issuer_party_id sentinel",
                };
                findings.push(finding(Id::J43, "barcode_sentinel_removed",
                    format!("{name} is missing {missing}. A NULL compares as NULL rather than as equal, so the constraint silently stops applying to the scope where the column is NULL — the shared catalogue for tenant_id, and every GTIN and internal code for issuer_party_id")));
            }
        }

        Ok((examined, findings))
    }

    pub fn j44_an_external_order_is_not_amended_here(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        // The population is every amendment against an externally-authoritative
        // order, so a run over orders that are all ours reports as examining
        // nothing rather than as proving something.
        let examined: i64 = c
            .query_one(
                "SELECT count(*)
                   FROM intention_amendment a
                   JOIN \"order\" o ON o.id = a.order_id
                   JOIN source_channel sc ON sc.id = o.source_channel_id
                  WHERE sc.authority = 'external'",
                &[],
            )?
            .get(0);

        let rows = c.query(
            "SELECT a.id::text, o.confirmation_number, sc.code, p.display_name
               FROM intention_amendment a
               JOIN \"order\" o ON o.id = a.order_id
               JOIN source_channel sc ON sc.id = o.source_channel_id
               LEFT JOIN person p ON p.id = a.recorded_by_id
              WHERE sc.authority = 'external'
                AND a.revision_class = 'world_event'
                AND a.recorded_by_id IS NOT NULL",
            &[],
        )?;

        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J44, "external_order_amended_locally",
                    format!("{} amended order {} on channel {}, whose record of authority is the counterparty's. A change of their intention has to arrive through the channel or as a succession",
                        r.get::<_, Option<String>>(3).unwrap_or_else(|| "somebody".into()),
                        r.get::<_, Option<String>>(1).unwrap_or_else(|| "?".into()),
                        r.get::<_, String>(2)))
            })
            .collect();
        Ok((examined as usize, findings))
    }
    /// D24's supply side, the demand arm. Widened by D53 to four coverage
    /// quantities and **narrowed by D99 back to one**.
    ///
    /// J3 folds allocations onto the stock cell; this folds the same rows onto
    /// the commitment they serve. **The two use different state sets and that is
    /// the point.** On the cell, a despatched allocation must not count: the
    /// stock has left. On the commitment it must, because despatched is the most
    /// covered a line can be.
    ///
    /// This check was written one commit before D53 from J3's state set, because
    /// both columns were called `allocated_quantity` and the name said they were
    /// the same question. They were not. The rename to `covered_quantity` is what
    /// stops the next reader making the same substitution.
    ///
    /// `short` and `released` appear in no set. Both are terminal ways for an
    /// allocation to stop covering anything.
    ///
    /// **Why only `covered_quantity` now.** D99 separates the four by what they
    /// are evidence of. Coverage asks how much of a commitment is *spoken for*,
    /// which is a question about intentions, and D12 makes an allocation exactly
    /// that — so this fold is right and stays. The other three assert what
    /// physically happened, and J68 measures them against the ledger instead.
    /// Leaving all four here would have kept one source answering two kinds of
    /// question, which is the mistake D53 caught between J3 and J31 and this is
    /// the same mistake one level further on.
    pub fn j31_line_coverage_is_the_fold(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        // The commitment's state set: everything that still stands, including
        // what has despatched, because that is the most covered a line can be.
        const COVERING: &str = "'allocated','picking','picked','packed','fulfilled'";
        let mut findings = vec![];

        let rows = c.query(
            format!(
                "SELECT fl.id::text, fl.covered_quantity, coalesce(a.q, 0)
                   FROM fulfilment_line fl
                   LEFT JOIN (SELECT fulfilment_line_id, sum(quantity)::bigint AS q
                                FROM stock_allocation
                               WHERE state IN ({COVERING})
                                 AND fulfilment_line_id IS NOT NULL
                               GROUP BY fulfilment_line_id) a
                          ON a.fulfilment_line_id = fl.id
                  WHERE fl.covered_quantity <> coalesce(a.q, 0)"
            )
            .as_str(),
            &[],
        )?;
        for r in &rows {
            findings.push(finding(Id::J31, "projection_drift",
                format!("fulfilment_line {} holds covered_quantity {} against allocations folding to {}",
                    r.get::<_, String>(0), r.get::<_, i64>(1), r.get::<_, i64>(2))));
        }

        let lines: i64 = c.query_one("SELECT count(*) FROM fulfilment_line", &[])?.get(0);
        Ok((lines as usize, findings))
    }

    /// D99, D100. The three progress quantities against the ledger that produced
    /// them.
    ///
    /// **This reported a gap and now asserts an agreement, and the query did not
    /// change when it flipped.** Between D99 and D100 the columns were folded from
    /// `stock_allocation.state` while this compared them to the ledger, so every
    /// line with a real pick disagreed and said so. D100 moved the fold; the same
    /// three predicates are now what the maintainer computes, and a finding here
    /// means the projection has drifted from its source rather than that the
    /// source is the wrong one.
    ///
    /// The rules are D99's, and none reads `reason`, which is a fixed vocabulary
    /// under D105 rather than a fold discriminator:
    ///
    /// - **picked** — the movement left a storage location. Re-handling after a
    ///   pick is holder-to-holder, so a consolidation from tote to carton names
    ///   the same line and is correctly not counted twice.
    /// - **packed** — the movement went into a carton whose winning status is
    ///   `sealed` or `despatched`. D100 reads `package.status`, a fold of
    ///   `package_event`, rather than `package.sealed_at`, which the application
    ///   may UPDATE. Opening a carton to repack it moves its status off `sealed`,
    ///   which is what stops a repack being counted at both cartons.
    /// - **despatched** — no `to` side at all. The exact mirror of D45's arrival
    ///   test, which is what stops a pick and a despatch against one line summing
    ///   to double the units that moved.
    ///
    /// A line picked straight from a bin onto the truck satisfies picked and
    /// despatched both, which is not a double-count: D53's sets are nested, and
    /// despatched units were picked.
    ///
    /// **A correction nets against what it corrects**, to any depth (D102, D103).
    /// The arithmetic lives in `stock_movement_effective`, which is the single
    /// definition all five readers of it share — D102 duplicated a simpler version
    /// into four places and nearly paid for it. Corrections never appear as rows of
    /// their own there, which is the half D100 had right: reversing a despatch
    /// produces a movement *into* a sealed carton, and it would otherwise read as
    /// packing and inflate the number it was recorded to fix.
    ///
    /// This must mirror `projection_fulfilment_rebuild` exactly. A check that folds
    /// the source differently from the maintainer reports drift that is really its
    /// own disagreement, which is the failure J26 had before D45 rewrote it.
    pub fn j68_outbound_progress_against_the_ledger(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        // (column, the predicate that selects the movements it should equal).
        // `p` is the package a movement went into, joined for the packed arm.
        const LEDGER: &[(&str, &str)] = &[
            ("picked_quantity", "m.from_location_id IS NOT NULL"),
            ("packed_quantity", "p.status IN ('sealed', 'despatched')"),
            ("despatched_quantity", "m.to_location_id IS NULL AND m.to_package_id IS NULL"),
        ];
        let mut findings = vec![];

        for (col, shape) in LEDGER {
            // Only lines the ledger says something about. A line with no movement
            // naming it is not evidence of disagreement -- it is a commitment
            // nobody has picked yet, which is the ordinary state of most of them.
            let rows = c.query(
                format!(
                    "SELECT fl.id::text, fl.{col}, g.q
                       FROM fulfilment_line fl
                       JOIN (SELECT m.fulfilment_line_id,
                                    sum(v.effective_quantity)::bigint AS q
                               FROM stock_movement m
                               JOIN stock_movement_effective v
                                 ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
                               LEFT JOIN package p ON p.id = m.to_package_id
                              WHERE m.fulfilment_line_id IS NOT NULL
                                AND {shape}
                              GROUP BY m.fulfilment_line_id) g
                         ON g.fulfilment_line_id = fl.id
                      WHERE fl.{col} <> g.q"
                )
                .as_str(),
                &[],
            )?;
            for r in &rows {
                findings.push(finding(Id::J68, "projection_drift",
                    format!("fulfilment_line {} holds {col} {} against a ledger folding to {}",
                        r.get::<_, String>(0), r.get::<_, i64>(1), r.get::<_, i64>(2))));
            }
        }

        // The denominator is what was compared: lines the ledger names, times the
        // three columns it answers for. Zero of them is the vacuous case.
        let named: i64 = c
            .query_one(
                "SELECT count(DISTINCT fulfilment_line_id) FROM stock_movement
                  WHERE fulfilment_line_id IS NOT NULL",
                &[],
            )?
            .get(0);
        Ok((named as usize * LEDGER.len(), findings))
    }

    /// D99's first obligation. A movement may name a fulfilment line while carrying
    /// a different item from it, and the fold would credit the commitment with the
    /// wrong goods.
    ///
    /// Not a CHECK, because it spans two rows — and not only for that reason. D12
    /// makes an allocation advisory and *"allowed to be wrong"*, and its founding
    /// case is the picker who finds lot B where the plan said lot A: the pick is a
    /// fact and the plan was wrong. An item that disagrees with its line is the
    /// same event one category up, so refusing the write would stop the floor over
    /// exactly the disagreement this system exists to record.
    ///
    /// The line's item arrives through `order_line`, because `fulfilment_line` does
    /// not carry one — it names the order line it fulfils and inherits the item
    /// from it, which is D15's shape and the reason a substitution is visible here
    /// rather than expressible as a column mismatch on one row.
    pub fn j69_a_movement_serves_its_line_s_item(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let findings = c
            .query(
                "SELECT m.id::text, fl.id::text, m.item_id::text, ol.item_id::text
                   FROM stock_movement m
                   JOIN fulfilment_line fl ON fl.id = m.fulfilment_line_id
                   JOIN order_line ol ON ol.id = fl.order_line_id
                  WHERE m.item_id <> ol.item_id",
                &[],
            )?
            .iter()
            .map(|r| {
                finding(Id::J69, "scan_mismatch",
                    format!("stock_movement {} serves fulfilment_line {} but moves item {} \
                             where the line commits item {}",
                        r.get::<_, String>(0), r.get::<_, String>(1),
                        r.get::<_, String>(2), r.get::<_, String>(3)))
            })
            .collect();

        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM stock_movement WHERE fulfilment_line_id IS NOT NULL",
                &[],
            )?
            .get(0);
        Ok((examined as usize, findings))
    }

    /// D99's second obligation, and the limit it named rather than hid.
    ///
    /// `picked_quantity` counts movements that left a storage *location*, because a
    /// cell can also be package-held — `stock.holder_location_id` and
    /// `holder_package_id` are exclusive by CHECK — and a pallet in a rack is
    /// storage. Picking from one is `from_package_id`, and the fold does not see it.
    ///
    /// The tempting patch, counting a from-package whose `fulfilment_id` is null,
    /// restores the consolidation double-count from the other side: a batch tote is
    /// not any one line's carton either. The complete rule is recursive — a holder
    /// is in a line's service once stock has entered it under that line's cause —
    /// and D100 did not build it.
    ///
    /// So the gap is reported instead. A movement naming a line, out of a package
    /// that never received stock under that same line, is a pick from package-held
    /// storage: real, correct to record, and missing from `picked_quantity`. That
    /// is D8's shape, and it is the difference between a number that is knowably
    /// J72: a single thing's size names the arrangement it was measured in.
    ///
    /// D138. An apron folded twice is 250x180x30 and the same apron in a heap is
    /// something else, so a length recorded against an `each` with no
    /// presentation beside it is a number the next operator can neither
    /// reproduce nor argue with. `POST /observations` refuses one — and a rule
    /// that lives only in a writer is a rule the *second* writer breaks. The
    /// prepack loader, a repair script and a future import all reach this table,
    /// and none of them go through that check.
    ///
    /// **Only `each`.** A carton is rigid: it has one arrangement, and asking
    /// for the word would be asking for ceremony.
    ///
    /// **Only ours.** A supplier's asserted each dimension is theirs to qualify.
    /// We cannot make a counterparty say how they folded it, and a finding we
    /// can never clear is a finding that trains people to ignore the list — the
    /// argument D8 makes about discrepancy being a designed output rather than a
    /// backlog.
    ///
    /// An absence carries no number, so there is nothing to reproduce and
    /// nothing to qualify.
    pub fn j72_a_single_things_size_names_its_arrangement(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        const UNQUALIFIED: &str = "
            SELECT coalesce(i.code, s.code, 'unknown'), m.code, o.id::text
              FROM observation o
              JOIN observable ob ON ob.id = o.observable_id
              JOIN observation_event e ON e.id = o.observation_event_id
              JOIN metric m ON m.id = o.metric_id
              LEFT JOIN item i ON i.id = ob.item_id
              LEFT JOIN item_style s ON s.id = ob.item_style_id
             WHERE ob.packaging_level = 'each'
               AND m.code IN ('length', 'width', 'height')
               AND o.absent_reason IS NULL
               AND e.asserted_by_party_id IS NULL
               AND e.presentation_id IS NULL";

        let findings = c
            .query(UNQUALIFIED, &[])?
            .iter()
            .map(|r| {
                finding(Id::J72, "identity_mismatch",
                    format!("{} records a {} of a single unit with no presentation, so what \
                             was measured is a number nobody can reproduce: observation {}",
                        r.get::<_, String>(0), r.get::<_, String>(1), r.get::<_, String>(2)))
            })
            .collect();

        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM observation o
                   JOIN observable ob ON ob.id = o.observable_id
                   JOIN metric m ON m.id = o.metric_id
                  WHERE ob.packaging_level = 'each'
                    AND m.code IN ('length', 'width', 'height')
                    AND o.absent_reason IS NULL",
                &[],
            )?
            .get(0);
        Ok((examined as usize, findings))
    }

    /// J71: no two active bins in one site claim the same walking position.
    ///
    /// **A finding rather than a unique index.** The real bin list has two such
    /// pairs — Brisbane's `I.48.07` and `K.36.05` both at 2592, `K.32.01` and
    /// `K.32.02` both at 2615 — so a unique constraint would refuse the import
    /// of true data. It would also make reordering need a spare value nobody
    /// wants to store: swapping two bins under uniqueness has no legal
    /// intermediate state.
    ///
    /// What it means is ambiguous in a way worth reporting rather than
    /// resolving: `K.32.01` and `K.32.02` are adjacent and were probably typed
    /// once, while `I.48.07` and `K.36.05` are two aisles apart and cannot both
    /// be reached at the same moment. A pick list ordered through either pair is
    /// non-deterministic, which is the practical cost.
    pub fn j71_one_bin_per_pick_position(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let findings = c
            .query(
                "SELECT s.code, a.pick_sequence, a.code, b.code
                   FROM location a
                   JOIN location b
                     ON b.tenant_id = a.tenant_id AND b.site_id = a.site_id
                    AND b.pick_sequence = a.pick_sequence AND b.id > a.id
                   JOIN site s ON s.id = a.site_id
                  WHERE a.pick_sequence IS NOT NULL AND a.active AND b.active
                  ORDER BY s.code, a.pick_sequence",
                &[],
            )?
            .iter()
            .map(|r| {
                finding(Id::J71, "identity_mismatch",
                    format!("site {} puts bins {} and {} both at pick position {}, so a pick \
                             list through either is in no defined order",
                        r.get::<_, String>(0), r.get::<_, String>(2),
                        r.get::<_, String>(3), r.get::<_, i32>(1)))
            })
            .collect();

        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM location WHERE pick_sequence IS NOT NULL AND active",
                &[],
            )?
            .get(0);
        Ok((examined as usize, findings))
    }

    /// incomplete and one that is quietly wrong.
    pub fn j70_no_pick_leaves_package_held_storage_uncounted(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let findings = c
            .query(
                "SELECT m.id::text, m.fulfilment_line_id::text, m.from_package_id::text,
                        m.quantity
                   FROM stock_movement m
                  WHERE m.fulfilment_line_id IS NOT NULL
                    AND m.from_package_id IS NOT NULL
                    AND m.reverses_movement_id IS NULL
                    AND NOT EXISTS (
                        SELECT 1 FROM stock_movement e
                         WHERE e.fulfilment_line_id = m.fulfilment_line_id
                           AND e.to_package_id = m.from_package_id
                           AND e.occurred_at <= m.occurred_at)",
                &[],
            )?
            .iter()
            .map(|r| {
                finding(Id::J70, "short_pick",
                    format!("stock_movement {} takes {} for fulfilment_line {} out of package {}, \
                             which nothing put there for that line: package-held storage, so the \
                             units are picked and picked_quantity does not count them",
                        r.get::<_, String>(0), r.get::<_, i64>(3),
                        r.get::<_, String>(1), r.get::<_, String>(2)))
            })
            .collect();

        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM stock_movement
                  WHERE fulfilment_line_id IS NOT NULL AND from_package_id IS NOT NULL",
                &[],
            )?
            .get(0);
        Ok((examined as usize, findings))
    }

    /// D53. What the four quantities mean together, which no single fold checks.
    ///
    /// Two claims. The first is that a commitment is never covered beyond itself:
    /// `stock_allocation` has no constraint tying the sum of its quantities to
    /// the line it serves, so over-committing is expressible and produces a
    /// negative `uncovered_quantity`. The partial index on `uncovered_quantity > 0`
    /// then silently drops the line, which is correct behaviour for the index and
    /// exactly wrong as a way to find out.
    ///
    /// The second is monotonicity, and **D100 turned it from cheap into
    /// load-bearing**. It used to hold by construction: one rebuild produced all
    /// four from nested state sets over one table, so violating it took a
    /// hand-edited column or a restore. D100 folds `covered_quantity` from
    /// `stock_allocation` and the other three from `stock_movement`, and nothing
    /// ties the two sources together — a pick with no allocation, or an over-pick,
    /// now produces `picked > covered` and no constraint prevents it.
    ///
    /// That is not a defect and D99 says so plainly: D12 makes an allocation
    /// advisory and *"allowed to be wrong"*, and the allocation fold did not make
    /// those cases safe, it made them invisible by discarding a real pick that no
    /// intention row carried. This check is where they become visible instead.
    ///
    /// A finding rather than a CHECK, because over-commitment is a planning error
    /// somebody has to unwind and refusing the write would leave it unrecorded.
    pub fn j56_coverage_is_bounded_and_monotone(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let mut findings = vec![];

        for r in &c.query(
            "SELECT id::text, quantity, covered_quantity FROM fulfilment_line
              WHERE covered_quantity > quantity",
            &[],
        )? {
            findings.push(finding(Id::J56, "commitment_over_covered",
                format!("fulfilment_line {} promises {} and is covered for {}",
                    r.get::<_, String>(0), r.get::<_, i64>(1), r.get::<_, i64>(2))));
        }

        for r in &c.query(
            "SELECT id::text, covered_quantity, picked_quantity,
                    packed_quantity, despatched_quantity
               FROM fulfilment_line
              WHERE NOT (despatched_quantity <= packed_quantity
                     AND packed_quantity <= picked_quantity
                     AND picked_quantity <= covered_quantity)",
            &[],
        )? {
            findings.push(finding(Id::J56, "coverage_not_monotone",
                format!("fulfilment_line {} reports covered {}, picked {}, packed {}, despatched {}, which no sequence of states produces",
                    r.get::<_, String>(0), r.get::<_, i64>(1), r.get::<_, i64>(2),
                    r.get::<_, i64>(3), r.get::<_, i64>(4))));
        }

        let examined: i64 = c.query_one("SELECT count(*) FROM fulfilment_line", &[])?.get(0);
        Ok((examined as usize, findings))
    }
    /// D42's fold, restated rather than re-run, and widened by D51 to the line.
    ///
    /// The register calls for the same shape as J6: the projection equals the
    /// fold of the log in `(occurred_at, recorded_at, id)` order. So this
    /// recomputes the last writer per covered column from `intention_amendment`
    /// and compares it to what is on the row. Calling
    /// `projection_order_rebuild` and diffing would check that the function is
    /// deterministic, which is not the claim.
    ///
    /// # What this can and cannot see
    ///
    /// The fold coalesces onto the row's own value, because D42 makes the
    /// projection *"the original plus the amendments"* and the original arrives
    /// on INSERT. That original is not recorded anywhere separate from the column
    /// it seeds, so a column no amendment has ever set has nothing to be compared
    /// against: whatever is there is the original by construction, and a check
    /// claiming otherwise would be inventing a source.
    ///
    /// This therefore examines every (subject, column) pair some amendment
    /// determines, and reports the size of that set rather than the number of
    /// orders. Where the two differ the projection has drifted; where no
    /// amendment exists there is nothing to drift from. That the base value is
    /// unrecoverable if the column is ever reset is a real property of this
    /// projection and not of the ledger folds, and it is question 137.
    pub fn j46_the_amendment_fold_holds(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        // Every covered value an amendment states, normalised to
        // (subject, column, value) so one query folds both subjects under one
        // ordering. Casting to text on both sides keeps the comparison honest
        // across four column types without a branch per type.
        const SETTINGS: &str = "
            SELECT 'order' AS kind, order_id AS subject, 'promised_from' AS col,
                   new_promised_from::text AS val, occurred_at, recorded_at, id
              FROM intention_amendment
             WHERE order_line_id IS NULL AND new_promised_from IS NOT NULL
            UNION ALL SELECT 'order', order_id, 'promised_to', new_promised_to::text,
                   occurred_at, recorded_at, id FROM intention_amendment
             WHERE order_line_id IS NULL AND new_promised_to IS NOT NULL
            UNION ALL SELECT 'order', order_id, 'required_by', new_required_by::text,
                   occurred_at, recorded_at, id FROM intention_amendment
             WHERE order_line_id IS NULL AND new_required_by IS NOT NULL
            UNION ALL SELECT 'order', order_id, 'state', new_state::text,
                   occurred_at, recorded_at, id FROM intention_amendment
             WHERE order_line_id IS NULL AND new_state IS NOT NULL
            UNION ALL SELECT 'order_line', order_line_id, 'quantity_ordered',
                   new_quantity_ordered::text, occurred_at, recorded_at, id
              FROM intention_amendment
             WHERE order_line_id IS NOT NULL AND new_quantity_ordered IS NOT NULL
            UNION ALL SELECT 'order_line', order_line_id, 'unit_price_minor',
                   new_unit_price_minor::text, occurred_at, recorded_at, id
              FROM intention_amendment
             WHERE order_line_id IS NOT NULL AND new_unit_price_minor IS NOT NULL
            UNION ALL SELECT 'order_line', order_line_id, 'price_basis_quantity',
                   new_price_basis_quantity::text, occurred_at, recorded_at, id
              FROM intention_amendment
             WHERE order_line_id IS NOT NULL AND new_price_basis_quantity IS NOT NULL
            UNION ALL SELECT 'order_line', order_line_id, 'line_state',
                   new_line_state::text, occurred_at, recorded_at, id
              FROM intention_amendment
             WHERE order_line_id IS NOT NULL AND new_line_state IS NOT NULL";

        // What the row actually holds, in the same shape.
        const ACTUAL: &str = "
            SELECT 'order' AS kind, id AS subject, 'promised_from' AS col,
                   promised_from::text AS val FROM \"order\"
            UNION ALL SELECT 'order', id, 'promised_to', promised_to::text FROM \"order\"
            UNION ALL SELECT 'order', id, 'required_by', required_by::text FROM \"order\"
            UNION ALL SELECT 'order', id, 'state', state::text FROM \"order\"
            UNION ALL SELECT 'order_line', id, 'quantity_ordered',
                   quantity_ordered::text FROM order_line
            UNION ALL SELECT 'order_line', id, 'unit_price_minor',
                   unit_price_minor::text FROM order_line
            UNION ALL SELECT 'order_line', id, 'price_basis_quantity',
                   price_basis_quantity::text FROM order_line
            UNION ALL SELECT 'order_line', id, 'line_state',
                   line_state::text FROM order_line";

        let winner = format!(
            "SELECT DISTINCT ON (kind, subject, col) kind, subject, col, val
               FROM ({SETTINGS}) s
              ORDER BY kind, subject, col, occurred_at DESC, recorded_at DESC, id DESC"
        );

        let examined: i64 = c
            .query_one(format!("SELECT count(*) FROM ({winner}) w").as_str(), &[])?
            .get(0);

        let rows = c.query(
            format!(
                "SELECT w.kind, w.subject::text, w.col, coalesce(a.val, '-'), w.val
                   FROM ({winner}) w
                   JOIN ({ACTUAL}) a
                     ON a.kind = w.kind AND a.subject = w.subject AND a.col = w.col
                  WHERE a.val IS DISTINCT FROM w.val"
            )
            .as_str(),
            &[],
        )?;

        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J46, "projection_drift",
                    format!("{} {} holds {} = {} and the amendments fold to {}",
                        r.get::<_, String>(0), r.get::<_, String>(1),
                        r.get::<_, String>(2), r.get::<_, String>(3),
                        r.get::<_, String>(4)))
            })
            .collect();
        Ok((examined as usize, findings))
    }
    /// D51. What making removal expressible makes possible.
    ///
    /// A removed line keeps its row, which is the point: `fulfilment_line`
    /// references it and a commitment that was made stays a fact. The cost is a
    /// state the model could not previously reach, where the floor is still
    /// committed to picking something the customer has withdrawn.
    ///
    /// This is a finding rather than a constraint on purpose, and the ordering is
    /// why. The customer removes the line at 09:00; the picker is already holding
    /// the carton. A CHECK would refuse the removal, which is refusing to record
    /// something that happened, and D5 exists to refuse that trade. So the
    /// removal lands, the commitment stands, and the disagreement goes to the
    /// queue with both halves named.
    ///
    /// A cancelled fulfilment is excluded because it is the resolution, not the
    /// problem.
    ///
    /// The population is every commitment rather than every removed line, so this
    /// reports what it inspected instead of going vacuous on a fixture with
    /// nothing wrong in it.
    pub fn j55_no_commitment_against_a_removed_line(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one("SELECT count(*) FROM fulfilment_line", &[])?
            .get(0);
        let rows = c.query(
            "SELECT l.id::text, f.id::text, fl.quantity, f.state::text
               FROM order_line l
               JOIN fulfilment_line fl ON fl.order_line_id = l.id
               JOIN fulfilment f ON f.id = fl.fulfilment_id
              WHERE l.line_state = 'removed' AND f.state <> 'cancelled'",
            &[],
        )?;
        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J55, "commitment_against_removed_line",
                    format!("fulfilment {} is {} and still commits {} against order_line {}, which has been removed",
                        r.get::<_, String>(1), r.get::<_, String>(3),
                        r.get::<_, i64>(2), r.get::<_, String>(0)))
            })
            .collect();
        Ok((examined as usize, findings))
    }

    /// J66. D95, and the number the inbound walk could not get.
    ///
    /// Every maintainer is a full-tenant fold that nothing on the write path may
    /// call, so a projected number is always behind the ledger by however long it
    /// has been since the scheduler ran. That is D25 working rather than a defect
    /// — the ledger is the truth and `stock` is a cache of it — but until D95
    /// nobody could say by how much, and nothing said how much was too much.
    ///
    /// **Two failures, and the second is the one that hides.** A projection that
    /// ran too long ago is visibly stale. A projection that has *never* run for a
    /// tenant has no row at all, and an anti-join written the obvious way passes
    /// over it in silence — which is this register's oldest lesson applied to its
    /// newest table.
    ///
    /// A finding rather than a block, per D8: a stale cache never stops the floor.
    pub fn j66_no_projection_is_staler_than_it_declared(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        // Every (tenant, step) pair that ought to have a run behind it.
        let examined: i64 = c
            .query_one(
                "SELECT (SELECT count(*) FROM tenant) * (SELECT count(*) FROM projection_step)",
                &[],
            )?
            .get(0);

        let rows = c.query(
            "SELECT t.slug, s.function_name, s.freshness_bound::text,
                    f.last_run_at IS NULL AS never_ran,
                    coalesce((now() - f.last_run_at)::text, '')
               FROM tenant t
               CROSS JOIN projection_step s
               LEFT JOIN projection_freshness f
                      ON f.tenant_id = t.id AND f.function_name = s.function_name
              WHERE f.last_run_at IS NULL
                 OR now() - f.last_run_at > s.freshness_bound",
            &[],
        )?;

        let findings = rows
            .iter()
            .map(|r| {
                if r.get::<_, bool>(3) {
                    finding(Id::J66, "projection_never_run",
                        format!("{} has never run for tenant {}, so every number it maintains is an initial value rather than a fold",
                            r.get::<_, String>(1), r.get::<_, String>(0)))
                } else {
                    finding(Id::J66, "projection_stale",
                        format!("{} last ran {} ago for tenant {}, past its declared bound of {}",
                            r.get::<_, String>(1), r.get::<_, String>(4),
                            r.get::<_, String>(0), r.get::<_, String>(2)))
                }
            })
            .collect();
        Ok((examined as usize, findings))
    }

    /// J67. D96, and the half of the collapse a function cannot hold.
    ///
    /// `asserted_unit_collapse` refuses a package whose SSCC disagrees with the
    /// declared one, which settles it at the moment of writing. **`package.sscc`
    /// is a projection of `package_event`**, so a relabel recorded afterwards
    /// moves one side of a comparison that has already been made — the same
    /// shape as J57, where a config can be edited after a movement named it.
    ///
    /// Only where both sides carry one. A pallet arriving with no readable label
    /// is D21's degradation ladder working rather than a finding, and a claim that
    /// declared no SSCC is a claim we can still receive against.
    pub fn j67_a_collapse_is_evidenced_by_its_licence_plate(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM asserted_unit u JOIN package p ON p.id = u.resolved_package_id
                  WHERE u.sscc IS NOT NULL AND p.sscc IS NOT NULL",
                &[],
            )?
            .get(0);

        let rows = c.query(
            "SELECT u.id::text, u.sscc, p.id::text, p.sscc
               FROM asserted_unit u
               JOIN package p ON p.id = u.resolved_package_id
              WHERE u.sscc IS NOT NULL AND p.sscc IS NOT NULL AND u.sscc <> p.sscc",
            &[],
        )?;

        let findings = rows
            .iter()
            .map(|r| {
                finding(Id::J67, "collapse_sscc_disagrees",
                    format!("asserted unit {} declares SSCC {} and the package it was collapsed onto, {}, now carries {} -- the licence plate moved after the match was made",
                        r.get::<_, String>(0), r.get::<_, String>(1),
                        r.get::<_, String>(2), r.get::<_, String>(3)))
            })
            .collect();
        Ok((examined as usize, findings))
    }

    /// J47. D43 wrote it; D96 supplied the column that made it expressible.
    ///
    /// D43 decided **one `goods_receipt` per delivery per demand document**, and
    /// both standards put the order level *above* the physical levels — so every
    /// pallet descends from exactly one order node and carries goods for exactly
    /// one purchase order. A pallet spanning two means the tree was built wrong,
    /// not that a supplier did something unusual, which is why this is worth a
    /// finding rather than a shrug.
    ///
    /// **The scope is the part that took a decision to become sayable.** It is not
    /// every node: a shipment node sitting above two order nodes spans two POs and
    /// is correct. It is every node we read as *physical*, which is what D96's
    /// `resolved_physical` records — so J47 was blocked less on the collapse it
    /// named than on the reading of the author's level vocabulary underneath it.
    ///
    /// Only the resolved side. `raw_po_reference` is a string in the author's
    /// words, and two spellings of one purchase order would read here as two
    /// orders — a finding about our parsing dressed as a finding about their
    /// pallet.
    pub fn j47_a_pallet_belongs_to_one_purchase_order(
        c: &mut Client,
    ) -> Result<(usize, Vec<Finding>), postgres::Error> {
        const SPAN: &str = "
            WITH RECURSIVE descend AS (
                SELECT u.id AS root, u.id AS node
                  FROM asserted_unit u WHERE u.resolved_physical
                UNION ALL
                SELECT d.root, c.id FROM descend d
                  JOIN asserted_unit c ON c.parent_asserted_unit_id = d.node
            ),
            span AS (
                SELECT d.root, u.resolved_package_id, pol.purchase_order_id
                  FROM descend d
                  JOIN asserted_unit u ON u.id = d.root
                  JOIN asserted_unit_content cc ON cc.asserted_unit_id = d.node
                  JOIN purchase_order_line pol
                    ON pol.id = cc.resolved_purchase_order_line_id
                 GROUP BY 1, 2, 3
            )";

        let examined: i64 = c
            .query_one(
                &format!(
                    "{SPAN}
                     SELECT (SELECT count(DISTINCT root) FROM span)
                          + (SELECT count(DISTINCT resolved_package_id) FROM span
                              WHERE resolved_package_id IS NOT NULL)"
                ),
                &[],
            )?
            .get(0);

        let mut findings = vec![];

        let subtrees = c.query(
            &format!(
                "{SPAN}
                 SELECT root::text, count(*)::bigint FROM span
                  GROUP BY root HAVING count(*) > 1"
            ),
            &[],
        )?;
        for r in &subtrees {
            findings.push(finding(Id::J47, "subtree_spans_purchase_orders",
                format!("asserted unit {} is a physical node whose contents resolve to {} purchase orders, so the declared tree puts one pallet under two demand documents",
                    r.get::<_, String>(0), r.get::<_, i64>(1))));
        }

        let packages = c.query(
            &format!(
                "{SPAN}
                 SELECT resolved_package_id::text, count(DISTINCT purchase_order_id)::bigint
                   FROM span WHERE resolved_package_id IS NOT NULL
                  GROUP BY resolved_package_id HAVING count(DISTINCT purchase_order_id) > 1"
            ),
            &[],
        )?;
        for r in &packages {
            findings.push(finding(Id::J47, "package_spans_purchase_orders",
                format!("package {} carries goods resolving to {} purchase orders, so no single goods_receipt can account for it",
                    r.get::<_, String>(0), r.get::<_, i64>(1))));
        }

        Ok((examined as usize, findings))
    }
}
