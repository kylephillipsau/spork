//! Recording a location-to-location move (or putaway) as one ledger row.
//!
//! `reason` is `move` or `putaway` (migration 59 CHECK). Neither is a fold
//! discriminator — D99/D100 fold by shape — but the vocabulary records what
//! the operator thought they were doing. Progress columns are not touched.

use uuid::Uuid;

/// A stock cell the mover is taking from.
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
    pub available_quantity: i64,
}

/// Destination location (must exist in the tenant).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub id: Uuid,
    pub tenant_id: Uuid,
}

#[derive(Clone, Debug)]
pub struct ProposedMove {
    pub tenant_id: Uuid,
    pub quantity: i64,
    pub from_stock_id: Uuid,
    pub to_location_id: Uuid,
    /// `move` or `putaway`.
    pub reason: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    NothingToMove,
    CellNotFound,
    LocationNotFound,
    DifferentTenant,
    /// Package-held cells are relocated by package_event, not this path.
    FromPackageHeld,
    CellHasNoHolder,
    SameLocation,
    BadReason,
    /// Soft: over-available (D5 still records).
    OverAvailable { available: i64, proposed: i64 },
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::NothingToMove => write!(f, "a move of no units is not a move"),
            Problem::CellNotFound => write!(f, "the stock cell was not found"),
            Problem::LocationNotFound => write!(f, "the destination location was not found"),
            Problem::DifferentTenant => {
                write!(f, "the cell or location belongs to another tenant")
            }
            Problem::FromPackageHeld => write!(
                f,
                "the cell is held in a package; relocate the package rather than moving the cell"
            ),
            Problem::CellHasNoHolder => write!(f, "the stock cell has no holder"),
            Problem::SameLocation => write!(f, "source and destination locations are the same"),
            Problem::BadReason => write!(f, "reason must be move or putaway"),
            Problem::OverAvailable { available, proposed } => write!(
                f,
                "moving {proposed} from a cell with {available} available; the write \
                 is still recorded (D5)"
            ),
        }
    }
}

pub fn is_hard(p: &Problem) -> bool {
    !matches!(p, Problem::OverAvailable { .. })
}

pub fn check(
    proposed: &ProposedMove,
    cell: Option<&StockCell>,
    location: Option<&Location>,
) -> Vec<Problem> {
    let mut problems = vec![];

    if proposed.quantity <= 0 {
        problems.push(Problem::NothingToMove);
    }
    if proposed.reason != "move" && proposed.reason != "putaway" {
        problems.push(Problem::BadReason);
    }

    let Some(cell) = cell else {
        problems.push(Problem::CellNotFound);
        return problems;
    };
    let Some(location) = location else {
        problems.push(Problem::LocationNotFound);
        return problems;
    };

    if cell.tenant_id != proposed.tenant_id || location.tenant_id != proposed.tenant_id {
        problems.push(Problem::DifferentTenant);
    }

    if cell.holder_package_id.is_some() {
        problems.push(Problem::FromPackageHeld);
    } else if cell.holder_location_id.is_none() {
        problems.push(Problem::CellHasNoHolder);
    } else if cell.holder_location_id == Some(location.id) {
        problems.push(Problem::SameLocation);
    }

    if proposed.quantity > cell.available_quantity {
        problems.push(Problem::OverAvailable {
            available: cell.available_quantity,
            proposed: proposed.quantity,
        });
    }

    problems
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveSides {
    pub item_id: Uuid,
    pub from_location_id: Uuid,
    pub from_lot_id: Option<Uuid>,
    pub from_status_id: Uuid,
    pub from_owner_id: Uuid,
    pub to_location_id: Uuid,
    pub to_lot_id: Option<Uuid>,
    pub to_status_id: Uuid,
    pub to_owner_id: Uuid,
}

pub fn sides(cell: &StockCell, to_location_id: Uuid) -> MoveSides {
    MoveSides {
        item_id: cell.item_id,
        from_location_id: cell.holder_location_id.expect("hard checks require location"),
        from_lot_id: cell.lot_id,
        from_status_id: cell.status_id,
        from_owner_id: cell.owner_id,
        to_location_id,
        to_lot_id: cell.lot_id,
        to_status_id: cell.status_id,
        to_owner_id: cell.owner_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn cell() -> StockCell {
        StockCell {
            id: u(1),
            tenant_id: u(10),
            item_id: u(100),
            holder_location_id: Some(u(2)),
            holder_package_id: None,
            lot_id: Some(u(3)),
            status_id: u(4),
            owner_id: u(5),
            quantity: 50,
            available_quantity: 40,
        }
    }

    fn loc() -> Location {
        Location {
            id: u(9),
            tenant_id: u(10),
        }
    }

    fn good() -> ProposedMove {
        ProposedMove {
            tenant_id: u(10),
            quantity: 10,
            from_stock_id: u(1),
            to_location_id: u(9),
            reason: "putaway".into(),
        }
    }

    #[test]
    fn a_well_formed_putaway_passes() {
        assert_eq!(check(&good(), Some(&cell()), Some(&loc())), vec![]);
    }

    #[test]
    fn over_available_is_soft() {
        let mut p = good();
        p.quantity = 100;
        let problems = check(&p, Some(&cell()), Some(&loc()));
        assert!(problems.iter().any(|x| matches!(x, Problem::OverAvailable { .. })));
        assert!(!problems.iter().any(is_hard));
    }

    #[test]
    fn package_held_is_hard() {
        let mut c = cell();
        c.holder_package_id = Some(u(7));
        c.holder_location_id = None;
        assert!(check(&good(), Some(&c), Some(&loc())).contains(&Problem::FromPackageHeld));
    }

    #[test]
    fn same_location_is_hard() {
        let mut p = good();
        p.to_location_id = u(2);
        let loc = Location {
            id: u(2),
            tenant_id: u(10),
        };
        assert!(check(&p, Some(&cell()), Some(&loc)).contains(&Problem::SameLocation));
    }
}
