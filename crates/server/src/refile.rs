//! Figures and photos recorded on the wrong card, filed on the right one (D219).
//!
//! A box of ten respirators was weighed, measured and photographed on its
//! item's carton card, because "carton of 10" read like "10/box" before an
//! item said what it is sold as (D218). What was recorded is true of the box;
//! it is filed against the carton. This moves it, the whole card's worth, to
//! another level of the same item.
//!
//! **Nothing is rewritten.** Observations are facts somebody recorded, and the
//! history of where they were filed is part of them:
//!
//! - each event on the wrong card with anything live on it is mirrored on the
//!   right one: same moment, same method, same arrangement, and
//!   `derived_from_event_id` naming the event it came from;
//! - every live figure is copied into its mirrored event, and then retracted
//!   where it was, by a retraction naming it (`retracts_observation_id`), so
//!   no older figure surfaces on the wrong card in its place;
//! - every photo not already moved is filed again in its mirrored event, with
//!   its cut, and the original marked moved to it (`observation_image_move`,
//!   D190), so photos to crop and the item's page both see the new one.
//!
//! The right card is the item's at that level under the case pack in force
//! now (D23): a box is only a definite thing to measure relative to one.
//! **A card that already has its own figures or photos is refused**, rather
//! than two sets mixed on it; move those away first.

use std::collections::HashMap;

use actix_web::{post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_events::{self, NewClientEvent};
use crate::error::ApiError;
use crate::routes::{caller, observable_for, SubjectRef};
use crate::tenancy::TenantScope;
use crate::AppState;

#[derive(Deserialize, Debug)]
pub struct RefileRequest {
    /// The card it was recorded on, and the one it belongs on: `each`,
    /// `inner` or `carton`.
    pub from: String,
    pub to: String,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct Refiled {
    /// Figures copied to the right card (and retracted on the wrong one).
    pub figures: i64,
    /// Photos filed again on the right card.
    pub photos: i64,
    /// The act was recorded before: nothing was done again.
    pub replay: bool,
}

/// A figure is live when nothing retracts or corrects it, and it is not
/// itself a retraction. `o` is the observation.
const LIVE: &str = "o.retracts_observation_id IS NULL
                AND NOT EXISTS (SELECT 1 FROM observation r WHERE r.retracts_observation_id = o.id)
                AND NOT EXISTS (SELECT 1 FROM observation c WHERE c.corrects_observation_id = o.id)";

/// A photo is still where it was taken when it has not been moved. `i` is the image.
const UNMOVED: &str =
    "NOT EXISTS (SELECT 1 FROM observation_image_move m WHERE m.observation_image_id = i.id)";

/// Move what one card of an item holds to another card of it.
#[post("/items/{id}/refile")]
pub async fn refile(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<RefileRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item = path.into_inner();
    let body = body.into_inner();
    let level = |l: &str| matches!(l, "each" | "inner" | "carton");
    if !level(&body.from) || !level(&body.to) {
        return Err(ApiError::Rejected(
            "a card is the item's each, its pack or its carton".into(),
        ));
    }
    if body.from == body.to {
        return Err(ApiError::Rejected("that is the card it is on".into()));
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
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    return Ok(Refiled { figures: 0, photos: 0, replay: true });
                }
                let (from, to) = (body.from.as_str(), body.to.as_str());
                let act = ev.client_event_id;
                let person = ev.recorded_by_id;
                let tenant = ev.tenant_id;

                // What is live on each card, by its level.
                let holds = |level: &'static str| {
                    let sql = format!(
                        "SELECT (SELECT count(*) FROM observable s
                                   JOIN observation o ON o.observable_id = s.id
                                  WHERE s.item_id = $1 AND s.packaging_level = {level}::packaging_level AND {LIVE}),
                                (SELECT count(*) FROM observable s
                                   JOIN observation_event e ON e.observable_id = s.id
                                   JOIN observation_image i ON i.observation_event_id = e.id
                                  WHERE s.item_id = $1 AND s.packaging_level = {level}::packaging_level AND {UNMOVED})"
                    );
                    async move {
                        let r = tx.query_one(&sql, &[&item]).await?;
                        Ok::<(i64, i64), ApiError>((r.get(0), r.get(1)))
                    }
                };
                let (figures, photos) = holds(match from {
                    "each" => "'each'",
                    "inner" => "'inner'",
                    _ => "'carton'",
                })
                .await?;
                if figures == 0 && photos == 0 {
                    return Err(ApiError::Rejected("there is nothing recorded on that card to move".into()));
                }
                let (there, pictured) = holds(match to {
                    "each" => "'each'",
                    "inner" => "'inner'",
                    _ => "'carton'",
                })
                .await?;
                if there > 0 || pictured > 0 {
                    return Err(ApiError::Rejected(
                        "the card it would move to has figures or photos of its own: move those away first".into(),
                    ));
                }

                // The right card, under the case pack in force now.
                let subject = SubjectRef {
                    package_id: None,
                    location_id: None,
                    lot_id: None,
                    item_id: Some(item),
                    item_style_id: None,
                    item_part_id: None,
                    packaging_level: Some(to.to_string()),
                };
                let (target, _) = observable_for(tx, tenant, &subject, body.occurred_at).await?;

                // Each event with anything live on it, mirrored on the right card.
                let mirrored: HashMap<Uuid, Uuid> = tx
                    .query(
                        &format!(
                            "WITH src AS (
                                 SELECT e.id, e.observed_at, e.method, e.presentation_id
                                   FROM observation_event e
                                   JOIN observable s ON s.id = e.observable_id
                                  WHERE s.item_id = $1 AND s.packaging_level = $2::text::packaging_level
                                    AND (EXISTS (SELECT 1 FROM observation o
                                                  WHERE o.observation_event_id = e.id AND {LIVE})
                                         OR EXISTS (SELECT 1 FROM observation_image i
                                                     WHERE i.observation_event_id = e.id AND {UNMOVED}))
                             )
                             INSERT INTO observation_event
                                 (tenant_id, client_event_id, observable_id, observed_at, recorded_by_id,
                                  method, ingestion_channel, derived_from_event_id, presentation_id)
                             SELECT $3, $4, $5, src.observed_at, $6, src.method, 'derived', src.id, src.presentation_id
                               FROM src
                             RETURNING derived_from_event_id, id"
                        ),
                        &[&item, &from, &tenant, &act, &target, &person],
                    )
                    .await?
                    .iter()
                    .map(|r| (r.get(0), r.get(1)))
                    .collect();

                // Every live figure, copied into its mirror.
                let copied = tx
                    .execute(
                        &format!(
                            "INSERT INTO observation
                                 (tenant_id, observation_event_id, observable_id, observed_at, client_event_id,
                                  metric_id, result_kind, dimension_id, value_numeric, value_instant,
                                  value_code_id, value_boolean, value_text, uncertainty_dimension_id,
                                  uncertainty_numeric, absent_reason, entered_value, entered_unit_id, confidence)
                             SELECT o.tenant_id, m.id, m.observable_id, m.observed_at, m.client_event_id,
                                    o.metric_id, o.result_kind, o.dimension_id, o.value_numeric, o.value_instant,
                                    o.value_code_id, o.value_boolean, o.value_text, o.uncertainty_dimension_id,
                                    o.uncertainty_numeric, o.absent_reason, o.entered_value, o.entered_unit_id,
                                    o.confidence
                               FROM observation o
                               JOIN observation_event m
                                 ON m.derived_from_event_id = o.observation_event_id
                                AND m.client_event_id = $1 AND m.observable_id = $2
                              WHERE {LIVE}"
                        ),
                        &[&act, &target],
                    )
                    .await?;

                // And retracted where they were, by an act of this person's now.
                tx.execute(
                    &format!(
                        "WITH live AS (
                             SELECT o.id, o.observable_id, o.metric_id, o.result_kind, o.dimension_id
                               FROM observation o
                               JOIN observable s ON s.id = o.observable_id
                              WHERE s.item_id = $1 AND s.packaging_level = $2::text::packaging_level AND {LIVE}
                         ),
                         ev AS (
                             INSERT INTO observation_event
                                 (tenant_id, client_event_id, observable_id, observed_at, recorded_by_id,
                                  method, ingestion_channel)
                             SELECT DISTINCT $3::uuid, $4::uuid, live.observable_id, $5::timestamptz, $6::uuid,
                                    'keyed'::observation_method, 'keyed'::ingestion_channel
                               FROM live
                             RETURNING id, observable_id
                         )
                         INSERT INTO observation
                             (tenant_id, observation_event_id, observable_id, observed_at, client_event_id,
                              metric_id, result_kind, dimension_id, absent_reason, retracts_observation_id)
                         SELECT $3, ev.id, live.observable_id, $5, $4, live.metric_id, live.result_kind,
                                live.dimension_id, 'retracted', live.id
                           FROM live JOIN ev ON ev.observable_id = live.observable_id"
                    ),
                    &[&item, &from, &tenant, &act, &body.occurred_at, &person],
                )
                .await?;

                // Every photo not moved before, filed again in its mirror with
                // its cut, and the original marked moved to it. Each move and
                // each cut is an act of its own, named from this one.
                let images = tx
                    .query(
                        &format!(
                            "SELECT i.id, i.observation_event_id, i.same_as_id
                               FROM observation_image i
                               JOIN observation_event e ON e.id = i.observation_event_id
                               JOIN observable s ON s.id = e.observable_id
                              WHERE s.item_id = $1 AND s.packaging_level = $2::text::packaging_level AND {UNMOVED}
                              ORDER BY i.captured_at, i.id"
                        ),
                        &[&item, &from],
                    )
                    .await?;
                let mut moved: HashMap<Uuid, Uuid> = HashMap::new();
                for image in &images {
                    let (old, event, same_as): (Uuid, Uuid, Option<Uuid>) = (image.get(0), image.get(1), image.get(2));
                    let Some(&into) = mirrored.get(&event) else { continue };
                    let part = |what: &str| NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: Uuid::new_v5(&act, format!("{what}:{old}").as_bytes()),
                        site_id: ev.site_id,
                        recorded_by_id: person,
                        submitted_at: body.occurred_at,
                    };
                    let same_as = same_as.map(|s| moved.get(&s).copied().unwrap_or(s));
                    let new: Uuid = tx
                        .query_one(
                            "INSERT INTO observation_image
                                 (tenant_id, observation_event_id, face, digest, mime, byte_count,
                                  width_px, height_px, captured_at, same_as_id)
                             SELECT tenant_id, $2, face, digest, mime, byte_count, width_px, height_px,
                                    captured_at, $3
                               FROM observation_image WHERE id = $1
                             RETURNING id",
                            &[&old, &into, &same_as],
                        )
                        .await?
                        .get(0);
                    let cut = part("cut");
                    let had_cut = tx
                        .query_opt(
                            "SELECT 1 FROM observation_image_cut WHERE observation_image_id = $1",
                            &[&old],
                        )
                        .await?
                        .is_some();
                    if had_cut {
                        client_events::claim_act(tx, &cut).await?;
                        tx.execute(
                            "INSERT INTO observation_image_cut
                                 (tenant_id, observation_image_id, client_event_id, recorded_by_id, corners,
                                  digest, mime, byte_count, width_px, height_px)
                             SELECT tenant_id, $2, $3, $4, corners, digest, mime, byte_count, width_px, height_px
                               FROM observation_image_cut WHERE observation_image_id = $1
                              ORDER BY recorded_at DESC, id DESC LIMIT 1",
                            &[&old, &new, &cut.client_event_id, &person],
                        )
                        .await?;
                    }
                    let shift = part("move");
                    client_events::claim_act(tx, &shift).await?;
                    tx.execute(
                        "INSERT INTO observation_image_move
                             (tenant_id, observation_image_id, moved_to_image_id, client_event_id, recorded_by_id)
                         VALUES ($1, $2, $3, $4, $5)",
                        &[&tenant, &old, &new, &shift.client_event_id, &person],
                    )
                    .await?;
                    moved.insert(old, new);
                }

                Ok(Refiled { figures: copied as i64, photos: moved.len() as i64, replay: false })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}
