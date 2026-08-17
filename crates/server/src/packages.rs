//! Creating, placing, and containing packages via `package_event`.
//!
//! Migration 9 grants the app INSERT on identity and packing columns of
//! `package`, not on barcode/sscc/status/location/parent — those are projections
//! of `package_event` (D29, J6). Create is two inserts (skeleton + `created`);
//! place is `placed` with a location; contain is `contained` with a parent
//! package (D97). The fold is compare-and-set on device clock (J6); nothing
//! here UPDATEs a projection column.

use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ProposedPackage {
    pub tenant_id: Uuid,
    pub package_id: Uuid,
    pub fulfilment_id: Option<Uuid>,
    pub sequence: Option<i32>,
    pub package_type_id: Option<Uuid>,
    /// Required: a `created` event asserts placement (D97).
    pub location_id: Uuid,
    pub barcode: Option<String>,
    pub sscc: Option<String>,
    pub source: String,
}

/// What the write path already loaded about the fulfilment, if named.
#[derive(Clone, Debug)]
pub struct FulfilmentRef {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub cancelled: bool,
}

#[derive(Clone, Debug)]
pub struct LocationRef {
    pub id: Uuid,
    pub tenant_id: Uuid,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    FulfilmentNotFound,
    LocationNotFound,
    DifferentTenant,
    FulfilmentCancelled,
    InvalidSource,
    /// SSCC is 18 digits when present (GS1).
    InvalidSscc,
    EmptyBarcode,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::FulfilmentNotFound => write!(f, "the fulfilment was not found"),
            Problem::LocationNotFound => write!(f, "the location was not found"),
            Problem::DifferentTenant => {
                write!(f, "the fulfilment or location belongs to another tenant")
            }
            Problem::FulfilmentCancelled => {
                write!(f, "the fulfilment is cancelled; no shipping carton for it")
            }
            Problem::InvalidSource => write!(
                f,
                "source must be one of operator_scan, label, asn, derived, correction, keyed"
            ),
            Problem::InvalidSscc => {
                write!(f, "sscc must be exactly 18 characters when provided")
            }
            Problem::EmptyBarcode => write!(f, "barcode, if provided, must not be empty"),
        }
    }
}

const SOURCES: &[&str] = &[
    "operator_scan",
    "label",
    "asn",
    "derived",
    "correction",
    "keyed",
];

pub fn check(
    proposed: &ProposedPackage,
    fulfilment: Option<&FulfilmentRef>,
    location: Option<&LocationRef>,
) -> Vec<Problem> {
    let mut problems = vec![];

    if !SOURCES.contains(&proposed.source.as_str()) {
        problems.push(Problem::InvalidSource);
    }

    if let Some(ref sscc) = proposed.sscc {
        if sscc.len() != 18 {
            problems.push(Problem::InvalidSscc);
        }
    }
    if let Some(ref b) = proposed.barcode {
        if b.is_empty() {
            problems.push(Problem::EmptyBarcode);
        }
    }

    let Some(location) = location else {
        problems.push(Problem::LocationNotFound);
        return problems;
    };
    if location.tenant_id != proposed.tenant_id {
        problems.push(Problem::DifferentTenant);
    }

    if proposed.fulfilment_id.is_some() {
        let Some(fulfilment) = fulfilment else {
            problems.push(Problem::FulfilmentNotFound);
            return problems;
        };
        if fulfilment.tenant_id != proposed.tenant_id {
            problems.push(Problem::DifferentTenant);
        }
        if fulfilment.cancelled {
            problems.push(Problem::FulfilmentCancelled);
        }
    }

    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn good() -> ProposedPackage {
        ProposedPackage {
            tenant_id: u(1),
            package_id: u(2),
            fulfilment_id: Some(u(3)),
            sequence: Some(1),
            package_type_id: None,
            location_id: u(4),
            barcode: Some("CARTON-1".into()),
            sscc: None,
            source: "operator_scan".into(),
        }
    }

    fn ful() -> FulfilmentRef {
        FulfilmentRef {
            id: u(3),
            tenant_id: u(1),
            cancelled: false,
        }
    }

    fn loc() -> LocationRef {
        LocationRef {
            id: u(4),
            tenant_id: u(1),
        }
    }

    #[test]
    fn a_well_formed_create_passes() {
        assert!(check(&good(), Some(&ful()), Some(&loc())).is_empty());
    }

    #[test]
    fn without_fulfilment_is_fine() {
        let mut p = good();
        p.fulfilment_id = None;
        assert!(check(&p, None, Some(&loc())).is_empty());
    }

    #[test]
    fn bad_source_fails() {
        let mut p = good();
        p.source = "telepathy".into();
        assert!(check(&p, Some(&ful()), Some(&loc())).contains(&Problem::InvalidSource));
    }

    #[test]
    fn short_sscc_fails() {
        let mut p = good();
        p.sscc = Some("123".into());
        assert!(check(&p, Some(&ful()), Some(&loc())).contains(&Problem::InvalidSscc));
    }

    #[test]
    fn cancelled_fulfilment_fails() {
        let mut f = ful();
        f.cancelled = true;
        assert!(check(&good(), Some(&f), Some(&loc())).contains(&Problem::FulfilmentCancelled));
    }

    #[test]
    fn missing_location_fails() {
        assert!(check(&good(), Some(&ful()), None).contains(&Problem::LocationNotFound));
    }

    // -----------------------------------------------------------------------
    // Place
    // -----------------------------------------------------------------------

    fn place_good() -> ProposedPlace {
        ProposedPlace {
            tenant_id: u(1),
            package_id: u(2),
            location_id: u(4),
            source: "operator_scan".into(),
        }
    }

    fn pkg() -> PackageRef {
        PackageRef {
            id: u(2),
            tenant_id: u(1),
            status: Some("created".into()),
            resolved_location_id: Some(u(9)),
            parent_package_id: None,
        }
    }

    #[test]
    fn a_well_formed_place_passes() {
        let (p, w) = check_place(&place_good(), Some(&pkg()), Some(&loc()));
        assert!(p.is_empty(), "{p:?}");
        assert!(w.is_empty());
    }

    #[test]
    fn same_location_is_soft() {
        let mut p = pkg();
        p.resolved_location_id = Some(u(4));
        let (problems, w) = check_place(&place_good(), Some(&p), Some(&loc()));
        assert!(!problems.iter().any(place_is_hard));
        assert_eq!(w, vec![PlaceProblem::AlreadyThere]);
    }

    #[test]
    fn despatched_cannot_be_placed() {
        let mut p = pkg();
        p.status = Some("despatched".into());
        let (problems, _) = check_place(&place_good(), Some(&p), Some(&loc()));
        assert!(problems.contains(&PlaceProblem::AlreadyTerminal {
            status: "despatched".into()
        }));
    }

    #[test]
    fn missing_package_is_hard() {
        let (problems, _) = check_place(&place_good(), None, Some(&loc()));
        assert!(problems.contains(&PlaceProblem::PackageNotFound));
    }
}

// ---------------------------------------------------------------------------
// Place (relocate a package to a location)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ProposedPlace {
    pub tenant_id: Uuid,
    pub package_id: Uuid,
    pub location_id: Uuid,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct PackageRef {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub status: Option<String>,
    pub resolved_location_id: Option<Uuid>,
    /// Projected parent after containment fold (J6).
    pub parent_package_id: Option<Uuid>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PlaceProblem {
    PackageNotFound,
    LocationNotFound,
    DifferentTenant,
    InvalidSource,
    /// Soft: projected location already matches; event still recorded (J6).
    AlreadyThere,
    /// Hard: despatched or voided packages are not relocated on this path.
    AlreadyTerminal { status: String },
}

impl std::fmt::Display for PlaceProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaceProblem::PackageNotFound => write!(f, "the package was not found"),
            PlaceProblem::LocationNotFound => write!(f, "the location was not found"),
            PlaceProblem::DifferentTenant => {
                write!(f, "the package or location belongs to another tenant")
            }
            PlaceProblem::InvalidSource => write!(
                f,
                "source must be one of operator_scan, label, asn, derived, correction, keyed"
            ),
            PlaceProblem::AlreadyThere => write!(
                f,
                "the package is already projected at that location; the place event is still \
                 recorded and the fold is compare-and-set (J6)"
            ),
            PlaceProblem::AlreadyTerminal { status } => write!(
                f,
                "a package in status {status} cannot be placed; despatched and voided \
                 packages leave this path"
            ),
        }
    }
}

pub fn place_is_hard(p: &PlaceProblem) -> bool {
    !matches!(p, PlaceProblem::AlreadyThere)
}

/// Check a place. Soft problems travel with success; hard ones block.
pub fn check_place(
    proposed: &ProposedPlace,
    package: Option<&PackageRef>,
    location: Option<&LocationRef>,
) -> (Vec<PlaceProblem>, Vec<PlaceProblem>) {
    let mut hard = vec![];
    let mut soft = vec![];

    if !SOURCES.contains(&proposed.source.as_str()) {
        hard.push(PlaceProblem::InvalidSource);
    }

    let Some(package) = package else {
        hard.push(PlaceProblem::PackageNotFound);
        return (hard, soft);
    };
    let Some(location) = location else {
        hard.push(PlaceProblem::LocationNotFound);
        return (hard, soft);
    };

    if package.tenant_id != proposed.tenant_id || location.tenant_id != proposed.tenant_id {
        hard.push(PlaceProblem::DifferentTenant);
    }

    if matches!(
        package.status.as_deref(),
        Some("despatched") | Some("voided")
    ) {
        hard.push(PlaceProblem::AlreadyTerminal {
            status: package.status.clone().unwrap_or_default(),
        });
    }

    if package.resolved_location_id == Some(proposed.location_id) {
        soft.push(PlaceProblem::AlreadyThere);
    }

    (hard, soft)
}

// ---------------------------------------------------------------------------
// Contain (nest package under a parent package)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ProposedContain {
    pub tenant_id: Uuid,
    /// Child package being nested.
    pub package_id: Uuid,
    /// Parent (pallet, outer carton, …).
    pub parent_package_id: Uuid,
    pub source: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ContainProblem {
    PackageNotFound,
    ParentNotFound,
    DifferentTenant,
    InvalidSource,
    /// Hard: cannot nest a package under itself.
    SelfParent,
    /// Hard: child or parent has left / been voided.
    AlreadyTerminal { which: &'static str, status: String },
    /// Soft: projected parent already matches.
    AlreadyContained,
}

impl std::fmt::Display for ContainProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainProblem::PackageNotFound => write!(f, "the package was not found"),
            ContainProblem::ParentNotFound => write!(f, "the parent package was not found"),
            ContainProblem::DifferentTenant => {
                write!(f, "the package or parent belongs to another tenant")
            }
            ContainProblem::InvalidSource => write!(
                f,
                "source must be one of operator_scan, label, asn, derived, correction, keyed"
            ),
            ContainProblem::SelfParent => {
                write!(f, "a package cannot be contained in itself")
            }
            ContainProblem::AlreadyTerminal { which, status } => write!(
                f,
                "the {which} is in status {status}; despatched and voided packages \
                 cannot change containment on this path"
            ),
            ContainProblem::AlreadyContained => write!(
                f,
                "the package is already projected under that parent; the contain event \
                 is still recorded and the fold is compare-and-set (J6)"
            ),
        }
    }
}

pub fn contain_is_hard(p: &ContainProblem) -> bool {
    !matches!(p, ContainProblem::AlreadyContained)
}

/// Check a contain. Soft problems travel with success; hard ones block.
pub fn check_contain(
    proposed: &ProposedContain,
    package: Option<&PackageRef>,
    parent: Option<&PackageRef>,
) -> (Vec<ContainProblem>, Vec<ContainProblem>) {
    let mut hard = vec![];
    let mut soft = vec![];

    if !SOURCES.contains(&proposed.source.as_str()) {
        hard.push(ContainProblem::InvalidSource);
    }

    if proposed.package_id == proposed.parent_package_id {
        hard.push(ContainProblem::SelfParent);
    }

    let Some(package) = package else {
        hard.push(ContainProblem::PackageNotFound);
        return (hard, soft);
    };
    let Some(parent) = parent else {
        hard.push(ContainProblem::ParentNotFound);
        return (hard, soft);
    };

    if package.tenant_id != proposed.tenant_id || parent.tenant_id != proposed.tenant_id {
        hard.push(ContainProblem::DifferentTenant);
    }

    if matches!(
        package.status.as_deref(),
        Some("despatched") | Some("voided")
    ) {
        hard.push(ContainProblem::AlreadyTerminal {
            which: "package",
            status: package.status.clone().unwrap_or_default(),
        });
    }
    if matches!(
        parent.status.as_deref(),
        Some("despatched") | Some("voided")
    ) {
        hard.push(ContainProblem::AlreadyTerminal {
            which: "parent",
            status: parent.status.clone().unwrap_or_default(),
        });
    }

    if package.parent_package_id == Some(proposed.parent_package_id) {
        soft.push(ContainProblem::AlreadyContained);
    }

    (hard, soft)
}

#[cfg(test)]
mod contain_tests {
    use super::*;

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn child() -> PackageRef {
        PackageRef {
            id: u(2),
            tenant_id: u(1),
            status: Some("placed".into()),
            resolved_location_id: None,
            parent_package_id: None,
        }
    }

    fn parent() -> PackageRef {
        PackageRef {
            id: u(3),
            tenant_id: u(1),
            status: Some("created".into()),
            resolved_location_id: Some(u(9)),
            parent_package_id: None,
        }
    }

    fn good() -> ProposedContain {
        ProposedContain {
            tenant_id: u(1),
            package_id: u(2),
            parent_package_id: u(3),
            source: "operator_scan".into(),
        }
    }

    #[test]
    fn a_well_formed_contain_passes() {
        let (h, s) = check_contain(&good(), Some(&child()), Some(&parent()));
        assert!(h.is_empty(), "{h:?}");
        assert!(s.is_empty());
    }

    #[test]
    fn self_parent_is_hard() {
        let mut p = good();
        p.parent_package_id = p.package_id;
        let (h, _) = check_contain(&p, Some(&child()), Some(&child()));
        assert!(h.contains(&ContainProblem::SelfParent));
    }

    #[test]
    fn despatched_child_is_hard() {
        let mut c = child();
        c.status = Some("despatched".into());
        let (h, _) = check_contain(&good(), Some(&c), Some(&parent()));
        assert!(h.iter().any(|x| matches!(
            x,
            ContainProblem::AlreadyTerminal {
                which: "package",
                ..
            }
        )));
    }

    #[test]
    fn already_under_parent_is_soft() {
        let mut c = child();
        c.parent_package_id = Some(u(3));
        let (h, s) = check_contain(&good(), Some(&c), Some(&parent()));
        assert!(h.is_empty());
        assert_eq!(s, vec![ContainProblem::AlreadyContained]);
        assert!(!s.iter().any(contain_is_hard));
    }
}
