//! The rules a correction has to satisfy, and the reason they live here.
//!
//! Migration 10 built `reverses_movement_id` and stopped. Four rules govern what
//! may be written through it, all four compare the new row to the row it
//! corrects, and Postgres has no cross-row CHECK. The obvious answer is a
//! trigger. S7 rejects triggers, D25 forbids the validation kind by name, and
//! `stock_movement` is the one table where a per-insert SELECT is a cost worth
//! caring about.
//!
//! So they live here, as a pure function over the target row and the proposed
//! correction. The write path has to read the target anyway to mirror its sides,
//! so this costs nothing it was not already paying. J50 to J52 assert the same
//! four rules against stored data, as findings, for rows that arrive by a
//! restore, a backfill or a path that has not been written yet.
//!
//! Nothing here rejects an observation of the floor. D5 protects that path and
//! this is not on it: a correction is a claim about the record, and an operator
//! whose correction is refused can still record what they see as an ordinary
//! movement. That difference is what makes rejecting admissible at all.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// The class split from `revision vocabulary, which is the whole point of the
/// vocabulary: stock that stopped existing is not stock that never existed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RevisionClass {
    RecordError,
    WorldEvent,
}

/// The five columns that make up one side of a movement. A populated side
/// carries the whole key or the balance silently forks, which is what
/// `stock_movement_from_whole_key_ck` exists to stop.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct Side {
    pub location_id: Option<Uuid>,
    pub package_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub status_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
}

#[derive(Clone, Debug)]
pub struct Movement {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub quantity: i64,
    pub occurred_at: DateTime<Utc>,
    pub from: Side,
    pub to: Side,
}

/// What is being asked for, before it is a row.
#[derive(Clone, Debug)]
pub struct ProposedCorrection {
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub quantity: i64,
    pub occurred_at: DateTime<Utc>,
    pub from: Side,
    pub to: Side,
    pub reverses_movement_id: Option<Uuid>,
    pub reason_class: Option<RevisionClass>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    /// The one with no symptom. A correction stamped at the moment of discovery
    /// rather than the moment it is about leaves every as-at query answering
    /// with the pre-correction number, permanently and silently.
    WrongOccurredAt {
        stamped: DateTime<Utc>,
        target: DateTime<Utc>,
    },
    /// A reversal that does not run the other way to its target is a second
    /// movement wearing a correction's label.
    DoesNotMirror,
    Overreaches { proposed: i64, target: i64 },
    DifferentItem,
    DifferentTenant,
    /// `record_error` means nothing moved, so it has to name what it corrects.
    ReasonWithoutTarget,
    /// A `world_event` reversing something is a contradiction: if it physically
    /// happened, it is a movement in its own right and reverses nothing.
    WorldEventOnACorrection,
    MissingReason,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::WrongOccurredAt { stamped, target } => write!(
                f,
                "the correction is stamped {stamped} but corrects a fact about {target}; a \
                 correction carries its target's occurred_at, and its own recorded_at is what \
                 says when the error was found"
            ),
            Problem::DoesNotMirror => write!(
                f,
                "the correction does not run the other way to the movement it reverses; reverse \
                 the target exactly, then post a separate movement for what should have been \
                 recorded"
            ),
            Problem::Overreaches { proposed, target } => {
                write!(f, "reversing {proposed} units of a movement of {target}")
            }
            Problem::DifferentItem => write!(f, "the correction names a different item"),
            Problem::DifferentTenant => write!(f, "the correction names another tenant's movement"),
            Problem::ReasonWithoutTarget => write!(
                f,
                "a record_error reason names no movement to correct; a record error is a claim \
                 about a specific earlier row"
            ),
            Problem::WorldEventOnACorrection => write!(
                f,
                "a world_event reason cannot reverse anything; if something physically happened \
                 it is a movement in its own right"
            ),
            Problem::MissingReason => write!(f, "a correction has to say why the record was wrong"),
        }
    }
}

/// Check a proposed correction against the movement it names.
///
/// `target` is `None` when nothing is being corrected, which is the ordinary
/// case: most movements are not corrections and only the reason class is
/// checked. Returns every problem rather than the first, because a person
/// fixing one and resubmitting to find another is the sort of thing that makes
/// people stop correcting, and a record nobody corrects rots while looking
/// healthy.
pub fn check(proposed: &ProposedCorrection, target: Option<&Movement>) -> Vec<Problem> {
    let mut problems = vec![];

    let Some(target) = target else {
        if proposed.reason_class == Some(RevisionClass::RecordError) {
            problems.push(Problem::ReasonWithoutTarget);
        }
        return problems;
    };

    match proposed.reason_class {
        None => problems.push(Problem::MissingReason),
        Some(RevisionClass::WorldEvent) => problems.push(Problem::WorldEventOnACorrection),
        Some(RevisionClass::RecordError) => {}
    }

    if proposed.tenant_id != target.tenant_id {
        problems.push(Problem::DifferentTenant);
    }
    if proposed.item_id != target.item_id {
        problems.push(Problem::DifferentItem);
    }
    if proposed.occurred_at != target.occurred_at {
        problems.push(Problem::WrongOccurredAt {
            stamped: proposed.occurred_at,
            target: target.occurred_at,
        });
    }
    if proposed.from != target.to || proposed.to != target.from {
        problems.push(Problem::DoesNotMirror);
    }
    if proposed.quantity > target.quantity {
        problems.push(Problem::Overreaches {
            proposed: proposed.quantity,
            target: target.quantity,
        });
    }

    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 4, h, m, 0).unwrap()
    }

    fn bin() -> Side {
        Side {
            location_id: Some(u(0xB1)),
            lot_id: Some(u(0x107)),
            status_id: Some(u(0x57A7)),
            owner_id: Some(u(0x9A24)),
            ..Default::default()
        }
    }

    /// Receive 100 into a bin at 09:14.
    fn receipt() -> Movement {
        Movement {
            id: u(1),
            tenant_id: u(0x7E11),
            item_id: u(0x17E1),
            quantity: 100,
            occurred_at: at(9, 14),
            from: Side::default(),
            to: bin(),
        }
    }

    /// Four of them were never there, found at 11:30.
    fn good() -> ProposedCorrection {
        let t = receipt();
        ProposedCorrection {
            tenant_id: t.tenant_id,
            item_id: t.item_id,
            quantity: 4,
            occurred_at: t.occurred_at,
            from: t.to,
            to: t.from,
            reverses_movement_id: Some(t.id),
            reason_class: Some(RevisionClass::RecordError),
        }
    }

    #[test]
    fn a_well_formed_correction_passes() {
        assert_eq!(check(&good(), Some(&receipt())), vec![]);
    }

    #[test]
    fn the_discovery_time_is_not_the_correction_time() {
        let mut p = good();
        p.occurred_at = at(11, 30);
        assert_eq!(
            check(&p, Some(&receipt())),
            vec![Problem::WrongOccurredAt {
                stamped: at(11, 30),
                target: at(9, 14),
            }]
        );
    }

    #[test]
    fn a_correction_runs_the_other_way() {
        let mut p = good();
        std::mem::swap(&mut p.from, &mut p.to);
        assert_eq!(check(&p, Some(&receipt())), vec![Problem::DoesNotMirror]);
    }

    #[test]
    fn a_correction_cannot_invent_stock() {
        let mut p = good();
        p.quantity = 400;
        assert_eq!(
            check(&p, Some(&receipt())),
            vec![Problem::Overreaches { proposed: 400, target: 100 }]
        );
    }

    #[test]
    fn a_world_event_reverses_nothing() {
        let mut p = good();
        p.reason_class = Some(RevisionClass::WorldEvent);
        assert_eq!(
            check(&p, Some(&receipt())),
            vec![Problem::WorldEventOnACorrection]
        );
    }

    #[test]
    fn a_record_error_has_to_name_what_it_corrects() {
        let mut p = good();
        p.reverses_movement_id = None;
        assert_eq!(check(&p, None), vec![Problem::ReasonWithoutTarget]);
    }

    #[test]
    fn a_correction_has_to_say_why() {
        let mut p = good();
        p.reason_class = None;
        assert_eq!(check(&p, Some(&receipt())), vec![Problem::MissingReason]);
    }

    #[test]
    fn an_ordinary_movement_is_not_checked_against_anything() {
        let mut p = good();
        p.reverses_movement_id = None;
        p.reason_class = Some(RevisionClass::WorldEvent);
        assert_eq!(check(&p, None), vec![]);
    }

    #[test]
    fn one_submission_reports_every_problem() {
        // Fixing one and resubmitting to find the next is how people learn to
        // stop correcting, so the caller gets the whole list at once.
        let mut p = good();
        p.occurred_at = at(11, 30);
        p.quantity = 400;
        p.item_id = u(0xBEEF);
        let problems = check(&p, Some(&receipt()));
        assert_eq!(problems.len(), 3, "{problems:?}");
    }

    #[test]
    fn a_correction_cannot_reach_another_tenant() {
        let mut p = good();
        p.tenant_id = u(0x2222);
        assert!(check(&p, Some(&receipt())).contains(&Problem::DifferentTenant));
    }
}
