//! A measurement put right, a figure at a time (D236).
//!
//! A length typed as 20 that was 20.5 is a figure that was never true, not a
//! thing that has changed since. So it is not measured again: it is
//! **corrected**, by an observation naming it (`corrects_observation_id`), as
//! the observation model always had it. The figure on file stops counting,
//! retroactively; its correction takes its place at the moment it was taken,
//! by the same method and in the same arrangement, so a newer measurement
//! still stands in front of it. Who put it right, and when, is the act's.
//!
//! Only what differs is put right: the rest of the card is left as it was
//! measured. A figure nobody recorded is measured, not corrected; and only
//! the item's own figures are, never its family's shown on its card.

use std::collections::HashMap;

use actix_web::{post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_events::{self, NewClientEvent};
use crate::error::ApiError;
use crate::routes::{caller, entered_quantity, Measurement};
use crate::tenancy::TenantScope;
use crate::AppState;

#[derive(Deserialize, Debug)]
pub struct CorrectRequest {
    /// The card: `each`, `inner` or `carton`.
    pub level: String,
    /// Its figures as they should have been, as typed with their units. Those
    /// equal to what is on file are left alone.
    pub measurements: Vec<Measurement>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct Corrected {
    /// Figures put right.
    pub figures: i64,
    /// The act was recorded before: nothing was done again.
    pub replay: bool,
}

/// The figure a card shows for a metric, as the projection has it, followed
/// to its newest correction where it has been put right since: its id, event,
/// subject, when it was taken and its value.
async fn shown(
    tx: &tokio_postgres::Transaction<'_>,
    item: Uuid,
    level: &str,
    metric: &str,
) -> Result<Option<(Uuid, Uuid, Uuid, DateTime<Utc>, i64)>, ApiError> {
    let row = tx
        .query_opt(
            "SELECT o.id FROM observation_current oc
               JOIN observable s ON s.id = oc.observable_id
               JOIN observation o ON o.id = oc.observation_id
               JOIN metric m ON m.id = oc.metric_id
              WHERE s.item_id = $1 AND s.packaging_level = $2::text::packaging_level
                AND m.code = $3 AND o.value_numeric IS NOT NULL
              ORDER BY o.observed_at DESC, o.id DESC
              LIMIT 1",
            &[&item, &level, &metric],
        )
        .await?;
    let Some(row) = row else { return Ok(None) };
    let mut id: Uuid = row.get(0);
    // The projection is rebuilt behind the act, so a figure put right a
    // moment ago may still be the one it shows.
    while let Some(next) = tx
        .query_opt(
            "SELECT id FROM observation WHERE corrects_observation_id = $1 ORDER BY id DESC LIMIT 1",
            &[&id],
        )
        .await?
    {
        id = next.get(0);
    }
    let o = tx
        .query_one(
            "SELECT id, observation_event_id, observable_id, observed_at, value_numeric FROM observation WHERE id = $1",
            &[&id],
        )
        .await?;
    Ok(Some((o.get(0), o.get(1), o.get(2), o.get(3), o.get(4))))
}

/// Put right figures on one of an item's own cards.
#[post("/items/{id}/corrections")]
pub async fn correct(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<CorrectRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item = path.into_inner();
    let body = body.into_inner();
    if !matches!(body.level.as_str(), "each" | "inner" | "carton") {
        return Err(ApiError::Rejected("a card is the item's each, its pack or its carton".into()));
    }
    if body.measurements.is_empty() {
        return Err(ApiError::Rejected("say which figures to put right".into()));
    }
    let ev = NewClientEvent {
        tenant_id: who.tenant_id,
        client_event_id: body.client_event_id,
        site_id: who.site_id,
        recorded_by_id: who.person_id,
        submitted_at: body.occurred_at,
    };
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                tx.query_opt("SELECT 1 FROM item WHERE id = $1", &[&item]).await?.ok_or(ApiError::NotFound)?;
                // What each figure is now and should be, read and checked
                // before the act is claimed: a figure nobody recorded refuses
                // the whole of it.
                let mut wrong = vec![];
                for m in &body.measurements {
                    let (Some(entered), Some(unit)) = (m.entered_value.as_deref(), m.unit.as_deref()) else {
                        return Err(ApiError::Rejected(format!("{} needs a value with its unit", m.metric)));
                    };
                    let Some((id, event, observable, at, was)) = shown(tx, item, &body.level, &m.metric).await? else {
                        return Err(ApiError::Rejected(format!(
                            "nothing recorded of its {} to put right: measure it",
                            m.metric.replace('_', " ")
                        )));
                    };
                    let (value, unit_id, dimension) = entered_quantity(tx, entered, unit).await?;
                    let measures: Option<Uuid> =
                        tx.query_one("SELECT dimension_id FROM observation WHERE id = $1", &[&id]).await?.get(0);
                    if measures != Some(dimension) {
                        return Err(ApiError::Rejected(format!("{unit} doesn't measure its {}", m.metric.replace('_', " "))));
                    }
                    if value != was {
                        wrong.push((id, event, observable, at, value, entered.to_string(), unit_id));
                    }
                }
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    return Ok(Corrected { figures: 0, replay: true });
                }
                // One look put right is one event: taken when it was, by the
                // method and in the arrangement it was, said by this person now.
                let mut events: HashMap<Uuid, Uuid> = HashMap::new();
                for (id, event, observable, at, value, entered, unit_id) in &wrong {
                    let into = match events.get(event) {
                        Some(e) => *e,
                        None => {
                            let e: Uuid = tx
                                .query_one(
                                    "INSERT INTO observation_event
                                         (tenant_id, client_event_id, observable_id, observed_at, recorded_by_id,
                                          method, ingestion_channel, presentation_id)
                                     SELECT $1, $2, $3, $4, $5, method, 'keyed', presentation_id
                                       FROM observation_event WHERE id = $6
                                     RETURNING id",
                                    &[&ev.tenant_id, &ev.client_event_id, observable, at, &ev.recorded_by_id, event],
                                )
                                .await?
                                .get(0);
                            events.insert(*event, e);
                            e
                        }
                    };
                    tx.execute(
                        "INSERT INTO observation
                             (tenant_id, observation_event_id, observable_id, observed_at, client_event_id,
                              metric_id, result_kind, dimension_id, value_numeric, entered_value,
                              entered_unit_id, corrects_observation_id)
                         SELECT tenant_id, $2, observable_id, observed_at, $3, metric_id, result_kind,
                                dimension_id, $4, $5::text::numeric, $6, id
                           FROM observation WHERE id = $1",
                        &[id, &into, &ev.client_event_id, value, entered, unit_id],
                    )
                    .await?;
                }
                if !wrong.is_empty() {
                    tx.execute("SELECT projection_mark_dirty($1, 'observation')", &[&ev.tenant_id]).await?;
                }
                Ok(Corrected { figures: wrong.len() as i64, replay: false })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}
