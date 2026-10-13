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
                let check: Uuid = tx
                    .query_one(
                        "INSERT INTO pack_check
                             (tenant_id, fulfilment_line_id, expected_quantity, seen_quantity,
                              occurred_at, client_event_id, recorded_by_id)
                         VALUES ($1, $2, $3, $4, $5, $6, $7)
                         RETURNING id",
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
                    .await?
                    .get(0);
                if b.seen_quantity < b.expected_quantity {
                    short(tx, &ev, check, b.fulfilment_line_id, b.expected_quantity, b.seen_quantity).await?;
                }
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// A short at the check is a finding (docs/packing-plan.md, settled 3): the
/// line's shortfall in the findings queue, naming who picked it, there or in
/// NetSuite. One open finding a line: counting it short again adds none.
async fn short(
    tx: &tokio_postgres::Transaction<'_>,
    ev: &NewClientEvent,
    check: Uuid,
    line: Uuid,
    expected: i64,
    seen: i64,
) -> Result<(), ApiError> {
    let open: bool = tx
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM discrepancy d
                              JOIN pack_check c ON c.id = d.pack_check_id
                             WHERE c.fulfilment_line_id = $1
                               AND d.state IN ('open', 'investigating'))",
            &[&line],
        )
        .await?
        .get(0);
    if open {
        return Ok(());
    }
    let about = tx
        .query_one(
            "SELECT ol.item_id,
                    coalesce(f.reference, o.confirmation_number, o.external_ref, 'an order'),
                    (SELECT ep.picked_by FROM external_pick ep WHERE ep.fulfilment_line_id = fl.id
                      ORDER BY ep.observed_at DESC, ep.recorded_at DESC, ep.id DESC LIMIT 1),
                    (SELECT p.display_name FROM stock_movement m
                       JOIN location l ON l.id = m.from_location_id
                       JOIN person p ON p.id = m.recorded_by_id
                      WHERE m.fulfilment_line_id = fl.id
                        AND l.kind IN ('pick_face', 'bulk', 'overflow')
                      ORDER BY m.occurred_at DESC LIMIT 1)
               FROM fulfilment_line fl
               JOIN fulfilment f ON f.id = fl.fulfilment_id
               JOIN order_line ol ON ol.id = fl.order_line_id
               JOIN \"order\" o ON o.id = f.order_id
              WHERE fl.id = $1",
            &[&line],
        )
        .await?;
    let (item, reference): (Uuid, String) = (about.get(0), about.get(1));
    let picked = match (about.get::<_, Option<String>>(2), about.get::<_, Option<String>>(3)) {
        (_, Some(here)) => format!("picked here by {here}"),
        (Some(there), None) => format!("picked in NetSuite by {there}"),
        (None, None) => "who picked it is not recorded".to_string(),
    };
    let detail = format!(
        "short at the pack bench: {seen} seen of {expected} on {reference}, {picked}"
    );
    // Whole numbers as literals: the quantities are numeric and this crate
    // has no NUMERIC ToSql, as the count's finding does.
    tx.execute(
        &format!(
            "INSERT INTO discrepancy (
                 tenant_id, kind, item_id, expected_quantity, observed_quantity,
                 pack_check_id, detail, detected_at, detected_by_id, state)
             VALUES ($1, 'short_pick', $2, {expected}::numeric, {seen}::numeric,
                     $3, $4, now(), $5, 'open')"
        ),
        &[&ev.tenant_id, &item, &check, &detail, &ev.recorded_by_id],
    )
    .await?;
    Ok(())
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
               FROM package_line_content c
               JOIN item i ON i.id = c.item_id
              WHERE c.package_id = $1
                AND NOT EXISTS (SELECT 1 FROM pack_check k
                                 WHERE k.fulfilment_line_id = c.fulfilment_line_id)
              GROUP BY c.fulfilment_line_id, i.code
             HAVING sum(c.quantity) > 0
              ORDER BY i.code",
            &[&package],
        )
        .await?
        .iter()
        .map(|r| r.get(0))
        .collect())
}
