//! A line checked at the pack bench. D244.
//!
//! The packer is shown what should be on the bench for a line and confirms it,
//! or says what they see. A check is a fact (`pack_check`, migration 136); a
//! line's newest check wins. The bench does not offer a seal while a line in
//! the carton is unchecked, and a seal that happens anyway is said (D5).

use actix_web::{post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_events::{self, NewClientEvent};
use crate::error::ApiError;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

#[derive(Deserialize, Debug)]
pub struct CheckLineRequest {
    pub fulfilment_line_id: Uuid,
    /// What the bench showed should be there, in the units the line counts.
    pub expected_quantity: i64,
    /// What the packer saw. The same figure is a confirmation.
    pub seen_quantity: i64,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// A line's newest check, as the bench shows it.
#[derive(Serialize, Debug)]
pub struct LineCheck {
    pub expected: i64,
    pub seen: i64,
    /// Who checked it, by name.
    pub by: Option<String>,
    pub at: DateTime<Utc>,
}

/// Check a line: what should be there, and what was seen. Sending it again is
/// the same act.
#[post("/pack-checks")]
pub async fn check_line(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<CheckLineRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let b = body.into_inner();
    if b.expected_quantity < 0 || b.seen_quantity < 0 {
        return Err(ApiError::Rejected("a count is nought or more".into()));
    }
    let ev = NewClientEvent {
        tenant_id: who.tenant_id,
        client_event_id: b.client_event_id,
        site_id: who.site_id,
        recorded_by_id: who.person_id,
        submitted_at: b.occurred_at,
    };
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                if tx
                    .query_opt("SELECT 1 FROM fulfilment_line WHERE id = $1", &[&b.fulfilment_line_id])
                    .await?
                    .is_none()
                {
                    return Err(ApiError::NotFound);
                }
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    return Ok(());
                }
                tx.execute(
                    "INSERT INTO pack_check
                         (tenant_id, fulfilment_line_id, expected_quantity, seen_quantity,
                          occurred_at, client_event_id, recorded_by_id)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)",
                    &[
                        &ev.tenant_id,
                        &b.fulfilment_line_id,
                        &b.expected_quantity,
                        &b.seen_quantity,
                        &b.occurred_at,
                        &ev.client_event_id,
                        &ev.recorded_by_id,
                    ],
                )
                .await?;
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// A line's newest check, or none.
pub async fn newest(tx: &tokio_postgres::Transaction<'_>, line: Uuid) -> Result<Option<LineCheck>, ApiError> {
    Ok(tx
        .query_opt(
            "SELECT c.expected_quantity, c.seen_quantity, p.display_name, c.occurred_at
               FROM pack_check c
               LEFT JOIN person p ON p.id = c.recorded_by_id
              WHERE c.fulfilment_line_id = $1
              ORDER BY c.occurred_at DESC, c.recorded_at DESC, c.id DESC
              LIMIT 1",
            &[&line],
        )
        .await?
        .map(|r| LineCheck {
            expected: r.get(0),
            seen: r.get(1),
            by: r.get(2),
            at: r.get(3),
        }))
}

/// The codes of the lines with units in a package that nobody has checked.
pub async fn unchecked_in(tx: &tokio_postgres::Transaction<'_>, package: Uuid) -> Result<Vec<String>, ApiError> {
    Ok(tx
        .query(
            "SELECT i.code
               FROM stock_movement m
               JOIN stock_movement_effective v
                 ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
               JOIN item i ON i.id = m.item_id
              WHERE m.to_package_id = $1
                AND m.fulfilment_line_id IS NOT NULL
                AND NOT EXISTS (SELECT 1 FROM pack_check c
                                 WHERE c.fulfilment_line_id = m.fulfilment_line_id)
              GROUP BY m.fulfilment_line_id, i.code
             HAVING sum(v.effective_quantity) > 0
              ORDER BY i.code",
            &[&package],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect())
}
