//! Recording a pick: the DTOs, the transaction, and `POST /picks`.
//!
//! **The first write path to move out of `routes.rs` (D160).** The judgement is
//! next door in `picking`, pure and tested without a database; this is the
//! persistence that acts on it. `routes.rs` keeps `configure`, which is the
//! registry, and nothing else about a pick.
//!
//! `caller` still comes from `crate::routes`, which is the one thread back into
//! the file this is leaving. It belongs in `auth` with the other fifty-four
//! handlers that use it, and moving it is a fifty-five-site change that has no
//! business in the commit that proves this pattern.

use actix_web::{post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_events::{self, ActInsert, NewClientEvent};
use crate::error::ApiError;
use crate::ledger_views::{self, ProjectionProgress, Progress};
use crate::picking::{self, ProposedPick};
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

#[derive(Deserialize, Debug)]
pub struct RecordPickRequest {
    pub fulfilment_line_id: Uuid,
    /// Stock cell to take from (carries location/lot/status/owner).
    pub from_stock_id: Uuid,
    /// Where it landed. **Exactly one of these**, which `picking::landing`
    /// checks once so nothing downstream has to (D166): a carton, a pallet or a
    /// tote is a package; the packing station is a location.
    pub to_package_id: Option<Uuid>,
    pub to_location_id: Option<Uuid>,
    pub quantity: i64,
    /// Client-minted event id (D5 offline).
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct RecordPickResponse {
    pub movement_id: Uuid,
    /// Soft problems that did not block the write (over-available, etc.).
    pub warnings: Vec<String>,
    /// Live progress for the line just served (ledger, not cache).
    pub ledger: Progress,
    /// Cached progress as stored (may lag until maintainers run).
    pub projection: ProjectionProgress,
}

/// Record a pick as one `stock_movement` row.
///
/// Does not UPDATE progress columns and does not call the projection maintainers
/// (those are the scheduler's). The ledger is the truth immediately; the folded
/// quantities catch up on the next rebuild.
#[post("/picks")]
pub async fn record_pick(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RecordPickRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    // Read before anything is loaded: a request naming both or neither is
    // malformed, and saying so costs no round trip.
    let landing = picking::landing(body.to_package_id, body.to_location_id)
        .map_err(|p| ApiError::Rejected(p.to_string()))?;
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let line_row = tx
                    .query_opt(
                        "SELECT fl.id, fl.tenant_id, ol.item_id, fl.quantity,
                                fl.picked_quantity, fl.despatched_quantity,
                                (f.state = 'cancelled')
                           FROM fulfilment_line fl
                           JOIN fulfilment f ON f.id = fl.fulfilment_id
                           JOIN order_line ol ON ol.id = fl.order_line_id
                          WHERE fl.id = $1",
                        &[&body.fulfilment_line_id],
                    )
                    .await?;
                let cell_row = tx
                    .query_opt(
                        "SELECT id, tenant_id, item_id, holder_location_id, holder_package_id,
                                lot_id, status_id, owner_id, quantity, available_quantity
                           FROM stock WHERE id = $1",
                        &[&body.from_stock_id],
                    )
                    .await?;
                let dest = match landing {
                    picking::Landing::Package(id) => tx
                        .query_opt("SELECT id, tenant_id FROM package WHERE id = $1", &[&id])
                        .await?
                        .map(|r| {
                            picking::Destination::Package(picking::Package {
                                id: r.get(0),
                                tenant_id: r.get(1),
                            })
                        }),
                    picking::Landing::Location(id) => tx
                        .query_opt(
                            "SELECT id, tenant_id, kind FROM location WHERE id = $1",
                            &[&id],
                        )
                        .await?
                        .map(|r| {
                            picking::Destination::Location(picking::Location {
                                id: r.get(0),
                                tenant_id: r.get(1),
                                kind: r.get(2),
                            })
                        }),
                };

                let line = line_row.as_ref().map(|r| picking::FulfilmentLine {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    item_id: r.get(2),
                    quantity: r.get(3),
                    picked_quantity: r.get(4),
                    despatched_quantity: r.get(5),
                    fulfilment_cancelled: r.get(6),
                });
                let cell = cell_row.as_ref().map(|r| picking::StockCell {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    item_id: r.get(2),
                    holder_location_id: r.get(3),
                    holder_package_id: r.get(4),
                    lot_id: r.get(5),
                    status_id: r.get(6),
                    owner_id: r.get(7),
                    quantity: r.get(8),
                    available_quantity: r.get(9),
                });
                let proposed = ProposedPick {
                    tenant_id: tenant,
                    quantity: body.quantity,
                    fulfilment_line_id: body.fulfilment_line_id,
                    from_stock_id: body.from_stock_id,
                    to: landing,
                };
                let problems =
                    picking::check(&proposed, line.as_ref(), cell.as_ref(), dest.as_ref());
                let hard: Vec<String> = problems
                    .iter()
                    .filter(|p| picking::is_hard(p))
                    .map(|p| p.to_string())
                    .collect();
                if !hard.is_empty() {
                    return Err(ApiError::Rejected(hard.join("; ")));
                }
                let warnings: Vec<String> = problems
                    .iter()
                    .filter(|p| !picking::is_hard(p))
                    .map(|p| p.to_string())
                    .collect();

                let cell = cell.expect("hard checks require a cell");
                let dest = dest.expect("hard checks require a destination");
                let line = line.expect("hard checks require a line");
                let sides = picking::sides(&cell, &dest);

                let mut warnings = warnings;
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
                            client_events::require_one_movement(tx, body.client_event_id)
                                .await?;
                        client_events::reject_quantity_mismatch(qty, body.quantity)?;
                        warnings.push(client_events::REPLAY_WARNING.into());
                        id
                    }
                    ActInsert::Fresh => {
                        let id: Uuid = tx
                            .query_one(
                                "INSERT INTO stock_movement (
                                     tenant_id, client_event_id, item_id, quantity,
                                     from_location_id, from_package_id, from_lot_id,
                                     from_status_id, from_owner_id,
                                     to_package_id, to_location_id,
                                     to_lot_id, to_status_id, to_owner_id,
                                     reason, occurred_at, recorded_by_id, fulfilment_line_id)
                                 VALUES (
                                     $1, $2, $3, $4,
                                     $5, $6, $7, $8, $9,
                                     $10, $11, $12, $13, $14,
                                     'pick', $15, $16, $17)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &body.client_event_id,
                                    &sides.item_id,
                                    &body.quantity,
                                    &sides.from_location_id,
                                    &sides.from_package_id,
                                    &sides.from_lot_id,
                                    &sides.from_status_id,
                                    &sides.from_owner_id,
                                    &sides.to_package_id,
                                    &sides.to_location_id,
                                    &sides.to_lot_id,
                                    &sides.to_status_id,
                                    &sides.to_owner_id,
                                    &body.occurred_at,
                                    &who.person_id,
                                    &body.fulfilment_line_id,
                                ],
                            )
                            .await?
                            .get(0);
                        tx.execute(
                            "SELECT projection_mark_dirty($1, 'pick')",
                            &[&tenant],
                        )
                        .await?;
                        id
                    }
                };

                let ledger = ledger_views::line_progress_ledger(
                    tx,
                    body.fulfilment_line_id,
                    line.quantity,
                )
                .await?;
                let proj_row = tx
                    .query_one(
                        "SELECT covered_quantity, picked_quantity, packed_quantity,
                                despatched_quantity, uncovered_quantity
                           FROM fulfilment_line WHERE id = $1",
                        &[&body.fulfilment_line_id],
                    )
                    .await?;
                let projection = ledger_views::line_progress_projection(
                    tx,
                    proj_row.get(0),
                    proj_row.get(1),
                    proj_row.get(2),
                    proj_row.get(3),
                    proj_row.get(4),
                )
                .await?;

                Ok(RecordPickResponse {
                    movement_id,
                    warnings,
                    ledger,
                    projection,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}
