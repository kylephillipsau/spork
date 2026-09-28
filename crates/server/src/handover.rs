//! Handing over goods picked elsewhere: `POST /handovers` (D172).
//!
//! At Melbourne the WMS handheld picks and NetSuite says so; migration 94
//! records that as `external_pick`, a report, never a movement. This is the
//! moment the goods reach this system's ledger: the packer takes them and puts
//! them down, on the staging spot or straight into a carton. One
//! `stock_movement` with no from side (an arrival, like a receipt) that names
//! the line it serves and the report it is the goods of.
//!
//! It is not a pick: `picked_quantity` counts movements out of storage (D166)
//! and an arrival leaves none, which is right, because the picking happened
//! elsewhere and `external_picked_quantity` already says so. Packed and
//! despatched fold by shape and need nothing new.
//!
//! D160's split: [`check`] judges from facts already read, with no database;
//! [`record_handover`] reads them and persists.

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
pub struct HandoverRequest {
    pub fulfilment_line_id: Uuid,
    pub quantity: i64,
    /// Where the goods were put down. **Exactly one**: a carton, or a location
    /// at the fulfilment's site (the staging spot).
    pub to_package_id: Option<Uuid>,
    pub to_location_id: Option<Uuid>,
    /// Client-minted (D5): a retry of the same handover is the same act.
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct HandoverResponse {
    pub movement_id: Uuid,
    /// Soft problems that did not stop the write.
    pub warnings: Vec<String>,
    /// What the other system reports picked on the line, now.
    pub reported: i64,
    /// What has been handed over against it, including this.
    pub handed: i64,
}

/// What [`check`] needs, already read.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// The line, if it exists in the caller's tenant.
    pub line: Option<Line>,
    /// The newest report on the line, if any, and the line's current level.
    pub report: Option<Uuid>,
    pub reported: i64,
    /// Already handed over against the line, net of corrections.
    pub handed: i64,
    /// The destination, if it exists in the caller's tenant.
    pub destination: Option<Destination>,
    /// Who owns goods the fulfilment's site holds.
    pub owner: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub item_id: Uuid,
    pub site_id: Option<Uuid>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Package,
    /// A location, and the site it is at.
    Location { site_id: Option<Uuid> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    NotPositive,
    NoLine,
    Cancelled,
    NotOneDestination,
    NoDestination,
    ElsewhereOnSite,
    NothingReported,
    NoOwner,
    /// Soft: more handed over than the other system reports picked.
    PastTheReport { reported: i64, handed: i64 },
}

impl Problem {
    pub fn is_hard(&self) -> bool {
        !matches!(self, Problem::PastTheReport { .. })
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::NotPositive => write!(f, "quantity must be more than nought"),
            Problem::NoLine => write!(f, "fulfilment line not found"),
            Problem::Cancelled => write!(f, "the fulfilment is cancelled"),
            Problem::NotOneDestination => {
                write!(f, "name exactly one of to_package_id and to_location_id")
            }
            Problem::NoDestination => write!(f, "destination not found"),
            Problem::ElsewhereOnSite => {
                write!(f, "the location is not at the fulfilment's site")
            }
            Problem::NothingReported => {
                write!(f, "nothing is reported picked on this line, so there is nothing to hand over")
            }
            Problem::NoOwner => write!(
                f,
                "this site does not say who owns the goods it holds; set its owner first"
            ),
            Problem::PastTheReport { reported, handed } => write!(
                f,
                "{handed} handed over against {reported} reported picked; the count at the \
                 bench is recorded, and the difference is a finding"
            ),
        }
    }
}

/// Everything wrong with handing over `quantity` given `facts`.
pub fn check(quantity: i64, one_destination: bool, facts: &Facts) -> Vec<Problem> {
    let mut out = vec![];
    if quantity <= 0 {
        out.push(Problem::NotPositive);
    }
    if !one_destination {
        out.push(Problem::NotOneDestination);
    }
    match &facts.line {
        None => out.push(Problem::NoLine),
        Some(line) => {
            if line.cancelled {
                out.push(Problem::Cancelled);
            }
            match facts.destination {
                None if one_destination => out.push(Problem::NoDestination),
                Some(Destination::Location { site_id }) if site_id != line.site_id => {
                    out.push(Problem::ElsewhereOnSite)
                }
                _ => {}
            }
        }
    }
    if facts.report.is_none() {
        out.push(Problem::NothingReported);
    }
    if facts.owner.is_none() && facts.line.is_some() {
        out.push(Problem::NoOwner);
    }
    if facts.report.is_some() && quantity > 0 && facts.handed + quantity > facts.reported {
        out.push(Problem::PastTheReport { reported: facts.reported, handed: facts.handed + quantity });
    }
    out
}

/// What has been handed over against a line, net of corrections (D103).
async fn handed(tx: &tokio_postgres::Transaction<'_>, line: Uuid) -> Result<i64, ApiError> {
    Ok(tx
        .query_one(
            "SELECT coalesce(sum(v.effective_quantity), 0)::bigint
               FROM stock_movement m
               JOIN stock_movement_effective v
                 ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
              WHERE m.fulfilment_line_id = $1 AND m.external_pick_id IS NOT NULL",
            &[&line],
        )
        .await?
        .get(0))
}

/// Record goods picked elsewhere arriving where they were put.
///
/// Does not update progress columns: the ledger is the truth now and the folds
/// catch up on the next rebuild, as for a pick.
#[post("/handovers")]
pub async fn record_handover(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<HandoverRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    let one_destination = body.to_package_id.is_some() != body.to_location_id.is_some();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let mut facts = Facts::default();
                if let Some(r) = tx
                    .query_opt(
                        "SELECT ol.item_id, f.site_id, (f.state = 'cancelled'), s.owner_party_id
                           FROM fulfilment_line fl
                           JOIN fulfilment f ON f.id = fl.fulfilment_id
                           JOIN order_line ol ON ol.id = fl.order_line_id
                           LEFT JOIN site s ON s.id = f.site_id
                          WHERE fl.id = $1",
                        &[&body.fulfilment_line_id],
                    )
                    .await?
                {
                    facts.line = Some(Line { item_id: r.get(0), site_id: r.get(1), cancelled: r.get(2) });
                    facts.owner = r.get(3);
                }

                // The line's level: each external line's newest report, summed.
                // And the newest report of all, which is what the handover names.
                let level = tx
                    .query_one(
                        "SELECT coalesce(sum(quantity), 0)::bigint,
                                (SELECT id FROM external_pick WHERE fulfilment_line_id = $1
                                  ORDER BY observed_at DESC, recorded_at DESC, id DESC LIMIT 1)
                           FROM (SELECT DISTINCT ON (external_line) quantity
                                   FROM external_pick WHERE fulfilment_line_id = $1
                                  ORDER BY external_line, observed_at DESC,
                                           recorded_at DESC, id DESC) newest",
                        &[&body.fulfilment_line_id],
                    )
                    .await?;
                facts.reported = level.get(0);
                facts.report = level.get(1);
                facts.handed = handed(tx, body.fulfilment_line_id).await?;

                facts.destination = if let Some(p) = body.to_package_id {
                    tx.query_opt("SELECT 1 FROM package WHERE id = $1", &[&p])
                        .await?
                        .map(|_| Destination::Package)
                } else if let Some(l) = body.to_location_id {
                    tx.query_opt("SELECT site_id FROM location WHERE id = $1", &[&l])
                        .await?
                        .map(|r| Destination::Location { site_id: r.get(0) })
                } else {
                    None
                };

                let problems = check(body.quantity, one_destination, &facts);
                let hard: Vec<String> =
                    problems.iter().filter(|p| p.is_hard()).map(|p| p.to_string()).collect();
                if !hard.is_empty() {
                    return Err(ApiError::Rejected(hard.join("; ")));
                }
                let mut warnings: Vec<String> =
                    problems.iter().filter(|p| !p.is_hard()).map(|p| p.to_string()).collect();
                let line = facts.line.clone().expect("hard checks require a line");

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;

                let movement_id = match act {
                    ActInsert::Replay => {
                        let (id, qty) =
                            client_events::require_one_movement(tx, body.client_event_id).await?;
                        client_events::reject_quantity_mismatch(qty, body.quantity)?;
                        warnings.push(client_events::REPLAY_WARNING.into());
                        id
                    }
                    ActInsert::Fresh => {
                        let id: Uuid = tx
                            .query_one(
                                "INSERT INTO stock_movement (
                                     tenant_id, client_event_id, item_id, quantity,
                                     to_package_id, to_location_id, to_status_id, to_owner_id,
                                     reason, occurred_at, recorded_by_id,
                                     fulfilment_line_id, external_pick_id)
                                 SELECT $1, $2, $3, $4, $5, $6, s.id, $7,
                                        'handover', $8, $9, $10, $11
                                   FROM inventory_status s
                                  WHERE s.code = 'available' AND s.tenant_id IS NULL
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &body.client_event_id,
                                    &line.item_id,
                                    &body.quantity,
                                    &body.to_package_id,
                                    &body.to_location_id,
                                    &facts.owner,
                                    &body.occurred_at,
                                    &who.person_id,
                                    &body.fulfilment_line_id,
                                    &facts.report,
                                ],
                            )
                            .await?
                            .get(0);
                        tx.execute("SELECT projection_mark_dirty($1, 'handover')", &[&tenant])
                            .await?;
                        id
                    }
                };

                let handed_now = handed(tx, body.fulfilment_line_id).await?;
                Ok(HandoverResponse {
                    movement_id,
                    warnings,
                    reported: facts.reported,
                    handed: handed_now,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> Option<Uuid> {
        Some(Uuid::from_u128(1))
    }

    fn ready() -> Facts {
        Facts {
            line: Some(Line { item_id: Uuid::nil(), site_id: site(), cancelled: false }),
            report: Some(Uuid::from_u128(9)),
            reported: 4,
            handed: 0,
            destination: Some(Destination::Package),
            owner: Some(Uuid::from_u128(7)),
        }
    }

    #[test]
    fn a_handover_within_the_report_is_clean() {
        assert!(check(4, true, &ready()).is_empty());
    }

    #[test]
    fn past_the_report_is_recorded_with_a_warning() {
        let p = check(5, true, &ready());
        assert_eq!(p, vec![Problem::PastTheReport { reported: 4, handed: 5 }]);
        assert!(!p[0].is_hard(), "the count at the bench outranks the report");
    }

    #[test]
    fn nothing_reported_is_nothing_to_hand_over() {
        let mut f = ready();
        f.report = None;
        f.reported = 0;
        assert!(check(1, true, &f).contains(&Problem::NothingReported));
    }

    #[test]
    fn a_site_that_does_not_say_who_owns_is_refused() {
        let mut f = ready();
        f.owner = None;
        assert!(check(1, true, &f).contains(&Problem::NoOwner));
    }

    #[test]
    fn staging_elsewhere_is_refused_and_both_destinations_are_malformed() {
        let mut f = ready();
        f.destination = Some(Destination::Location { site_id: Some(Uuid::from_u128(2)) });
        assert!(check(1, true, &f).contains(&Problem::ElsewhereOnSite));
        f.destination = Some(Destination::Location { site_id: site() });
        assert!(check(1, true, &f).is_empty(), "a location at the fulfilment's site");
        assert!(check(1, false, &f).contains(&Problem::NotOneDestination));
    }

    #[test]
    fn a_cancelled_fulfilment_and_nought_are_refused() {
        let mut f = ready();
        f.line.as_mut().unwrap().cancelled = true;
        let p = check(0, true, &f);
        assert!(p.contains(&Problem::Cancelled) && p.contains(&Problem::NotPositive));
    }
}
