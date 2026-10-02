//! What a subject is packed in. D191.
//!
//! GS1's packaging type code, said of a capture subject: the each, the inner
//! pack, the carton, a family's carton, a variant or a part. A type with six
//! flat faces is photographed side by side, cut to its faces and drawn as a
//! box; any other has a photo and what else is worth taking. Nothing said is
//! treated as a box, which is what every subject was before this.

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_events::{self, NewClientEvent};
use crate::error::ApiError;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// One of GS1's packaging types.
#[derive(Serialize, Debug)]
pub struct PackagingType {
    pub code: String,
    pub name: String,
    /// GS1's definition, as published.
    pub definition: String,
    /// A box with six flat faces: photographed side by side and drawn.
    pub six_sided: bool,
    /// Offered first, in this order; absent for the rest.
    pub common: Option<i16>,
}

/// Every packaging type, the common ones first.
#[get("/packaging-types")]
pub async fn packaging_types(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        "SELECT code, name, definition, six_sided, common FROM packaging_type
                          ORDER BY common NULLS LAST, name",
                        &[],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| PackagingType {
                        code: r.get(0),
                        name: r.get(1),
                        definition: r.get(2),
                        six_sided: r.get(3),
                        common: r.get(4),
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

#[derive(Deserialize, Debug)]
pub struct SayPackedInRequest {
    /// The subject, one arm of four, as `POST /observations` takes it.
    pub item_id: Option<Uuid>,
    pub item_style_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub item_part_id: Option<Uuid>,
    /// With an item or a family: `each`, `inner` or `carton`.
    pub packaging_level: Option<String>,
    /// A GS1 packaging type code.
    pub packaging_type: String,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// Say what a subject is packed in. Saying it again is the same act.
#[post("/packaging")]
pub async fn say_packed_in(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<SayPackedInRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let body = body.into_inner();
    let arms = [body.item_id, body.item_style_id, body.lot_id, body.item_part_id];
    if arms.iter().filter(|a| a.is_some()).count() != 1 {
        return Err(ApiError::Rejected("say it of one thing: an item, a family, a variant or a part".into()));
    }
    let levelled = body.item_id.is_some() || body.item_style_id.is_some();
    match body.packaging_level.as_deref() {
        Some("each" | "inner" | "carton") if levelled => {}
        None if !levelled => {}
        _ if levelled => return Err(ApiError::Rejected("an item or a family needs its level: each, inner or carton".into())),
        _ => return Err(ApiError::Rejected("a variant or a part has no level".into())),
    }
    let ev = NewClientEvent {
        tenant_id: who.tenant_id,
        client_event_id: body.client_event_id,
        site_id: who.site_id,
        recorded_by_id: who.person_id,
        submitted_at: body.occurred_at,
    };
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                tx.query_opt("SELECT 1 FROM packaging_type WHERE code = $1", &[&body.packaging_type])
                    .await?
                    .ok_or_else(|| ApiError::Rejected("that is not a GS1 packaging type code".into()))?;
                let there = tx
                    .query_one(
                        "SELECT EXISTS (SELECT 1 FROM item WHERE id = $1)
                             OR EXISTS (SELECT 1 FROM item_style WHERE id = $2)
                             OR EXISTS (SELECT 1 FROM lot WHERE id = $3)
                             OR EXISTS (SELECT 1 FROM item_part WHERE id = $4)",
                        &[&body.item_id, &body.item_style_id, &body.lot_id, &body.item_part_id],
                    )
                    .await?
                    .get::<_, bool>(0);
                if !there {
                    return Err(ApiError::NotFound);
                }
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    return Ok(());
                }
                tx.execute(
                    "INSERT INTO subject_packaging
                         (tenant_id, item_id, item_style_id, lot_id, item_part_id, packaging_level,
                          packaging_type, client_event_id, recorded_by_id)
                     VALUES ($1, $2, $3, $4, $5, $6::text::packaging_level, $7, $8, $9)",
                    &[
                        &ev.tenant_id,
                        &body.item_id,
                        &body.item_style_id,
                        &body.lot_id,
                        &body.item_part_id,
                        &body.packaging_level,
                        &body.packaging_type,
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
