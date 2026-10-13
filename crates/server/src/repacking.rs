//! Units moved from one carton to another. D246.
//!
//! One `stock_movement` from a package-held cell to another package, naming the
//! line it serves: the shape D99 already counts as not picked again. What a
//! package holds is then what went in less what has gone on to another holder
//! (`package_line_content`, migration 138), which is what packed reads.
//!
//! The bench offers a repack only between open cartons. One that happens
//! anyway, out of or into a sealed one, is still what happened and is
//! recorded with a warning (D5).

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
    /// The cell the units come from: held in a package.
    pub from_stock_id: Uuid,
    pub to_package_id: Uuid,
    pub quantity: i64,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct RepackResponse {
    pub movement_id: Uuid,
    pub warnings: Vec<String>,
}

/// What the check reads about the cell the units come from.
#[derive(Clone, Debug)]
pub struct Cell {
    pub holder_package_id: Option<Uuid>,
    pub quantity: i64,
}

/// What the check reads about a package, its status from its events.
#[derive(Clone, Debug)]
pub struct Package {
    pub id: Uuid,
    pub status: Option<String>,
    pub fulfilment_id: Option<Uuid>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RepackProblem {
    NoQuantity,
    LineNotFound,
    FulfilmentCancelled,
    CellNotFound,
    /// The cell is on a shelf, not in a package: that is a pick.
    NotInAPackage,
    DestinationNotFound,
    SamePackage,
    /// Hard: the destination left the site or was voided.
    DestinationClosed { status: String },
    /// Soft: more than the cell holds.
    OverHeld { held: i64 },
    /// Soft: taken out of a sealed carton.
    FromSealed,
    /// Soft: put into a sealed carton.
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
            RepackProblem::CellNotFound => write!(f, "the stock it comes from was not found"),
            RepackProblem::NotInAPackage => {
                write!(f, "that stock is on a shelf, not in a carton: record a pick")
            }
            RepackProblem::DestinationNotFound => write!(f, "the carton it goes into was not found"),
            RepackProblem::SamePackage => write!(f, "it is in that carton already"),
            RepackProblem::DestinationClosed { status } => {
                write!(f, "the carton it goes into is {status}")
            }
            RepackProblem::OverHeld { held } => write!(f, "more than the {held} there"),
            RepackProblem::FromSealed => {
                write!(f, "taken out of a sealed carton: open it first")
            }
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

/// The line, as the check needs it: whether it is there, and its order.
#[derive(Clone, Debug)]
pub struct Line {
    pub fulfilment_id: Uuid,
    pub cancelled: bool,
}

/// What is wrong with moving `quantity` from the cell to the package.
pub fn check(
    quantity: i64,
    line: Option<&Line>,
    cell: Option<&Cell>,
    from: Option<&Package>,
    to: Option<&Package>,
) -> Vec<RepackProblem> {
    let mut problems = vec![];
    if quantity <= 0 {
        problems.push(RepackProblem::NoQuantity);
    }
    match line {
        None => problems.push(RepackProblem::LineNotFound),
        Some(l) if l.cancelled => problems.push(RepackProblem::FulfilmentCancelled),
        Some(_) => {}
    }
    match cell {
        None => problems.push(RepackProblem::CellNotFound),
        Some(c) if c.holder_package_id.is_none() => problems.push(RepackProblem::NotInAPackage),
        Some(c) if quantity > c.quantity => problems.push(RepackProblem::OverHeld { held: c.quantity }),
        Some(_) => {}
    }
    let Some(to) = to else {
        problems.push(RepackProblem::DestinationNotFound);
        return problems;
    };
    if from.is_some_and(|f| f.id == to.id) {
        problems.push(RepackProblem::SamePackage);
    }
    match to.status.as_deref() {
        Some(s @ ("despatched" | "voided")) => {
            problems.push(RepackProblem::DestinationClosed { status: s.into() })
        }
        Some("sealed") => problems.push(RepackProblem::IntoSealed),
        _ => {}
    }
    if from.is_some_and(|f| f.status.as_deref() == Some("sealed")) {
        problems.push(RepackProblem::FromSealed);
    }
    if let (Some(l), Some(order)) = (line, to.fulfilment_id) {
        if order != l.fulfilment_id {
            problems.push(RepackProblem::OtherOrder);
        }
    }
    problems
}

/// Move units of a line from one carton to another.
#[post("/repacks")]
pub async fn repack(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RepackRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let b = body.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let line = tx
                    .query_opt(
                        "SELECT f.id, f.state = 'cancelled'
                           FROM fulfilment_line fl JOIN fulfilment f ON f.id = fl.fulfilment_id
                          WHERE fl.id = $1",
                        &[&b.fulfilment_line_id],
                    )
                    .await?
                    .map(|r| Line { fulfilment_id: r.get(0), cancelled: r.get(1) });
                let cell_row = tx
                    .query_opt(
                        "SELECT holder_package_id, quantity, item_id, lot_id, status_id, owner_id
                           FROM stock WHERE id = $1",
                        &[&b.from_stock_id],
                    )
                    .await?;
                let cell = cell_row.as_ref().map(|r| Cell { holder_package_id: r.get(0), quantity: r.get(1) });
                let package = |id: Uuid| async move {
                    Ok::<_, ApiError>(
                        tx.query_opt(
                            "SELECT id, package_status_now(id), fulfilment_id FROM package WHERE id = $1",
                            &[&id],
                        )
                        .await?
                        .map(|r| Package { id: r.get(0), status: r.get(1), fulfilment_id: r.get(2) }),
                    )
                };
                let from = match cell.as_ref().and_then(|c| c.holder_package_id) {
                    Some(id) => package(id).await?,
                    None => None,
                };
                let to = package(b.to_package_id).await?;

                let problems = check(b.quantity, line.as_ref(), cell.as_ref(), from.as_ref(), to.as_ref());
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
                        let r = cell_row.expect("hard checks require a cell");
                        let (from_package, item, lot, status, owner): (Option<Uuid>, Uuid, Option<Uuid>, Option<Uuid>, Option<Uuid>) =
                            (r.get(0), r.get(2), r.get(3), r.get(4), r.get(5));
                        // The goods keep their lot, status and owner: only the
                        // carton holding them changes.
                        let id: Uuid = tx
                            .query_one(
                                "INSERT INTO stock_movement (
                                     tenant_id, client_event_id, item_id, quantity,
                                     from_package_id, from_lot_id, from_status_id, from_owner_id,
                                     to_package_id, to_lot_id, to_status_id, to_owner_id,
                                     reason, occurred_at, recorded_by_id, fulfilment_line_id)
                                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $6, $7, $8,
                                         'move', $10, $11, $12)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &b.client_event_id,
                                    &item,
                                    &b.quantity,
                                    &from_package,
                                    &lot,
                                    &status,
                                    &owner,
                                    &b.to_package_id,
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
    fn line() -> Line {
        Line { fulfilment_id: u(1), cancelled: false }
    }
    fn carton(id: u128, status: Option<&str>) -> Package {
        Package { id: u(id), status: status.map(Into::into), fulfilment_id: Some(u(1)) }
    }
    fn cell() -> Cell {
        Cell { holder_package_id: Some(u(10)), quantity: 6 }
    }

    #[test]
    fn between_two_open_cartons_of_the_order_passes() {
        let p = check(4, Some(&line()), Some(&cell()), Some(&carton(10, None)), Some(&carton(11, None)));
        assert!(p.is_empty(), "{p:?}");
    }

    #[test]
    fn a_sealed_carton_either_side_is_said_not_refused() {
        let p = check(4, Some(&line()), Some(&cell()), Some(&carton(10, Some("sealed"))), Some(&carton(11, Some("sealed"))));
        assert_eq!(p, vec![RepackProblem::IntoSealed, RepackProblem::FromSealed]);
        assert!(!p.iter().any(is_hard));
    }

    #[test]
    fn a_shelf_the_same_carton_or_a_despatched_one_is_refused() {
        let shelf = Cell { holder_package_id: None, quantity: 6 };
        let p = check(4, Some(&line()), Some(&shelf), None, Some(&carton(11, None)));
        assert!(p.contains(&RepackProblem::NotInAPackage));
        let p = check(4, Some(&line()), Some(&cell()), Some(&carton(10, None)), Some(&carton(10, None)));
        assert!(p.contains(&RepackProblem::SamePackage));
        let p = check(4, Some(&line()), Some(&cell()), Some(&carton(10, None)), Some(&carton(11, Some("despatched"))));
        assert!(p.iter().any(is_hard), "{p:?}");
    }
}
