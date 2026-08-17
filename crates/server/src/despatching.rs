//! Seal, open, and despatch: package events, and stock leaving the building.
//!
//! **Seal** is one `package_event` (`kind = sealed`). Nothing moves. Packed
//! progress reads `package.status` after the fold (D100), not `sealed_at` —
//! though the app may stamp `sealed_at` as a freeze-time column (migration 9).
//!
//! **Open** is one `package_event` (`kind = opened`) — rework after a seal.
//! Clears `sealed_at` when the app may write it; does not UPDATE status.
//!
//! **Despatch** is two facts: a `despatched` event on the package, and a
//! `stock_movement` with no `to` side out of the package (D99: the mirror of
//! D45's arrival test), naming the fulfilment line. Progress columns are not
//! UPDATEd here.
//!
//! Floor observations (D5). Soft problems travel with success; hard ones block
//! the write.

use uuid::Uuid;

// ---------------------------------------------------------------------------
// Seal
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ProposedSeal {
    pub tenant_id: Uuid,
    pub package_id: Uuid,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct PackageState {
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// Folded status, if the maintainers have run.
    pub status: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SealProblem {
    PackageNotFound,
    DifferentTenant,
    InvalidSource,
    /// Soft: status already sealed or despatched (or we already have the event).
    AlreadySealed,
}

impl std::fmt::Display for SealProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SealProblem::PackageNotFound => write!(f, "the package was not found"),
            SealProblem::DifferentTenant => {
                write!(f, "the package belongs to another tenant")
            }
            SealProblem::InvalidSource => write!(
                f,
                "source must be one of operator_scan, label, asn, derived, correction, keyed"
            ),
            SealProblem::AlreadySealed => write!(
                f,
                "the package is already sealed or despatched; a second seal event is still \
                 recorded if you insist, but the fold will not move"
            ),
        }
    }
}

pub fn seal_is_hard(p: &SealProblem) -> bool {
    !matches!(p, SealProblem::AlreadySealed)
}

const SOURCES: &[&str] = &[
    "operator_scan",
    "label",
    "asn",
    "derived",
    "correction",
    "keyed",
];

pub fn check_seal(proposed: &ProposedSeal, package: Option<&PackageState>) -> Vec<SealProblem> {
    let mut problems = vec![];
    if !SOURCES.contains(&proposed.source.as_str()) {
        problems.push(SealProblem::InvalidSource);
    }
    let Some(package) = package else {
        problems.push(SealProblem::PackageNotFound);
        return problems;
    };
    if package.tenant_id != proposed.tenant_id {
        problems.push(SealProblem::DifferentTenant);
    }
    if matches!(
        package.status.as_deref(),
        Some("sealed") | Some("despatched")
    ) {
        problems.push(SealProblem::AlreadySealed);
    }
    problems
}

// ---------------------------------------------------------------------------
// Open (unseal for rework)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ProposedOpen {
    pub tenant_id: Uuid,
    pub package_id: Uuid,
    pub source: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum OpenProblem {
    PackageNotFound,
    DifferentTenant,
    InvalidSource,
    /// Hard: left the site or voided — not reworkable on this path.
    Terminal { status: String },
    /// Soft: projected status is not sealed (still open / placed / created).
    NotSealed,
    /// Soft: already showing opened.
    AlreadyOpen,
}

impl std::fmt::Display for OpenProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenProblem::PackageNotFound => write!(f, "the package was not found"),
            OpenProblem::DifferentTenant => {
                write!(f, "the package belongs to another tenant")
            }
            OpenProblem::InvalidSource => write!(
                f,
                "source must be one of operator_scan, label, asn, derived, correction, keyed"
            ),
            OpenProblem::Terminal { status } => write!(
                f,
                "a package in status {status} cannot be opened; despatched and voided \
                 packages leave this path"
            ),
            OpenProblem::NotSealed => write!(
                f,
                "the package is not projected as sealed; the open event is still recorded \
                 for rework if the floor says so (D5)"
            ),
            OpenProblem::AlreadyOpen => write!(
                f,
                "the package is already projected as opened; a second open event is still \
                 recorded and the fold is compare-and-set (J6)"
            ),
        }
    }
}

pub fn open_is_hard(p: &OpenProblem) -> bool {
    !matches!(p, OpenProblem::NotSealed | OpenProblem::AlreadyOpen)
}

pub fn check_open(proposed: &ProposedOpen, package: Option<&PackageState>) -> Vec<OpenProblem> {
    let mut problems = vec![];
    if !SOURCES.contains(&proposed.source.as_str()) {
        problems.push(OpenProblem::InvalidSource);
    }
    let Some(package) = package else {
        problems.push(OpenProblem::PackageNotFound);
        return problems;
    };
    if package.tenant_id != proposed.tenant_id {
        problems.push(OpenProblem::DifferentTenant);
    }
    match package.status.as_deref() {
        Some("despatched") | Some("voided") => {
            problems.push(OpenProblem::Terminal {
                status: package.status.clone().unwrap_or_default(),
            });
        }
        Some("opened") => problems.push(OpenProblem::AlreadyOpen),
        Some("sealed") => {}
        // null, created, placed, contained, …
        _ => problems.push(OpenProblem::NotSealed),
    }
    problems
}

// ---------------------------------------------------------------------------
// Despatch
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ProposedDespatch {
    pub tenant_id: Uuid,
    pub package_id: Uuid,
    pub fulfilment_line_id: Uuid,
    pub quantity: i64,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct FulfilmentLine {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub quantity: i64,
    pub despatched_quantity: i64,
    pub fulfilment_cancelled: bool,
}

/// Stock held in the package that will leave.
#[derive(Clone, Debug)]
pub struct PackageStock {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub holder_package_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub status_id: Uuid,
    pub owner_id: Uuid,
    pub quantity: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DespatchProblem {
    NothingToDespatch,
    PackageNotFound,
    LineNotFound,
    StockNotFound,
    DifferentTenant,
    LineCancelled,
    WrongPackage,
    /// Soft: cell short.
    OverQuantity { available: i64, proposed: i64 },
    /// Soft: already fully despatched on the projection.
    LineAlreadyDespatched,
    InvalidSource,
}

impl std::fmt::Display for DespatchProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DespatchProblem::NothingToDespatch => {
                write!(f, "a despatch of no units is not a despatch")
            }
            DespatchProblem::PackageNotFound => write!(f, "the package was not found"),
            DespatchProblem::LineNotFound => write!(f, "the fulfilment line was not found"),
            DespatchProblem::StockNotFound => {
                write!(f, "no stock cell in the package matches the line's item")
            }
            DespatchProblem::DifferentTenant => {
                write!(f, "the package, line or cell belongs to another tenant")
            }
            DespatchProblem::LineCancelled => {
                write!(f, "the fulfilment is cancelled; nothing is left to despatch")
            }
            DespatchProblem::WrongPackage => {
                write!(f, "the stock cell is not held in this package")
            }
            DespatchProblem::OverQuantity { available, proposed } => write!(
                f,
                "despatching {proposed} from a cell of {available}; the write is still \
                 recorded (D5) and the short is a finding rather than a refusal"
            ),
            DespatchProblem::LineAlreadyDespatched => write!(
                f,
                "the line's despatched quantity already meets its commitment; the movement \
                 is still recorded if it happened on the floor"
            ),
            DespatchProblem::InvalidSource => write!(
                f,
                "source must be one of operator_scan, label, asn, derived, correction, keyed"
            ),
        }
    }
}

pub fn despatch_is_hard(p: &DespatchProblem) -> bool {
    !matches!(
        p,
        DespatchProblem::OverQuantity { .. } | DespatchProblem::LineAlreadyDespatched
    )
}

pub fn check_despatch(
    proposed: &ProposedDespatch,
    package: Option<&PackageState>,
    line: Option<&FulfilmentLine>,
    stock: Option<&PackageStock>,
) -> Vec<DespatchProblem> {
    let mut problems = vec![];

    if proposed.quantity <= 0 {
        problems.push(DespatchProblem::NothingToDespatch);
    }
    if !SOURCES.contains(&proposed.source.as_str()) {
        problems.push(DespatchProblem::InvalidSource);
    }

    let Some(package) = package else {
        problems.push(DespatchProblem::PackageNotFound);
        return problems;
    };
    let Some(line) = line else {
        problems.push(DespatchProblem::LineNotFound);
        return problems;
    };
    let Some(stock) = stock else {
        problems.push(DespatchProblem::StockNotFound);
        return problems;
    };

    if package.tenant_id != proposed.tenant_id
        || line.tenant_id != proposed.tenant_id
        || stock.tenant_id != proposed.tenant_id
    {
        problems.push(DespatchProblem::DifferentTenant);
    }
    if line.fulfilment_cancelled {
        problems.push(DespatchProblem::LineCancelled);
    }
    if stock.holder_package_id != Some(package.id) {
        problems.push(DespatchProblem::WrongPackage);
    }
    // Item: movement will carry stock.item_id; J69 if it disagrees with the line.

    if proposed.quantity > stock.quantity {
        problems.push(DespatchProblem::OverQuantity {
            available: stock.quantity,
            proposed: proposed.quantity,
        });
    }
    if line.despatched_quantity >= line.quantity {
        problems.push(DespatchProblem::LineAlreadyDespatched);
    }

    let _ = line.item_id;
    let _ = stock.item_id;
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn pkg() -> PackageState {
        PackageState {
            id: u(1),
            tenant_id: u(10),
            status: Some("sealed".into()),
        }
    }

    fn line() -> FulfilmentLine {
        FulfilmentLine {
            id: u(2),
            tenant_id: u(10),
            item_id: u(100),
            quantity: 20,
            despatched_quantity: 0,
            fulfilment_cancelled: false,
        }
    }

    fn stock() -> PackageStock {
        PackageStock {
            id: u(3),
            tenant_id: u(10),
            item_id: u(100),
            holder_package_id: Some(u(1)),
            lot_id: Some(u(4)),
            status_id: u(5),
            owner_id: u(6),
            quantity: 20,
        }
    }

    fn good_despatch() -> ProposedDespatch {
        ProposedDespatch {
            tenant_id: u(10),
            package_id: u(1),
            fulfilment_line_id: u(2),
            quantity: 10,
            source: "operator_scan".into(),
        }
    }

    #[test]
    fn seal_of_open_package_passes() {
        let p = ProposedSeal {
            tenant_id: u(10),
            package_id: u(1),
            source: "operator_scan".into(),
        };
        let mut pkg = pkg();
        pkg.status = Some("open".into());
        assert!(check_seal(&p, Some(&pkg)).is_empty());
    }

    #[test]
    fn seal_when_already_sealed_is_soft() {
        let p = ProposedSeal {
            tenant_id: u(10),
            package_id: u(1),
            source: "operator_scan".into(),
        };
        let problems = check_seal(&p, Some(&pkg()));
        assert!(problems.contains(&SealProblem::AlreadySealed));
        assert!(!problems.iter().any(seal_is_hard));
    }

    #[test]
    fn open_of_sealed_package_passes() {
        let p = ProposedOpen {
            tenant_id: u(10),
            package_id: u(1),
            source: "operator_scan".into(),
        };
        assert!(check_open(&p, Some(&pkg())).is_empty());
    }

    #[test]
    fn open_of_despatched_is_hard() {
        let p = ProposedOpen {
            tenant_id: u(10),
            package_id: u(1),
            source: "operator_scan".into(),
        };
        let mut pkg = pkg();
        pkg.status = Some("despatched".into());
        let problems = check_open(&p, Some(&pkg));
        assert!(problems.iter().any(|x| matches!(x, OpenProblem::Terminal { .. })));
        assert!(problems.iter().any(open_is_hard));
    }

    #[test]
    fn open_when_not_sealed_is_soft() {
        let p = ProposedOpen {
            tenant_id: u(10),
            package_id: u(1),
            source: "operator_scan".into(),
        };
        let mut pkg = pkg();
        pkg.status = Some("placed".into());
        let problems = check_open(&p, Some(&pkg));
        assert!(problems.contains(&OpenProblem::NotSealed));
        assert!(!problems.iter().any(open_is_hard));
    }

    #[test]
    fn a_well_formed_despatch_passes() {
        assert!(
            check_despatch(&good_despatch(), Some(&pkg()), Some(&line()), Some(&stock()))
                .is_empty()
        );
    }

    #[test]
    fn over_quantity_is_soft() {
        let mut d = good_despatch();
        d.quantity = 50;
        let problems =
            check_despatch(&d, Some(&pkg()), Some(&line()), Some(&stock()));
        assert!(matches!(
            problems
                .iter()
                .find(|p| matches!(p, DespatchProblem::OverQuantity { .. })),
            Some(DespatchProblem::OverQuantity {
                available: 20,
                proposed: 50
            })
        ));
        assert!(!problems.iter().any(despatch_is_hard));
    }

    #[test]
    fn wrong_package_is_hard() {
        let mut s = stock();
        s.holder_package_id = Some(u(99));
        let problems =
            check_despatch(&good_despatch(), Some(&pkg()), Some(&line()), Some(&s));
        assert!(problems.contains(&DespatchProblem::WrongPackage));
        assert!(problems.iter().any(despatch_is_hard));
    }

    #[test]
    fn nothing_to_despatch_is_hard() {
        let mut d = good_despatch();
        d.quantity = 0;
        let problems =
            check_despatch(&d, Some(&pkg()), Some(&line()), Some(&stock()));
        assert!(problems.contains(&DespatchProblem::NothingToDespatch));
    }
}
