//! Finding queue transitions (discrepancy state).
//!
//! D8: findings never block the floor; they move through a declared lifecycle.
//! Investigation is the hand-off from "just raised" to "someone owns it" before
//! resolution (e.g. inventory adjust with `resolving_movement_id`).
//! Accept closes without a ledger write — manager sign-off that the model stands.

/// Allowed states on `discrepancy.state`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FindingState {
    Open,
    Investigating,
    Resolved,
    Accepted,
}

impl FindingState {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "open" => Some(Self::Open),
            "investigating" => Some(Self::Investigating),
            "resolved" => Some(Self::Resolved),
            "accepted" => Some(Self::Accepted),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Investigating => "investigating",
            Self::Resolved => "resolved",
            Self::Accepted => "accepted",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum InvestigateProblem {
    NotFound,
    /// Soft: already investigating.
    AlreadyInvestigating,
    /// Hard: closed findings are not reopened on this path.
    Terminal { state: String },
}

impl std::fmt::Display for InvestigateProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvestigateProblem::NotFound => write!(f, "the finding was not found"),
            InvestigateProblem::AlreadyInvestigating => write!(
                f,
                "the finding is already under investigation; the write is idempotent"
            ),
            InvestigateProblem::Terminal { state } => write!(
                f,
                "a finding in state {state} cannot move to investigating on this path"
            ),
        }
    }
}

pub fn investigate_is_hard(p: &InvestigateProblem) -> bool {
    !matches!(p, InvestigateProblem::AlreadyInvestigating)
}

/// Plan open → investigating. Soft when already investigating.
pub fn check_investigate(current: Option<&str>) -> Result<(), InvestigateProblem> {
    let Some(s) = current else {
        return Err(InvestigateProblem::NotFound);
    };
    match FindingState::parse(s) {
        Some(FindingState::Open) => Ok(()),
        Some(FindingState::Investigating) => Err(InvestigateProblem::AlreadyInvestigating),
        Some(FindingState::Resolved) | Some(FindingState::Accepted) => {
            Err(InvestigateProblem::Terminal {
                state: s.to_string(),
            })
        }
        None => Err(InvestigateProblem::Terminal {
            state: s.to_string(),
        }),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AcceptProblem {
    NotFound,
    /// Soft: already accepted.
    AlreadyAccepted,
    /// Hard: ledger-resolved findings stay resolved; unknown states refused.
    Terminal { state: String },
    /// Hard: sign-off needs who accepted.
    MissingActor,
    /// Hard: sign-off needs a reason (no silent close).
    MissingReason,
}

impl std::fmt::Display for AcceptProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcceptProblem::NotFound => write!(f, "the finding was not found"),
            AcceptProblem::AlreadyAccepted => write!(
                f,
                "the finding is already accepted; the write is idempotent"
            ),
            AcceptProblem::Terminal { state } => write!(
                f,
                "a finding in state {state} cannot move to accepted on this path"
            ),
            AcceptProblem::MissingActor => write!(
                f,
                "recorded_by_id is required to accept a finding"
            ),
            AcceptProblem::MissingReason => write!(
                f,
                "reason is required to accept a finding without a ledger write"
            ),
        }
    }
}

pub fn accept_is_hard(p: &AcceptProblem) -> bool {
    !matches!(p, AcceptProblem::AlreadyAccepted)
}

/// Plan open|investigating → accepted. Soft when already accepted.
///
/// Does not write stock. Distinct from resolve-via-adjust (`resolving_movement_id`).
pub fn check_accept(
    current: Option<&str>,
    has_actor: bool,
    reason: Option<&str>,
) -> Result<(), AcceptProblem> {
    if !has_actor {
        return Err(AcceptProblem::MissingActor);
    }
    let reason = reason.map(str::trim).filter(|s| !s.is_empty());
    if reason.is_none() {
        return Err(AcceptProblem::MissingReason);
    }
    let Some(s) = current else {
        return Err(AcceptProblem::NotFound);
    };
    match FindingState::parse(s) {
        Some(FindingState::Open) | Some(FindingState::Investigating) => Ok(()),
        Some(FindingState::Accepted) => Err(AcceptProblem::AlreadyAccepted),
        Some(FindingState::Resolved) => Err(AcceptProblem::Terminal {
            state: s.to_string(),
        }),
        None => Err(AcceptProblem::Terminal {
            state: s.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_may_investigate() {
        assert!(check_investigate(Some("open")).is_ok());
    }

    #[test]
    fn already_investigating_is_soft() {
        let e = check_investigate(Some("investigating")).unwrap_err();
        assert_eq!(e, InvestigateProblem::AlreadyInvestigating);
        assert!(!investigate_is_hard(&e));
    }

    #[test]
    fn resolved_is_hard() {
        let e = check_investigate(Some("resolved")).unwrap_err();
        assert!(investigate_is_hard(&e));
    }

    #[test]
    fn missing_is_hard() {
        assert_eq!(
            check_investigate(None).unwrap_err(),
            InvestigateProblem::NotFound
        );
    }

    #[test]
    fn open_and_investigating_may_accept() {
        assert!(check_accept(Some("open"), true, Some("within tolerance")).is_ok());
        assert!(check_accept(Some("investigating"), true, Some("re-count ok")).is_ok());
    }

    #[test]
    fn already_accepted_is_soft() {
        let e = check_accept(Some("accepted"), true, Some("again")).unwrap_err();
        assert_eq!(e, AcceptProblem::AlreadyAccepted);
        assert!(!accept_is_hard(&e));
    }

    #[test]
    fn resolved_cannot_accept() {
        let e = check_accept(Some("resolved"), true, Some("no")).unwrap_err();
        assert!(accept_is_hard(&e));
        assert!(matches!(e, AcceptProblem::Terminal { .. }));
    }

    #[test]
    fn accept_requires_actor_and_reason() {
        assert_eq!(
            check_accept(Some("open"), false, Some("ok")).unwrap_err(),
            AcceptProblem::MissingActor
        );
        assert_eq!(
            check_accept(Some("open"), true, None).unwrap_err(),
            AcceptProblem::MissingReason
        );
        assert_eq!(
            check_accept(Some("open"), true, Some("  ")).unwrap_err(),
            AcceptProblem::MissingReason
        );
    }
}
