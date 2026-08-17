//! Cycle-count inventory adjust: one `stock_movement` (`reason = adjustment`).
//!
//! D8: prefer [`crate::counting`] / `POST /counts` to preserve the assertion and
//! raise `count_variance`. This path is the **resolution movement** half: post
//! the delta as a world-event adjustment when the floor decides the ledger
//! should move (optionally after investigating the finding).
//!
//! That is distinct from [`crate::correction`]: a correction reverses a prior
//! movement (`record_error`); an inventory adjust says the world (or our belief
//! about a bin) changed and does not reverse anything.
//!
//! - **counted > system** → inbound adjustment (to the cell), e.g. `found`
//! - **counted < system** → outbound adjustment (from the cell), e.g. `damaged`
//! - **equal** → no movement (soft); the agreement is still useful to log upstream
//!
//! Requires `adjustment_reason.class = world_event`. Record-error reasons belong
//! on `/corrections` with `reverses_movement_id`.

use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StockCell {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub holder_location_id: Option<Uuid>,
    pub holder_package_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub status_id: Uuid,
    pub owner_id: Uuid,
    /// Projected on-hand for the cell (fold of movements).
    pub quantity: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdjustmentReason {
    pub id: Uuid,
    /// `record_error` | `world_event`
    pub class: String,
    pub code: String,
}

/// Absolute floor count for a cell (cycle-count style).
#[derive(Clone, Debug)]
pub struct ProposedAdjustment {
    pub tenant_id: Uuid,
    pub stock_id: Uuid,
    /// What the operator counted in the cell (base units).
    pub counted_quantity: i64,
    pub reason: AdjustmentReason,
    /// Optional open finding this movement resolves (D8).
    pub discrepancy_id: Option<Uuid>,
}

/// Open finding loaded for resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub kind: String,
    pub state: String,
    pub stock_count_id: Option<Uuid>,
    /// From stock_count when present.
    pub count_stock_id: Option<Uuid>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    CellNotFound,
    DifferentTenant,
    CellHasNoHolder,
    NegativeCount,
    /// Soft: count matches system; nothing to write.
    ZeroVariance { quantity: i64 },
    /// Record-error vocabulary belongs on the correction path.
    RecordErrorReason { code: String },
    UnknownReasonClass { class: String },
    MissingReason,
    FindingNotFound,
    FindingWrongTenant,
    FindingAlreadyResolved,
    /// Soft: resolving a non–count_variance finding.
    FindingUnexpectedKind { kind: String },
    /// Finding's stock_count names a different cell than this adjust.
    FindingCellMismatch,
    /// Soft: no finding linked; resolution is unattached to investigation.
    NoFindingLinked,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::CellNotFound => write!(f, "the stock cell was not found"),
            Problem::DifferentTenant => {
                write!(f, "the stock cell belongs to another tenant")
            }
            Problem::CellHasNoHolder => write!(f, "the stock cell has no holder"),
            Problem::NegativeCount => {
                write!(f, "a counted quantity cannot be negative")
            }
            Problem::ZeroVariance { quantity } => write!(
                f,
                "counted quantity equals system quantity ({quantity}); no adjustment movement"
            ),
            Problem::RecordErrorReason { code } => write!(
                f,
                "adjustment reason {code} is a record_error; reverse a prior movement via \
                 /corrections instead of posting an inventory adjust"
            ),
            Problem::UnknownReasonClass { class } => {
                write!(f, "unknown adjustment_reason class {class}")
            }
            Problem::MissingReason => write!(f, "an adjustment requires an adjustment_reason"),
            Problem::FindingNotFound => write!(f, "the discrepancy was not found"),
            Problem::FindingWrongTenant => {
                write!(f, "the discrepancy belongs to another tenant")
            }
            Problem::FindingAlreadyResolved => {
                write!(f, "the discrepancy is already resolved")
            }
            Problem::FindingUnexpectedKind { kind } => write!(
                f,
                "resolving discrepancy kind {kind}; count_variance is the usual target"
            ),
            Problem::FindingCellMismatch => write!(
                f,
                "the finding's stock_count is for a different cell than this adjustment"
            ),
            Problem::NoFindingLinked => write!(
                f,
                "no discrepancy_id: ledger will move without resolving a finding (D8 prefers \
                 /counts first)"
            ),
        }
    }
}

pub fn is_hard(p: &Problem) -> bool {
    !matches!(
        p,
        Problem::ZeroVariance { .. }
            | Problem::FindingUnexpectedKind { .. }
            | Problem::NoFindingLinked
    )
}

/// Direction of the ledger write once the check passes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Stock appears at the cell (to side only).
    Increase { quantity: i64 },
    /// Stock leaves the cell (from side only).
    Decrease { quantity: i64 },
}

pub fn check(
    proposed: &ProposedAdjustment,
    cell: Option<&StockCell>,
    finding: Option<&Finding>,
) -> (Vec<Problem>, Option<Direction>) {
    let mut problems = vec![];

    if proposed.counted_quantity < 0 {
        problems.push(Problem::NegativeCount);
    }

    match proposed.reason.class.as_str() {
        "world_event" => {}
        "record_error" => problems.push(Problem::RecordErrorReason {
            code: proposed.reason.code.clone(),
        }),
        other => problems.push(Problem::UnknownReasonClass {
            class: other.to_string(),
        }),
    }

    if proposed.discrepancy_id.is_some() {
        match finding {
            None => problems.push(Problem::FindingNotFound),
            Some(f) => {
                if f.tenant_id != proposed.tenant_id {
                    problems.push(Problem::FindingWrongTenant);
                }
                if f.state == "resolved" || f.state == "accepted" {
                    problems.push(Problem::FindingAlreadyResolved);
                }
                if f.kind != "count_variance" {
                    problems.push(Problem::FindingUnexpectedKind {
                        kind: f.kind.clone(),
                    });
                }
                if let Some(csid) = f.count_stock_id {
                    if csid != proposed.stock_id {
                        problems.push(Problem::FindingCellMismatch);
                    }
                }
            }
        }
    } else {
        problems.push(Problem::NoFindingLinked);
    }

    let Some(cell) = cell else {
        problems.push(Problem::CellNotFound);
        return (problems, None);
    };

    if cell.tenant_id != proposed.tenant_id {
        problems.push(Problem::DifferentTenant);
    }

    if cell.holder_location_id.is_none() && cell.holder_package_id.is_none() {
        problems.push(Problem::CellHasNoHolder);
    }

    if problems.iter().any(is_hard) {
        return (problems, None);
    }

    let delta = proposed.counted_quantity - cell.quantity;
    let direction = if delta == 0 {
        problems.push(Problem::ZeroVariance {
            quantity: cell.quantity,
        });
        None
    } else if delta > 0 {
        Some(Direction::Increase { quantity: delta })
    } else {
        Some(Direction::Decrease {
            quantity: -delta,
        })
    };

    (problems, direction)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn cell(qty: i64) -> StockCell {
        StockCell {
            id: u(1),
            tenant_id: u(10),
            item_id: u(100),
            holder_location_id: Some(u(2)),
            holder_package_id: None,
            lot_id: Some(u(3)),
            status_id: u(4),
            owner_id: u(5),
            quantity: qty,
        }
    }

    fn found() -> AdjustmentReason {
        AdjustmentReason {
            id: u(9),
            class: "world_event".into(),
            code: "found".into(),
        }
    }

    fn good(counted: i64) -> ProposedAdjustment {
        ProposedAdjustment {
            tenant_id: u(10),
            stock_id: u(1),
            counted_quantity: counted,
            reason: found(),
            discrepancy_id: None,
        }
    }

    fn finding() -> Finding {
        Finding {
            id: u(20),
            tenant_id: u(10),
            kind: "count_variance".into(),
            state: "open".into(),
            stock_count_id: Some(u(21)),
            count_stock_id: Some(u(1)),
        }
    }

    #[test]
    fn short_count_decreases() {
        let (p, d) = check(&good(47), Some(&cell(50)), None);
        assert!(!p.iter().any(is_hard));
        assert_eq!(d, Some(Direction::Decrease { quantity: 3 }));
        assert!(p.contains(&Problem::NoFindingLinked));
    }

    #[test]
    fn over_count_increases() {
        let (p, d) = check(&good(55), Some(&cell(50)), None);
        assert!(!p.iter().any(is_hard));
        assert_eq!(d, Some(Direction::Increase { quantity: 5 }));
    }

    #[test]
    fn match_is_soft_noop() {
        let (p, d) = check(&good(50), Some(&cell(50)), None);
        assert!(p.iter().any(|x| matches!(x, Problem::ZeroVariance { .. })));
        assert!(!p.iter().any(is_hard));
        assert_eq!(d, None);
    }

    #[test]
    fn record_error_is_hard() {
        let mut p = good(40);
        p.reason.class = "record_error".into();
        p.reason.code = "miscount".into();
        let (problems, _) = check(&p, Some(&cell(50)), None);
        assert!(problems
            .iter()
            .any(|x| matches!(x, Problem::RecordErrorReason { .. })));
    }

    #[test]
    fn negative_count_is_hard() {
        let (p, _) = check(&good(-1), Some(&cell(50)), None);
        assert!(p.contains(&Problem::NegativeCount));
    }

    #[test]
    fn linked_finding_resolves_cleanly() {
        let mut p = good(47);
        p.discrepancy_id = Some(u(20));
        let (problems, d) = check(&p, Some(&cell(50)), Some(&finding()));
        assert!(!problems.iter().any(is_hard), "{problems:?}");
        assert_eq!(d, Some(Direction::Decrease { quantity: 3 }));
        assert!(!problems.contains(&Problem::NoFindingLinked));
    }

    #[test]
    fn already_resolved_finding_is_hard() {
        let mut p = good(47);
        p.discrepancy_id = Some(u(20));
        let mut f = finding();
        f.state = "resolved".into();
        let (problems, _) = check(&p, Some(&cell(50)), Some(&f));
        assert!(problems.contains(&Problem::FindingAlreadyResolved));
    }

    #[test]
    fn finding_cell_mismatch_is_hard() {
        let mut p = good(47);
        p.discrepancy_id = Some(u(20));
        let mut f = finding();
        f.count_stock_id = Some(u(99));
        let (problems, _) = check(&p, Some(&cell(50)), Some(&f));
        assert!(problems.contains(&Problem::FindingCellMismatch));
    }
}
