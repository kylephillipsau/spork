//! What goes to the carrier as it is, and which boxes the bench suggests. D196.
//!
//! The pack bench's suggestion (D195) chooses a box for what is left and
//! arranges it. Two things it could not know: that a roll in its own box can
//! travel as it is, and that a shovel box is for shovels. Both are said here,
//! and the suggestion reads them.
//!
//! Whether a thing ships as it is, is said of a capture subject the way what
//! it is packed in is (D191): the newest saying wins, a variant takes its
//! item's carton's and an item's carton its family's carton's. Unsaid, a
//! carton does and anything else does not (`ships_as_is`, migration 113).
//! Whether it must stay the way up it stands is said the same way (D200,
//! `keeps_upright`, migration 114); unsaid, any way up will do.

use actix_web::{post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::client_events::{self, NewClientEvent};
use crate::error::ApiError;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// A subject, one arm of four, as `POST /packaging` takes it.
#[derive(Deserialize, Debug)]
pub struct SubjectArms {
    pub item_id: Option<Uuid>,
    pub item_style_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub item_part_id: Option<Uuid>,
    /// With an item or a family: `each`, `inner` or `carton`.
    pub packaging_level: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct SayShipsAsIsRequest {
    #[serde(flatten)]
    pub subject: SubjectArms,
    /// It goes to the carrier as it is, rather than into a box.
    pub as_it_is: bool,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// Say whether a subject ships as it is. Saying it again is the same act.
#[post("/shipping")]
pub async fn say_ships_as_is(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<SayShipsAsIsRequest>,
) -> Result<HttpResponse, ApiError> {
    let b = body.into_inner();
    say(&state, &req, b.subject, Said::ShipsAsIs(b.as_it_is), b.client_event_id, b.occurred_at).await
}

#[derive(Deserialize, Debug)]
pub struct SayUprightRequest {
    #[serde(flatten)]
    pub subject: SubjectArms,
    /// It stays the way up it stands: turned round, never onto its side (D200).
    pub upright: bool,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// Say whether a subject must stay the way up it stands (D200). Saying it
/// again is the same act.
#[post("/upright")]
pub async fn say_upright(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<SayUprightRequest>,
) -> Result<HttpResponse, ApiError> {
    let b = body.into_inner();
    say(&state, &req, b.subject, Said::Upright(b.upright), b.client_event_id, b.occurred_at).await
}

/// A yes or no said of a subject, and the table its sayings are kept in.
enum Said {
    ShipsAsIs(bool),
    Upright(bool),
}

/// One saying of a subject: checked as `POST /packaging` checks one, and
/// written once however often it is sent.
async fn say(
    state: &web::Data<AppState>,
    req: &HttpRequest,
    subject: SubjectArms,
    said: Said,
    client_event_id: Uuid,
    occurred_at: DateTime<Utc>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(state, req).await?;
    let arms = [subject.item_id, subject.item_style_id, subject.lot_id, subject.item_part_id];
    if arms.iter().filter(|a| a.is_some()).count() != 1 {
        return Err(ApiError::Rejected("say it of one thing: an item, a family, a variant or a part".into()));
    }
    let levelled = subject.item_id.is_some() || subject.item_style_id.is_some();
    match subject.packaging_level.as_deref() {
        Some("each" | "inner" | "carton") if levelled => {}
        None if !levelled => {}
        _ if levelled => return Err(ApiError::Rejected("an item or a family needs its level: each, inner or carton".into())),
        _ => return Err(ApiError::Rejected("a variant or a part has no level".into())),
    }
    let ev = NewClientEvent {
        tenant_id: who.tenant_id,
        client_event_id,
        site_id: who.site_id,
        recorded_by_id: who.person_id,
        submitted_at: occurred_at,
    };
    let (table, column, value) = match said {
        Said::ShipsAsIs(v) => ("subject_shipping", "as_it_is", v),
        Said::Upright(v) => ("subject_upright", "upright", v),
    };
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let there = tx
                    .query_one(
                        "SELECT EXISTS (SELECT 1 FROM item WHERE id = $1)
                             OR EXISTS (SELECT 1 FROM item_style WHERE id = $2)
                             OR EXISTS (SELECT 1 FROM lot WHERE id = $3)
                             OR EXISTS (SELECT 1 FROM item_part WHERE id = $4)",
                        &[&subject.item_id, &subject.item_style_id, &subject.lot_id, &subject.item_part_id],
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
                    &format!(
                        "INSERT INTO {table}
                             (tenant_id, item_id, item_style_id, lot_id, item_part_id, packaging_level,
                              {column}, client_event_id, recorded_by_id)
                         VALUES ($1, $2, $3, $4, $5, $6::text::packaging_level, $7, $8, $9)"
                    ),
                    &[
                        &ev.tenant_id,
                        &subject.item_id,
                        &subject.item_style_id,
                        &subject.lot_id,
                        &subject.item_part_id,
                        &subject.packaging_level,
                        &value,
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

#[derive(Deserialize, Debug)]
pub struct BoxWeightRequest {
    /// The most the goods in it may weigh, in grams; null for no limit.
    pub max_payload_g: Option<i64>,
}

/// Say the most a box's goods may weigh, or that there is no limit (D199).
/// The suggestion fills a box no heavier. The workspace's own boxes only, as
/// with [`suggest_box`].
#[post("/package-types/{id}/max-weight")]
pub async fn box_weight(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<BoxWeightRequest>,
) -> Result<HttpResponse, ApiError> {
    let refused = "a box's limit is more than nothing; leave it empty for none";
    set_box_grams(&state, &req, path.into_inner(), BoxGrams::MostGoods, body.max_payload_g, refused).await
}

#[derive(Deserialize, Debug)]
pub struct BoxEmptyWeightRequest {
    /// What the box weighs empty, in grams; null when nobody has weighed one.
    pub tare_weight_g: Option<i64>,
}

/// Say what a box weighs empty (D224), so a box of goods weighs its goods and
/// itself by the record. The workspace's own boxes only, as with [`suggest_box`].
#[post("/package-types/{id}/empty-weight")]
pub async fn box_empty_weight(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<BoxEmptyWeightRequest>,
) -> Result<HttpResponse, ApiError> {
    let refused = "an empty box weighs more than nothing; leave it empty until one is weighed";
    set_box_grams(&state, &req, path.into_inner(), BoxGrams::Empty, body.tare_weight_g, refused).await
}

/// A weight a workspace says of one of its boxes.
enum BoxGrams {
    /// The most its goods may weigh (D199).
    MostGoods,
    /// What it weighs empty (D224).
    Empty,
}

/// Say one of a box's weights in grams, or clear it. Only the workspace's own
/// boxes: one the platform ships is not the workspace's to change, and is
/// refused as not found.
async fn set_box_grams(
    state: &web::Data<AppState>,
    req: &HttpRequest,
    id: Uuid,
    which: BoxGrams,
    grams: Option<i64>,
    refused: &str,
) -> Result<HttpResponse, ApiError> {
    let who = caller(state, req).await?;
    if grams.is_some_and(|g| g <= 0) {
        return Err(ApiError::Rejected(refused.into()));
    }
    let sql = match which {
        BoxGrams::MostGoods => "UPDATE package_type SET max_payload_g = $2 WHERE id = $1 AND tenant_id IS NOT NULL",
        BoxGrams::Empty => "UPDATE package_type SET tare_weight_g = $2 WHERE id = $1 AND tenant_id IS NOT NULL",
    };
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                if tx.execute(sql, &[&id, &grams]).await? == 0 {
                    return Err(ApiError::NotFound);
                }
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize, Debug)]
pub struct SuggestBoxRequest {
    /// Whether the bench's suggestion may choose this box.
    pub suggested: bool,
}

/// Say whether the bench's suggestion may choose a box (D196). A setting of
/// the workspace's own boxes: one the platform ships is not the workspace's to
/// change, and is refused as not found.
#[post("/package-types/{id}/suggested")]
pub async fn suggest_box(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SuggestBoxRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let id = path.into_inner();
    let suggested = body.suggested;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let changed = tx
                    .execute(
                        "UPDATE package_type SET suggested = $2 WHERE id = $1 AND tenant_id IS NOT NULL",
                        &[&id, &suggested],
                    )
                    .await?;
                if changed == 0 {
                    return Err(ApiError::NotFound);
                }
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}
