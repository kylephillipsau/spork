//! Recording a pick: one ledger row out of storage, into wherever it went.
//!
//! D99 makes a pick a movement that left a storage *location* and names the
//! fulfilment line it served. Progress is a fold of that ledger; nothing here
//! UPDATEs `picked_quantity`.
//!
//! # Why the checks are the shape they are
//!
//! A pick is an observation of the floor (D5). Refusing it because the cell is
//! short, or because the lot is not the planned one, would discard a true record
//! of what happened — the same trade D5 refuses for a missing lot label. So:
//!
//! - **Hard problems** block the write: missing sides, zero quantity, the line
//!   and the cell disagree about the item in a way that means the request is
//!   malformed (cell not found, line not found, holders incomplete).
//! - **Soft problems** travel with a successful write: over-available, line
//!   already fully despatched. They are findings the caller can raise; they do
//!   not stop the scan.
//!
//! J69 and J70 assert related properties as jobs against stored data for rows
//! that arrive by a restore or a path that has not been written yet.
//!
//! # Where a pick lands (D166)
//!
//! It used to be a package, required. The ledger has never agreed: `to_location_id`
//! and `to_package_id` are an exclusive pair on `stock_movement` with whole-key
//! CHECKs behind them, and the write path was narrower than the row it wrote.
//!
//! The floor is narrower still than either. A forklift picks a large order
//! straight onto a **pallet**, which is a package and always worked. A picker
//! with a **trolley** takes goods off the shelf and puts them down at the
//! packing station — and a trolley is not a container anybody should record. It
//! is a person's hands with wheels; naming it would be inventing a holder to
//! keep the model tidy, which is the failure this register keeps finding. What
//! actually changed is that the goods left the shelf and are now at the packing
//! station, and `location.kind` has had `staging` in it since migration 1.
//!
//! So the destination is a location **or** a package, exactly one — the same
//! exclusive arm D24 gives the stock key itself. [`Destination`] makes the
//! third state unrepresentable once the request has been read.

/// The write itself: the DTOs, the transaction and the `POST /picks` handler.
///
/// **Beside the judgement rather than inside it (D160).** Everything above
/// this line is pure and tested without a database; everything in `record` is
/// the persistence that acts on what it decides. One name, two files, and the
/// rule that a gerund module is pure stays true of the file it is written in.
pub mod record;


use uuid::Uuid;

/// A stock cell the picker is taking from, as the write path already holds it.
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

/// The commitment line being served.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FulfilmentLine {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub quantity: i64,
    pub picked_quantity: i64,
    pub despatched_quantity: i64,
    pub fulfilment_cancelled: bool,
}

/// Destination package: a shipping carton, a pallet, a tote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub id: Uuid,
    pub tenant_id: Uuid,
}

/// Somewhere the goods were put down: the packing station, a staging bay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// One of `pick_face`, `bulk`, `staging`, `dock`, `overflow`.
    pub kind: String,
}

/// Where a pick landed, as the write path loaded it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    Package(Package),
    Location(Location),
}

impl Destination {
    pub fn tenant_id(&self) -> Uuid {
        match self {
            Destination::Package(p) => p.tenant_id,
            Destination::Location(l) => l.tenant_id,
        }
    }
}

/// Where a pick is *asked* to land, before anything is loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Landing {
    Package(Uuid),
    Location(Uuid),
}

/// Read the two optional fields of a request into the one thing they mean.
///
/// **The exclusivity is checked once, here, and then cannot be got wrong.**
/// The wire carries two nullable ids because JSON has no sum type; everything
/// downstream carries a [`Landing`], so no handler and no test has to remember
/// that both-or-neither is a thing that can be asked for.
pub fn landing(package: Option<Uuid>, location: Option<Uuid>) -> Result<Landing, Problem> {
    match (package, location) {
        (Some(p), None) => Ok(Landing::Package(p)),
        (None, Some(l)) => Ok(Landing::Location(l)),
        (None, None) => Err(Problem::NoDestination),
        (Some(_), Some(_)) => Err(Problem::TwoDestinations),
    }
}

/// What is being asked for, before it is a row.
#[derive(Clone, Debug)]
pub struct ProposedPick {
    pub tenant_id: Uuid,
    pub quantity: i64,
    pub fulfilment_line_id: Uuid,
    pub from_stock_id: Uuid,
    pub to: Landing,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    NothingToPick,
    LineNotFound,
    CellNotFound,
    PackageNotFound,
    LocationNotFound,
    /// The request named neither a package nor a location to land in.
    NoDestination,
    /// It named both. One movement has one destination — the ledger's own
    /// `num_nonnulls(to_location_id, to_package_id) <= 1`.
    TwoDestinations,
    DifferentTenant,
    /// The cell is package-held, not location-held. Still a legal floor fact
    /// (J70 reports the progress gap); named so the caller can see it.
    FromPackageHeldStorage,
    /// Cell has no holder at all — incomplete key.
    CellHasNoHolder,
    LineCancelled,
    /// Soft: taking more than available. D5 allows the write; the cell goes
    /// negative and the discrepancy queue exists for a reason.
    OverAvailable { available: i64, proposed: i64 },
    /// Soft: line already fully despatched on the projection. Still recordable.
    LineAlreadyDespatched,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::NothingToPick => write!(f, "a pick of no units is not a pick"),
            Problem::LineNotFound => write!(f, "the fulfilment line was not found"),
            Problem::CellNotFound => write!(f, "the stock cell was not found"),
            Problem::PackageNotFound => write!(f, "the destination package was not found"),
            Problem::LocationNotFound => write!(f, "the destination location was not found"),
            Problem::NoDestination => write!(
                f,
                "a pick has to land somewhere: name a package to put it in or a \
                 location to put it down at"
            ),
            Problem::TwoDestinations => write!(
                f,
                "a pick lands in one place: name a package or a location, not both"
            ),
            Problem::DifferentTenant => {
                write!(f, "the line, cell or package belongs to another tenant")
            }
            Problem::FromPackageHeldStorage => write!(
                f,
                "the cell is held in a package rather than a location; the pick is \
                 recordable but picked_quantity does not count it (J70)"
            ),
            Problem::CellHasNoHolder => write!(f, "the stock cell has no holder"),
            Problem::LineCancelled => {
                write!(f, "the fulfilment is cancelled; nothing is left to pick for it")
            }
            Problem::OverAvailable { available, proposed } => write!(
                f,
                "picking {proposed} from a cell with {available} available; the write \
                 is still recorded (D5) and the short is a finding rather than a refusal"
            ),
            Problem::LineAlreadyDespatched => write!(
                f,
                "the line's despatched quantity already meets its commitment; the pick \
                 is still recorded if it happened on the floor"
            ),
        }
    }
}

/// Hard problems block the insert; soft ones do not.
pub fn is_hard(p: &Problem) -> bool {
    !matches!(
        p,
        Problem::OverAvailable { .. }
            | Problem::LineAlreadyDespatched
            | Problem::FromPackageHeldStorage
    )
}

/// Check a proposed pick against the rows the write path has already loaded.
///
/// Returns every problem found, hard and soft, in the same spirit as
/// [`crate::correction::check`]: all of them rather than the first.
pub fn check(
    proposed: &ProposedPick,
    line: Option<&FulfilmentLine>,
    cell: Option<&StockCell>,
    destination: Option<&Destination>,
) -> Vec<Problem> {
    let mut problems = vec![];

    if proposed.quantity <= 0 {
        problems.push(Problem::NothingToPick);
    }

    let Some(line) = line else {
        problems.push(Problem::LineNotFound);
        return problems;
    };
    let Some(cell) = cell else {
        problems.push(Problem::CellNotFound);
        return problems;
    };
    let Some(destination) = destination else {
        problems.push(match proposed.to {
            Landing::Package(_) => Problem::PackageNotFound,
            Landing::Location(_) => Problem::LocationNotFound,
        });
        return problems;
    };

    if line.tenant_id != proposed.tenant_id
        || cell.tenant_id != proposed.tenant_id
        || destination.tenant_id() != proposed.tenant_id
    {
        problems.push(Problem::DifferentTenant);
    }

    if line.fulfilment_cancelled {
        problems.push(Problem::LineCancelled);
    }

    if cell.holder_location_id.is_none() && cell.holder_package_id.is_none() {
        problems.push(Problem::CellHasNoHolder);
    } else if cell.holder_location_id.is_none() && cell.holder_package_id.is_some() {
        problems.push(Problem::FromPackageHeldStorage);
    }

    // Item mismatch: the request names a cell; the movement will carry the cell's
    // item. If that disagrees with the line, J69 raises a finding after the fact.
    // We do not refuse here — D12's founding case is exactly that substitution.

    if proposed.quantity > cell.available_quantity {
        problems.push(Problem::OverAvailable {
            available: cell.available_quantity,
            proposed: proposed.quantity,
        });
    }

    if line.despatched_quantity >= line.quantity {
        problems.push(Problem::LineAlreadyDespatched);
    }

    let _ = cell.item_id; // carried onto the movement by the write path
    let _ = line.item_id;
    problems
}

/// The from/to sides a successful pick writes, taken from the cell and package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickSides {
    pub item_id: Uuid,
    pub from_location_id: Option<Uuid>,
    pub from_package_id: Option<Uuid>,
    pub from_lot_id: Option<Uuid>,
    pub from_status_id: Uuid,
    pub from_owner_id: Uuid,
    /// Exactly one of these is set, which is the ledger's own
    /// `num_nonnulls(to_location_id, to_package_id) <= 1` said in Rust.
    pub to_package_id: Option<Uuid>,
    pub to_location_id: Option<Uuid>,
    pub to_lot_id: Option<Uuid>,
    pub to_status_id: Uuid,
    pub to_owner_id: Uuid,
}

pub fn sides(cell: &StockCell, destination: &Destination) -> PickSides {
    let (to_package_id, to_location_id) = match destination {
        Destination::Package(p) => (Some(p.id), None),
        Destination::Location(l) => (None, Some(l.id)),
    };
    PickSides {
        item_id: cell.item_id,
        from_location_id: cell.holder_location_id,
        from_package_id: cell.holder_package_id,
        from_lot_id: cell.lot_id,
        from_status_id: cell.status_id,
        from_owner_id: cell.owner_id,
        to_package_id,
        to_location_id,
        // Lot/status/owner travel with the goods.
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

    fn line() -> FulfilmentLine {
        FulfilmentLine {
            id: u(1),
            tenant_id: u(10),
            item_id: u(100),
            quantity: 40,
            picked_quantity: 0,
            despatched_quantity: 0,
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
            lot_id: Some(u(4)),
            status_id: u(5),
            owner_id: u(6),
            quantity: 50,
            available_quantity: 50,
        }
    }

    fn pkg() -> Destination {
        Destination::Package(Package {
            id: u(7),
            tenant_id: u(10),
        })
    }

    /// The packing station: where a trolley pick is put down (D166).
    fn station() -> Destination {
        Destination::Location(Location {
            id: u(8),
            tenant_id: u(10),
            kind: "staging".into(),
        })
    }

    fn good() -> ProposedPick {
        ProposedPick {
            tenant_id: u(10),
            quantity: 10,
            fulfilment_line_id: u(1),
            from_stock_id: u(2),
            to: Landing::Package(u(7)),
        }
    }

    #[test]
    fn a_well_formed_pick_passes() {
        assert!(check(&good(), Some(&line()), Some(&cell()), Some(&pkg())).is_empty());
    }

    #[test]
    fn nothing_to_pick_is_hard() {
        let mut p = good();
        p.quantity = 0;
        let problems = check(&p, Some(&line()), Some(&cell()), Some(&pkg()));
        assert!(problems.contains(&Problem::NothingToPick));
        assert!(problems.iter().any(is_hard));
    }

    #[test]
    fn over_available_is_soft() {
        let mut p = good();
        p.quantity = 100;
        let problems = check(&p, Some(&line()), Some(&cell()), Some(&pkg()));
        assert!(matches!(
            problems.iter().find(|x| matches!(x, Problem::OverAvailable { .. })),
            Some(Problem::OverAvailable {
                available: 50,
                proposed: 100
            })
        ));
        assert!(!problems.iter().any(is_hard));
    }

    #[test]
    fn package_held_source_is_soft() {
        let mut c = cell();
        c.holder_location_id = None;
        c.holder_package_id = Some(u(9));
        let problems = check(&good(), Some(&line()), Some(&c), Some(&pkg()));
        assert!(problems.contains(&Problem::FromPackageHeldStorage));
        assert!(!problems.iter().any(is_hard));
    }

    #[test]
    fn cancelled_line_is_hard() {
        let mut l = line();
        l.fulfilment_cancelled = true;
        let problems = check(&good(), Some(&l), Some(&cell()), Some(&pkg()));
        assert!(problems.contains(&Problem::LineCancelled));
        assert!(problems.iter().any(is_hard));
    }

    #[test]
    fn sides_carry_the_cell_across() {
        let s = sides(&cell(), &pkg());
        assert_eq!(s.from_location_id, Some(u(3)));
        assert_eq!(s.to_package_id, Some(u(7)));
        assert_eq!(s.to_location_id, None);
        assert_eq!(s.from_status_id, s.to_status_id);
        assert_eq!(s.item_id, u(100));
    }

    // ── D166: a pick lands in one place, and it need not be a package ────

    #[test]
    fn a_pick_put_down_at_the_station_names_the_location_and_no_package() {
        // **The ledger's own exclusive arm.** `stock_movement` has
        // `CHECK (num_nonnulls(to_location_id, to_package_id) <= 1)`, and a
        // pick that filled both would be refused by the database — which is a
        // 500 to somebody holding a scanner rather than an answer.
        let s = sides(&cell(), &station());
        assert_eq!(s.to_location_id, Some(u(8)));
        assert_eq!(s.to_package_id, None);
        // And the goods are the same goods: lot, status and owner travel.
        assert_eq!(s.from_lot_id, s.to_lot_id);
        assert_eq!(s.from_owner_id, s.to_owner_id);
    }

    #[test]
    fn a_station_is_as_good_a_destination_as_a_carton() {
        // The trolley walk, checked the same way the bench's pick is. Nothing
        // about landing at a location is a problem to report.
        let p = ProposedPick {
            to: Landing::Location(u(8)),
            ..good()
        };
        assert!(check(&p, Some(&line()), Some(&cell()), Some(&station())).is_empty());
    }

    #[test]
    fn a_destination_that_is_not_there_says_which_kind_it_was_looking_for() {
        // A missing package and a missing location are different sentences,
        // because they send somebody to look in different places.
        let p = ProposedPick { to: Landing::Location(u(8)), ..good() };
        assert_eq!(
            check(&p, Some(&line()), Some(&cell()), None),
            vec![Problem::LocationNotFound]
        );
        assert_eq!(
            check(&good(), Some(&line()), Some(&cell()), None),
            vec![Problem::PackageNotFound]
        );
    }

    #[test]
    fn a_pick_lands_in_exactly_one_place() {
        // **Checked once, and then unrepresentable.** The wire carries two
        // nullable ids because JSON has no sum type; `Landing` is what stops
        // every handler and every test downstream having to remember that
        // both-or-neither can be asked for.
        assert_eq!(landing(Some(u(7)), None), Ok(Landing::Package(u(7))));
        assert_eq!(landing(None, Some(u(8))), Ok(Landing::Location(u(8))));
        assert_eq!(landing(None, None), Err(Problem::NoDestination));
        assert_eq!(landing(Some(u(7)), Some(u(8))), Err(Problem::TwoDestinations));
    }

    #[test]
    fn a_tenants_station_is_not_another_tenants() {
        // The same check the package arm has always had, through the arm that
        // did not exist until D166.
        let p = ProposedPick { to: Landing::Location(u(8)), ..good() };
        let theirs = Destination::Location(Location {
            id: u(8),
            tenant_id: u(999),
            kind: "staging".into(),
        });
        assert!(check(&p, Some(&line()), Some(&cell()), Some(&theirs))
            .contains(&Problem::DifferentTenant));
    }
}
