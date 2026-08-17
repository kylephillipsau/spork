//! The receipt handover: which claims move from a promise to the stock it became.
//!
//! D24 states it in one sentence: *"At receipt, in the same transaction as the
//! movements, allocations are re-pointed at the new `stock_id`, `bound_at` is
//! stamped, and `origin_expected_supply_id` is retained."*
//!
//! Migration 21 built everything around that sentence and nothing that performs
//! it, and the fixture said so immediately: with a receipt folded, a fully
//! received promise went on carrying the claims made against it and
//! `quantity_promisable` came out negative. J58 is the finding that catches the
//! state; this is the code that stops producing it.
//!
//! # Why it lives here
//!
//! Same reason as [`crate::correction`]. The decision spans rows — which claims,
//! against which promise, covered by how much arrived — and Postgres has no
//! cross-row CHECK. S7 rejects triggers and D25 forbids the validation kind by
//! name. The write path has to read the claims anyway in order to update them,
//! so a pure function over what it already holds costs nothing it was not paying.
//!
//! J4, J9 and J58 assert the same properties against stored data, as findings,
//! for rows that arrive by a restore, a backfill, or a path nobody has written.
//!
//! # What this is not
//!
//! It does not decide **who** gets the goods. That is the allocator, and question
//! 26 has not been answered. This runs after the fact on claims that already
//! exist, and its only ordering decision is which of them the arrival covers when
//! it cannot cover all — for which the rule is the dullest defensible one: the
//! claim bound first is served first.
//!
//! It does not split a claim. An arrival that covers part of a larger claim
//! re-points nothing for it. Splitting an allocation is an act with a decision
//! behind it — questions 26 and 34 both circle it — and a receiver quietly
//! halving somebody's commitment is not a receipt, it is a re-allocation
//! performed by the wrong person.

use chrono::{DateTime, Utc};
use nylonite_policy::{apply_clamps, explain, resolve, Candidate, PolicyKind};
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::error::ApiError;

/// The states in which a claim still claims something. Identical to J3's and
/// J4's set, and identical for the same reason: a despatched or released
/// allocation has stopped claiming, whichever side it claimed from.
pub const ACTIVE: &[&str] = &["allocated", "picking", "picked", "packed"];

/// A claim standing against a promise, as the write path already holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub expected_supply_id: Option<Uuid>,
    pub quantity: i64,
    pub state: String,
    pub bound_at: DateTime<Utc>,
}

/// Goods landing against a promise, and the stock cell they became.
#[derive(Clone, Debug)]
pub struct Arrival {
    pub tenant_id: Uuid,
    pub expected_supply_id: Uuid,
    /// The cell the receipt movement created or added to. J5's resolved cell,
    /// not a location: a claim holds a cell.
    pub stock_id: Uuid,
    pub quantity_received: i64,
    pub occurred_at: DateTime<Utc>,
    /// Whether the promise has closed. A closed promise still hands over what
    /// physically arrived; what it must not do is pretend the rest is coming.
    pub promise_closed: bool,
}

/// One claim moving from the promise to the stock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repoint {
    pub allocation_id: Uuid,
    pub stock_id: Uuid,
    /// D24: stamped at the handover, because the claim is now bound to something
    /// that exists rather than to something promised.
    pub bound_at: DateTime<Utc>,
    /// D24: retained, so "this unit was cross-docked against Coles PO 88421"
    /// stays answerable after the promise it names has been fully received.
    pub origin_expected_supply_id: Uuid,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    /// A claim on another tenant's books cannot be moved by our receipt. RLS
    /// would hide it; this says so rather than silently seeing nothing.
    DifferentTenant { claim: Uuid },
    /// A claim against some other promise has no business in this handover.
    DifferentPromise { claim: Uuid },
    /// The arrival covers less than the claims standing against it, and no claim
    /// was split. Not an error: a partial delivery is an ordinary event. It is
    /// reported because the difference between "everything moved" and "some of it
    /// did" is the difference between a promise that can be closed and one that
    /// cannot.
    Uncovered { claim: Uuid, quantity: i64 },
    /// Claims still stand against a promise that has closed and cannot deliver
    /// the rest. This is J58's finding raised at the moment it becomes true,
    /// rather than by a job the next time it runs.
    ClaimsOutliveThePromise { total: i64, received: i64 },
    /// A receipt of nothing is not a receipt.
    NothingArrived,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::DifferentTenant { claim } => write!(
                f,
                "allocation {claim} belongs to another tenant and cannot be moved by this receipt"
            ),
            Problem::DifferentPromise { claim } => write!(
                f,
                "allocation {claim} claims a different promise and is not part of this handover"
            ),
            Problem::Uncovered { claim, quantity } => write!(
                f,
                "allocation {claim} claims {quantity} and what arrived does not cover it, so it \
                 stays against the promise; splitting it is the allocator's decision, not the \
                 receiver's"
            ),
            Problem::ClaimsOutliveThePromise { total, received } => write!(
                f,
                "{total} is still claimed against a promise that closed having delivered \
                 {received}; the difference is committed to somebody and is not coming"
            ),
            Problem::NothingArrived => write!(f, "a receipt of no units is not a receipt"),
        }
    }
}

/// Plan the handover for one arrival.
///
/// Returns the re-points to apply and every problem found, in the same spirit as
/// [`crate::correction::check`]: all of them rather than the first, because a
/// receiver who fixes one thing and resubmits to find another stops reporting
/// what they see.
///
/// **The problems do not veto the re-points.** A partial delivery produces both a
/// list of claims that moved and a note about the ones that did not, and both are
/// true. D5 is why: this is on the path of an observation of the floor, and goods
/// on the dock are goods on the dock. The only thing that yields no re-points is
/// an arrival of nothing, which is not an observation of anything.
pub fn plan(arrival: &Arrival, claims: &[Claim]) -> (Vec<Repoint>, Vec<Problem>) {
    let mut problems = vec![];
    let mut repoints = vec![];

    if arrival.quantity_received <= 0 {
        problems.push(Problem::NothingArrived);
        return (repoints, problems);
    }

    // Everything that is genuinely a claim on this promise. The rejected ones are
    // named rather than skipped, because a claim silently ignored during a
    // handover is how a commitment goes missing.
    let mut eligible: Vec<&Claim> = vec![];
    for c in claims {
        if c.tenant_id != arrival.tenant_id {
            problems.push(Problem::DifferentTenant { claim: c.id });
            continue;
        }
        if c.expected_supply_id != Some(arrival.expected_supply_id) {
            problems.push(Problem::DifferentPromise { claim: c.id });
            continue;
        }
        // An inactive claim is not a problem and not a candidate. It stopped
        // claiming of its own accord, which is what those states mean.
        if !ACTIVE.contains(&c.state.as_str()) {
            continue;
        }
        eligible.push(c);
    }

    // First bound, first served. Deterministic, explainable to the person whose
    // order did not get the stock, and deliberately not clever: anything that
    // weighed urgency or customer against each other would be the allocator
    // making a decision at receipt time, which is question 26 and not this.
    eligible.sort_by(|a, b| a.bound_at.cmp(&b.bound_at).then(a.id.cmp(&b.id)));

    let mut remaining = arrival.quantity_received;
    for c in &eligible {
        if c.quantity <= remaining {
            remaining -= c.quantity;
            repoints.push(Repoint {
                allocation_id: c.id,
                stock_id: arrival.stock_id,
                bound_at: arrival.occurred_at,
                origin_expected_supply_id: arrival.expected_supply_id,
            });
        } else {
            problems.push(Problem::Uncovered {
                claim: c.id,
                quantity: c.quantity,
            });
        }
    }

    // A closed promise cannot deliver what is still claimed against it. Raised
    // here as well as by J58 because the job runs later and this is the moment
    // somebody is standing at the dock able to do something about it.
    if arrival.promise_closed {
        let stranded: i64 = eligible
            .iter()
            .filter(|c| !repoints.iter().any(|r| r.allocation_id == c.id))
            .map(|c| c.quantity)
            .sum();
        if stranded > 0 {
            problems.push(Problem::ClaimsOutliveThePromise {
                total: stranded,
                received: arrival.quantity_received,
            });
        }
    }

    (repoints, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        use chrono::TimeZone;
        Utc.with_ymd_and_hms(2026, 8, 4, h, m, 0).unwrap()
    }

    fn arrival() -> Arrival {
        Arrival {
            tenant_id: u(1),
            expected_supply_id: u(10),
            stock_id: u(20),
            quantity_received: 100,
            occurred_at: at(9, 0),
            promise_closed: false,
        }
    }

    fn claim(id: u128, quantity: i64, bound: (u32, u32)) -> Claim {
        Claim {
            id: u(id),
            tenant_id: u(1),
            expected_supply_id: Some(u(10)),
            quantity,
            state: "allocated".into(),
            bound_at: at(bound.0, bound.1),
        }
    }

    #[test]
    fn a_covered_claim_moves_to_the_stock_it_was_promised() {
        let (repoints, problems) = plan(&arrival(), &[claim(2, 30, (7, 0))]);
        assert_eq!(problems, vec![]);
        assert_eq!(
            repoints,
            vec![Repoint {
                allocation_id: u(2),
                stock_id: u(20),
                bound_at: at(9, 0),
                origin_expected_supply_id: u(10),
            }]
        );
    }

    #[test]
    fn the_promise_it_came_from_is_remembered() {
        // D24's third clause, and the reason it exists: after the handover
        // `expected_supply_id` says what is claimed now, so without this the
        // purchase order a cross-docked unit was committed against is gone.
        let (repoints, _) = plan(&arrival(), &[claim(2, 30, (7, 0))]);
        assert_eq!(repoints[0].origin_expected_supply_id, u(10));
    }

    #[test]
    fn the_binding_is_restamped_at_the_moment_of_arrival() {
        let (repoints, _) = plan(&arrival(), &[claim(2, 30, (7, 0))]);
        assert_eq!(repoints[0].bound_at, at(9, 0));
    }

    #[test]
    fn first_bound_is_first_served() {
        let a = Arrival { quantity_received: 50, ..arrival() };
        let (repoints, problems) = plan(&a, &[claim(3, 40, (8, 0)), claim(2, 40, (7, 0))]);
        assert_eq!(repoints.len(), 1);
        assert_eq!(repoints[0].allocation_id, u(2), "the earlier binding is served");
        assert_eq!(problems, vec![Problem::Uncovered { claim: u(3), quantity: 40 }]);
    }

    #[test]
    fn a_claim_larger_than_the_arrival_is_not_split() {
        let a = Arrival { quantity_received: 40, ..arrival() };
        let (repoints, problems) = plan(&a, &[claim(2, 90, (7, 0))]);
        assert_eq!(repoints, vec![], "splitting is the allocator's decision");
        assert_eq!(problems, vec![Problem::Uncovered { claim: u(2), quantity: 90 }]);
    }

    #[test]
    fn an_inactive_claim_is_neither_moved_nor_complained_about() {
        let mut released = claim(2, 30, (7, 0));
        released.state = "released".into();
        let (repoints, problems) = plan(&arrival(), &[released]);
        assert_eq!(repoints, vec![]);
        assert_eq!(problems, vec![], "it stopped claiming of its own accord");
    }

    #[test]
    fn a_claim_on_another_tenant_is_named_rather_than_skipped() {
        let mut other = claim(2, 30, (7, 0));
        other.tenant_id = u(99);
        let (repoints, problems) = plan(&arrival(), &[other]);
        assert_eq!(repoints, vec![]);
        assert_eq!(problems, vec![Problem::DifferentTenant { claim: u(2) }]);
    }

    #[test]
    fn a_claim_against_another_promise_is_not_part_of_this_handover() {
        let mut elsewhere = claim(2, 30, (7, 0));
        elsewhere.expected_supply_id = Some(u(11));
        let (_, problems) = plan(&arrival(), &[elsewhere]);
        assert_eq!(problems, vec![Problem::DifferentPromise { claim: u(2) }]);
    }

    #[test]
    fn a_closed_promise_still_hands_over_what_arrived() {
        // The goods are on the dock. D5 is why this does not refuse.
        let a = Arrival { quantity_received: 30, promise_closed: true, ..arrival() };
        let (repoints, problems) = plan(&a, &[claim(2, 30, (7, 0))]);
        assert_eq!(repoints.len(), 1);
        assert_eq!(problems, vec![], "everything claimed was covered");
    }

    #[test]
    fn claims_outliving_a_closed_promise_are_raised_at_the_dock() {
        // J58 finds this state later. Here somebody is standing in front of the
        // pallet and can still do something about it.
        let a = Arrival { quantity_received: 30, promise_closed: true, ..arrival() };
        let (repoints, problems) = plan(&a, &[claim(2, 30, (7, 0)), claim(3, 50, (8, 0))]);
        assert_eq!(repoints.len(), 1);
        assert_eq!(
            problems,
            vec![
                Problem::Uncovered { claim: u(3), quantity: 50 },
                Problem::ClaimsOutliveThePromise { total: 50, received: 30 },
            ]
        );
    }

    #[test]
    fn an_arrival_of_nothing_is_not_a_receipt() {
        let a = Arrival { quantity_received: 0, ..arrival() };
        let (repoints, problems) = plan(&a, &[claim(2, 30, (7, 0))]);
        assert_eq!(repoints, vec![]);
        assert_eq!(problems, vec![Problem::NothingArrived]);
    }

    #[test]
    fn one_receipt_reports_every_problem() {
        let mut other_tenant = claim(4, 10, (6, 0));
        other_tenant.tenant_id = u(99);
        let a = Arrival { quantity_received: 20, ..arrival() };
        let (_, problems) = plan(&a, &[other_tenant, claim(2, 90, (7, 0))]);
        assert_eq!(problems.len(), 2, "not just the first");
    }

    #[test]
    fn exactly_covered_leaves_nothing_behind() {
        let a = Arrival { quantity_received: 60, ..arrival() };
        let (repoints, problems) = plan(&a, &[claim(2, 30, (7, 0)), claim(3, 30, (8, 0))]);
        assert_eq!(repoints.len(), 2);
        assert_eq!(problems, vec![]);
    }
}

// ---------------------------------------------------------------------------
// The disposition: what a resolved receiving policy decides about a line
// ---------------------------------------------------------------------------

/// A receiving policy version, as the caller resolved it.
///
/// The caller does the resolving — `policy_candidate`, `resolve`, `policy_value`,
/// `apply_clamps` — because D22 puts the precedence order in code and D83 puts the
/// clamp directions beside it. **What arrives here is already clamped**, so a
/// tenant's twenty-five percent has become the platform's ten before this function
/// sees it, and nothing here can undo that.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReceivingPolicy {
    /// The `receiving_policy` row that won. Stamped on the line whatever the
    /// outcome, which is J63: *every act a policy governed names the version that
    /// governed it.*
    pub version: Uuid,
    /// `None` when the version states no over-delivery bound, which the column
    /// permits. **Not zero**, and the distinction is [`disposition`]'s to read.
    pub tolerance_over_pct: Option<f64>,
    pub require_lot: bool,
}

/// A line as counted at the dock.
///
/// **Both quantities are in the item's base unit**, which until D92 was true of
/// one of them. `expected_quantity` is a snapshot of the promise and always was
/// base; `entered_quantity` was a count of cartons, and this function subtracted
/// one from the other. The over-receipt tolerance was therefore inoperative for
/// every line entered in anything but eaches: twelve cartons against a hundred
/// units read as an eighty-eight-unit shortfall rather than as a twenty-unit
/// over-delivery, and no finding was ever raised.
///
/// `goods_receipt_line.quantity` is where the counted base quantity now lives, so
/// the caller has one to pass. The entered form stays on the row as the thing we
/// can quote back, per Principle 5, and never reaches this function.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CountedLine {
    pub expected_quantity: i64,
    /// What was counted, converted to the item's base unit.
    pub quantity: i64,
    pub has_lot: bool,
}

/// What the policy decided, and which version decided it.
#[derive(Clone, Debug, PartialEq)]
pub struct Disposition {
    /// Always present, and always the resolved version — **including on a
    /// refusal**. A rejection is a governed act too, and "why was this rejected"
    /// is the same question as "why was this accepted" with the same answer
    /// missing if nothing records it.
    pub receiving_policy_id: Uuid,
    pub accept: bool,
    /// A finding to raise alongside, in `discrepancy.kind` terms.
    pub raise: Option<&'static str>,
}

/// Decide a line against a resolved receiving policy.
///
/// The rules are the policy's own fields and nothing else. There is deliberately
/// no threshold, grace or rounding invented here: D83 declared which fields clamp
/// and in which direction, and a rule this function made up would be a fourth
/// place the answer lives.
///
/// **Under-delivery is not a refusal.** Short is a fact about what arrived, and
/// D5's floor rule says a receiver is never stopped from recording what is in
/// front of them; the shortfall closes the promise short and is visible in
/// `quantity_closed_short`. `tolerance_under_pct` governs when that becomes a
/// counterparty conversation, which is a claim against the supplier rather than a
/// reason to refuse the goods.
pub fn disposition(policy: &ReceivingPolicy, line: &CountedLine) -> Disposition {
    let stamp = Disposition {
        receiving_policy_id: policy.version,
        accept: true,
        raise: None,
    };

    // A lot the policy requires and the receiver does not have. Refused rather
    // than accepted-with-a-finding, because accepting it puts stock on the floor
    // that cannot be recalled by lot, and D14 makes expiry a property of the lot.
    if policy.require_lot && !line.has_lot {
        return Disposition { accept: false, raise: Some("lot_missing"), ..stamp };
    }

    // Over-delivery beyond what the policy permits. Recorded, not refused: the
    // pallet is on the dock and D5 will not have a receiver blocked to protect a
    // number. What the tolerance decides is whether it is a discrepancy.
    // Base against base. D92: these were different vocabularies until the receipt
    // line stored a canonical quantity of its own.
    if line.quantity > line.expected_quantity {
        let over = (line.quantity - line.expected_quantity) as f64;
        // **A policy that states no tolerance has not stated a tolerance of
        // zero.** Reading the absence as a number is the mistake `packing_factor`
        // is documented against — *"a made-up 1 there would be a wrong answer
        // rather than a missing one"* — and here both made-up numbers are wrong
        // in opposite directions: zero makes every overage a finding, infinity
        // makes none. So the absence is read as what it is, an overage nobody
        // configured a bound for, and an overage nobody can judge is precisely
        // what D8 built the queue to hold.
        let within = policy
            .tolerance_over_pct
            .is_some_and(|pct| over <= line.expected_quantity as f64 * pct / 100.0);
        if !within {
            return Disposition { raise: Some("over_receipt"), ..stamp };
        }
    }

    stamp
}

// ---------------------------------------------------------------------------
// Entered packaging → base units (Principle 5, D92 / Q173)
// ---------------------------------------------------------------------------

/// Packaging levels on `goods_receipt_line.entered_packaging_level`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackagingLevel {
    Each,
    Inner,
    Carton,
    Layer,
    Pallet,
}

impl PackagingLevel {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "each" => Some(Self::Each),
            "inner" => Some(Self::Inner),
            "carton" => Some(Self::Carton),
            "layer" => Some(Self::Layer),
            "pallet" => Some(Self::Pallet),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Each => "each",
            Self::Inner => "inner",
            Self::Carton => "carton",
            Self::Layer => "layer",
            Self::Pallet => "pallet",
        }
    }

    pub fn needs_config(self) -> bool {
        !matches!(self, Self::Each)
    }
}

/// Factors from `item_packing_config` — same cascade as SQL `packing_factor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackingConfig {
    pub id: Uuid,
    pub item_id: Uuid,
    pub units_per_inner: Option<i32>,
    pub inners_per_carton: Option<i32>,
    pub cartons_per_layer: Option<i32>,
    pub layers_per_pallet: Option<i32>,
}

/// What the dock entered, before conversion to base units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnteredCount {
    pub entered_quantity: i64,
    pub level: PackagingLevel,
    pub config_id: Option<Uuid>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EnteredProblem {
    NothingEntered,
    AmbiguousInput,
    UnknownLevel { level: String },
    ConfigRequired { level: PackagingLevel },
    ConfigWrongItem,
    IncompleteCascade { level: PackagingLevel },
    BaseMismatch {
        entered: i64,
        factor: i64,
        stated: i64,
    },
    NonPositive,
}

impl std::fmt::Display for EnteredProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnteredProblem::NothingEntered => {
                write!(f, "a receipt needs quantity or an entered packaging count")
            }
            EnteredProblem::AmbiguousInput => write!(
                f,
                "provide either base quantity alone (eaches) or entered_quantity with \
                 entered_packaging_level"
            ),
            EnteredProblem::UnknownLevel { level } => {
                write!(f, "unknown packaging level {level}")
            }
            EnteredProblem::ConfigRequired { level } => write!(
                f,
                "packaging level {} requires item_packing_config_id",
                level.as_str()
            ),
            EnteredProblem::ConfigWrongItem => write!(
                f,
                "item_packing_config is for a different item than this receipt line"
            ),
            EnteredProblem::IncompleteCascade { level } => write!(
                f,
                "item_packing_config is incomplete for packaging level {}",
                level.as_str()
            ),
            EnteredProblem::BaseMismatch {
                entered,
                factor,
                stated,
            } => write!(
                f,
                "quantity {stated} does not match entered {entered} × factor {factor} \
                 (expected {})",
                entered * factor
            ),
            EnteredProblem::NonPositive => {
                write!(f, "entered quantity must be positive")
            }
        }
    }
}

/// Factor for one unit of `level` under this config. `None` if the cascade is
/// incomplete (same as SQL `packing_factor` returning NULL).
pub fn packing_factor(config: Option<&PackingConfig>, level: PackagingLevel) -> Option<i64> {
    match level {
        PackagingLevel::Each => Some(1),
        PackagingLevel::Inner => config?.units_per_inner.map(|n| n as i64),
        PackagingLevel::Carton => {
            let c = config?;
            Some((c.units_per_inner? as i64) * (c.inners_per_carton? as i64))
        }
        PackagingLevel::Layer => {
            let c = config?;
            Some(
                (c.units_per_inner? as i64)
                    * (c.inners_per_carton? as i64)
                    * (c.cartons_per_layer? as i64),
            )
        }
        PackagingLevel::Pallet => {
            let c = config?;
            Some(
                (c.units_per_inner? as i64)
                    * (c.inners_per_carton? as i64)
                    * (c.cartons_per_layer? as i64)
                    * (c.layers_per_pallet? as i64),
            )
        }
    }
}

/// Resolve dock input to canonical base units + entered form to store (Q173).
///
/// - Base `quantity` alone → eaches (entered = quantity, level = each).
/// - Entered form → convert via packing factor; optional base must agree.
pub fn resolve_count(
    base_quantity: Option<i64>,
    entered_quantity: Option<i64>,
    level_text: Option<&str>,
    config: Option<&PackingConfig>,
    line_item_id: Uuid,
) -> Result<(i64, EnteredCount), EnteredProblem> {
    match (base_quantity, entered_quantity, level_text) {
        (Some(q), None, None) => {
            if q <= 0 {
                return Err(EnteredProblem::NonPositive);
            }
            Ok((
                q,
                EnteredCount {
                    entered_quantity: q,
                    level: PackagingLevel::Each,
                    config_id: None,
                },
            ))
        }
        (base, Some(entered), Some(level_s)) => {
            if entered <= 0 {
                return Err(EnteredProblem::NonPositive);
            }
            let level = PackagingLevel::parse(level_s).ok_or_else(|| {
                EnteredProblem::UnknownLevel {
                    level: level_s.to_string(),
                }
            })?;
            if level.needs_config() && config.is_none() {
                return Err(EnteredProblem::ConfigRequired { level });
            }
            if let Some(c) = config {
                if c.item_id != line_item_id {
                    return Err(EnteredProblem::ConfigWrongItem);
                }
            }
            let factor = packing_factor(config, level)
                .ok_or(EnteredProblem::IncompleteCascade { level })?;
            let expected_base = entered.saturating_mul(factor);
            if expected_base <= 0 {
                return Err(EnteredProblem::NonPositive);
            }
            if let Some(q) = base {
                if q != expected_base {
                    return Err(EnteredProblem::BaseMismatch {
                        entered,
                        factor,
                        stated: q,
                    });
                }
            }
            Ok((
                expected_base,
                EnteredCount {
                    entered_quantity: entered,
                    level,
                    config_id: config.map(|c| c.id),
                },
            ))
        }
        (None, None, None) => Err(EnteredProblem::NothingEntered),
        _ => Err(EnteredProblem::AmbiguousInput),
    }
}

#[cfg(test)]
mod packing_tests {
    use super::*;

    fn cfg() -> PackingConfig {
        PackingConfig {
            id: Uuid::from_u128(1),
            item_id: Uuid::from_u128(2),
            units_per_inner: Some(10),
            inners_per_carton: Some(1),
            cartons_per_layer: Some(8),
            layers_per_pallet: Some(5),
        }
    }

    #[test]
    fn eaches_from_base_alone() {
        let (base, e) = resolve_count(Some(100), None, None, None, Uuid::from_u128(2)).unwrap();
        assert_eq!(base, 100);
        assert_eq!(e.level, PackagingLevel::Each);
        assert_eq!(e.entered_quantity, 100);
    }

    #[test]
    fn twelve_cartons_of_ten() {
        let c = cfg();
        let (base, e) =
            resolve_count(None, Some(12), Some("carton"), Some(&c), c.item_id).unwrap();
        assert_eq!(base, 120);
        assert_eq!(e.entered_quantity, 12);
        assert_eq!(e.level, PackagingLevel::Carton);
        assert_eq!(e.config_id, Some(c.id));
    }

    #[test]
    fn stated_base_must_agree() {
        let c = cfg();
        let err = resolve_count(Some(100), Some(12), Some("carton"), Some(&c), c.item_id)
            .unwrap_err();
        assert!(matches!(err, EnteredProblem::BaseMismatch { .. }));
    }

    #[test]
    fn carton_requires_config() {
        let err = resolve_count(None, Some(12), Some("carton"), None, Uuid::from_u128(2))
            .unwrap_err();
        assert!(matches!(err, EnteredProblem::ConfigRequired { .. }));
    }
}

// ---------------------------------------------------------------------------
// Resolve receiving policy for a dock act (D22 / D82 / D83 / D89)
// ---------------------------------------------------------------------------

/// A resolution, and everything the caller has to do something about.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedReceiving {
    pub policy: ReceivingPolicy,
    /// Set when two or more bindings tied on **every** declared dimension.
    ///
    /// **The caller raises `discrepancy.kind = 'policy_ambiguous'` with this
    /// text.** That is D22's decision — most-specific-wins cannot decide a tie,
    /// so it is raised rather than picked arbitrarily — and J16's population.
    /// Returning it as a response warning instead was the same mistake in a
    /// smaller place: the floor takes the lower binding id and carries on, which
    /// is right, and then the disagreement disappears when the handheld drops
    /// the reply. A finding that nobody can point at is a finding nobody chases.
    pub ambiguity: Option<String>,
}

/// Load candidates + values for receiving, resolve and clamp into a
/// [`ReceivingPolicy`] ready for [`disposition`].
///
/// `party_id` is the supplier (counterparty). `site_id` is optional space depth.
///
/// **Nothing is defaulted here.** A field absent from the winning version is
/// reported rather than replaced with a plausible number, on the precedent
/// `packing_factor` set one migration earlier: *"a made-up 1 there would be a
/// wrong answer rather than a missing one"*. `require_lot` is the sharp case,
/// because the plausible number is `false` and the failure is silent — a partial
/// config quietly stops requiring lot capture, and [`disposition`] says what that
/// costs: stock on the floor that cannot be recalled by lot.
pub async fn resolve_receiving_policy(
    tx: &Transaction<'_>,
    tenant_id: Uuid,
    item_id: Uuid,
    party_id: Option<Uuid>,
    site_id: Option<Uuid>,
    at: DateTime<Utc>,
) -> Result<ResolvedReceiving, ApiError> {
    let rows = tx
        .query(
            "SELECT c.policy_binding_id, c.tenancy, c.product, c.counterparty,
                    c.space, c.ownership, c.metric
               FROM policy_candidate(
                        $1, 'receiving', $2, $3, $4, NULL, NULL, NULL, $5) c",
            &[&tenant_id, &item_id, &party_id, &site_id, &at],
        )
        .await?;

    // Migration 65 ships an all-NULL platform binding for this kind, so an empty
    // candidate set means the schema is incomplete rather than that the tenant
    // has configured nothing. Kept as a refusal because `disposition` would have
    // no version to stamp and J63 nothing to read — but it is now unreachable on
    // any database the migration set built, which is the difference between a
    // guard and a way to block the dock.
    if rows.is_empty() {
        return Err(ApiError::Rejected(
            "no receiving policy binding matches, not even the platform default; \
             the schema is missing migration 65"
                .into(),
        ));
    }

    let candidates: Vec<Candidate> = rows
        .iter()
        .map(|r| {
            let binding: Uuid = r.get(0);
            Candidate {
                binding: binding.as_u128(),
                tenancy: r.get(1),
                product: r.get(2),
                counterparty: r.get(3),
                space: r.get(4),
                ownership: r.get(5),
                metric: r.get(6),
            }
        })
        .collect();

    let Some(r) = resolve(PolicyKind::Receiving, &candidates) else {
        return Err(ApiError::Rejected(
            "receiving policy resolve returned no winner".into(),
        ));
    };
    // `explain()` rather than a sentence written here. Question 79 put the
    // explanation in the resolver crate so it ships *with* the ordering it
    // describes; a second wording maintained beside it is a second thing to
    // drift.
    let ambiguity = (!r.ambiguous_with.is_empty()).then(|| explain(PolicyKind::Receiving, &r));

    let binding_ids: Vec<Uuid> = candidates
        .iter()
        .map(|c| Uuid::from_u128(c.binding))
        .collect();

    let vals = tx
        .query(
            "SELECT policy_binding_id, field, value::float8
               FROM policy_value('receiving', $1::uuid[], $2)
              WHERE value IS NOT NULL",
            &[&binding_ids, &at],
        )
        .await?;

    let owned: Vec<(u128, String, f64)> = vals
        .iter()
        .map(|row| {
            let id: Uuid = row.get(0);
            (id.as_u128(), row.get(1), row.get(2))
        })
        .collect();
    let values: Vec<(u128, &str, f64)> = owned
        .iter()
        .map(|(b, f, v)| (*b, f.as_str(), *v))
        .collect();

    let raw = |field: &str| -> Option<f64> {
        values
            .iter()
            .find(|(b, f, _)| *b == r.winner.binding && *f == field)
            .map(|(_, _, v)| *v)
    };

    // `receiving_policy.require_lot` is NOT NULL, so a version in force always
    // states one and its absence here means the value set and the version set
    // disagree. Refused rather than assumed: `false` is the plausible answer and
    // the permissive one, which is the combination that gets a wrong default
    // shipped and never noticed.
    let Some(require_lot_raw) = raw("require_lot") else {
        return Err(ApiError::Rejected(format!(
            "receiving binding {} won but states no require_lot; its value row and \
             its version row disagree",
            Uuid::from_u128(r.winner.binding)
        )));
    };
    let mut require_lot = require_lot_raw != 0.0;

    // Nullable in the schema, so absence is a real state rather than a fault:
    // this policy sets no bound on over-delivery. `disposition` reads the
    // absence, and does not read it as zero.
    let mut tolerance_over_pct = raw("tolerance_over_pct");

    for moved in apply_clamps(PolicyKind::Receiving, r.winner.binding, &values) {
        match moved.field {
            "tolerance_over_pct" => tolerance_over_pct = Some(moved.applied),
            "require_lot" => require_lot = moved.applied != 0.0,
            _ => {}
        }
    }

    let winner_binding = Uuid::from_u128(r.winner.binding);
    // `query_opt`, so a database error stays a database error. The previous form
    // mapped every failure to `Rejected`, which turned a dropped connection or a
    // permission fault into a 400 telling the receiver their policy was
    // misconfigured.
    let version: Uuid = tx
        .query_opt(
            "SELECT v.id FROM receiving_policy v
              WHERE v.policy_binding_id = $1
                AND v.effective @> $2::timestamptz
              ORDER BY lower(v.effective) DESC
              LIMIT 1",
            &[&winner_binding, &at],
        )
        .await?
        .ok_or_else(|| {
            ApiError::Rejected(format!(
                "receiving binding {winner_binding} won but has no version in force at \
                 the act time"
            ))
        })?
        .get(0);

    Ok(ResolvedReceiving {
        policy: ReceivingPolicy {
            version,
            tolerance_over_pct,
            require_lot,
        },
        ambiguity,
    })
}

#[cfg(test)]
mod disposition_tests {
    use super::*;

    fn policy(tolerance_over_pct: f64, require_lot: bool) -> ReceivingPolicy {
        ReceivingPolicy { version: u(9), tolerance_over_pct: Some(tolerance_over_pct), require_lot }
    }
    /// A version that states no over-delivery bound. The column is nullable, so
    /// this is a configuration somebody can actually save.
    fn unbounded(require_lot: bool) -> ReceivingPolicy {
        ReceivingPolicy { version: u(9), tolerance_over_pct: None, require_lot }
    }
    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }
    /// Both in base units, which is the whole of D92.
    fn line(expected: i64, counted: i64, has_lot: bool) -> CountedLine {
        CountedLine { expected_quantity: expected, quantity: counted, has_lot }
    }

    #[test]
    fn within_tolerance_is_accepted_and_still_names_the_policy() {
        let d = disposition(&policy(10.0, false), &line(100, 108, true));
        assert!(d.accept);
        assert_eq!(d.raise, None);
        assert_eq!(d.receiving_policy_id, u(9));
    }

    #[test]
    fn over_tolerance_is_recorded_rather_than_refused() {
        // D5: the floor is never blocked to protect a number. The pallet is on
        // the dock either way; the tolerance decides whether it is a discrepancy.
        let d = disposition(&policy(10.0, false), &line(100, 125, true));
        assert!(d.accept);
        assert_eq!(d.raise, Some("over_receipt"));
    }

    #[test]
    fn the_clamped_tolerance_is_what_decides() {
        // The tenant asked for 25 and D83's ceiling brought it to 10 before this
        // function saw it. 125 against 100 is inside 25 and outside 10, so this
        // is the assertion that the clamp reaches the floor rather than stopping
        // at the resolver.
        assert_eq!(disposition(&policy(25.0, false), &line(100, 125, true)).raise, None);
        assert_eq!(
            disposition(&policy(10.0, false), &line(100, 125, true)).raise,
            Some("over_receipt")
        );
    }

    #[test]
    fn a_required_lot_that_is_missing_is_refused() {
        let d = disposition(&policy(10.0, true), &line(100, 100, false));
        assert!(!d.accept);
        assert_eq!(d.raise, Some("lot_missing"));
    }

    #[test]
    fn a_refusal_names_the_policy_that_refused() {
        // J63 is about governed acts, not about accepted ones. "Why was this
        // rejected" has the same answer missing if nothing records it.
        let d = disposition(&policy(10.0, true), &line(100, 100, false));
        assert_eq!(d.receiving_policy_id, u(9));
    }

    #[test]
    fn the_tolerance_reads_base_units_and_not_a_carton_count() {
        // The regression D92 exists for. Twelve cartons of ten against a promise
        // of a hundred is a twenty-unit over-delivery and outside a ten percent
        // tolerance. Passed as a carton count -- which is what the caller had
        // before the receipt line stored a base quantity -- `12 > 100` is false
        // and the check never fires at all.
        assert_eq!(
            disposition(&policy(10.0, false), &line(100, 120, true)).raise,
            Some("over_receipt")
        );
        assert_eq!(
            disposition(&policy(10.0, false), &line(100, 12, true)).raise,
            None,
            "twelve of anything is under a hundred; this is the reading that hid the bug"
        );
    }

    #[test]
    fn no_stated_tolerance_sends_an_overage_to_the_queue() {
        // The absence is not zero and it is not infinity. An over-delivery under
        // a policy that set no bound is accepted -- D5, the pallet is on the dock
        // -- and raised, because nothing here can say it was within anything.
        let d = disposition(&unbounded(false), &line(100, 101, true));
        assert!(d.accept);
        assert_eq!(d.raise, Some("over_receipt"));
    }

    #[test]
    fn no_stated_tolerance_still_leaves_an_exact_delivery_alone() {
        // The other half, and the one that would fail if the absence were read
        // as "always raise" rather than as "no bound to be inside of".
        assert_eq!(disposition(&unbounded(false), &line(100, 100, true)).raise, None);
        assert_eq!(disposition(&unbounded(false), &line(100, 60, true)).raise, None);
    }

    #[test]
    fn short_delivery_is_a_fact_rather_than_a_refusal() {
        let d = disposition(&policy(10.0, false), &line(100, 60, true));
        assert!(d.accept);
        assert_eq!(d.raise, None, "a shortfall closes the promise short; it does not refuse goods");
    }
}
