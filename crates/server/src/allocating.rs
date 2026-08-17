//! Claiming stock for a fulfilment line: one `stock_allocation` row.
//!
//! An allocation is an **intention** (principle 2, D12), not a floor observation.
//! That is why refusal is admissible here in a way it is not on a pick (D5): the
//! planner is not discarding a true record of what happened; it is declining to
//! write a commitment that would already be false.
//!
//! # Scope of this first path
//!
//! **Directed, cell-bound only.** The caller names the fulfilment line and the
//! stock cell. Scoring (FEFO, travel, policy weights) is not here — D13 keeps
//! that in `allocation_policy`, and question 26 is still open for *who* runs and
//! when. This module is the act: write the claim once the decision is made.
//!
//! Pre-receipt claims against `expected_supply` are a second arm (D24); they are
//! not this path. Rebinding and auto-pick of candidates wait on the re-allocator
//! (question 34) and on planner_decision when automation arrives. **Release** of
//! a still-`allocated` claim is here: the planner undoes its own intention.
//!
//! # Covering set
//!
//! Matches J31 / the live ledger view: `allocated, picking, picked, packed,
//! fulfilled`. `short` and `released` cover nothing. Soft over-available on the
//! cell is warned; over-covering the line is hard — J56 is a finding about
//! stored data and the write path should not mint it.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// States that contribute to `covered_quantity` (J31).
pub const COVERING: &[&str] = &["allocated", "picking", "picked", "packed", "fulfilled"];

/// States that claim cell quantity for J3 (`allocated_quantity` on stock).
pub const CELL_CLAIMING: &[&str] = &["allocated", "picking", "picked", "packed"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StockCell {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub holder_location_id: Option<Uuid>,
    pub holder_package_id: Option<Uuid>,
    pub quantity: i64,
    pub available_quantity: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FulfilmentLine {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub quantity: i64,
    /// Sum of covering allocations already on this line (caller loads).
    pub covered_quantity: i64,
    pub fulfilment_cancelled: bool,
}

#[derive(Clone, Debug)]
pub struct ProposedAllocation {
    pub tenant_id: Uuid,
    pub fulfilment_line_id: Uuid,
    pub stock_id: Uuid,
    pub quantity: i64,
    pub firm: bool,
    pub bound_at: DateTime<Utc>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    NothingToAllocate,
    LineNotFound,
    CellNotFound,
    DifferentTenant,
    LineCancelled,
    /// Cell and line name different items. Substitution is a floor fact on a
    /// pick (D12/J69); on an intention it is a planner mistake.
    ItemMismatch { line: Uuid, cell: Uuid },
    /// Soft: claim exceeds free quantity on the cell. Still writeable so a
    /// soft hold can be recorded; available goes negative after rebuild (D5's
    /// cousin for intentions — the discrepancy queue is for the short).
    OverAvailable { available: i64, proposed: i64 },
    /// Hard: would push covering sum past the commitment (J56).
    OverCovers {
        line_quantity: i64,
        already_covered: i64,
        proposed: i64,
    },
    /// Package-held cells are pickable; first path is location-held only so the
    /// floor walk stays simple. Widen when carton-held allocation is needed.
    PackageHeldCell,
    CellHasNoHolder,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::NothingToAllocate => {
                write!(f, "an allocation of no units is not an allocation")
            }
            Problem::LineNotFound => write!(f, "the fulfilment line was not found"),
            Problem::CellNotFound => write!(f, "the stock cell was not found"),
            Problem::DifferentTenant => {
                write!(f, "the line or cell belongs to another tenant")
            }
            Problem::LineCancelled => {
                write!(f, "the fulfilment is cancelled; nothing can be allocated for it")
            }
            Problem::ItemMismatch { line, cell } => write!(
                f,
                "the line commits item {line} but the cell holds {cell}; an allocation \
                 is an intention and must name matching item sides"
            ),
            Problem::OverAvailable { available, proposed } => write!(
                f,
                "claiming {proposed} from a cell with {available} available; the claim \
                 is still recorded and the short is a finding after rebuild"
            ),
            Problem::OverCovers {
                line_quantity,
                already_covered,
                proposed,
            } => write!(
                f,
                "claiming {proposed} when the line is already covered for {already_covered} \
                 of {line_quantity}; over-cover is refused (J56)"
            ),
            Problem::PackageHeldCell => write!(
                f,
                "the cell is held in a package; this path only claims location-held stock"
            ),
            Problem::CellHasNoHolder => write!(f, "the stock cell has no holder"),
        }
    }
}

pub fn is_hard(p: &Problem) -> bool {
    !matches!(p, Problem::OverAvailable { .. })
}

/// Check a directed allocation against the line and cell the write path loaded.
pub fn check(
    proposed: &ProposedAllocation,
    line: Option<&FulfilmentLine>,
    cell: Option<&StockCell>,
) -> Vec<Problem> {
    let mut problems = vec![];

    if proposed.quantity <= 0 {
        problems.push(Problem::NothingToAllocate);
    }

    let Some(line) = line else {
        problems.push(Problem::LineNotFound);
        return problems;
    };
    let Some(cell) = cell else {
        problems.push(Problem::CellNotFound);
        return problems;
    };

    if line.tenant_id != proposed.tenant_id || cell.tenant_id != proposed.tenant_id {
        problems.push(Problem::DifferentTenant);
    }

    if line.fulfilment_cancelled {
        problems.push(Problem::LineCancelled);
    }

    if line.item_id != cell.item_id {
        problems.push(Problem::ItemMismatch {
            line: line.item_id,
            cell: cell.item_id,
        });
    }

    if cell.holder_package_id.is_some() {
        problems.push(Problem::PackageHeldCell);
    } else if cell.holder_location_id.is_none() {
        problems.push(Problem::CellHasNoHolder);
    }

    if proposed.quantity > cell.available_quantity {
        problems.push(Problem::OverAvailable {
            available: cell.available_quantity,
            proposed: proposed.quantity,
        });
    }

    let remaining = line.quantity - line.covered_quantity;
    if proposed.quantity > remaining {
        problems.push(Problem::OverCovers {
            line_quantity: line.quantity,
            already_covered: line.covered_quantity,
            proposed: proposed.quantity,
        });
    }

    problems
}

// ---------------------------------------------------------------------------
// Release a claim
// ---------------------------------------------------------------------------

/// An existing allocation row, as the write path holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allocation {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub state: String,
    pub firm: bool,
    pub fulfilment_line_id: Option<Uuid>,
    pub quantity: i64,
}

#[derive(Clone, Debug)]
pub struct ProposedRelease {
    pub tenant_id: Uuid,
    pub allocation_id: Uuid,
    /// Required when the claim is firm (D24: re-allocator may not steal; a
    /// person still may, but must say so).
    pub force: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReleaseProblem {
    NotFound,
    DifferentTenant,
    AlreadyReleased,
    /// Only `allocated` may be released on this path. Mid-pick / packed /
    /// fulfilled claims need a different act (or a re-allocator).
    NotReleasable { state: String },
    /// Firm claim and the caller did not set force.
    FirmWithoutForce,
}

impl std::fmt::Display for ReleaseProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReleaseProblem::NotFound => write!(f, "the allocation was not found"),
            ReleaseProblem::DifferentTenant => {
                write!(f, "the allocation belongs to another tenant")
            }
            ReleaseProblem::AlreadyReleased => {
                write!(f, "the allocation is already released")
            }
            ReleaseProblem::NotReleasable { state } => write!(
                f,
                "an allocation in state {state} cannot be released on this path; only \
                 allocated claims may be withdrawn here"
            ),
            ReleaseProblem::FirmWithoutForce => write!(
                f,
                "the claim is firm; pass force=true to release it deliberately (D24)"
            ),
        }
    }
}

/// Soft: firm claim released with force — worth logging, not blocking.
#[derive(Debug, PartialEq, Eq)]
pub enum ReleaseWarning {
    FirmForced,
}

impl std::fmt::Display for ReleaseWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReleaseWarning::FirmForced => write!(
                f,
                "released a firm claim with force=true; the re-allocator would not have stolen it"
            ),
        }
    }
}

/// Check a proposed release. All problems are hard (block the write).
pub fn check_release(
    proposed: &ProposedRelease,
    allocation: Option<&Allocation>,
) -> (Vec<ReleaseProblem>, Vec<ReleaseWarning>) {
    let mut problems = vec![];
    let mut warnings = vec![];

    let Some(a) = allocation else {
        problems.push(ReleaseProblem::NotFound);
        return (problems, warnings);
    };

    if a.tenant_id != proposed.tenant_id {
        problems.push(ReleaseProblem::DifferentTenant);
        return (problems, warnings);
    }

    if a.state == "released" {
        problems.push(ReleaseProblem::AlreadyReleased);
        return (problems, warnings);
    }

    if a.state != "allocated" {
        problems.push(ReleaseProblem::NotReleasable {
            state: a.state.clone(),
        });
        return (problems, warnings);
    }

    if a.firm && !proposed.force {
        problems.push(ReleaseProblem::FirmWithoutForce);
        return (problems, warnings);
    }

    if a.firm && proposed.force {
        warnings.push(ReleaseWarning::FirmForced);
    }

    (problems, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn at() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 4, 10, 0, 0).unwrap()
    }

    fn line() -> FulfilmentLine {
        FulfilmentLine {
            id: u(1),
            tenant_id: u(10),
            item_id: u(100),
            quantity: 40,
            covered_quantity: 10,
            fulfilment_cancelled: false,
        }
    }

    fn cell() -> StockCell {
        StockCell {
            id: u(2),
            tenant_id: u(10),
            item_id: u(100),
            holder_location_id: Some(u(3)),
            holder_package_id: None,
            quantity: 50,
            available_quantity: 40,
        }
    }

    fn good() -> ProposedAllocation {
        ProposedAllocation {
            tenant_id: u(10),
            fulfilment_line_id: u(1),
            stock_id: u(2),
            quantity: 20,
            firm: false,
            bound_at: at(),
        }
    }

    #[test]
    fn a_well_formed_claim_passes() {
        assert_eq!(check(&good(), Some(&line()), Some(&cell())), vec![]);
    }

    #[test]
    fn over_cover_is_hard() {
        let mut p = good();
        p.quantity = 50;
        let problems = check(&p, Some(&line()), Some(&cell()));
        assert!(problems.iter().any(|x| matches!(x, Problem::OverCovers { .. })));
        assert!(problems.iter().any(is_hard));
    }

    #[test]
    fn over_available_is_soft() {
        let mut c = cell();
        c.available_quantity = 5;
        let problems = check(&good(), Some(&line()), Some(&c));
        assert!(problems.iter().any(|x| matches!(x, Problem::OverAvailable { .. })));
        assert!(!problems.iter().any(is_hard));
    }

    #[test]
    fn item_mismatch_is_hard() {
        let mut c = cell();
        c.item_id = u(999);
        assert!(check(&good(), Some(&line()), Some(&c))
            .iter()
            .any(|x| matches!(x, Problem::ItemMismatch { .. })));
    }

    #[test]
    fn package_held_is_hard() {
        let mut c = cell();
        c.holder_package_id = Some(u(7));
        c.holder_location_id = None;
        assert!(check(&good(), Some(&line()), Some(&c)).contains(&Problem::PackageHeldCell));
    }

    #[test]
    fn cancelled_line_is_hard() {
        let mut l = line();
        l.fulfilment_cancelled = true;
        assert!(check(&good(), Some(&l), Some(&cell())).contains(&Problem::LineCancelled));
    }

    fn alloc(state: &str, firm: bool) -> Allocation {
        Allocation {
            id: u(9),
            tenant_id: u(10),
            state: state.into(),
            firm,
            fulfilment_line_id: Some(u(1)),
            quantity: 20,
        }
    }

    fn release(force: bool) -> ProposedRelease {
        ProposedRelease {
            tenant_id: u(10),
            allocation_id: u(9),
            force,
        }
    }

    #[test]
    fn releasing_allocated_passes() {
        let (p, w) = check_release(&release(false), Some(&alloc("allocated", false)));
        assert!(p.is_empty(), "{p:?}");
        assert!(w.is_empty());
    }

    #[test]
    fn firm_needs_force() {
        let (p, _) = check_release(&release(false), Some(&alloc("allocated", true)));
        assert!(p.contains(&ReleaseProblem::FirmWithoutForce));
    }

    #[test]
    fn firm_with_force_warns() {
        let (p, w) = check_release(&release(true), Some(&alloc("allocated", true)));
        assert!(p.is_empty(), "{p:?}");
        assert_eq!(w, vec![ReleaseWarning::FirmForced]);
    }

    #[test]
    fn packed_is_not_releasable() {
        let (p, _) = check_release(&release(false), Some(&alloc("packed", false)));
        assert!(matches!(
            p.as_slice(),
            [ReleaseProblem::NotReleasable { state }] if state == "packed"
        ));
    }

    #[test]
    fn already_released_is_hard() {
        let (p, _) = check_release(&release(false), Some(&alloc("released", false)));
        assert!(p.contains(&ReleaseProblem::AlreadyReleased));
    }
}
