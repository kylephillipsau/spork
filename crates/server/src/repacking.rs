//! A line's units moved between holders at the bench. D246.
//!
//! From a carton to another carton, out of a carton onto the bench, or from the
//! bench into a carton: one `stock_movement` naming the line it serves, from
//! one holder to the other. The bench is the site's pack location. Neither
//! side is storage, so nothing is picked again (D99, D166); what a package
//! holds is what went in less what has gone on to another holder
//! (`package_line_content`, migration 138), which is what packed reads.
//!
//! **What moves is said by the ledger, not the stock projection**: the lot,
//! status and owner of the line's units in the holder they leave, from the
//! movements that put them there, so a move straight after a pick needs no
//! fold to have run.
//!
//! Taking units out of a carton puts them on the bench (docs/packing-plan.md,
//! settled 2): out of the carton, still on their line, to be packed again or
//! moved back to their bin. Taking out used to reverse the pick, which after a
//! repack reversed the repack and sent them back to the other carton.
//!
//! The bench offers a move only between open cartons. One that happens anyway,
//! out of or into a sealed one, is still what happened and is recorded with a
//! warning (D5).

use actix_web::{post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_events::{self, ActInsert, NewClientEvent};
use crate::error::ApiError;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

#[derive(Deserialize, Debug)]
pub struct RepackRequest {
    pub fulfilment_line_id: Uuid,
    /// Where the units are now: a carton, or the bench. Exactly one.
    #[serde(default)]
    pub from_package_id: Option<Uuid>,
    #[serde(default)]
    pub from_location_id: Option<Uuid>,
    /// Where they go: a carton, or the bench. Exactly one.
    #[serde(default)]
    pub to_package_id: Option<Uuid>,
    #[serde(default)]
    pub to_location_id: Option<Uuid>,
    /// Which lot, when the holder has more than one of the line's.
    #[serde(default)]
    pub lot_id: Option<Uuid>,
    pub quantity: i64,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct RepackResponse {
    pub movement_id: Uuid,
    pub warnings: Vec<String>,
}

/// A holder at the bench, as the request names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    Package(Uuid),
    Location(Uuid),
}

/// Read the two optional ids of one side into the one holder they mean.
pub fn holder(package: Option<Uuid>, location: Option<Uuid>) -> Option<Holder> {
    match (package, location) {
        (Some(p), None) => Some(Holder::Package(p)),
        (None, Some(l)) => Some(Holder::Location(l)),
        _ => None,
    }
}

/// What the check reads about one side, loaded.
#[derive(Clone, Debug)]
pub struct Side {
    pub holder: Holder,
    /// A package's status from its events (`package_status_now`).
    pub status: Option<String>,
    /// A package's order.
    pub fulfilment_id: Option<Uuid>,
}

/// The line, as the check needs it.
#[derive(Clone, Debug)]
pub struct Line {
    pub fulfilment_id: Uuid,
    pub cancelled: bool,
    /// The bench: the line's site's pack location (migration 97).
    pub pack_location: Option<Uuid>,
}

/// What the holder the units leave has of the line, by lot, status and owner.
#[derive(Clone, Debug, Default)]
pub struct Held {
    /// All of it.
    pub total: i64,
    /// How many kinds of it: two lots are two.
    pub kinds: usize,
    /// The most of any one kind, which is the kind that moves.
    pub most: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RepackProblem {
    NoQuantity,
    LineNotFound,
    FulfilmentCancelled,
    /// The request names both a carton and a location on one side, or neither.
    NotOneFrom,
    NotOneTo,
    FromNotFound,
    ToNotFound,
    /// A location other than the bench: from a shelf is a pick, to one a move.
    NotTheBench,
    SameHolder,
    /// Hard: the carton it goes into left the site or was voided.
    DestinationClosed { status: String },
    /// Hard: the holder has none of the line to move.
    HoldsNone,
    /// Hard: more than the one lot holds, of a line it holds in several.
    SeveralKinds,
    /// Soft: more than the holder has of the line.
    OverHeld { held: i64 },
    /// Soft: out of a sealed carton.
    FromSealed,
    /// Soft: into a sealed carton.
    IntoSealed,
    /// Soft: the carton it goes into is another order's.
    OtherOrder,
}

impl std::fmt::Display for RepackProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepackProblem::NoQuantity => write!(f, "move one or more"),
            RepackProblem::LineNotFound => write!(f, "the order line was not found"),
            RepackProblem::FulfilmentCancelled => write!(f, "the order is cancelled"),
            RepackProblem::NotOneFrom => {
                write!(f, "name exactly one of from_package_id and from_location_id")
            }
            RepackProblem::NotOneTo => write!(f, "name exactly one of to_package_id and to_location_id"),
            RepackProblem::FromNotFound => write!(f, "where it comes from was not found"),
            RepackProblem::ToNotFound => write!(f, "where it goes was not found"),
            RepackProblem::NotTheBench => write!(
                f,
                "a carton or the bench only: from a shelf is a pick, and to one a move"
            ),
            RepackProblem::SameHolder => write!(f, "it is there already"),
            RepackProblem::DestinationClosed { status } => {
                write!(f, "the carton it goes into is {status}")
            }
            RepackProblem::HoldsNone => write!(f, "there is none of that line there"),
            RepackProblem::SeveralKinds => {
                write!(f, "more than one lot of it is there: move each by its lot")
            }
            RepackProblem::OverHeld { held } => write!(f, "more than the {held} there"),
            RepackProblem::FromSealed => write!(f, "taken out of a sealed carton: open it first"),
            RepackProblem::IntoSealed => write!(f, "put into a sealed carton: open it first"),
            RepackProblem::OtherOrder => write!(f, "the carton it goes into is another order's"),
        }
    }
}

pub fn is_hard(p: &RepackProblem) -> bool {
    !matches!(
        p,
        RepackProblem::OverHeld { .. }
            | RepackProblem::FromSealed
            | RepackProblem::IntoSealed
            | RepackProblem::OtherOrder
    )
}

/// What is wrong with moving `quantity` of the line from one side to the other.
/// `from` and `to` are `None` when the request did not name one holder for the
/// side, or what it named was not found; `named_*` says which.
pub fn check(
    quantity: i64,
    line: Option<&Line>,
    (named_from, from): (bool, Option<&Side>),
    (named_to, to): (bool, Option<&Side>),
    held: &Held,
) -> Vec<RepackProblem> {
    let mut problems = vec![];
    if quantity <= 0 {
        problems.push(RepackProblem::NoQuantity);
    }
    let Some(line) = line else {
        problems.push(RepackProblem::LineNotFound);
        return problems;
    };
    if line.cancelled {
        problems.push(RepackProblem::FulfilmentCancelled);
    }
    let from = match (named_from, from) {
        (false, _) => {
            problems.push(RepackProblem::NotOneFrom);
            None
        }
        (true, None) => {
            problems.push(RepackProblem::FromNotFound);
            None
        }
        (true, Some(s)) => Some(s),
    };
    let to = match (named_to, to) {
        (false, _) => {
            problems.push(RepackProblem::NotOneTo);
            None
        }
        (true, None) => {
            problems.push(RepackProblem::ToNotFound);
            None
        }
        (true, Some(s)) => Some(s),
    };
    let (Some(from), Some(to)) = (from, to) else {
        return problems;
    };
    let off_the_bench =
        |s: &Side| matches!(s.holder, Holder::Location(l) if Some(l) != line.pack_location);
    if off_the_bench(from) || off_the_bench(to) {
        problems.push(RepackProblem::NotTheBench);
    }
    if from.holder == to.holder {
        problems.push(RepackProblem::SameHolder);
    }
    match to.status.as_deref() {
        Some(s @ ("despatched" | "voided")) => {
            problems.push(RepackProblem::DestinationClosed { status: s.into() })
        }
        Some("sealed") => problems.push(RepackProblem::IntoSealed),
        _ => {}
    }
    if from.status.as_deref() == Some("sealed") {
        problems.push(RepackProblem::FromSealed);
    }
    if to.fulfilment_id.is_some_and(|o| o != line.fulfilment_id) {
        problems.push(RepackProblem::OtherOrder);
    }
    if held.total <= 0 {
        problems.push(RepackProblem::HoldsNone);
    } else if held.kinds > 1 && quantity > held.most {
        problems.push(RepackProblem::SeveralKinds);
    } else if quantity > held.total {
        problems.push(RepackProblem::OverHeld { held: held.total });
    }
    problems
}

/// Move a line's units between holders at the bench.
#[post("/repacks")]
pub async fn repack(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RepackRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let b = body.into_inner();
    let from_holder = holder(b.from_package_id, b.from_location_id);
    let to_holder = holder(b.to_package_id, b.to_location_id);
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let line = tx
                    .query_opt(
                        "SELECT f.id, f.state = 'cancelled', s.pack_location_id
                           FROM fulfilment_line fl
                           JOIN fulfilment f ON f.id = fl.fulfilment_id
                           LEFT JOIN site s ON s.id = f.site_id
                          WHERE fl.id = $1",
                        &[&b.fulfilment_line_id],
                    )
                    .await?
                    .map(|r| Line { fulfilment_id: r.get(0), cancelled: r.get(1), pack_location: r.get(2) });
                let side = |h: Holder| async move {
                    Ok::<_, ApiError>(match h {
                        Holder::Package(id) => tx
                            .query_opt(
                                "SELECT package_status_now(id), fulfilment_id FROM package WHERE id = $1",
                                &[&id],
                            )
                            .await?
                            .map(|r| Side { holder: h, status: r.get(0), fulfilment_id: r.get(1) }),
                        Holder::Location(id) => tx
                            .query_opt("SELECT 1 FROM location WHERE id = $1", &[&id])
                            .await?
                            .map(|_| Side { holder: h, status: None, fulfilment_id: None }),
                    })
                };
                let from = match from_holder {
                    Some(h) => side(h).await?,
                    None => None,
                };
                let to = match to_holder {
                    Some(h) => side(h).await?,
                    None => None,
                };

                // What the holder has of the line, by lot, status and owner,
                // from the movements that put it there and took it away.
                let (from_package, from_location) = match from_holder {
                    Some(Holder::Package(p)) => (Some(p), None),
                    Some(Holder::Location(l)) => (None, Some(l)),
                    None => (None, None),
                };
                let kinds = tx
                    .query(
                        "SELECT item_id, lot, status, owner, sum(q)::bigint
                           FROM (SELECT m.item_id, m.to_lot_id AS lot, m.to_status_id AS status,
                                        m.to_owner_id AS owner, v.effective_quantity AS q
                                   FROM stock_movement m
                                   JOIN stock_movement_effective v
                                     ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
                                  WHERE m.fulfilment_line_id = $1
                                    AND (m.to_package_id = $2 OR m.to_location_id = $3)
                                 UNION ALL
                                 SELECT m.item_id, m.from_lot_id, m.from_status_id,
                                        m.from_owner_id, -v.effective_quantity
                                   FROM stock_movement m
                                   JOIN stock_movement_effective v
                                     ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
                                  WHERE m.fulfilment_line_id = $1
                                    AND (m.from_package_id = $2 OR m.from_location_id = $3)) x
                          WHERE $4::uuid IS NULL OR lot = $4
                          GROUP BY item_id, lot, status, owner
                         HAVING sum(q) > 0
                          ORDER BY 5 DESC",
                        &[&b.fulfilment_line_id, &from_package, &from_location, &b.lot_id],
                    )
                    .await?;
                let held = Held {
                    total: kinds.iter().map(|r| r.get::<_, i64>(4)).sum(),
                    kinds: kinds.len(),
                    most: kinds.first().map(|r| r.get(4)).unwrap_or(0),
                };

                let problems = check(
                    b.quantity,
                    line.as_ref(),
                    (from_holder.is_some(), from.as_ref()),
                    (to_holder.is_some(), to.as_ref()),
                    &held,
                );
                let hard: Vec<String> = problems.iter().filter(|p| is_hard(p)).map(|p| p.to_string()).collect();
                if !hard.is_empty() {
                    return Err(ApiError::Rejected(hard.join("; ")));
                }
                let mut warnings: Vec<String> =
                    problems.iter().filter(|p| !is_hard(p)).map(|p| p.to_string()).collect();

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: b.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: b.occurred_at,
                    },
                )
                .await?;
                let movement_id = match act {
                    ActInsert::Replay => {
                        let (id, qty) = client_events::require_one_movement(tx, b.client_event_id).await?;
                        client_events::reject_quantity_mismatch(qty, b.quantity)?;
                        warnings.push(client_events::REPLAY_WARNING.into());
                        id
                    }
                    ActInsert::Fresh => {
                        let kind = kinds.first().expect("hard checks require something held");
                        let (item, lot, status, owner): (Uuid, Option<Uuid>, Option<Uuid>, Option<Uuid>) =
                            (kind.get(0), kind.get(1), kind.get(2), kind.get(3));
                        // The goods keep their lot, status and owner: only what
                        // holds them changes.
                        let id: Uuid = tx
                            .query_one(
                                "INSERT INTO stock_movement (
                                     tenant_id, client_event_id, item_id, quantity,
                                     from_package_id, from_location_id,
                                     from_lot_id, from_status_id, from_owner_id,
                                     to_package_id, to_location_id,
                                     to_lot_id, to_status_id, to_owner_id,
                                     reason, occurred_at, recorded_by_id, fulfilment_line_id)
                                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $7, $8, $9,
                                         'move', $12, $13, $14)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &b.client_event_id,
                                    &item,
                                    &b.quantity,
                                    &from_package,
                                    &from_location,
                                    &lot,
                                    &status,
                                    &owner,
                                    &b.to_package_id,
                                    &b.to_location_id,
                                    &b.occurred_at,
                                    &who.person_id,
                                    &b.fulfilment_line_id,
                                ],
                            )
                            .await?
                            .get(0);
                        tx.execute("SELECT projection_mark_dirty($1, 'repack')", &[&tenant]).await?;
                        id
                    }
                };
                Ok(RepackResponse { movement_id, warnings })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }
    /// The bench is location 99.
    fn line() -> Line {
        Line { fulfilment_id: u(1), cancelled: false, pack_location: Some(u(99)) }
    }
    fn carton(id: u128, status: Option<&str>) -> Side {
        Side { holder: Holder::Package(u(id)), status: status.map(Into::into), fulfilment_id: Some(u(1)) }
    }
    fn place(id: u128) -> Side {
        Side { holder: Holder::Location(u(id)), status: None, fulfilment_id: None }
    }
    fn six() -> Held {
        Held { total: 6, kinds: 1, most: 6 }
    }
    fn named(s: &Side) -> (bool, Option<&Side>) {
        (true, Some(s))
    }

    #[test]
    fn between_two_open_cartons_or_onto_the_bench_passes() {
        let p = check(4, Some(&line()), named(&carton(10, None)), named(&carton(11, None)), &six());
        assert!(p.is_empty(), "{p:?}");
        let p = check(4, Some(&line()), named(&carton(10, None)), named(&place(99)), &six());
        assert!(p.is_empty(), "taken out onto the bench: {p:?}");
        let p = check(4, Some(&line()), named(&place(99)), named(&carton(11, None)), &six());
        assert!(p.is_empty(), "from the bench into a carton: {p:?}");
    }

    #[test]
    fn a_sealed_carton_either_side_is_said_not_refused() {
        let p = check(4, Some(&line()), named(&carton(10, Some("sealed"))), named(&carton(11, Some("sealed"))), &six());
        assert_eq!(p, vec![RepackProblem::IntoSealed, RepackProblem::FromSealed]);
        assert!(!p.iter().any(is_hard));
    }

    #[test]
    fn a_shelf_the_same_holder_nothing_held_or_two_lots_is_refused() {
        let hard = |p: Vec<RepackProblem>, want: RepackProblem| {
            assert!(p.contains(&want), "{p:?}");
            assert!(p.iter().any(is_hard));
        };
        hard(check(4, Some(&line()), named(&place(7)), named(&carton(11, None)), &six()), RepackProblem::NotTheBench);
        hard(check(4, Some(&line()), named(&carton(10, None)), named(&carton(10, None)), &six()), RepackProblem::SameHolder);
        hard(check(4, Some(&line()), named(&carton(10, None)), named(&carton(11, None)), &Held::default()), RepackProblem::HoldsNone);
        let two_lots = Held { total: 6, kinds: 2, most: 4 };
        hard(check(5, Some(&line()), named(&carton(10, None)), named(&carton(11, None)), &two_lots), RepackProblem::SeveralKinds);
        hard(
            check(4, Some(&line()), named(&carton(10, None)), named(&carton(11, Some("despatched"))), &six()),
            RepackProblem::DestinationClosed { status: "despatched".into() },
        );
    }
}
