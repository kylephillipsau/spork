//! Recording a stock count: the assertion, not the ledger write.
//!
//! D8: the count is preserved forever; comparing it to system quantity may
//! raise `discrepancy.kind = count_variance`. The ledger is only written if a
//! later resolution posts an adjustment (`/adjustments`).
//!
//! D9: challenge fields travel with the count when the capture was challenged.

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
    pub quantity: i64,
}

/// Absolute floor count for an existing cell.
#[derive(Clone, Debug)]
pub struct ProposedCount {
    pub tenant_id: Uuid,
    pub stock_id: Uuid,
    pub counted_quantity: i64,
    pub blind: bool,
    pub challenged: bool,
    pub challenge_context: Option<String>,
    pub confirmed: Option<bool>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    CellNotFound,
    DifferentTenant,
    CellHasNoHolder,
    NegativeCount,
    /// Challenged without context text (D9).
    ChallengeWithoutContext,
    /// Confirmed set when nothing was challenged.
    ConfirmedWithoutChallenge,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::CellNotFound => write!(f, "the stock cell was not found"),
            Problem::DifferentTenant => {
                write!(f, "the stock cell belongs to another tenant")
            }
            Problem::CellHasNoHolder => write!(f, "the stock cell has no holder"),
            Problem::NegativeCount => write!(f, "a counted quantity cannot be negative"),
            Problem::ChallengeWithoutContext => write!(
                f,
                "a challenged count must carry challenge_context (what we told the operator)"
            ),
            Problem::ConfirmedWithoutChallenge => write!(
                f,
                "confirmed only applies when the count was challenged"
            ),
        }
    }
}

/// Outcome after a valid count is checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountPlan {
    pub system_quantity: i64,
    pub counted_quantity: i64,
    pub variance: i64,
    /// True when a count_variance finding should be raised.
    pub raise_variance: bool,
}

pub fn check(proposed: &ProposedCount, cell: Option<&StockCell>) -> Result<CountPlan, Vec<Problem>> {
    let mut problems = vec![];

    if proposed.counted_quantity < 0 {
        problems.push(Problem::NegativeCount);
    }
    if proposed.challenged
        && proposed
            .challenge_context
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(true)
    {
        problems.push(Problem::ChallengeWithoutContext);
    }
    if proposed.confirmed.is_some() && !proposed.challenged {
        problems.push(Problem::ConfirmedWithoutChallenge);
    }

    let Some(cell) = cell else {
        problems.push(Problem::CellNotFound);
        return Err(problems);
    };

    if cell.tenant_id != proposed.tenant_id {
        problems.push(Problem::DifferentTenant);
    }
    if cell.holder_location_id.is_none() && cell.holder_package_id.is_none() {
        problems.push(Problem::CellHasNoHolder);
    }

    if !problems.is_empty() {
        return Err(problems);
    }

    let variance = proposed.counted_quantity - cell.quantity;
    Ok(CountPlan {
        system_quantity: cell.quantity,
        counted_quantity: proposed.counted_quantity,
        variance,
        raise_variance: variance != 0,
    })
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
            lot_id: None,
            status_id: u(4),
            owner_id: u(5),
            quantity: qty,
        }
    }

    fn good(counted: i64) -> ProposedCount {
        ProposedCount {
            tenant_id: u(10),
            stock_id: u(1),
            counted_quantity: counted,
            blind: false,
            challenged: false,
            challenge_context: None,
            confirmed: None,
        }
    }

    #[test]
    fn match_raises_no_finding() {
        let plan = check(&good(50), Some(&cell(50))).unwrap();
        assert!(!plan.raise_variance);
        assert_eq!(plan.variance, 0);
    }

    #[test]
    fn short_raises_finding() {
        let plan = check(&good(47), Some(&cell(50))).unwrap();
        assert!(plan.raise_variance);
        assert_eq!(plan.variance, -3);
    }

    #[test]
    fn challenge_needs_context() {
        let mut p = good(47);
        p.challenged = true;
        assert!(check(&p, Some(&cell(50))).is_err());
    }

    #[test]
    fn challenge_with_context_ok() {
        let mut p = good(47);
        p.challenged = true;
        p.challenge_context = Some("system 50, last movement 3d ago".into());
        p.confirmed = Some(true);
        assert!(check(&p, Some(&cell(50))).is_ok());
    }
}
