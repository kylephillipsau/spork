//! The structural invariants, as code.
//!
//! `docs/invariants.md` says what this becomes: "each entry is a test carrying
//! its own metadata, this file is generated from the suite, and CI fails when
//! the generated file differs from the committed one. The identifier is then a
//! constant in code, so allocating the same number twice stops being a mistake
//! anyone can make and becomes a compile error."
//!
//! The identifier half is done here. [`Id`] is an enum, so writing `S8` twice
//! will not compile, and [`spec`] matches on it exhaustively, so adding a
//! variant without describing it will not compile either. That is the numbering
//! collision the register already suffered once, made impossible rather than
//! merely noticed.
//!
//! # Vacuity is the point
//!
//! The register's sharpest observation is that a check asserting an absence
//! passes when its population is empty. Thirty-six of the eighty-six do that.
//! So a suite that reports pass or fail is lying by omission: it cannot tell you
//! whether a check examined ten thousand objects and found nothing wrong, or
//! examined nothing at all and had nothing to find.
//!
//! [`Outcome`] therefore carries how many objects were examined, and the runner
//! reports `VACUOUS` separately from `PASS`. A vacuous result is not a failure.
//! It is a result that proves nothing, and knowing which is which is the
//! difference between a suite that guards the design and one that reports
//! success on the day it stops being checked.

pub mod golden;
pub mod jobs;

use postgres::Client;
use std::fmt;

/// Connect and take the suite's exclusive lock, releasing it when the returned
/// client is dropped.
///
/// The test binaries share one database and several of them write to it: the
/// replay-order properties insert and rebuild, and D68's no-op property runs a
/// rebuild twice and asserts the second one touched nothing. Cargo runs tests
/// within a binary on several threads and runs the binaries themselves in
/// parallel, so those two facts are in direct conflict — **a rebuild is only a
/// no-op if nobody else is writing.**
///
/// Measured on the commit before D72: five parallel runs of the job-asserted
/// binary, three of them red, and the same six tests green every time on one
/// thread. It had nothing to do with the schema and everything to do with the
/// harness, and it means a green suite was partly a scheduling accident.
///
/// A session-level advisory lock is the smallest fix that survives both kinds of
/// parallelism — it is database-wide, so it serialises across binaries too, which
/// `--test-threads=1` would not.
pub fn connect_exclusive(url: &str) -> Client {
    let mut client = Client::connect(url, postgres::NoTls).expect("connect");
    client
        .execute("SELECT pg_advisory_lock(8720072)", &[])
        .expect("take the suite lock");
    client
}


/// Every structural invariant. Duplicating a variant is a compile error, which
/// is the property `docs/invariants.md` asked for.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[allow(clippy::upper_case_acronyms)]
pub enum Id {
    S1, S2, S3, S4, S5, S6, S7, S8, S9, S10,
    S11, S12, S13, S14, S15, S16, S17, S18, S19, S20,
    S21, S22, S23, S24, S25, S26, S27, S28, S29, S30,
    S31, S32, S33, S34, S35, S36, S37, S38, S39, S40,
    S41, S42, S43, S44, S45, S46, S47, S48, S49, S50, S51, S52, S53, S54,
    S55, S56,
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Every invariant in the structural class, in register order.
pub const ALL: &[Id] = &[
    Id::S1, Id::S2, Id::S3, Id::S4, Id::S5, Id::S6, Id::S7, Id::S8, Id::S9, Id::S10,
    Id::S11, Id::S12, Id::S13, Id::S14, Id::S15, Id::S16, Id::S17, Id::S18, Id::S19, Id::S20,
    Id::S21, Id::S22, Id::S23, Id::S24, Id::S25, Id::S26, Id::S27, Id::S28, Id::S29, Id::S30,
    Id::S31, Id::S32, Id::S33, Id::S34, Id::S35, Id::S36, Id::S37, Id::S38,
    Id::S39, Id::S40, Id::S41, Id::S42, Id::S43, Id::S44, Id::S45, Id::S46,
    Id::S47, Id::S48, Id::S49, Id::S50, Id::S51, Id::S52, Id::S53, Id::S54,
    Id::S55, Id::S56,
];

pub struct Invariant {
    pub id: Id,
    /// What the register says, abbreviated to the claim itself.
    pub statement: &'static str,
    /// The decisions that own it.
    pub owners: &'static str,
    /// True when the check asserts an absence, and therefore passes on an empty
    /// population. The register's vacuity column.
    pub asserts_absence: bool,
    pub check: Check,
}

/// What a pending check is waiting for.
///
/// This was a string, and the string was the defect. *"observation arrives with
/// the measurement migration"* is a claim about the schema, stored in the one
/// place the schema cannot reach, so it went on reading as a deliberate gap for
/// three migrations after `observation` arrived. D49 named that failure mode for
/// S5 and migration 12 fixed exactly one instance of it.
///
/// So a blocker is now a thing rather than a sentence, and S42 checks that it is
/// still blocking. The reason text is rendered from this rather than written
/// beside it, which is the same move D25 makes everywhere else: derive the claim
/// from the fact instead of storing both and hoping they agree.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Blocker {
    /// A database object that must still be absent: a table, a view, a function,
    /// or a `table.column`. **S42 asserts it really is absent**, so a check that
    /// names something which has since been built fails rather than dozing.
    Absent(&'static str),
    /// Something the database cannot see: a code-side registry, an AST pass, a
    /// question nobody has answered. Not checkable from SQL, and counted
    /// separately so the size of the unverifiable set is a number in the report
    /// rather than a silence.
    Elsewhere(&'static str),
}

impl fmt::Display for Blocker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Blocker::Absent(name) => write!(f, "{name} does not exist yet"),
            Blocker::Elsewhere(what) => write!(f, "{what} does not exist yet"),
        }
    }
}

/// Renders a blocker list as the sentence the report used to store.
pub fn reason(blockers: &[Blocker]) -> String {
    blockers
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

pub enum Check {
    /// The objects this inspects do not exist yet. Carries what it is waiting
    /// for, so the report says why rather than leaving a silent gap — and so
    /// S42 can tell whether the waiting is still true.
    Pending(&'static [Blocker]),
    Run(fn(&mut Client) -> Result<Outcome, postgres::Error>),
}

/// The result of running a check.
///
/// `examined` is not decoration. It is what separates a check that looked at
/// something from one that had nothing to look at.
pub struct Outcome {
    pub examined: usize,
    pub violations: Vec<String>,
}

impl Outcome {
    pub fn new(examined: usize, violations: Vec<String>) -> Self {
        Self { examined, violations }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Examined at least one object and found no violation.
    Pass,
    /// Passed, but examined nothing. Proves the absence of nothing.
    Vacuous,
    Fail,
    /// Not implementable yet.
    Pending,
}

/// Every invariant's metadata and check, matched exhaustively so that adding an
/// [`Id`] without describing it fails to compile.
pub fn spec(id: Id) -> Invariant {
    use Id::*;
    match id {
        S1 => Invariant { id, owners: "D4, D12, D20, D24", asserts_absence: false,
            statement: "Every stock key column except tenant_id and item_id appears on stock_movement as a from_/to_ pair, under an explicit name map",
            check: Check::Run(checks::s1_stock_key_pairs_on_movement) },
        S2 => Invariant { id, owners: "D24, Q91", asserts_absence: false,
            statement: "Every table naming a stock cell carries the complete column set",
            check: Check::Run(checks::s2_stock_cell_tables_carry_whole_key) },
        S3 => Invariant { id, owners: "D10 corrected, D16", asserts_absence: true,
            statement: "Every demand/cause CHECK on a grouping table is <= 1, never = 1",
            check: Check::Run(checks::s3_demand_checks_are_at_most_one) },
        S4 => Invariant { id, owners: "D21, D26", asserts_absence: false,
            statement: "Every table registers exactly one provenance and one role",
            check: Check::Pending(&[Blocker::Elsewhere("the code-side table registry")]) },
        S5 => Invariant { id, owners: "D25", asserts_absence: false,
            statement: "The three-way diff: every @projection column is registered, every registry row names a live column and a live function, and every projection_%_rebuild function has at least one registry row",
            check: Check::Run(checks::s5_the_projection_registry_agrees) },
        S6 => Invariant { id, owners: "D25", asserts_absence: false,
            statement: "Every fact table has no UPDATE and no DELETE granted to the app role",
            check: Check::Run(checks::s6_fact_tables_are_append_only) },
        S7 => Invariant { id, owners: "D25", asserts_absence: false,
            statement: "Every trigger maps to a registered projection. No trigger implements rules, validation, defaults or cascades",
            check: Check::Run(checks::s7_no_unregistered_triggers) },
        S8 => Invariant { id, owners: "D18, D25", asserts_absence: false,
            statement: "Every RLS-protected table with a SECURITY DEFINER maintainer has FORCE ROW LEVEL SECURITY",
            check: Check::Run(checks::s8_rls_tables_force_rls) },
        S9 => Invariant { id, owners: "D19 amended", asserts_absence: false,
            statement: "Exactly three RLS shapes exist, selected by category. The shared-reference shape is a read/write pair, per D55",
            check: Check::Run(checks::s9_three_rls_shapes) },
        S10 => Invariant { id, owners: "Principle 3", asserts_absence: true,
            statement: "The model contains no jsonb column",
            check: Check::Run(checks::s10_no_jsonb) },
        S11 => Invariant { id, owners: "D22", asserts_absence: true,
            statement: "No policy column names a field, operator, comparator or expression",
            check: Check::Run(checks::s11_no_policy_logic_columns) },
        S12 => Invariant { id, owners: "D22", asserts_absence: false,
            statement: "policy_binding has no num_nonnulls CHECK; its uniqueness is NULLS NOT DISTINCT and non-deferrable",
            check: Check::Run(checks::s12_policy_binding_has_no_num_nonnulls) },
        S13 => Invariant { id, owners: "D22", asserts_absence: false,
            statement: "The policy_kind enum, the %_policy table set and the Rust PolicyKind registry are the same set",
            check: Check::Run(checks::s13_policy_kind_sets_agree) },
        S14 => Invariant { id, owners: "D22", asserts_absence: false,
            statement: "Every %_policy table has its kind CHECK, composite FK and effective-range exclusion constraint",
            check: Check::Run(checks::s14_policy_tables_have_the_template) },
        S15 => Invariant { id, owners: "D22", asserts_absence: false,
            statement: "Tenancy is index 0 of every kind's DIMENSIONS const",
            check: Check::Run(checks::s15_tenancy_outranks_everything) },
        S16 => Invariant { id, owners: "D22", asserts_absence: false,
            statement: "Every consuming column is named <kind>_policy_id and is an FK to the value table",
            check: Check::Run(checks::s16_consuming_columns_are_named_by_kind) },
        S17 => Invariant { id, owners: "D21", asserts_absence: true,
            statement: "Every foreign key from a non-assertion table to an assertion table is nullable",
            check: Check::Run(checks::s17_a_reference_to_a_claim_is_optional) },
        S18 => Invariant { id, owners: "D21, D25", asserts_absence: true,
            statement: "No table in the assertion set has a status or state column",
            check: Check::Run(checks::s18_no_claim_carries_our_status) },
        S19 => Invariant { id, owners: "D5, D11, D25", asserts_absence: true,
            statement: "Every fact row has a client_event FK, and fact.recorded_by_id agrees with client_event",
            check: Check::Run(checks::s19_fact_tables_reference_client_event) },
        S20 => Invariant { id, owners: "D26", asserts_absence: false,
            statement: "Every generated ext_ table matches its scheme's compiled fields and has RLS with FORCE",
            check: Check::Run(checks::s20_generated_tables_have_forced_rls) },
        S21 => Invariant { id, owners: "D23", asserts_absence: true,
            statement: "Every application metric-code literal appears in the reserved seed",
            check: Check::Pending(&[Blocker::Elsewhere("application code emitting metric-code literals")]) },
        S22 => Invariant { id, owners: "Principle 5", asserts_absence: true,
            statement: "No canonical unit has a non-zero offset or a factor other than 1/1",
            check: Check::Run(checks::s22_canonical_units_are_identity) },
        S23 => Invariant { id, owners: "Principle 6, D22", asserts_absence: true,
            statement: "No resolver call appears inside a loop",
            check: Check::Pending(&[Blocker::Elsewhere("an AST pass over the application crates")]) },
        S24 => Invariant { id, owners: "D24 supply side", asserts_absence: false,
            statement: "expected_supply has one partial unique index per provenance arm, and no unique index over (item_id, owner_id, status_id)",
            check: Check::Run(checks::s24_one_unique_index_per_arm) },
        S25 => Invariant { id, owners: "D24 supply side", asserts_absence: false,
            statement: "Availability indexes on stock and expected_supply both carry owner_id and status_id in the key",
            check: Check::Run(checks::s25_availability_indexes_carry_owner_and_status) },
        S26 => Invariant { id, owners: "D24 supply side", asserts_absence: false,
            statement: "expected_supply carries at most five maintained quantity columns; a sixth requires a recorded decision",
            check: Check::Run(checks::s26_five_maintained_quantities) },
        S27 => Invariant { id, owners: "D26", asserts_absence: true,
            statement: "The outbox source reference is never dereferenced",
            check: Check::Pending(&[Blocker::Elsewhere("the query register")]) },
        S28 => Invariant { id, owners: "D24, Q91", asserts_absence: false,
            statement: "Exactly one foreign key targets stock(id), and it is stock_allocation.stock_id",
            check: Check::Run(checks::s28_one_fk_targets_stock) },
        S29 => Invariant { id, owners: "D26, D25, Q91", asserts_absence: true,
            statement: "stock and package_content carry neither the attachable nor the subscribable capability flag",
            check: Check::Pending(&[Blocker::Elsewhere("the code-side table registry")]) },
        S30 => Invariant { id, owners: "D25, Q91", asserts_absence: false,
            statement: "Projection tables have no DELETE granted to the app role",
            check: Check::Run(checks::s30_projections_have_no_delete) },
        S31 => Invariant { id, owners: "Principle 3, Q89", asserts_absence: true,
            statement: "Every identification datum on activity_event is a typed column",
            check: Check::Pending(&[Blocker::Absent("activity_event")]) },
        S32 => Invariant { id, owners: "D18/J20, D25, Q89", asserts_absence: false,
            statement: "activity_event is range-partitioned on occurred_at with local indexes from the first migration",
            check: Check::Pending(&[Blocker::Absent("activity_event")]) },
        S33 => Invariant { id, owners: "D20, Q90", asserts_absence: true,
            statement: "No party.gs1_company_prefix is all-zero or absent-yet-used; SSCC issuance is gated on an explicit number_range row",
            check: Check::Pending(&[Blocker::Absent("number_range")]) },
        S34 => Invariant { id, owners: "D31", asserts_absence: false,
            statement: "For every retention_floor row, live data or the archive index reaches back to now() - minimum_age",
            check: Check::Pending(&[Blocker::Absent("retention_floor")]) },
        S35 => Invariant { id, owners: "D43, built by D96", asserts_absence: true,
            statement: "Every asserted_unit node read as non-physical has a NULL sscc and no resolved_package_id, and both halves are CHECKs rather than a job",
            check: Check::Run(checks::s35_a_document_node_is_not_a_thing) },
        S36 => Invariant { id, owners: "D45", asserts_absence: true,
            statement: "No table in the receipt set carries a stored received quantity, variance or accumulator column",
            check: Check::Run(checks::s36_no_receipt_accumulators) },
        S37 => Invariant { id, owners: "D46", asserts_absence: false,
            statement: "location.zone_id is a composite FK including site_id, and zone carries the matching unique key",
            check: Check::Run(checks::s37_location_zone_fk_is_composite) },
        S38 => Invariant { id, owners: "D8, D47", asserts_absence: false,
            statement: "stock_movement.reverses_movement_id is a self-referencing FK, and the two in-row correction CHECKs are present",
            check: Check::Run(checks::s38_the_correction_link_is_constrained) },
        S39 => Invariant { id, owners: "D48", asserts_absence: false,
            statement: "intention_amendment.revision_class is NOT NULL and carries no default, and the correction-reason CHECK is present",
            check: Check::Run(checks::s39_amendments_state_their_class) },
        S40 => Invariant { id, owners: "D40, D50", asserts_absence: true,
            statement: "No table in the order set carries a stored line total, extended amount or tax column",
            check: Check::Run(checks::s40_no_order_totals) },
        S41 => Invariant { id, owners: "D42, D51", asserts_absence: false,
            statement: "Every intention_amendment covered column is named by both the subject CHECK and the changes-something CHECK, and the line FK is composite through order_id",
            check: Check::Run(checks::s41_amendments_name_one_subject) },
        S42 => Invariant { id, owners: "D49, D52", asserts_absence: false,
            statement: "Every database object a pending check names as its blocker is actually absent, across both classes",
            check: Check::Run(checks::s42_pending_checks_are_still_blocked) },
        S43 => Invariant { id, owners: "D25, D53", asserts_absence: false,
            statement: "Every projection_rebuild row names a function whose body actually writes the column it claims to maintain. S5's fourth leg",
            check: Check::Run(checks::s43_a_registered_maintainer_writes_its_column) },
        S44 => Invariant { id, owners: "D15, D25, D53", asserts_absence: true,
            statement: "No table in the fulfilment set carries a stored progress, completion or rollup status column",
            check: Check::Run(checks::s44_no_stored_fulfilment_rollup) },
        S45 => Invariant { id, owners: "D25, D54", asserts_absence: true,
            statement: "On every table the application may write, each non-projection column is in one of its grant lists. A column left out can never hold a value",
            check: Check::Run(checks::s45_column_grants_are_complete) },
        S46 => Invariant { id, owners: "D18, D19, D55", asserts_absence: false,
            statement: "Every shared-reference table pairs a FOR SELECT read policy with a write policy that declares an explicit WITH CHECK. A policy with no WITH CHECK inherits its USING, which on a shared table authorises writing what it meant to allow reading",
            check: Check::Run(checks::s46_shared_tables_guard_their_writes) },
        S47 => Invariant { id, owners: "D57", asserts_absence: true,
            statement: "No column defaults to gen_random_uuid(). Identifiers are time-ordered, so inserts append at the index's right edge rather than scattering",
            check: Check::Run(checks::s47_identifiers_are_time_ordered) },
        S48 => Invariant { id, owners: "D40, D58", asserts_absence: true,
            statement: "No table in the movement set carries a landed cost, valuation or cost-tax column. Same boundary S40 draws for price, on the other side of it",
            check: Check::Run(checks::s48_no_cost_derivations) },
        S49 => Invariant { id, owners: "D35, D64", asserts_absence: false,
            statement: "Every projection maintainer is a declared step exactly once and every declared step is a live function. S5's third leg over the wider set, because a step is not a rebuild and owns no column",
            check: Check::Run(checks::s49_every_maintainer_is_a_declared_step) },
        S50 => Invariant { id, owners: "D22, D70", asserts_absence: false,
            statement: "Every table a resolution reads contributes to policy_epoch. A resolver input the epoch cannot see is a cached answer that never goes stale, which is the failure question 80 named",
            check: Check::Run(checks::s50_the_epoch_sees_every_resolver_input) },

        S51 => Invariant { id, owners: "D55, D79", asserts_absence: true,
            statement: "Every foreign key to a strictly tenant-owned table carries that table's tenant_id, and every such table offers the (id, tenant_id) key to carry it. RLS filters reads; only the key refuses a write naming another tenant's row",
            check: Check::Run(checks::s51_a_reference_cannot_cross_a_tenant) },
        S52 => Invariant { id, owners: "D82, D85", asserts_absence: true,
            statement: "Every %_policy value table is named by both policy_candidate and policy_value. A kind whose table neither function knows resolves to nothing at all -- not to a default, not to an error -- and every structural check still passes",
            check: Check::Run(checks::s52_the_resolver_knows_every_value_table) },
        S53 => Invariant { id, owners: "D18, D55, D94", asserts_absence: true,
            statement: "No SECURITY DEFINER function is owned by a SUPERUSER or BYPASSRLS role. A definer owned by one runs outside row-level security, so handing the application such a function hands it a tenancy escape",
            check: Check::Run(checks::s53_a_definer_stays_inside_row_security) },
        S54 => Invariant { id, owners: "D89, D90, D94", asserts_absence: false,
            statement: "Every mediated_write column: the app has no UPDATE, the mediation owner does, and the named function is a SECURITY DEFINER owned by that role. Both directions, so a column protected by nothing and a function protecting nothing are each visible",
            check: Check::Run(checks::s54_a_mediated_write_is_actually_mediated) },
        S55 => Invariant { id, owners: "D101, D104", asserts_absence: false,
            statement: "Every projected column is written only by its registered family, and a column with more than one writer is on the declared multi-writer allowlist with exactly those co-owners. S43's converse",
            check: Check::Run(checks::s55_a_projected_column_has_one_writer) },
        S56 => Invariant { id, owners: "D104", asserts_absence: false,
            statement: "Every VOLATILE projection_% function body carries a last-changed marker naming the migration that last replaced it. A CREATE OR REPLACE that pastes an older body keeps the older marker",
            check: Check::Run(checks::s56_a_maintainer_says_when_it_last_changed) },
    }
}

pub fn run(client: &mut Client, id: Id) -> Result<(Verdict, Outcome), postgres::Error> {
    let inv = spec(id);
    match inv.check {
        Check::Pending(_) => Ok((Verdict::Pending, Outcome::new(0, vec![]))),
        Check::Run(f) => {
            let outcome = f(client)?;
            // Vacuity is a property of the population, not of the wording.
            //
            // The register frames it as belonging to entries that assert an
            // absence, because an anti-join over an empty table passes. That is
            // true and it is not the general case: *any* universally quantified
            // check proves nothing over an empty population. "Every RLS table
            // forces RLS" passes just as emptily when there are no RLS tables as
            // "no table has a jsonb column" does when there are no tables.
            //
            // So the verdict comes from what was examined rather than from how
            // the entry was phrased. `asserts_absence` stays as the register's
            // own column, which is metadata about the claim, not about this run.
            let verdict = if !outcome.violations.is_empty() {
                Verdict::Fail
            } else if outcome.examined == 0 {
                Verdict::Vacuous
            } else {
                Verdict::Pass
            };
            Ok((verdict, outcome))
        }
    }
}

pub mod checks {
    use super::Outcome;
    use postgres::Client;

    /// Tables the application owns. Used where a check needs a denominator that
    /// is not "everything in the schema".
    const APP_SCHEMA: &str = "public";

    /// The stock key, and what each column is called on `stock_movement`.
    ///
    /// The register says "under an explicit name map" because the names differ:
    /// the cell says `holder_location_id` and the ledger says
    /// `from_location_id`/`to_location_id`. A check that guessed the mapping
    /// would either miss a renamed column or invent a violation.
    const STOCK_KEY_NAME_MAP: &[(&str, &str)] = &[
        ("holder_location_id", "location_id"),
        ("holder_package_id", "package_id"),
        ("lot_id", "lot_id"),
        ("status_id", "status_id"),
        ("owner_id", "owner_id"),
    ];

    fn columns_of(c: &mut Client, table: &str) -> Result<Vec<String>, postgres::Error> {
        Ok(c.query(
            "SELECT column_name FROM information_schema.columns
              WHERE table_schema = $1 AND table_name = $2",
            &[&APP_SCHEMA, &table],
        )?
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect())
    }

    /// S1. The check that caught D20 breaking D12: `owner_id` joined the stock
    /// key and `stock_movement` never got the pair, so "each column is
    /// rebuildable from its own source" was false and ownership transfer without
    /// physical movement was inexpressible.
    pub fn s1_stock_key_pairs_on_movement(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let stock = columns_of(c, "stock")?;
        if stock.is_empty() {
            return Ok(Outcome::new(0, vec![]));
        }
        let movement = columns_of(c, "stock_movement")?;
        let mut violations = vec![];
        let mut examined = 0;
        for (cell, ledger) in STOCK_KEY_NAME_MAP {
            if !stock.iter().any(|c| c == cell) {
                violations.push(format!("stock has no {cell}, so the name map is stale"));
                continue;
            }
            examined += 1;
            for side in ["from", "to"] {
                let want = format!("{side}_{ledger}");
                if !movement.contains(&want) {
                    violations.push(format!(
                        "stock.{cell} has no {want} on stock_movement: that column is not rebuildable"
                    ));
                }
            }
        }
        Ok(Outcome::new(examined, violations))
    }

    /// S2. The "or FK to stock.id" disjunction was removed: under a reapable
    /// stock the two are not equivalent, so a table naming a cell must carry the
    /// whole column set rather than a reference.
    pub fn s2_stock_cell_tables_carry_whole_key(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let key: Vec<&str> = STOCK_KEY_NAME_MAP.iter().map(|(cell, _)| *cell).collect();
        let tables = c.query(
            "SELECT DISTINCT table_name FROM information_schema.columns
              WHERE table_schema = $1 AND column_name = 'holder_location_id'
                AND table_name <> 'stock'",
            &[&APP_SCHEMA],
        )?;
        let mut violations = vec![];
        for row in &tables {
            let t: String = row.get(0);
            let cols = columns_of(c, &t)?;
            for k in &key {
                if !cols.iter().any(|c| c == k) {
                    violations.push(format!("{t} names a stock cell but has no {k}"));
                }
            }
        }
        Ok(Outcome::new(tables.len(), violations))
    }

    /// S6. Append-only is a grant, not a convention. Fact tables are identified
    /// by their table comment until the code-side registry exists.
    pub fn s6_fact_tables_are_append_only(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let facts = c.query(
            "SELECT c.relname FROM pg_class c
              WHERE c.relnamespace = to_regnamespace($1) AND c.relkind = 'r'
                AND obj_description(c.oid, 'pg_class') LIKE 'FACT%'",
            &[&APP_SCHEMA],
        )?;
        let mut violations = vec![];
        for row in &facts {
            let t: String = row.get(0);
            let bad = c.query(
                "SELECT grantee, privilege_type FROM information_schema.role_table_grants
                  WHERE table_name = $1 AND privilege_type IN ('UPDATE','DELETE')
                    AND grantee NOT IN ('postgres','PUBLIC')",
                &[&t],
            )?;
            for b in &bad {
                violations.push(format!(
                    "{t} grants {} to {}: a fact is what happened",
                    b.get::<_, String>(1),
                    b.get::<_, String>(0)
                ));
            }
        }
        Ok(Outcome::new(facts.len(), violations))
    }

    /// S19, structural half. The row-level agreement between
    /// `fact.recorded_by_id` and `client_event.recorded_by_id` needs data and
    /// belongs to the job-asserted class.
    pub fn s19_fact_tables_reference_client_event(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        let facts = c.query(
            "SELECT c.relname FROM pg_class c
              WHERE c.relnamespace = to_regnamespace($1) AND c.relkind = 'r'
                AND obj_description(c.oid, 'pg_class') LIKE 'FACT%'
                AND c.relname <> 'client_event'",
            &[&APP_SCHEMA],
        )?;
        let mut violations = vec![];
        for row in &facts {
            let t: String = row.get(0);
            let has = c.query_one(
                "SELECT count(*) FROM pg_constraint
                  WHERE conrelid = to_regclass($1) AND contype = 'f'
                    AND confrelid = 'client_event'::regclass",
                &[&t],
            )?;
            if has.get::<_, i64>(0) == 0 {
                violations.push(format!("{t} is a fact with no client_event reference"));
            }
        }
        Ok(Outcome::new(facts.len(), violations))
    }

    /// S28. D24 made `stock` reapable, so a durable foreign key into it is a
    /// reference to a row that may legitimately vanish. Exactly one is allowed.
    pub fn s28_one_fk_targets_stock(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT conrelid::regclass::text, conname, pg_get_constraintdef(oid)
               FROM pg_constraint
              WHERE contype = 'f' AND confrelid = 'stock'::regclass",
            &[],
        )?;
        let violations = rows
            .iter()
            .filter(|r| r.get::<_, String>(0) != "stock_allocation")
            .map(|r| {
                format!(
                    "{} references stock(id): only stock_allocation.stock_id may. {}",
                    r.get::<_, String>(0),
                    r.get::<_, String>(2)
                )
            })
            .collect();
        Ok(Outcome::new(rows.len(), violations))
    }

    /// S3, narrowed to what the register actually claims.
    ///
    /// The first version of this check flagged every `num_nonnulls(...) = 1` in
    /// the schema and failed on `stock`'s holder CHECK and on the actor CHECKs.
    /// Both are `= 1` deliberately, and D23's discriminated-union rule is the
    /// reason:
    ///
    /// > Typed nullable FKs with a mutual-exclusion CHECK are correct when the
    /// > arms are alternative identities of one referent, where exactly one is
    /// > structurally required and "none" is meaningless. They strain when the
    /// > arms are distinct relationships that merely happen to be exclusive
    /// > today, because there exclusivity is a policy, and policies turn out to
    /// > be wrong.
    ///
    /// A holder is a location or a package and never neither. An actor is a
    /// person or an automation and never neither. Those are identities. Causes
    /// and demands are relationships, and S3 is about those only.
    ///
    /// Which set a constraint belongs to is not derivable from the catalogue, so
    /// The Rust half of S13's three-way diff.
    ///
    /// The register asserts the `policy_kind` enum, the `%_policy` table set and
    /// this registry are the same set. Three representations of one fact, which
    /// is the shape that drifts, so the check compares all three rather than
    /// any two of them.
    pub const POLICY_KINDS: &[&str] = &["allocation", "receiving", "shelf_life"];

    /// S12 asserts an absence, and the absence is the design.
    ///
    /// A `num_nonnulls` CHECK on `policy_binding` would reverse the semantics
    /// and forbid the all-NULL scope, which is the platform-shipped default that
    /// shipped defaults and clamping both require. Somebody will add one for
    /// tidiness eventually, and it would break the resolver rather than tighten
    /// it.
    pub fn s12_policy_binding_has_no_num_nonnulls(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        let exists = c.query_one(
            "SELECT count(*) FROM information_schema.tables
              WHERE table_schema = $1 AND table_name = 'policy_binding'",
            &[&APP_SCHEMA],
        )?;
        if exists.get::<_, i64>(0) == 0 {
            return Ok(Outcome::new(0, vec![]));
        }
        let mut violations = vec![];
        let bad = c.query(
            "SELECT conname, pg_get_constraintdef(oid) FROM pg_constraint
              WHERE conrelid = to_regclass('policy_binding') AND contype = 'c'
                AND pg_get_constraintdef(oid) ILIKE '%num_nonnulls%'",
            &[],
        )?;
        for r in &bad {
            violations.push(format!(
                "policy_binding.{} forbids the all-NULL platform default: {}",
                r.get::<_, String>(0), r.get::<_, String>(1)));
        }
        let uniq = c.query(
            "SELECT conname, pg_get_constraintdef(oid), condeferrable FROM pg_constraint
              WHERE conrelid = to_regclass('policy_binding') AND contype = 'u'",
            &[],
        )?;
        match uniq.iter().find(|r| r.get::<_, String>(1).contains("NULLS NOT DISTINCT")) {
            None => violations.push(
                "policy_binding's scope uniqueness is not NULLS NOT DISTINCT, so two \
                 bindings can claim one scope and the resolver has two winners".into()),
            Some(r) if r.get::<_, bool>(2) =>
                violations.push(format!("policy_binding.{} is deferrable", r.get::<_, String>(0))),
            _ => {}
        }
        Ok(Outcome::new(uniq.len() + bad.len(), violations))
    }

    pub fn s13_policy_kind_sets_agree(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let enum_vals: Vec<String> = c.query(
            "SELECT e.enumlabel FROM pg_enum e JOIN pg_type t ON t.oid = e.enumtypid
              WHERE t.typname = 'policy_kind' ORDER BY e.enumsortorder", &[])?
            .iter().map(|r| r.get::<_, String>(0)).collect();
        if enum_vals.is_empty() {
            return Ok(Outcome::new(0, vec![]));
        }
        let tables: Vec<String> = c.query(
            "SELECT replace(table_name, '_policy', '') FROM information_schema.tables
              WHERE table_schema = $1 AND table_name LIKE '%\\_policy' ORDER BY table_name",
            &[&APP_SCHEMA])?
            .iter().map(|r| r.get::<_, String>(0)).collect();
        let mut violations = vec![];
        let mut declared_unbuilt = 0usize;
        for k in &enum_vals {
            if !tables.contains(k) {
                violations.push(format!("policy_kind '{k}' has no {k}_policy table"));
            }
            if !spork_policy::ALL.iter().any(|p| p.as_str() == k) {
                violations.push(format!("policy_kind '{k}' is not in the Rust registry"));
            }
        }
        for tb in &tables {
            if !enum_vals.contains(tb) {
                violations.push(format!("{tb}_policy exists with no policy_kind value"));
            }
        }
        // D22 names eleven kinds and three have value tables. A registry entry
        // with no enum value is therefore **not** a violation -- it is a kind
        // that is designed and unbuilt, which is the state the register
        // records elsewhere as pending. What would be a violation is the reverse, and
        // that is checked above: an enum value the registry has never heard of.
        for k in spork_policy::ALL {
            if !enum_vals.iter().any(|e| e == k.as_str()) {
                declared_unbuilt += 1;
            }
        }
        let _ = declared_unbuilt;
        Ok(Outcome::new(enum_vals.len(), violations))
    }

    /// S14. One template asserted over every value table, so a new kind cannot
    /// arrive with two of the three pieces.
    pub fn s14_policy_tables_have_the_template(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        let tables = c.query(
            "SELECT table_name FROM information_schema.tables
              WHERE table_schema = $1 AND table_name LIKE '%\\_policy'",
            &[&APP_SCHEMA])?;
        let mut violations = vec![];
        for row in &tables {
            let tb: String = row.get(0);
            let cons = c.query(
                "SELECT contype::text, pg_get_constraintdef(oid) FROM pg_constraint
                  WHERE conrelid = to_regclass($1)", &[&tb])?;
            let defs: Vec<(String, String)> = cons.iter()
                .map(|r| (r.get::<_, String>(0), r.get::<_, String>(1))).collect();
            if !defs.iter().any(|(ty, d)| ty == "c" && d.contains("kind =")) {
                violations.push(format!("{tb} has no CHECK pinning its kind"));
            }
            if !defs.iter().any(|(ty, d)| ty == "f" && d.contains("policy_binding(id, kind)")) {
                violations.push(format!(
                    "{tb} has no composite FK to (policy_binding.id, kind), so a value can \
                     attach to a binding of another kind"));
            }
            if !defs.iter().any(|(ty, d)| ty == "x" && d.contains("effective")) {
                violations.push(format!(
                    "{tb} has no effective-range exclusion, so one scope can hold two \
                     values at one instant"));
            }
        }
        Ok(Outcome::new(tables.len(), violations))
    }

    /// S16. A consuming column is named `<kind>_policy_id` and is a foreign key
    /// to that value table, so the resolver's output is traceable from any row
    /// that used it. Vacuous until something consumes a policy.
    pub fn s16_consuming_columns_are_named_by_kind(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT c.relname, a.attname FROM pg_attribute a
               JOIN pg_class c ON c.oid = a.attrelid
              WHERE c.relnamespace = to_regnamespace($1) AND c.relkind = 'r'
                AND a.attnum > 0 AND a.attname LIKE '%\\_policy\\_id'
                AND c.relname NOT LIKE '%\\_policy'",
            &[&APP_SCHEMA])?;
        let mut violations = vec![];
        for r in &rows {
            let (tb, col): (String, String) = (r.get(0), r.get(1));
            let kind = col.trim_end_matches("_policy_id");
            let n = c.query_one(
                "SELECT count(*) FROM pg_constraint
                  WHERE conrelid = to_regclass($1) AND contype = 'f'
                    AND confrelid = to_regclass($2)",
                &[&tb, &format!("{kind}_policy")])?;
            if n.get::<_, i64>(0) == 0 {
                violations.push(format!("{tb}.{col} is not a foreign key to {kind}_policy"));
            }
        }
        Ok(Outcome::new(rows.len(), violations))
    }

    /// the naming convention is the declaration: a mutual-exclusion CHECK over a
    /// cause or demand set is named `%_cause_ck` or `%_demand_ck`.
    pub fn s3_demand_checks_are_at_most_one(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT conrelid::regclass::text, conname, pg_get_constraintdef(oid)
               FROM pg_constraint
              WHERE connamespace = to_regnamespace($1)
                AND contype = 'c'
                AND (conname LIKE '%\\_cause\\_ck' OR conname LIKE '%\\_demand\\_ck')",
            &[&APP_SCHEMA],
        )?;
        // `<= 1` contains `= 1` as a substring, so a naive match flags exactly
        // the form this exists to require. It went unnoticed because S3 had no
        // constraint to examine until D61 wrote the first one, and a vacuous
        // check cannot be wrong out loud.
        //
        // The register's own first lesson is that a wrong invariant is worse
        // than a missing one because it confers confidence. This was wrong in
        // the other direction, which costs less and is no more correct.
        fn demands_exactly_one(def: &str) -> bool {
            def.replace("<= 1", "").replace(">= 1", "").contains("= 1")
        }
        let violations = rows
            .iter()
            .filter(|r| demands_exactly_one(&r.get::<_, String>(2)))
            .map(|r| {
                format!(
                    "{}.{} demands exactly one cause: {}",
                    r.get::<_, String>(0),
                    r.get::<_, String>(1),
                    r.get::<_, String>(2)
                )
            })
            .collect();
        Ok(Outcome::new(rows.len(), violations))
    }

    pub fn s7_no_unregistered_triggers(c: &mut Client) -> Result<Outcome, postgres::Error> {
        // Until a projection registry exists, the assertion is the stronger one:
        // there are no triggers at all. D25 forbids triggers that implement
        // rules, validation, defaults or cascades, and nothing so far needs one.
        let rows = c.query(
            "SELECT c.relname, t.tgname
               FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid
              WHERE NOT t.tgisinternal
                AND c.relnamespace = to_regnamespace($1)",
            &[&APP_SCHEMA],
        )?;
        let violations = rows
            .iter()
            .map(|r| format!("{} has trigger {}", r.get::<_, String>(0), r.get::<_, String>(1)))
            .collect();
        Ok(Outcome::new(rows.len(), violations))
    }

    pub fn s8_rls_tables_force_rls(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT relname, relforcerowsecurity
               FROM pg_class
              WHERE relnamespace = to_regnamespace($1)
                AND relkind = 'r' AND relrowsecurity",
            &[&APP_SCHEMA],
        )?;
        let violations = rows
            .iter()
            .filter(|r| !r.get::<_, bool>(1))
            .map(|r| format!("{} has RLS without FORCE", r.get::<_, String>(0)))
            .collect();
        Ok(Outcome::new(rows.len(), violations))
    }

    /// D19 as amended, widened by D55.
    ///
    /// Global (no policy), shared reference, and tenant-scoped. A shape outside
    /// the set means somebody wrote a bespoke predicate, which is how a tenancy
    /// hole gets in.
    ///
    /// The shared-reference shape is now a **pair** rather than one policy, and
    /// the reason is the hole D55 found. One permissive policy carrying one
    /// expression governs reads and writes together, because Postgres uses the
    /// USING expression as the WITH CHECK when none is given — so "readable when
    /// shared or ours" silently meant "writable when shared or ours", and any
    /// tenant could mint or edit rows every other tenant reads.
    ///
    /// So there are three permitted expressions rather than two, and S46 asserts
    /// the two halves of the pair always appear together. This check on its own
    /// cannot: it reads `polqual`, and the read half was correct the whole time.
    pub fn s9_three_rls_shapes(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT c.relname, p.polname, pg_get_expr(p.polqual, p.polrelid)
               FROM pg_policy p JOIN pg_class c ON c.oid = p.polrelid
              WHERE c.relnamespace = to_regnamespace($1)",
            &[&APP_SCHEMA],
        )?;
        const PERMITTED: &[&str] = &[
            // Shared reference, read half.
            "((tenant_id IS NULL) OR (tenant_id = current_tenant()))",
            // Shared reference, write half. The platform arm is what lets the
            // role that ships the catalogue go on shipping it.
            "((tenant_id = current_tenant()) OR ((tenant_id IS NULL) AND is_platform()))",
            // Tenant-scoped. Correct for writes as it stands, because the
            // implicit WITH CHECK is the same predicate.
            "(tenant_id = current_tenant())",
            // A `%_policy` value table, whose tenancy is real and indirect: it
            // has no tenant of its own and is readable exactly when its binding
            // is. D85 added this after finding all three existing value tables
            // with RLS **disabled** and the application holding SELECT, INSERT
            // and UPDATE on them -- D55's hole in the one table class D79's
            // audit could not see, because it keyed on a tenant_id column and
            // these have none.
            "(EXISTS ( SELECT 1 FROM policy_binding b WHERE ((b.id = policy_binding_id) AND ((b.tenant_id IS NULL) OR (b.tenant_id = current_tenant())))))",
        ];
        // Postgres stores a policy expression with the table's own name on any
        // column it qualifies, so the value-table shape reads differently on
        // each of the four tables that carry it. Normalised here rather than
        // listed four times, and the whitespace with it: `pg_get_expr` wraps.
        let normalise = |table: &str, expr: &str| -> String {
            expr.replace(&format!("{table}."), "")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        let violations = rows
            .iter()
            .filter(|r| {
                let expr = normalise(&r.get::<_, String>(0), &r.get::<_, String>(2));
                !PERMITTED.contains(&expr.as_str())
            })
            .map(|r| {
                format!(
                    "{}.{} is a shape outside the permitted set: {}",
                    r.get::<_, String>(0),
                    r.get::<_, String>(1),
                    r.get::<_, String>(2)
                )
            })
            .collect();
        Ok(Outcome::new(rows.len(), violations))
    }

    pub fn s10_no_jsonb(c: &mut Client) -> Result<Outcome, postgres::Error> {
        // Principle 3. party_message.payload is bytea, deliberately, and is the
        // only place a foreign document lands.
        let rows = c.query(
            "SELECT table_name, column_name, data_type
               FROM information_schema.columns
              WHERE table_schema = $1 AND data_type IN ('json','jsonb')",
            &[&APP_SCHEMA],
        )?;
        let violations = rows
            .iter()
            .map(|r| {
                format!(
                    "{}.{} is {}",
                    r.get::<_, String>(0),
                    r.get::<_, String>(1),
                    r.get::<_, String>(2)
                )
            })
            .collect();
        // Examined is the column count, not the violation count: this check has
        // a real population as soon as any table exists.
        let total = c.query_one(
            "SELECT count(*) FROM information_schema.columns WHERE table_schema = $1",
            &[&APP_SCHEMA],
        )?;
        Ok(Outcome::new(total.get::<_, i64>(0) as usize, violations))
    }

    pub fn s11_no_policy_logic_columns(c: &mut Client) -> Result<Outcome, postgres::Error> {
        // D13 made greppable. The register records that the first draft of the
        // policy design failed this check, on a column called band_axis.
        let denied = [
            "field", "attribute", "column", "operator", "comparator", "expression",
            "condition", "rule", "action", "target", "sql", "script",
        ];
        let rows = c.query(
            "SELECT table_name, column_name
               FROM information_schema.columns
              WHERE table_schema = $1
                AND (table_name LIKE 'policy%' OR table_name LIKE '%\\_policy')",
            &[&APP_SCHEMA],
        )?;
        let violations = rows
            .iter()
            .filter(|r| denied.contains(&r.get::<_, String>(1).as_str()))
            .map(|r| format!("{}.{} names logic", r.get::<_, String>(0), r.get::<_, String>(1)))
            .collect();
        Ok(Outcome::new(rows.len(), violations))
    }

    pub fn s20_generated_tables_have_forced_rls(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT relname, relrowsecurity, relforcerowsecurity
               FROM pg_class
              WHERE relnamespace = to_regnamespace($1)
                AND relkind = 'r' AND relname LIKE 'ext\\_%'",
            &[&APP_SCHEMA],
        )?;
        let violations = rows
            .iter()
            .filter(|r| !r.get::<_, bool>(1) || !r.get::<_, bool>(2))
            .map(|r| format!("{} lacks RLS with FORCE", r.get::<_, String>(0)))
            .collect();
        Ok(Outcome::new(rows.len(), violations))
    }

    pub fn s22_canonical_units_are_identity(c: &mut Client) -> Result<Outcome, postgres::Error> {
        // Principle 5. A canonical unit that is not the identity means stored
        // values are in something other than what every reader assumes.
        let rows = c.query(
            "SELECT d.code, u.code, u.factor_num, u.factor_den, u.offset_num
               FROM dimension d JOIN unit u ON u.id = d.canonical_unit_id",
            &[],
        )?;
        let violations = rows
            .iter()
            .filter(|r| {
                r.get::<_, i64>(2) != 1 || r.get::<_, i64>(3) != 1 || r.get::<_, i64>(4) != 0
            })
            .map(|r| {
                format!(
                    "canonical unit {} of {} is not the identity",
                    r.get::<_, String>(1),
                    r.get::<_, String>(0)
                )
            })
            .collect();
        Ok(Outcome::new(rows.len(), violations))
    }

    pub fn s30_projections_have_no_delete(c: &mut Client) -> Result<Outcome, postgres::Error> {
        // Projections are identified for now by a table comment naming them.
        // When the code-side registry exists this becomes a bidirectional diff.
        let rows = c.query(
            "SELECT c.relname
               FROM pg_class c
              WHERE c.relnamespace = to_regnamespace($1) AND c.relkind = 'r'
                AND obj_description(c.oid, 'pg_class') LIKE 'PROJECTION%'",
            &[&APP_SCHEMA],
        )?;
        // The register is precise and the first version of this check was not.
        // S30 is DELETE, and it is about the application role. S5 covers UPDATE
        // and S6 covers fact tables, so widening this one duplicates them and
        // then disagrees with them.
        //
        // spork_projection_owner holds INSERT and UPDATE on stock, and that
        // is the design rather than a violation: D35 puts every write to a
        // projection inside a SECURITY DEFINER function owned by a role with no
        // members. It is NOLOGIN, which is what keeps it outside J36's "no login
        // role holds INSERT, UPDATE or DELETE on a projection".
        let mut violations = vec![];
        for r in &rows {
            let t: String = r.get(0);
            let g = c.query(
                "SELECT grantee, privilege_type FROM information_schema.role_table_grants
                  WHERE table_name = $1 AND privilege_type = 'DELETE'
                    AND grantee = 'spork_app'",
                &[&t],
            )?;
            for row in &g {
                violations.push(format!(
                    "{} grants DELETE to {}: a projection is rebuilt, not deleted from",
                    t,
                    row.get::<_, String>(0)
                ));
            }
        }
        Ok(Outcome::new(rows.len(), violations))
    }

    pub fn s36_no_receipt_accumulators(c: &mut Client) -> Result<Outcome, postgres::Error> {
        // D45. Received is a fold over stock_movement. A stored total is the
        // documented double-count bug written into the schema.
        let denied = ["quantity_received", "received_quantity", "variance", "quantity_variance"];
        let rows = c.query(
            "SELECT table_name, column_name
               FROM information_schema.columns
              WHERE table_schema = $1
                AND table_name IN ('goods_receipt','goods_receipt_line','expected_supply')",
            &[&APP_SCHEMA],
        )?;
        let violations = rows
            .iter()
            .filter(|r| {
                let col: String = r.get(1);
                let tbl: String = r.get(0);
                // expected_supply.quantity_received is a maintained projection
                // and is the one place the name is legitimate.
                denied.contains(&col.as_str()) && tbl != "expected_supply"
            })
            .map(|r| format!("{}.{} is an accumulator", r.get::<_, String>(0), r.get::<_, String>(1)))
            .collect();
        Ok(Outcome::new(rows.len(), violations))
    }

    pub fn s37_location_zone_fk_is_composite(c: &mut Client) -> Result<Outcome, postgres::Error> {
        // D46. A plain zone_id FK lets a location sit in another site's zone,
        // and every zone-scoped policy resolution for it then returns that other
        // site's answer with nothing complaining.
        let rows = c.query(
            "SELECT conname, pg_get_constraintdef(oid)
               FROM pg_constraint
              WHERE conrelid = 'location'::regclass AND contype = 'f'
                AND pg_get_constraintdef(oid) LIKE '%zone%'",
            &[],
        )?;
        let mut violations = vec![];
        if rows.is_empty() {
            violations.push("location has no foreign key to zone".into());
        }
        for r in &rows {
            let def: String = r.get(1);
            if !(def.contains("(zone_id, site_id)") && def.contains("zone(id, site_id)")) {
                violations.push(format!(
                    "{} is not composite on site_id: {}",
                    r.get::<_, String>(0),
                    def
                ));
            }
        }
        Ok(Outcome::new(rows.len(), violations))
    }
    /// D47. What is left in the schema once the trigger is gone.
    ///
    /// The four cross-row rules moved to the application, because S7 rejected
    /// the trigger that held them and D25 forbids the validation kind by name.
    /// Two rules survive as in-row CHECKs, and they are the two that carry the
    /// whole discriminator: `reverses_movement_id` set means we were wrong, and
    /// null means the world changed. Everything downstream reads it that way, so
    /// a correction admitted without a stated reason makes the two classes
    /// indistinguishable again, which is the ambiguity migration 10 exists to
    /// remove.
    ///
    /// Checked here rather than trusted because a CHECK is droppable in one
    /// statement and nothing would report it.
    pub fn s38_the_correction_link_is_constrained(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        let mut violations = vec![];

        let fk = c.query(
            "SELECT conname, pg_get_constraintdef(oid)
               FROM pg_constraint
              WHERE conrelid = to_regclass('stock_movement') AND contype = 'f'
                AND pg_get_constraintdef(oid) LIKE '%reverses_movement_id%'",
            &[],
        )?;
        if fk.is_empty() {
            violations.push(
                "stock_movement.reverses_movement_id has no foreign key to stock_movement".into(),
            );
        }
        for r in &fk {
            let def: String = r.get(1);
            // D79 made this composite. The property S38 is about is that the
            // link is self-referencing and constrained, not the arity of the
            // key -- `REFERENCES stock_movement(id, tenant_id)` is the same
            // link with a tenant carried alongside, and reading the old form
            // literally would have made hardening the tenancy boundary look
            // like a regression.
            let self_referencing = def.contains("REFERENCES stock_movement(id)")
                || def.contains("REFERENCES stock_movement(id, tenant_id)");
            if !self_referencing {
                violations.push(format!(
                    "{} does not reference stock_movement(id): {}",
                    r.get::<_, String>(0),
                    def
                ));
            }
        }

        let ck = c.query(
            "SELECT conname FROM pg_constraint
              WHERE conrelid = to_regclass('stock_movement') AND contype = 'c'
                AND conname LIKE 'stock_movement_reversal%'",
            &[],
        )?;
        let present: Vec<String> = ck.iter().map(|r| r.get(0)).collect();
        for required in [
            "stock_movement_reversal_has_reason_ck",
            "stock_movement_reversal_not_self_ck",
        ] {
            if !present.iter().any(|n| n == required) {
                violations.push(format!("{required} is missing"));
            }
        }

        Ok(Outcome::new(fk.len() + ck.len(), violations))
    }
    /// D48. The default is the part worth asserting.
    ///
    /// Migration 11 adds `revision_class` with a default so the existing rows
    /// backfill, then drops it, so every writer afterwards has to say which kind
    /// of amendment it is making. Restoring the default would be a one-line
    /// change that reintroduces the exact failure: a correction written without
    /// thinking becomes a `world_event`, sorts by the moment it was typed rather
    /// than the moment it is about, and quietly reverts whatever the customer
    /// changed in between.
    ///
    /// So this checks for the absence of a default as much as for NOT NULL.
    pub fn s39_amendments_state_their_class(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let mut violations = vec![];

        let col = c.query(
            "SELECT is_nullable, column_default, udt_name
               FROM information_schema.columns
              WHERE table_schema = $1 AND table_name = 'intention_amendment'
                AND column_name = 'revision_class'",
            &[&APP_SCHEMA],
        )?;
        if col.is_empty() {
            violations.push("intention_amendment has no revision_class column".into());
        }
        for r in &col {
            if r.get::<_, String>(0) != "NO" {
                violations.push("revision_class is nullable, so an amendment can decline to say what it is".into());
            }
            if let Some(default) = r.get::<_, Option<String>>(1) {
                violations.push(format!(
                    "revision_class carries a default ({default}), so a correction written without \
                     thinking becomes a world_event and sorts by the wrong moment"
                ));
            }
            if r.get::<_, String>(2) != "revision_class" {
                violations.push(format!(
                    "revision_class is of type {}, not the shared revision_class enum",
                    r.get::<_, String>(2)
                ));
            }
        }

        let ck = c.query(
            "SELECT conname FROM pg_constraint
              WHERE conrelid = to_regclass('intention_amendment') AND contype = 'c'
                AND conname = 'intention_amendment_correction_has_reason_ck'",
            &[],
        )?;
        if ck.is_empty() {
            violations.push("intention_amendment_correction_has_reason_ck is missing".into());
        }

        Ok(Outcome::new(col.len() + ck.len(), violations))
    }
    /// D25's bidirectional diff, with the leg that was missing.
    ///
    /// The register described two directions: a commented column must be
    /// registered, and a registry row must name a live column. Both were true of
    /// `order` while nothing about it worked, because its four projection
    /// columns were neither commented nor registered and the two sides agreed by
    /// being equally empty. The function was the only witness that anything was
    /// wrong: `projection_order_rebuild` folds `intention_amendment` and UPDATEs
    /// four columns that the registry had never heard of.
    ///
    /// So the diff is three-way. A rebuild function is evidence that a
    /// projection exists, and a function with no registry rows is a projection
    /// nobody declared.
    ///
    /// Naming carries the contract: `projection_%_rebuild` is a rebuild and must
    /// be registered. `projection_package_stamp` and
    /// `projection_stock_resolve_locations` are steps within one, and their
    /// columns are registered under the rebuild they belong to.
    pub fn s5_the_projection_registry_agrees(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let mut violations = vec![];
        let mut examined = 0usize;

        // 1. Every commented column is registered.
        let unregistered = c.query(
            "SELECT c.relname, a.attname
               FROM pg_class c
               JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0
               JOIN pg_namespace n ON n.oid = c.relnamespace
              WHERE n.nspname = $1
                AND col_description(c.oid, a.attnum) LIKE '@projection %'
                AND NOT EXISTS (SELECT 1 FROM projection_rebuild r
                                 WHERE r.table_name = c.relname
                                   AND r.column_name = a.attname)",
            &[&APP_SCHEMA],
        )?;
        examined += unregistered.len();
        for r in &unregistered {
            violations.push(format!(
                "{}.{} is commented @projection but is in no registry row",
                r.get::<_, String>(0), r.get::<_, String>(1)));
        }

        // 2. Every registry row names a column that exists, and a function that does.
        let rows = c.query(
            "SELECT r.table_name, r.column_name, r.function_name,
                    to_regclass(quote_ident(r.table_name)) IS NULL AS no_table,
                    NOT EXISTS (SELECT 1 FROM pg_class c
                                  JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0
                                 WHERE c.relname = r.table_name AND a.attname = r.column_name)
                        AS no_column,
                    NOT EXISTS (SELECT 1 FROM pg_proc p WHERE p.proname = r.function_name)
                        AS no_function
               FROM projection_rebuild r",
            &[],
        )?;
        examined += rows.len();
        for r in &rows {
            let (tbl, col, func): (String, String, String) = (r.get(0), r.get(1), r.get(2));
            if r.get::<_, bool>(3) {
                violations.push(format!("registry names table {tbl}, which does not exist"));
            } else if r.get::<_, bool>(4) {
                violations.push(format!("registry names {tbl}.{col}, which does not exist"));
            }
            if r.get::<_, bool>(5) {
                violations.push(format!("registry names function {func}, which does not exist"));
            }
        }

        // 3. Every rebuild function is registered. The leg that catches a
        //    projection maintained in code and declared nowhere.
        let orphans = c.query(
            "SELECT p.proname
               FROM pg_proc p
               JOIN pg_namespace n ON n.oid = p.pronamespace
              WHERE n.nspname = $1
                AND p.proname LIKE 'projection\\_%\\_rebuild'
                AND NOT EXISTS (SELECT 1 FROM projection_rebuild r
                                 WHERE r.function_name = p.proname)",
            &[&APP_SCHEMA],
        )?;
        examined += orphans.len();
        for r in &orphans {
            violations.push(format!(
                "{} maintains a projection that appears in no registry row, so nothing \
                 marks its columns and every guard over them reads as satisfied",
                r.get::<_, String>(0)));
        }

        Ok(Outcome::new(examined.max(rows.len()), violations))
    }
    /// D50, borrowing S36's shape.
    ///
    /// D40 keeps invoice rendering out and produces charge lines and what they
    /// were computed from. An extended amount is quantity times price, so
    /// storing one creates a number that can disagree with its own inputs, which
    /// is what D25 forbids for maintained tables and what S36 already asserts for
    /// received quantity.
    ///
    /// Tax is on the list for a different reason. `unit_price_minor` is
    /// tax-exclusive by decision, and the way that decision erodes is a
    /// `tax_minor` column appearing beside it and making the exclusivity of the
    /// first column ambiguous to anyone reading the schema.
    ///
    /// `consignment.price_minor` is deliberately out of scope: a carrier's
    /// quoted charge is a number they gave us, not one we computed.
    pub fn s40_no_order_totals(c: &mut Client) -> Result<Outcome, postgres::Error> {
        const TABLES: &[&str] = &["order", "order_line", "fulfilment", "fulfilment_line"];
        const DENIED: &[&str] = &[
            "total", "total_minor", "line_total", "line_total_minor",
            "subtotal", "subtotal_minor", "grand_total", "grand_total_minor",
            "extended_amount", "extended_price", "extended_price_minor",
            "amount", "amount_minor", "tax", "tax_minor", "gst", "gst_minor",
        ];
        let rows = c.query(
            "SELECT c.relname, a.attname
               FROM pg_class c
               JOIN pg_namespace n ON n.oid = c.relnamespace
               JOIN pg_attribute a ON a.attrelid = c.oid
                                  AND a.attnum > 0 AND NOT a.attisdropped
              WHERE n.nspname = $1 AND c.relname = ANY($2) AND a.attname = ANY($3)",
            &[&APP_SCHEMA, &TABLES, &DENIED],
        )?;
        // The population is every column on the declared tables, so an empty
        // result is a real absence rather than a table set that does not exist.
        let examined: i64 = c
            .query_one(
                "SELECT count(*)
                   FROM pg_class c
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   JOIN pg_attribute a ON a.attrelid = c.oid
                                      AND a.attnum > 0 AND NOT a.attisdropped
                  WHERE n.nspname = $1 AND c.relname = ANY($2)",
                &[&APP_SCHEMA, &TABLES],
            )?
            .get(0);
        let violations = rows
            .iter()
            .map(|r| {
                format!("{}.{} stores a number derivable from quantity and price",
                    r.get::<_, String>(0), r.get::<_, String>(1))
            })
            .collect();
        Ok(Outcome::new(examined as usize, violations))
    }
    /// D51, and the leg that matters is the diff rather than the list.
    ///
    /// `intention_amendment` now has eight covered columns across two subjects,
    /// and three CHECKs stand between them and nonsense: one says an amendment
    /// changes something, one says it names an order or a line but not both, and
    /// one says a price moves as a pair.
    ///
    /// A check that asserted those three constraints exist by name would pass
    /// forever while the thing it guards rots, because the way this breaks is not
    /// a dropped constraint. It is a ninth covered column added to the table and
    /// left out of the CHECK definitions, which is exactly how
    /// `intention_amendment_changes_something_ck` came to cover four columns on a
    /// table that had four and then did not.
    ///
    /// So the covered set is read out of the catalogue rather than written here,
    /// and every `new_%` column must appear inside both definitions. Adding a
    /// covered column without wiring it in fails this check on the next run,
    /// which is D25's bidirectional diff pointed at a constraint body.
    pub fn s41_amendments_name_one_subject(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let mut violations = vec![];

        let covered: Vec<String> = c
            .query(
                "SELECT column_name FROM information_schema.columns
                  WHERE table_schema = $1 AND table_name = 'intention_amendment'
                    AND column_name LIKE 'new\\_%'
                  ORDER BY column_name",
                &[&APP_SCHEMA],
            )?
            .iter()
            .map(|r| r.get::<_, String>(0))
            .collect();

        if covered.is_empty() {
            violations.push(
                "intention_amendment has no new_% covered column, so the fold folds nothing".into(),
            );
        }

        let defs = c.query(
            "SELECT conname, pg_get_constraintdef(oid)
               FROM pg_constraint
              WHERE conrelid = to_regclass('intention_amendment')
                AND conname IN ('intention_amendment_changes_something_ck',
                                'intention_amendment_subject_ck',
                                'intention_amendment_price_pair_ck',
                                'intention_amendment_line_fk')",
            &[],
        )?;
        let def_of = |name: &str| -> Option<String> {
            defs.iter()
                .find(|r| r.get::<_, String>(0) == name)
                .map(|r| r.get::<_, String>(1))
        };

        for required in [
            "intention_amendment_changes_something_ck",
            "intention_amendment_subject_ck",
            "intention_amendment_price_pair_ck",
            "intention_amendment_line_fk",
        ] {
            if def_of(required).is_none() {
                violations.push(format!("{required} is missing"));
            }
        }

        // Every covered column is inside both CHECKs. The subject CHECK is the
        // one that decides which fold a value belongs to, so a column absent from
        // it can be written under either subject and folded under neither.
        for guard in [
            "intention_amendment_changes_something_ck",
            "intention_amendment_subject_ck",
        ] {
            if let Some(def) = def_of(guard) {
                for col in &covered {
                    if !def.contains(col.as_str()) {
                        violations.push(format!(
                            "{col} is a covered column that {guard} does not name, so an \
                             amendment can set it without the guard seeing it"
                        ));
                    }
                }
            }
        }

        // Composite through order_id. Without it an amendment can name order A
        // and a line belonging to order C: the fold writes the line, and every
        // report about A reads consistent while the wrong line moved.
        if let Some(def) = def_of("intention_amendment_line_fk") {
            if !def.contains("REFERENCES order_line(id, order_id)") {
                violations.push(format!(
                    "intention_amendment_line_fk is not composite through order_id, so an \
                     amendment can name a line of another order: {def}"
                ));
            }
        }

        Ok(Outcome::new(covered.len() + defs.len(), violations))
    }
    /// D52. The check that watches the checks that are not running.
    ///
    /// D49 named three ways a guard fails: it was wrong, it examined nothing, or
    /// **its stated precondition has since been met while it goes on reading as
    /// deliberate**. Migration 12 fixed the third for S5 and swept nothing else,
    /// and by migration 14 fourteen entries across both classes were waiting on
    /// things that had already been built — `observation` since migration 7,
    /// `policy_binding` since migration 8, `party` since migration 2.
    ///
    /// The reason that happened is structural rather than careless. A blocker was
    /// prose, so it was a claim about the schema stored in the one place the
    /// schema could not reach, and nothing could contradict it. Now it is a named
    /// object and this asserts the object is still missing.
    ///
    /// **Every** named object must be absent, not merely one of them. S33 waited
    /// on "party and number_range" long after `party` arrived, and a rule that
    /// accepted the entry because `number_range` was still missing would let that
    /// half rot for as long as the other half held.
    ///
    /// `Blocker::Elsewhere` covers what SQL cannot see — a code registry, an AST
    /// pass, an undecided question. Those are counted and reported rather than
    /// checked, because a number is harder to overlook than a silence.
/// The tables whose rows are assertions, or parts of one.
    ///
    /// `party_message` is deliberately not here. It is the artefact a claim
    /// arrived in rather than the claim, which is why it may carry `parse_status`
    /// without S18 objecting — parsing is something we do to a message, not a
    /// position on somebody's statement.
    pub const ASSERTION_TABLES: &[&str] = &[
        "assertion",
        "assertion_stance",
        "assertion_check",
        "despatch_advice",
        "document_response",
        "asserted_unit",
        "asserted_unit_content",
    ];

    /// S17. D21: an assertion may always be absent.
    ///
    /// Rung zero of the degradation ladder is *blind receipt* — a truck arrives
    /// with no paperwork and the floor still works. That is a schema property
    /// rather than a workflow branch, and this is the property: **no table
    /// outside the mechanism may require a claim to exist.** A NOT NULL foreign
    /// key to an assertion is a supplier's message becoming a precondition for
    /// our own operation.
    ///
    /// Scoped to exclude the mechanism's own tables, because inside it the
    /// references are structural — `assertion_stance.assertion_id` is a position
    /// on a claim and cannot be a position on nothing. The first draft of this
    /// check failed on exactly that, which is why the scope is written down.
    pub fn s17_a_reference_to_a_claim_is_optional(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT con.conrelid::regclass::text, con.confrelid::regclass::text,
                    a.attname
               FROM pg_constraint con
               JOIN unnest(con.conkey) AS k(attnum) ON true
               JOIN pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = k.attnum
              WHERE con.contype = 'f'
                AND con.confrelid::regclass::text = ANY($1)
                AND con.conrelid::regclass::text <> ALL($1)
                AND a.attnotnull
                AND a.attname <> 'tenant_id'",
            &[&ASSERTION_TABLES],
        )?;
        let mut violations = vec![];
        for r in &rows {
            violations.push(format!(
                "{}.{} is NOT NULL and references the assertion table {}, so the row \
                 cannot exist without a counterparty's claim",
                r.get::<_, String>(0),
                r.get::<_, String>(2),
                r.get::<_, String>(1)
            ));
        }
        // Examined: every FK into the set from outside it.
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM pg_constraint con
                  WHERE con.contype = 'f'
                    AND con.confrelid::regclass::text = ANY($1)
                    AND con.conrelid::regclass::text <> ALL($1)",
                &[&ASSERTION_TABLES],
            )?
            .get(0);
        Ok(Outcome::new(examined as usize, violations))
    }

    /// S18. D21 and D25: our position on a claim is a fact of ours.
    ///
    /// A `status` column on an assertion would be us writing on their statement,
    /// which is the one thing the category exists to prevent. `assertion_stance`
    /// holds it instead, and its column is named `stance` for the same reason.
    pub fn s18_no_claim_carries_our_status(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT table_name, column_name FROM information_schema.columns
              WHERE table_schema = $2 AND table_name = ANY($1)
                AND column_name IN ('status', 'state')
              ORDER BY table_name, column_name",
            &[&ASSERTION_TABLES, &APP_SCHEMA],
        )?;
        let mut violations = vec![];
        for r in &rows {
            violations.push(format!(
                "{}.{} names our position on the row that carries their claim",
                r.get::<_, String>(0),
                r.get::<_, String>(1)
            ));
        }
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM information_schema.columns
                  WHERE table_schema = $2 AND table_name = ANY($1)",
                &[&ASSERTION_TABLES, &APP_SCHEMA],
            )?
            .get(0);
        Ok(Outcome::new(examined as usize, violations))
    }

    /// S51. A write may not name another tenant's row.
    ///
    /// D55 found a cross-tenant write hole open since migration 1 and closed it
    /// by hand. This is the class, closed by construction: **RLS filters what a
    /// tenant reads and says nothing about what a row may point at.**
    ///
    /// Scoped to *strictly owned* tables — `tenant_id NOT NULL` — because a
    /// shared-reference table's platform rows have no tenant and a composite key
    /// to one is impossible, not merely unfashionable. That half is J64's, as
    /// data, because it cannot be a key.
    ///
    /// Two constraints are exempt by name, and both are tenant-determined through
    /// a column another invariant requires: `location_zone_fk` carries `site_id`
    /// for S37, and `intention_amendment_line_fk` carries `order_id` for S41.
    /// **An exemption that is listed is a decision; one that is silent is a
    /// hole** — the phrasing J19 already earned.
    pub fn s51_a_reference_cannot_cross_a_tenant(c: &mut Client) -> Result<Outcome, postgres::Error> {
        const EXEMPT: &[&str] = &["location_zone_fk", "intention_amendment_line_fk"];
        let mut violations = vec![];

        // Every FK into a strictly-owned table, and whether the target's
        // tenant_id participates.
        let rows = c.query(
            "WITH strict AS (
                 SELECT c.oid, c.relname FROM pg_class c
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   JOIN pg_attribute a ON a.attrelid = c.oid
                        AND a.attname = 'tenant_id' AND a.attnotnull
                  WHERE n.nspname = $1 AND c.relkind = 'r'),
             src AS (
                 SELECT c.oid, c.relname FROM pg_class c
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   JOIN pg_attribute a ON a.attrelid = c.oid AND a.attname = 'tenant_id'
                  WHERE n.nspname = $1 AND c.relkind = 'r')
             SELECT s.relname, t.relname, k.conname,
                    EXISTS (SELECT 1 FROM unnest(k.confkey) y
                              JOIN pg_attribute pa ON pa.attrelid = k.confrelid AND pa.attnum = y
                             WHERE pa.attname = 'tenant_id')
               FROM pg_constraint k
               JOIN src s ON s.oid = k.conrelid
               JOIN strict t ON t.oid = k.confrelid
              WHERE k.contype = 'f'
              ORDER BY 1, 2, 3",
            &[&APP_SCHEMA],
        )?;
        let examined = rows.len();

        // A pair is covered if any of its constraints carries the tenant: the
        // others are then constrained through it.
        use std::collections::{HashMap, HashSet};
        let mut covered: HashMap<(String, String), bool> = HashMap::new();
        let mut names: HashMap<(String, String), Vec<String>> = HashMap::new();
        for r in &rows {
            let key = (r.get::<_, String>(0), r.get::<_, String>(1));
            let carries: bool = r.get(3);
            *covered.entry(key.clone()).or_insert(false) |= carries;
            names.entry(key).or_default().push(r.get(2));
        }
        let exempt: HashSet<&str> = EXEMPT.iter().copied().collect();
        for (pair, ok) in &covered {
            if *ok {
                continue;
            }
            if names[pair].iter().all(|n| exempt.contains(n.as_str())) {
                continue;
            }
            violations.push(format!(
                "{} references {} without carrying its tenant ({}), so a row may name another tenant's",
                pair.0,
                pair.1,
                names[pair].join(", ")
            ));
        }

        // And the key that makes carrying it possible.
        for r in &c.query(
            "WITH strict AS (
                 SELECT c.oid, c.relname FROM pg_class c
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   JOIN pg_attribute a ON a.attrelid = c.oid
                        AND a.attname = 'tenant_id' AND a.attnotnull
                  WHERE n.nspname = $1 AND c.relkind = 'r')
             SELECT t.relname FROM strict t
              WHERE EXISTS (SELECT 1 FROM pg_constraint f WHERE f.confrelid = t.oid AND f.contype = 'f')
                AND EXISTS (SELECT 1 FROM pg_attribute a WHERE a.attrelid = t.oid AND a.attname = 'id')
                AND NOT EXISTS (
                    SELECT 1 FROM pg_constraint k
                     WHERE k.conrelid = t.oid AND k.contype IN ('p', 'u')
                       AND (SELECT array_agg(a.attname::text ORDER BY a.attname::text)
                              FROM unnest(k.conkey) x
                              JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = x)
                           = ARRAY['id', 'tenant_id']::text[])
              ORDER BY 1",
            &[&APP_SCHEMA],
        )? {
            violations.push(format!(
                "{} is referenced and strictly tenant-owned but offers no (id, tenant_id) key to reference it by",
                r.get::<_, String>(0)
            ));
        }
        Ok(Outcome::new(examined, violations))
    }

    /// S15. Tenancy is index 0 of every kind's ordering.
    ///
    /// D22: without it, *"a per-kind order ranking Product above Tenancy would
    /// let our default outrank a tenant's own configuration: a correctness hole,
    /// not a support surface."* A platform-shipped binding must lose to a
    /// tenant's every time, on every kind, and that is a property of the
    /// declaration rather than of any resolution.
    ///
    /// Checks well-formedness too, because an ordering that named five
    /// dimensions would leave some pairs unorderable and the failure would
    /// present as a tie rather than as a missing declaration.
    pub fn s15_tenancy_outranks_everything(_c: &mut Client) -> Result<Outcome, postgres::Error> {
        let mut violations = vec![];
        for &k in spork_policy::ALL {
            if let Err(e) = spork_policy::ordering_is_well_formed(k) {
                violations.push(e);
            }
        }
        Ok(Outcome::new(spork_policy::ALL.len(), violations))
    }

    /// S52. The resolver knows every value table.
    ///
    /// Migration 38 predicted this coupling in its own closing comment and
    /// misattributed the guard: *"a fourth would have to be added here, which is
    /// exactly the coupling S50 exists to catch."* S50 is about what the policy
    /// **epoch** can see; this is about what the **resolver** can see. Two
    /// different questions that read alike, and D85 walked into the gap within
    /// the hour -- a fourth kind resolved to nothing at all, because
    /// `policy_candidate` did not know its value table existed.
    ///
    /// **Resolving to nothing is the worst available failure.** It is not an
    /// error and not a default: the caller is told no policy applies, which is a
    /// legitimate answer it cannot distinguish from a missing table.
    pub fn s52_the_resolver_knows_every_value_table(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        let tables: Vec<String> = c
            .query(
                "SELECT table_name FROM information_schema.tables
                  WHERE table_schema = $1 AND table_name LIKE '%\\_policy'
                  ORDER BY table_name",
                &[&APP_SCHEMA],
            )?
            .iter()
            .map(|r| r.get(0))
            .collect();

        let mut violations = vec![];
        for f in ["policy_candidate", "policy_value"] {
            let body: String = c
                .query_one(
                    "SELECT pg_get_functiondef(p.oid) FROM pg_proc p
                       JOIN pg_namespace n ON n.oid = p.pronamespace
                      WHERE n.nspname = $2 AND p.proname = $1 LIMIT 1",
                    &[&f, &APP_SCHEMA],
                )?
                .get(0);
            for t in &tables {
                if !body.contains(t.as_str()) {
                    violations.push(format!(
                        "{f} does not read {t}, so that kind resolves to nothing rather than to its policy"
                    ));
                }
            }
        }
        Ok(Outcome::new(tables.len() * 2, violations))
    }

        pub fn s42_pending_checks_are_still_blocked(c: &mut Client) -> Result<Outcome, postgres::Error> {
        use crate::{jobs, Blocker, Check};

        // Both classes, flattened to (id, blockers). Structural and job-asserted
        // are separate enums with separate specs, and a sweep that covered one of
        // them is exactly the mistake this exists to stop repeating.
        let mut pending: Vec<(String, &'static [Blocker])> = vec![];
        for &id in super::ALL {
            if let Check::Pending(b) = super::spec(id).check {
                pending.push((id.to_string(), b));
            }
        }
        for &id in jobs::ALL {
            if let jobs::Check::Pending(b) = jobs::spec(id).check {
                pending.push((id.to_string(), b));
            }
        }

        let mut violations = vec![];
        let mut examined = 0usize;
        let mut elsewhere = 0usize;

        for (id, blockers) in &pending {
            if blockers.is_empty() {
                violations.push(format!(
                    "{id} is pending and names nothing it is waiting for, which is \
                     indistinguishable from an oversight"
                ));
                continue;
            }
            for b in *blockers {
                match b {
                    Blocker::Elsewhere(_) => elsewhere += 1,
                    Blocker::Absent(name) => {
                        examined += 1;
                        // A table, view, sequence or function, or `table.column`.
                        let exists: bool = match name.split_once('.') {
                            Some((table, column)) => c
                                .query_one(
                                    "SELECT EXISTS (SELECT 1 FROM information_schema.columns
                                       WHERE table_schema = $1 AND table_name = $2
                                         AND column_name = $3)",
                                    &[&APP_SCHEMA, &table, &column],
                                )?
                                .get(0),
                            None => c
                                .query_one(
                                    "SELECT to_regclass($1) IS NOT NULL
                                         OR EXISTS (SELECT 1 FROM pg_proc p
                                                      JOIN pg_namespace n ON n.oid = p.pronamespace
                                                     WHERE n.nspname = $2 AND p.proname = $1)",
                                    &[&name, &APP_SCHEMA],
                                )?
                                .get(0),
                        };
                        if exists {
                            violations.push(format!(
                                "{id} is pending on {name}, which exists. The gap reads as \
                                 deliberate and is not"
                            ));
                        }
                    }
                }
            }
        }

        if elsewhere > 0 {
            // Not a violation. A number, so the unverifiable set is visible.
            eprintln!(
                "  S42: {elsewhere} blockers name something outside the database and \
                 cannot be checked here"
            );
        }

        Ok(Outcome::new(examined, violations))
    }
    /// D53. S5's fourth leg, and the one that catches a maintainer maintaining
    /// nothing.
    ///
    /// S5 checks three things: a commented column is registered, a registry row
    /// names a live table, column and function, and every rebuild function has a
    /// registry row. `stock.allocated_quantity` satisfied all three from
    /// migration 3 onward and was never computed. The table existed, the column
    /// existed, `projection_stock_rebuild` existed, and its body did not contain
    /// the string `allocated_quantity` anywhere.
    ///
    /// Migration 12 found the case where the function was missing entirely. This
    /// is the quieter one: the function is present, so every existence check
    /// passes, and the column simply keeps whatever it was last set to. It was
    /// harmless only because no allocation had ever been written.
    ///
    /// # The family, not the function
    ///
    /// S5 already records that `projection_package_stamp` and
    /// `projection_stock_resolve_locations` are steps within a rebuild and that
    /// their columns are registered under the rebuild they belong to. So the unit
    /// is the family — every `projection_<subject>%` function — rather than the
    /// one function the registry names.
    ///
    /// The first draft of this check demanded the named function itself, and it
    /// reported `package.placement_event_id` and `package.placement_occurred_at`,
    /// which `projection_package_stamp` does write. Encoding the documented shape
    /// rather than the convenient one keeps it catching the real case:
    /// `stock.allocated_quantity` was named by no function in its family at all.
    ///
    /// # What this can and cannot see
    ///
    /// It greps the function body for the column name, which is the same
    /// greppability S11 relies on and has the same limit: a maintainer writing
    /// through dynamic SQL would pass while doing nothing. Every rebuild here is
    /// an explicit UPDATE naming its columns, so the approximation holds today and
    /// stops holding the moment one is not — worth stating rather than
    /// discovering.
    ///
    /// It also cannot see whether every step actually runs. A scheduler invoking
    /// the rebuild and skipping the stamp leaves a registered projection stale
    /// forever, and nothing here would say so. That is question 139.
    pub fn s43_a_registered_maintainer_writes_its_column(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        // The family prefix is the registered name with `_rebuild` removed, so
        // `projection_stock_rebuild` claims `projection_stock%`.
        const UNWRITTEN: &str = "
            SELECT r.table_name, r.column_name, r.function_name
              FROM projection_rebuild r
             WHERE EXISTS (SELECT 1 FROM pg_proc p
                             JOIN pg_namespace n ON n.oid = p.pronamespace
                            WHERE n.nspname = $1 AND p.proname = r.function_name)
               AND NOT EXISTS (
                     SELECT 1 FROM pg_proc p
                       JOIN pg_namespace n ON n.oid = p.pronamespace
                      WHERE n.nspname = $1
                        AND p.proname LIKE
                            regexp_replace(r.function_name, '_rebuild$', '') || '%'
                        AND position(r.column_name IN pg_get_functiondef(p.oid)) > 0)";

        let rows = c.query(UNWRITTEN, &[&APP_SCHEMA])?;
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM projection_rebuild r
                  WHERE EXISTS (SELECT 1 FROM pg_proc p
                                  JOIN pg_namespace n ON n.oid = p.pronamespace
                                 WHERE n.nspname = $1 AND p.proname = r.function_name)",
                &[&APP_SCHEMA],
            )?
            .get(0);
        let violations = rows
            .iter()
            .map(|r| {
                format!(
                    "{}.{} is registered to {}, and no function in that family names it, so \
                     the column keeps whatever it was last set to while every existence \
                     check passes",
                    r.get::<_, String>(0), r.get::<_, String>(1), r.get::<_, String>(2)
                )
            })
            .collect();
        Ok(Outcome::new(examined as usize, violations))
    }
    /// D54. Every guard in this suite watches for too much privilege. This is the
    /// other direction, and nothing was watching it.
    ///
    /// `order.currency` was added by D50 in migration 13 and granted to nobody.
    /// No role could INSERT it and no role could UPDATE it, and no maintainer
    /// wrote it, so it could only ever hold NULL — while J54 requires an order to
    /// name a currency before any of its lines may be priced. The application
    /// could price a line and could never make that legal.
    ///
    /// S6 and S30 assert that facts and projections have *no* write privilege.
    /// J36 asserts that `@projection` columns are not writable by the
    /// application. All three are looking for an excess. A column writable by
    /// nothing at all is the opposite mistake and it is just as silent, because
    /// every check that could have noticed was pointed the other way.
    ///
    /// # Finding the rule that matches the design
    ///
    /// The first draft asked whether *any* role could write the column. It
    /// reported eleven things and none of them was `order.currency`, because
    /// `spork_projection_owner` holds table-wide UPDATE on `order` and a
    /// table-wide grant covers every column including ones added later. It would
    /// never have caught the defect it was written for.
    ///
    /// The second draft asked whether `spork_app` could write it, and reported
    /// thirty-eight columns the application is deliberately not allowed to touch:
    /// `dimension`, `metric_code`, `person_tenant`, the registry itself. Most of
    /// this schema is read-only to the application on purpose.
    ///
    /// What actually characterises the defect is narrower and per table. **On a
    /// table the application may write, every column that is not a projection is
    /// in one of its grant lists.** That is the state a column-level grant is
    /// meant to leave the table in, and the way it breaks is always the same:
    /// somebody adds a column to a table whose grants were already enumerated,
    /// and enumerations do not update themselves. It has now happened twice —
    /// migration 9's projection columns, caught by D49, and `order.currency`,
    /// caught here.
    ///
    /// A projection is a column registered in `projection_rebuild` **or**
    /// commented `@projection`, either arm, because migration 12's
    /// `@projection(pending)` marks a column deliberately protected from the
    /// application before any registry row exists.
    pub fn s45_column_grants_are_complete(c: &mut Client) -> Result<Outcome, postgres::Error> {
        const UNGRANTED: &str = "
            SELECT c.relname, a.attname
              FROM pg_class c
              JOIN pg_namespace n ON n.oid = c.relnamespace
              JOIN pg_attribute a ON a.attrelid = c.oid
                                 AND a.attnum > 0 AND NOT a.attisdropped
             WHERE n.nspname = $1
               AND c.relkind = 'r'
               AND a.attgenerated = ''
               -- Only tables the application may write at all.
               AND EXISTS (SELECT 1 FROM information_schema.column_privileges cp
                            WHERE cp.table_schema = $1 AND cp.table_name = c.relname
                              AND cp.grantee = 'spork_app'
                              AND cp.privilege_type IN ('INSERT','UPDATE'))
               -- A projection is owned by a maintainer and must not be granted.
               AND coalesce(col_description(c.oid, a.attnum), '') NOT LIKE '@projection%'
               AND NOT EXISTS (SELECT 1 FROM projection_rebuild pr
                                WHERE pr.table_name = c.relname
                                  AND pr.column_name = a.attname)
               AND NOT has_column_privilege('spork_app', c.oid, a.attnum, 'INSERT')
               AND NOT has_column_privilege('spork_app', c.oid, a.attnum, 'UPDATE')
             ORDER BY c.relname, a.attnum";

        let rows = c.query(UNGRANTED, &[&APP_SCHEMA])?;
        let examined: i64 = c
            .query_one(
                "SELECT count(*)
                   FROM pg_class c
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   JOIN pg_attribute a ON a.attrelid = c.oid
                                      AND a.attnum > 0 AND NOT a.attisdropped
                  WHERE n.nspname = $1 AND c.relkind = 'r' AND a.attgenerated = ''
                    AND EXISTS (SELECT 1 FROM information_schema.column_privileges cp
                                 WHERE cp.table_schema = $1 AND cp.table_name = c.relname
                                   AND cp.grantee = 'spork_app'
                                   AND cp.privilege_type IN ('INSERT','UPDATE'))",
                &[&APP_SCHEMA],
            )?
            .get(0);
        let violations = rows
            .iter()
            .map(|r| {
                format!(
                    "{}.{} is on a table the application writes, is not a projection, and is in \
                     neither grant list, so nothing can ever put a value in it",
                    r.get::<_, String>(0), r.get::<_, String>(1))
            })
            .collect();
        Ok(Outcome::new(examined as usize, violations))
    }
    /// D24 supply side. The idempotency guard, and the index that must not exist.
    ///
    /// One partial unique index per provenance arm is what makes reprocessing a
    /// supplier message safe: the second delivery of the same ASN finds its row
    /// rather than creating a twin. **And explicitly no unique index over
    /// `(item_id, owner_id, status_id)`**, because two purchase orders may
    /// promise the same goods to the same place and they are two promises, not
    /// one counted twice.
    ///
    /// The arm set is read from the catalogue rather than listed, so the check
    /// widens by itself as the transfer, asserted and return arms arrive.
    pub fn s24_one_unique_index_per_arm(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let mut violations = vec![];
        let arms: Vec<String> = c
            .query(
                "SELECT column_name FROM information_schema.columns
                  WHERE table_schema = $1 AND table_name = 'expected_supply'
                    AND (column_name LIKE '%\\_line_id' OR column_name = 'asserted_unit_content_id')
                  ORDER BY column_name",
                &[&APP_SCHEMA],
            )?
            .iter()
            .map(|r| r.get::<_, String>(0))
            .collect();

        for arm in &arms {
            let covered: bool = c
                .query_one(
                    "SELECT EXISTS (SELECT 1 FROM pg_index i
                                      JOIN pg_class ic ON ic.oid = i.indexrelid
                                     WHERE i.indrelid = to_regclass('expected_supply')
                                       AND i.indisunique
                                       AND i.indpred IS NOT NULL
                                       AND pg_get_indexdef(i.indexrelid) LIKE '%' || $1 || '%')",
                    &[&arm],
                )?
                .get(0);
            if !covered {
                violations.push(format!(
                    "{arm} is a provenance arm with no partial unique index, so reprocessing \
                     the message that produced it makes a second row"
                ));
            }
        }

        let forbidden: bool = c
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM pg_index i
                                 WHERE i.indrelid = to_regclass('expected_supply')
                                   AND i.indisunique
                                   AND pg_get_indexdef(i.indexrelid) LIKE '%item_id%'
                                   AND pg_get_indexdef(i.indexrelid) LIKE '%owner_id%'
                                   AND pg_get_indexdef(i.indexrelid) LIKE '%status_id%')",
                &[],
            )?
            .get(0);
        if forbidden {
            violations.push(
                "a unique index spans (item_id, owner_id, status_id), which collapses two \
                 purchase orders promising the same goods into one promise"
                    .into(),
            );
        }

        Ok(Outcome::new(arms.len() + 1, violations))
    }
    /// D24 supply side. Availability is one indexed read, on both sides.
    ///
    /// On-hand availability reads `stock`; available-to-promise over future
    /// supply reads `expected_supply`. Both must carry `owner_id` and `status_id`
    /// **in the index key**, because otherwise the query has to join
    /// `inventory_status` to find out whether a row counts, and a join in the
    /// availability path is the cost D24 built this shape to avoid.
    pub fn s25_availability_indexes_carry_owner_and_status(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        const TABLES: &[&str] = &["stock", "expected_supply"];
        let mut violations = vec![];
        let mut examined = 0usize;

        for t in TABLES {
            let rows = c.query(
                "SELECT ic.relname, pg_get_indexdef(i.indexrelid)
                   FROM pg_index i
                   JOIN pg_class ic ON ic.oid = i.indexrelid
                  WHERE i.indrelid = to_regclass($1)
                    AND ic.relname LIKE '%availability%'",
                &[t],
            )?;
            examined += rows.len();
            if rows.is_empty() {
                violations.push(format!("{t} has no availability index at all"));
            }
            for r in &rows {
                // The key, not the INCLUDE payload: a column in INCLUDE cannot
                // be searched on, which is the whole distinction here.
                let def: String = r.get(1);
                let key = def.split(" INCLUDE ").next().unwrap_or(&def).to_string();
                for col in ["owner_id", "status_id"] {
                    if !key.contains(col) {
                        violations.push(format!(
                            "{}.{} does not carry {col} in its key, so the availability read \
                             has to join to find out whether a row counts",
                            t, r.get::<_, String>(0)));
                    }
                }
            }
        }
        Ok(Outcome::new(examined, violations))
    }
    /// D24 supply side. Five maintained quantities, and a sixth is a decision.
    ///
    /// The cap is the point rather than the number. `expected_supply` folds
    /// intentions, assertions and facts into one row, and every additional
    /// maintained quantity is another thing that can disagree with the others.
    /// Generated columns are excluded because they cannot disagree — they are the
    /// arithmetic, not another input to it.
    pub fn s26_five_maintained_quantities(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT a.attname
               FROM pg_class c
               JOIN pg_namespace n ON n.oid = c.relnamespace
               JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
              WHERE n.nspname = $1 AND c.relname = 'expected_supply'
                AND a.attname LIKE 'quantity\\_%'
                AND a.attgenerated = ''
              ORDER BY a.attname",
            &[&APP_SCHEMA],
        )?;
        let names: Vec<String> = rows.iter().map(|r| r.get(0)).collect();
        let mut violations = vec![];
        if names.len() > 5 {
            violations.push(format!(
                "expected_supply carries {} maintained quantity columns ({}), and D24 caps \
                 them at five so that a sixth has to be a recorded decision",
                names.len(), names.join(", ")));
        }
        Ok(Outcome::new(names.len(), violations))
    }
    /// D70. A cache is only as good as the thing that invalidates it.
    ///
    /// Question 80: *"a missed invalidation means the floor runs on stale weights
    /// and nothing detects it."* The epoch is what detects it, so **an input the
    /// epoch cannot see is an answer that never goes stale** — which is worse
    /// than a slow resolver and quieter than a wrong one.
    ///
    /// The resolver reads the scope table, the value tables, and the closures
    /// D22's matching language walks. Each has to reach the epoch by some route:
    /// `policy_change` for the values, `policy_binding` for the scope, and
    /// `projection_step` for the closures, because a closure has no timestamp of
    /// its own and S7 forbids the trigger that would give it one.
    ///
    /// Read from the catalogue rather than listed, so a fourth `%_policy` table —
    /// which D22 expects, seven of the ten named kinds have no value table yet —
    /// is covered the day it is created rather than the day somebody remembers.
    ///
    /// **The effective-range half is deliberately not here.** No epoch can see a
    /// range opening or closing, because nothing is written when it does; that is
    /// `policy_next_boundary`, and it is a different question from this one.
    pub fn s50_the_epoch_sees_every_resolver_input(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        let body: String = c
            .query_one(
                "SELECT pg_get_functiondef(p.oid)
                   FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
                  WHERE n.nspname = $1 AND p.proname = 'policy_epoch'",
                &[&APP_SCHEMA],
            )?
            .get(0);

        // What a resolution reads: the scope, the values, and the closures.
        let inputs: Vec<String> = c
            .query(
                "SELECT table_name FROM information_schema.tables
                  WHERE table_schema = $1
                    AND (table_name LIKE '%\\_policy'
                         OR table_name IN ('policy_binding',
                                           'item_class_closure', 'party_class_closure'))
                  ORDER BY table_name",
                &[&APP_SCHEMA],
            )?
            .iter()
            .map(|r| r.get::<_, String>(0))
            .collect();

        // How each may reach the epoch. A value table reaches it through the
        // change record D22 pairs with every version and J13 asserts; a closure
        // through the maintainer that rebuilds it.
        let route = |t: &str| -> &'static str {
            if t.ends_with("_closure") {
                "projection_step"
            } else if t == "policy_binding" {
                "policy_binding"
            } else {
                "policy_change"
            }
        };

        let mut violations = vec![];
        for t in &inputs {
            let r = route(t);
            if !body.contains(r) {
                violations.push(format!(
                    "a resolution reads {t} and the epoch does not reach it, by {r} or \
                     otherwise, so a change to it leaves every cached answer valid forever"
                ));
            }
        }
        Ok(Outcome::new(inputs.len(), violations))
    }
    /// D64. The order the maintainers run in, checked against what exists.
    ///
    /// S5's third leg reads `projection_%_rebuild` and is right to: a rebuild
    /// with no registry row is a projection nobody declared. **It cannot see a
    /// step.** `projection_package_stamp` and `projection_stock_resolve_locations`
    /// do not match that pattern, own no column, and exist to finish what another
    /// function started — so nothing required them to be registered and nothing
    /// said they had to run.
    ///
    /// The order lived in fifteen hand-written calls in the fixture. A scheduler
    /// written from the decision record would have called eight functions and
    /// skipped two, and S43 would have passed throughout, because the family does
    /// write those columns.
    ///
    /// So this is S5's diff over the wider set, in both directions. The only
    /// exclusion is `projection_run_all`: an orchestrator that ran itself would
    /// not terminate.
    pub fn s49_every_maintainer_is_a_declared_step(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        // Orchestrators and request helpers are not steps: they call steps or
        // mark work. D107 added run_dirty / refresh_tenant / mark_dirty beside
        // run_all. Listed rather than pattern-matched so a new real maintainer
        // still has to appear in projection_step.
        const NOT_STEPS: &[&str] = &[
            "projection_run_all",
            "projection_run_dirty",
            "projection_refresh_tenant",
            "projection_mark_dirty",
        ];
        let mut violations = vec![];

        // 1. Every maintainer is declared.
        //
        // **Scoped to VOLATILE functions**, because the name prefix alone is not
        // the property being asserted. A maintainer writes; D95's
        // `projection_age` reads, and it tripped this check on the first run
        // after it was written. The scope is `provolatile = 'v'` rather than a
        // name exception because Postgres enforces it — a STABLE function
        // *cannot* write, so it cannot be a maintainer no matter what it is
        // called, and a future reader in this family is covered without anyone
        // remembering to add it. S17's scope is written down for the same reason.
        let undeclared = c.query(
            "SELECT p.proname
               FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
              WHERE n.nspname = $1
                AND p.proname LIKE 'projection\\_%'
                AND p.proname <> ALL ($2)
                AND p.provolatile = 'v'
                AND NOT EXISTS (SELECT 1 FROM projection_step s
                                 WHERE s.function_name = p.proname)",
            &[&APP_SCHEMA, &NOT_STEPS],
        )?;
        for r in &undeclared {
            violations.push(format!(
                "{} maintains a projection and is in no step, so nothing says it has to run \
                 and a scheduler reading the registry would never call it",
                r.get::<_, String>(0)));
        }

        // 2. Every declared step exists.
        let phantom = c.query(
            "SELECT s.function_name FROM projection_step s
              WHERE NOT EXISTS (SELECT 1 FROM pg_proc p
                                  JOIN pg_namespace n ON n.oid = p.pronamespace
                                 WHERE n.nspname = $1 AND p.proname = s.function_name)",
            &[&APP_SCHEMA],
        )?;
        for r in &phantom {
            violations.push(format!(
                "step {} names a function that does not exist, so the run either errors or \
                 silently stops depending on who wrote the caller",
                r.get::<_, String>(0)));
        }

        let declared: i64 = c
            .query_one("SELECT count(*) FROM projection_step", &[])?
            .get(0);
        Ok(Outcome::new(declared as usize + undeclared.len(), violations))
    }
    /// D57. The identifier decision, made permanent.
    ///
    /// This asserts the **absence** of `gen_random_uuid()` rather than the
    /// presence of `uuidv7()`, and the asymmetry is the point. Nothing is going
    /// to revert a column that already defaults to v7. What is going to happen is
    /// a new table written from the pattern of the forty-three that came before
    /// it, because that is what copying an existing `CREATE TABLE` gives you.
    ///
    /// Phrased the other way it would also be wrong: a column may legitimately
    /// have no default at all, and demanding `uuidv7()` everywhere would fail on
    /// every one that takes its identifier from somewhere else.
    pub fn s47_identifiers_are_time_ordered(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT c.relname, a.attname
               FROM pg_class c
               JOIN pg_namespace n ON n.oid = c.relnamespace
               JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
               JOIN pg_attrdef d ON d.adrelid = c.oid AND d.adnum = a.attnum
              WHERE n.nspname = $1 AND c.relkind = 'r'
                AND pg_get_expr(d.adbin, d.adrelid) = 'gen_random_uuid()'",
            &[&APP_SCHEMA],
        )?;
        // The population is every uuid column carrying any default, so a schema
        // that stopped defaulting identifiers altogether reports as examining
        // nothing rather than as passing.
        let examined: i64 = c
            .query_one(
                "SELECT count(*)
                   FROM pg_class c
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0
                                      AND NOT a.attisdropped
                   JOIN pg_attrdef d ON d.adrelid = c.oid AND d.adnum = a.attnum
                  WHERE n.nspname = $1 AND c.relkind = 'r'
                    AND format_type(a.atttypid, a.atttypmod) = 'uuid'",
                &[&APP_SCHEMA],
            )?
            .get(0);
        let violations = rows
            .iter()
            .map(|r| {
                format!(
                    "{}.{} defaults to gen_random_uuid(), which is random, so every insert \
                     lands at an arbitrary point in the index instead of its right edge",
                    r.get::<_, String>(0), r.get::<_, String>(1))
            })
            .collect();
        Ok(Outcome::new(examined as usize, violations))
    }
    /// D58, borrowing S40's shape for the other side of the money boundary.
    ///
    /// D50 said what a price is not and S40 keeps that true. This is the same
    /// sentence about cost. `stock_movement.unit_cost_minor` is what we paid for
    /// the goods; the ways it erodes are a landed cost, a valuation, or a tax
    /// column appearing beside it.
    ///
    /// A landed cost is freight and duty apportioned across lines, which is a
    /// computation over facts living elsewhere — store one and it disagrees with
    /// its own inputs the moment a freight invoice is corrected. A valuation
    /// depends on a costing method nobody has chosen and D40 puts stock valuation
    /// outside this system entirely.
    ///
    /// `consignment.price_minor` stays out of scope for the same reason S40
    /// leaves it out: a carrier's quoted charge is a number they gave us.
    pub fn s48_no_cost_derivations(c: &mut Client) -> Result<Outcome, postgres::Error> {
        const TABLES: &[&str] = &["stock_movement", "stock", "lot", "item"];
        const DENIED: &[&str] = &[
            "landed_cost", "landed_cost_minor", "total_cost", "total_cost_minor",
            "extended_cost", "extended_cost_minor", "valuation", "valuation_minor",
            "value_minor", "average_cost", "average_cost_minor", "standard_cost",
            "standard_cost_minor", "cost_tax", "cost_tax_minor",
        ];
        let rows = c.query(
            "SELECT c.relname, a.attname
               FROM pg_class c
               JOIN pg_namespace n ON n.oid = c.relnamespace
               JOIN pg_attribute a ON a.attrelid = c.oid
                                  AND a.attnum > 0 AND NOT a.attisdropped
              WHERE n.nspname = $1 AND c.relname = ANY($2) AND a.attname = ANY($3)",
            &[&APP_SCHEMA, &TABLES, &DENIED],
        )?;
        let examined: i64 = c
            .query_one(
                "SELECT count(*)
                   FROM pg_class c
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   JOIN pg_attribute a ON a.attrelid = c.oid
                                      AND a.attnum > 0 AND NOT a.attisdropped
                  WHERE n.nspname = $1 AND c.relname = ANY($2)",
                &[&APP_SCHEMA, &TABLES],
            )?
            .get(0);
        let violations = rows
            .iter()
            .map(|r| {
                format!("{}.{} stores a cost derived from facts that live elsewhere",
                    r.get::<_, String>(0), r.get::<_, String>(1))
            })
            .collect();
        Ok(Outcome::new(examined as usize, violations))
    }
    /// D55. The write side of tenancy, which nothing was looking at.
    ///
    /// Every RLS check in this suite reads `polqual` — the USING expression —
    /// because that is where tenancy visibly lives. S8 checks FORCE is on, S9
    /// checks the shape is one of the permitted ones. Both were passing on the
    /// shared-reference tables while any tenant could write rows every other
    /// tenant reads, because **a policy with no WITH CHECK gets its USING
    /// expression as one**, and "readable when shared or ours" is a disastrous
    /// thing to say about writes.
    ///
    /// Three claims, and each is a way the pair can rot:
    ///
    /// 1. A shared-read policy is `FOR SELECT`. If it were `FOR ALL` it would OR
    ///    itself back into the write path and undo the split.
    /// 2. It never appears without its write half. A table with only the read
    ///    policy is either unwritable or, if somebody "fixes" that by widening
    ///    the read policy, back where it started.
    /// 3. The write half declares an explicit WITH CHECK. Omitting it is exactly
    ///    the original defect, and the omission is invisible in `pg_policy`
    ///    except as a NULL nobody thought to look at.
    ///
    /// What this cannot do is prove the predicate is right; it proves the pair is
    /// shaped as declared. The behaviour — that `spork_app` cannot insert,
    /// update or delete a row with a NULL tenant, and that `spork_platform`
    /// can — is verified by the negative controls, which is where behaviour is
    /// checked in this repository.
    pub fn s46_shared_tables_guard_their_writes(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        const SHARED_READ: &str = "((tenant_id IS NULL) OR (tenant_id = current_tenant()))";
        let mut violations = vec![];

        let rows = c.query(
            "SELECT c.relname, p.polname, p.polcmd::text,
                    p.polwithcheck IS NOT NULL AS has_check,
                    pg_get_expr(p.polqual, p.polrelid) AS using_expr
               FROM pg_policy p JOIN pg_class c ON c.oid = p.polrelid
              WHERE c.relnamespace = to_regnamespace($1)",
            &[&APP_SCHEMA],
        )?;

        // Every table carrying the shared-read expression, and how.
        let readers: Vec<(String, String, String)> = rows
            .iter()
            .filter(|r| r.get::<_, String>(4) == SHARED_READ)
            .map(|r| (r.get(0), r.get(1), r.get(2)))
            .collect();

        for (tbl, pol, cmd) in &readers {
            // 1. Read-only. 'r' is SELECT; '*' is ALL.
            if cmd != "r" {
                violations.push(format!(
                    "{tbl}.{pol} admits shared rows and applies to {cmd} rather than SELECT \
                     alone, so it ORs itself back into the write path"
                ));
            }
            // 2. Paired with a write half.
            let write = rows.iter().find(|r| {
                r.get::<_, String>(0) == *tbl && r.get::<_, String>(4) != SHARED_READ
            });
            match write {
                None => violations.push(format!(
                    "{tbl} admits shared rows for reading and has no write policy beside it"
                )),
                // 3. The write half states its WITH CHECK rather than inheriting.
                Some(w) if !w.get::<_, bool>(3) => violations.push(format!(
                    "{}.{} has no explicit WITH CHECK, so it inherits its USING expression, \
                     which is the defect this pair exists to remove",
                    tbl,
                    w.get::<_, String>(1)
                )),
                Some(_) => {}
            }
        }

        Ok(Outcome::new(readers.len(), violations))
    }
    /// D53, borrowing S40's shape for a column that was dropped rather than
    /// never added.
    ///
    /// D25 sketched `fulfilment.progress` as a projection over allocations,
    /// movements, packages and consignment. Two things were wrong with it.
    /// `stock_movement` carries no reference to a fulfilment, so the source it
    /// named is unreachable; and a commitment is picked, packed and despatched to
    /// three different extents at once, so one value has to choose which of them
    /// it means.
    ///
    /// D25's own argument for dropping `order.fulfilment_status` — a third hop,
    /// an order has few fulfilments, compute it on read — applies one level down
    /// and was not applied there. A fulfilment has few lines.
    ///
    /// So the column is gone and this keeps it gone. A rollup that looks obviously
    /// useful is exactly the kind that reappears, and reappearing quietly is what
    /// the register exists to prevent.
    pub fn s44_no_stored_fulfilment_rollup(c: &mut Client) -> Result<Outcome, postgres::Error> {
        const TABLES: &[&str] = &["fulfilment", "order", "consignment"];
        const DENIED: &[&str] = &[
            "progress", "progress_status", "fulfilment_status", "completion",
            "completion_pct", "percent_complete", "pick_status", "pack_status",
        ];
        let rows = c.query(
            "SELECT c.relname, a.attname
               FROM pg_class c
               JOIN pg_namespace n ON n.oid = c.relnamespace
               JOIN pg_attribute a ON a.attrelid = c.oid
                                  AND a.attnum > 0 AND NOT a.attisdropped
              WHERE n.nspname = $1 AND c.relname = ANY($2) AND a.attname = ANY($3)",
            &[&APP_SCHEMA, &TABLES, &DENIED],
        )?;
        let examined: i64 = c
            .query_one(
                "SELECT count(*)
                   FROM pg_class c
                   JOIN pg_namespace n ON n.oid = c.relnamespace
                   JOIN pg_attribute a ON a.attrelid = c.oid
                                      AND a.attnum > 0 AND NOT a.attisdropped
                  WHERE n.nspname = $1 AND c.relname = ANY($2)",
                &[&APP_SCHEMA, &TABLES],
            )?
            .get(0);
        let violations = rows
            .iter()
            .map(|r| {
                format!(
                    "{}.{} stores a rollup of its lines, which can disagree with them and \
                     has to pick one of picked, packed and despatched to mean",
                    r.get::<_, String>(0), r.get::<_, String>(1))
            })
            .collect();
        Ok(Outcome::new(examined as usize, violations))
    }

    /// S53. D94, and the reason the obvious fix for question 161 was not taken.
    ///
    /// `SECURITY DEFINER` runs the body as the function's owner. **A superuser
    /// bypasses row-level security unconditionally**, and so does any role with
    /// BYPASSRLS, so a definer owned by one is a hole in the tenancy boundary --
    /// and a definer the application is granted EXECUTE on is a hole handed over
    /// deliberately. That is D55's write hole arriving through a door built for
    /// safety.
    ///
    /// The safe shape was already here before anything checked it: fifteen
    /// projection definers owned by `spork_projection_owner`, which is neither,
    /// against tables with FORCE ROW LEVEL SECURITY so RLS applies to the owner
    /// too. This asserts that nobody takes the shortcut next time.
    pub fn s53_a_definer_stays_inside_row_security(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT p.proname, r.rolname, r.rolsuper, r.rolbypassrls
               FROM pg_proc p
               JOIN pg_namespace n ON n.oid = p.pronamespace
               JOIN pg_roles r ON r.oid = p.proowner
              WHERE n.nspname = 'public' AND p.prosecdef
                AND (r.rolsuper OR r.rolbypassrls)",
            &[],
        )?;
        let violations = rows
            .iter()
            .map(|r| {
                format!(
                    "{} is SECURITY DEFINER and owned by {}, which {}, so its body runs outside row-level security",
                    r.get::<_, String>(0),
                    r.get::<_, String>(1),
                    if r.get::<_, bool>(2) { "is a superuser" } else { "has BYPASSRLS" }
                )
            })
            .collect();
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
                  WHERE n.nspname = 'public' AND p.prosecdef",
                &[],
            )?
            .get(0);
        Ok(Outcome::new(examined as usize, violations))
    }

    /// S54. D94. The declared side of the mediated write, diffed both ways.
    ///
    /// D90 named the pattern -- *"where a cross-row rule cannot be a constraint,
    /// it becomes a grant plus a function"* -- and **both of its instances were
    /// half-built, in opposite directions**: D90's function had no privilege and
    /// could not run, D89's app kept the privilege and could skip the function.
    /// Each was invisible to every check in the suite, because a convention nobody
    /// wrote down is a convention nothing can test.
    ///
    /// Four properties per registered column, and each catches a different way of
    /// getting it wrong: the app must not hold UPDATE (or the function is
    /// optional), the mediation owner must (or the function cannot run), the
    /// function must exist, and it must be a definer owned by that role (or it
    /// runs as the caller and is back to having no privilege).
    pub fn s54_a_mediated_write_is_actually_mediated(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT m.table_name, m.column_name, m.function_name,
                    EXISTS (SELECT 1 FROM information_schema.column_privileges g
                             WHERE g.table_name = m.table_name
                               AND g.column_name = m.column_name
                               AND g.grantee = 'spork_app'
                               AND g.privilege_type = 'UPDATE') AS app_may_write,
                    EXISTS (SELECT 1 FROM information_schema.column_privileges g
                             WHERE g.table_name = m.table_name
                               AND g.column_name = m.column_name
                               AND g.grantee = 'spork_mediation_owner'
                               AND g.privilege_type = 'UPDATE') AS owner_may_write,
                    EXISTS (SELECT 1 FROM pg_proc p
                              JOIN pg_namespace n ON n.oid = p.pronamespace
                             WHERE n.nspname = 'public'
                               AND p.proname = m.function_name) AS function_exists,
                    EXISTS (SELECT 1 FROM pg_proc p
                              JOIN pg_namespace n ON n.oid = p.pronamespace
                              JOIN pg_roles r ON r.oid = p.proowner
                             WHERE n.nspname = 'public'
                               AND p.proname = m.function_name
                               AND p.prosecdef
                               AND r.rolname = 'spork_mediation_owner') AS properly_owned
               FROM mediated_write m",
            &[],
        )?;

        let mut violations = vec![];
        for r in &rows {
            let (t, col, f): (String, String, String) = (r.get(0), r.get(1), r.get(2));
            if r.get::<_, bool>(3) {
                violations.push(format!(
                    "{t}.{col} is mediated by {f} and the app still holds UPDATE on it, so the function is optional"
                ));
            }
            if !r.get::<_, bool>(4) {
                violations.push(format!(
                    "{t}.{col} is mediated by {f} and the mediation owner has no UPDATE on it, so the function cannot perform the write it exists for"
                ));
            }
            if !r.get::<_, bool>(5) {
                violations.push(format!("{t}.{col} names {f}, which does not exist"));
            } else if !r.get::<_, bool>(6) {
                violations.push(format!(
                    "{f} guards {t}.{col} and is not a SECURITY DEFINER owned by spork_mediation_owner, so it runs with the caller's rights -- which are the rights it exists to withhold"
                ));
            }
        }
        Ok(Outcome::new(rows.len(), violations))
    }

    /// S35. D43 wrote it, D96 gave it something to be true of.
    ///
    /// The 856's `S` and `O` levels are documents and its `T` and `P` levels are
    /// pallets and cartons, and D43 stores the order level as a node for that
    /// reason. So a declared tree contains rows that are not things, and **a
    /// document node must carry no licence plate and become no package.**
    ///
    /// This was pending for eleven migrations on "the collapse at receipt does not
    /// exist yet", which was true of the code and hid that the column the sentence
    /// needed was missing too. It is now two CHECKs rather than a job, which is a
    /// stronger result than the register asked for: the rows cannot be written at
    /// all rather than being found afterwards.
    ///
    /// So this asserts the constraints exist, in S12's idiom — the presence of a
    /// guard rather than the absence of what it guards against — because a CHECK
    /// that was dropped leaves no violating row behind to find.
    pub fn s35_a_document_node_is_not_a_thing(c: &mut Client) -> Result<Outcome, postgres::Error> {
        let required = [
            ("asserted_unit_document_has_no_sscc_ck", "a node read as a document carries no SSCC"),
            ("asserted_unit_document_has_no_package_ck", "a node read as a document becomes no package"),
        ];
        let mut violations = vec![];
        for (name, says) in required {
            let present: bool = c
                .query_one(
                    "SELECT EXISTS (SELECT 1 FROM pg_constraint
                                     WHERE conrelid = 'asserted_unit'::regclass
                                       AND contype = 'c' AND conname = $1)",
                    &[&name],
                )?
                .get(0);
            if !present {
                violations.push(format!(
                    "{name} is gone, so nothing enforces that {says}"
                ));
            }
        }
        // And the property itself, over whatever rows exist.
        let rows = c.query(
            "SELECT id::text, level_code, sscc IS NOT NULL AS has_sscc
               FROM asserted_unit
              WHERE resolved_physical = false
                AND (sscc IS NOT NULL OR resolved_package_id IS NOT NULL)",
            &[],
        )?;
        for r in &rows {
            violations.push(format!(
                "asserted_unit {} is read as a document at level '{}' and still {}",
                r.get::<_, String>(0),
                r.get::<_, String>(1),
                if r.get::<_, bool>(2) { "carries an SSCC" } else { "resolves to a package" }
            ));
        }
        let examined: i64 = c
            .query_one("SELECT count(*) FROM asserted_unit WHERE resolved_physical IS NOT NULL", &[])?
            .get(0);
        Ok(Outcome::new(examined as usize + required.len(), violations))
    }

    /// S55. D104. S43's converse: the family writes the column, and nothing else does.
    ///
    /// S43 checks that a registered family actually writes the column it claims.
    /// **Nothing checked the other direction.** D101 is what that costs:
    /// `stock.resolved_location_id` was written by `projection_stock_rebuild` at
    /// ordinal 30 and by `projection_stock_resolve_locations` at 70, each correct
    /// about the cells it knew and neither aware of the other. Values were always
    /// right, so J1 and J7 passed for fifty-two migrations; only D68's write count
    /// could see the oscillation, and only once the fixture reached a package-held
    /// cell.
    ///
    /// The shared write was invisible to S43 by construction: both functions are in
    /// the same `projection_stock%` family, and S43's unit is the family. So this
    /// check does three things S43 cannot:
    ///
    /// 1. Finds every `projection_%` VOLATILE function whose body **writes** the
    ///    column on the registered table (UPDATE … SET col =, or INSERT INTO table
    ///    with a DO UPDATE SET col =), not merely mentions its name.
    /// 2. Requires every such writer to be in the registered family.
    /// 3. Requires a column with more than one writer to be on the **declared
    ///    multi-writer allowlist**, with exactly the co-owners listed. The D101 pair
    ///    is the only entry today: step 30 owns the location-held arm and step 70 the
    ///    package-held arm of the same two columns. A third writer, or a dual write
    ///    of `stock.quantity`, fails without anyone having to wait for a fixture
    ///    shape.
    ///
    /// Greppable, with S11's and S43's limit: a maintainer writing through dynamic
    /// SQL would pass while doing the wrong thing. The unit of report is one
    /// projected column.
    pub fn s55_a_projected_column_has_one_writer(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        // Declared co-owners for columns two family members deliberately share by
        // arm. Adding a row here is a decision, not a silence: D101 is why.
        // Sorted, so the catalogue comparison is order-independent.
        const MULTI_WRITER: &[(&str, &str, &str)] = &[
            (
                "stock",
                "resolved_location_id",
                "projection_stock_rebuild,projection_stock_resolve_locations",
            ),
            (
                "stock",
                "site_id",
                "projection_stock_rebuild,projection_stock_resolve_locations",
            ),
        ];

        // One query: every registered column with the sorted list of writers that
        // both touch its table and assign its column. Table match allows the
        // optional schema qualifier and the quoted form UPDATE "order" uses.
        const CATALOGUE: &str = "
            WITH funcs AS (
                SELECT p.proname,
                       lower(pg_get_functiondef(p.oid)) AS def
                  FROM pg_proc p
                  JOIN pg_namespace n ON n.oid = p.pronamespace
                 WHERE n.nspname = $1
                   AND p.proname LIKE 'projection\\_%'
                   AND p.provolatile = 'v'
                   -- Orchestrators / request helpers are not column writers
                   -- (same set as S49's NOT_STEPS; D107).
                   AND p.proname <> ALL (ARRAY[
                        'projection_run_all',
                        'projection_run_dirty',
                        'projection_refresh_tenant',
                        'projection_mark_dirty'
                   ])
            ),
            hits AS (
                SELECT r.table_name, r.column_name, r.function_name AS registered,
                       f.proname AS writer
                  FROM projection_rebuild r
                  JOIN funcs f
                    ON f.def ~ ('(update|insert[[:space:]]+into)[[:space:]]+(public\\.)?\"?'
                                || r.table_name || '\"?[[:space:]]')
                   AND f.def ~ ('(^|[^a-z0-9_])' || r.column_name || '[[:space:]]*=')
                 WHERE EXISTS (SELECT 1 FROM pg_proc p
                                 JOIN pg_namespace n ON n.oid = p.pronamespace
                                WHERE n.nspname = $1 AND p.proname = r.function_name)
            )
            SELECT table_name, column_name, registered,
                   coalesce(string_agg(writer, ',' ORDER BY writer), '') AS writers
              FROM hits
             GROUP BY table_name, column_name, registered
             ORDER BY table_name, column_name";

        let rows = c.query(CATALOGUE, &[&APP_SCHEMA])?;
        // Also examine registered columns with zero writers — S43 covers the
        // family-writes-it half, but a column with no table-scoped write at all is
        // still a finding for this converse when the greppable form would have
        // seen one. Count examined from the registry, not only the hits.
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM projection_rebuild r
                  WHERE EXISTS (SELECT 1 FROM pg_proc p
                                  JOIN pg_namespace n ON n.oid = p.pronamespace
                                 WHERE n.nspname = $1 AND p.proname = r.function_name)",
                &[&APP_SCHEMA],
            )?
            .get(0);

        let mut violations = vec![];
        let mut seen = std::collections::HashSet::new();
        for r in &rows {
            let (table, col, registered, writers_csv): (String, String, String, String) =
                (r.get(0), r.get(1), r.get(2), r.get(3));
            seen.insert((table.clone(), col.clone()));
            let writers: Vec<&str> = if writers_csv.is_empty() {
                vec![]
            } else {
                writers_csv.split(',').collect()
            };
            let family = registered
                .strip_suffix("_rebuild")
                .unwrap_or(registered.as_str());

            let outsiders: Vec<&str> = writers
                .iter()
                .copied()
                .filter(|w| !w.starts_with(family))
                .collect();
            if !outsiders.is_empty() {
                violations.push(format!(
                    "{table}.{col} is registered to {registered} and also written by {}, \
                     which is outside the {family}% family, so two rebuilds can disagree \
                     about the same column with nothing to say they share it",
                    outsiders.join(", ")
                ));
            }

            if writers.len() > 1 {
                let allowed = MULTI_WRITER
                    .iter()
                    .find(|(t, c, _)| *t == table && *c == col)
                    .map(|(_, _, co)| *co);
                match allowed {
                    None => {
                        violations.push(format!(
                            "{table}.{col} is written by {} and is not on the multi-writer \
                             allowlist, so either one of them should stop or the sharing \
                             needs a decision the way D101's pair did",
                            writers.join(", ")
                        ));
                    }
                    Some(expected) => {
                        if writers_csv != expected {
                            violations.push(format!(
                                "{table}.{col} is multi-writer allowlisted for [{expected}] \
                                 but the catalogue found [{writers_csv}], so the declaration \
                                 and the bodies have drifted"
                            ));
                        }
                    }
                }
            }
        }

        // Registered columns that produced no hit at all still count as examined;
        // S43 is responsible for "family writes it". Nothing further here.
        let _ = seen;

        Ok(Outcome::new(examined as usize, violations))
    }

    /// S56. D104. A maintainer says which migration last changed it.
    ///
    /// Migration 56 was first written against migration 21's body of
    /// `projection_expected_supply_rebuild`, because that is the migration D45 and
    /// J26 name. Two later migrations had replaced it — D65's `received_in_full`
    /// close and D68's idempotency guard — so rebuilding from 21 silently reverted
    /// both. D68's write-counting test caught it within a minute.
    ///
    /// **A `CREATE OR REPLACE` carries no record of what it replaced**, and the
    /// decision record names the migration that introduced a function rather than
    /// the one that last changed it. The marker is a body comment so that pasting
    /// an older body keeps the older marker:
    ///
    /// ```text
    /// -- last changed: migration NNN (Dxxx)
    /// ```
    ///
    /// Scoped to VOLATILE `projection_%` functions for the same reason as S49: a
    /// maintainer writes, and `projection_age` only reads. The orchestrator is
    /// included — it is a maintainer of the run, and it has been replaced too.
    pub fn s56_a_maintainer_says_when_it_last_changed(
        c: &mut Client,
    ) -> Result<Outcome, postgres::Error> {
        let rows = c.query(
            "SELECT p.proname
               FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
              WHERE n.nspname = $1
                AND p.proname LIKE 'projection\\_%'
                AND p.provolatile = 'v'
                AND pg_get_functiondef(p.oid)
                    !~ '-- last changed: migration [0-9]+ \\(D[0-9]+\\)'
              ORDER BY p.proname",
            &[&APP_SCHEMA],
        )?;
        let examined: i64 = c
            .query_one(
                "SELECT count(*) FROM pg_proc p
                   JOIN pg_namespace n ON n.oid = p.pronamespace
                  WHERE n.nspname = $1
                    AND p.proname LIKE 'projection\\_%'
                    AND p.provolatile = 'v'",
                &[&APP_SCHEMA],
            )?
            .get(0);

        let violations = rows
            .iter()
            .map(|r| {
                format!(
                    "{} has no last-changed marker, so the next CREATE OR REPLACE \
                     has no record of which body it is replacing and will look at the \
                     migration that introduced it — which is how migration 56 nearly \
                     reverted two decisions",
                    r.get::<_, String>(0)
                )
            })
            .collect();
        Ok(Outcome::new(examined as usize, violations))
    }
}
