//! A round thing wrapped in its photographs. D240.
//!
//! A bucket's side has no faces to cut a photograph to (D191). At a computer,
//! its photographs of its side are unwrapped onto the measured shape (D213)
//! and laid together into one picture of the whole side; its lid and base are
//! straightened into discs. The browser does the work, from the photographs
//! and the shape, as it cuts a box's faces (D176); this keeps what it made and
//! says what is still waiting.

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_events::{self, NewClientEvent};
use crate::error::ApiError;
use crate::images;
use crate::routes::caller;
use crate::shipping::{arms_exist, check_arms, SubjectArms};
use crate::tenancy::TenantScope;
use crate::AppState;

/// The round things with photographs of their side not yet wrapped: packed in
/// a round type, two sides or more photographed, and no wrapping made from
/// the newest of them. One row per subject, with the item whose page shows it.
/// A query of its own, so the queue and its count cannot disagree.
pub const UNWRAPPED: &str = "
    WITH newest AS (
        SELECT DISTINCT ON (o.id, oi.face)
               o.id AS observable_id, oi.id AS image_id, oi.face,
               o.item_id, o.item_style_id, o.lot_id, o.item_part_id, o.packaging_level
          FROM observable o
          JOIN observation_event e ON e.observable_id = o.id
          JOIN observation_image oi ON oi.observation_event_id = e.id
         WHERE oi.face IN ('front', 'right', 'back', 'left')
           AND NOT EXISTS (SELECT 1 FROM observation_image_move mv WHERE mv.observation_image_id = oi.id)
         ORDER BY o.id, oi.face, oi.captured_at DESC, oi.id DESC
    ),
    sides AS (
        SELECT observable_id, item_id, item_style_id, lot_id, item_part_id, packaging_level,
               array_agg(image_id) AS images
          FROM newest
         GROUP BY observable_id, item_id, item_style_id, lot_id, item_part_id, packaging_level
        HAVING count(*) >= 2
    )
    SELECT s.item_id, s.item_style_id, s.lot_id, s.item_part_id, s.packaging_level::text AS level,
           coalesce(s.item_id, p.item_id, (SELECT lt.item_id FROM lot lt WHERE lt.id = s.lot_id),
                    (SELECT v.id FROM item v WHERE v.style_id = s.item_style_id ORDER BY v.code LIMIT 1)) AS open_item
      FROM sides s
      LEFT JOIN item_part p ON p.id = s.item_part_id
      JOIN LATERAL packed_in(s.item_id, s.item_style_id, s.lot_id, s.item_part_id, s.packaging_level) pk ON true
      JOIN packaging_type t ON t.code = pk.packaging_type AND t.round
     WHERE NOT EXISTS (
           SELECT 1 FROM wrap_of(s.item_id, s.item_style_id, s.lot_id, s.item_part_id, s.packaging_level) w
            WHERE w.made_from @> s.images)";

/// A round thing waiting to be wrapped: the subject, and the item whose page
/// shows it, where its shape and photographs are read.
#[derive(Serialize, Debug)]
pub struct Unwrapped {
    pub item_id: Option<Uuid>,
    pub item_style_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub item_part_id: Option<Uuid>,
    pub packaging_level: Option<String>,
    pub open_item: Uuid,
    pub code: String,
    pub description: String,
}

/// The round things whose photographs of their side wait to be wrapped (D240).
#[get("/photos/unwrapped")]
pub async fn unwrapped(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        &format!(
                            "SELECT u.item_id, u.item_style_id, u.lot_id, u.item_part_id, u.level, u.open_item,
                                    i.code, i.description
                               FROM ({UNWRAPPED}) u
                               JOIN item i ON i.id = u.open_item
                              ORDER BY i.code"
                        ),
                        &[],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| Unwrapped {
                        item_id: r.get(0),
                        item_style_id: r.get(1),
                        lot_id: r.get(2),
                        item_part_id: r.get(3),
                        packaging_level: r.get(4),
                        open_item: r.get(5),
                        code: r.get(6),
                        description: r.get(7),
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

#[derive(Deserialize, Debug)]
pub struct WrapRequest {
    #[serde(flatten)]
    pub subject: SubjectArms,
    /// Its side unwrapped, where `POST /images` kept it.
    pub side: String,
    /// Its lid and its base as discs, where photographed.
    pub lid: Option<String>,
    pub base: Option<String>,
    /// Open, with no lid (D241): its inside wall unwrapped, and its floor.
    pub inside: Option<String>,
    pub floor: Option<String>,
    /// The photographs it was made from.
    pub made_from: Vec<Uuid>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// Keep a round thing's wrapping (D240): the newest is how it is drawn.
/// Saying it again is the same act.
#[post("/round-wraps")]
pub async fn record_wrap(req: HttpRequest, state: web::Data<AppState>, body: web::Json<WrapRequest>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let b = body.into_inner();
    check_arms(&b.subject)?;
    if b.made_from.is_empty() {
        return Err(ApiError::Rejected("a wrapping is made from the photographs of its side".into()));
    }
    // Each picture as its row will say it: its type and size, from its bytes.
    let mut pictures = vec![];
    if b.lid.is_some() && (b.inside.is_some() || b.floor.is_some()) {
        return Err(ApiError::Rejected("a thing with a lid is not open: say its lid or its inside, not both".into()));
    }
    for (part, digest) in [
        ("side", Some(&b.side)),
        ("lid", b.lid.as_ref()),
        ("base", b.base.as_ref()),
        ("inside", b.inside.as_ref()),
        ("floor", b.floor.as_ref()),
    ] {
        let Some(digest) = digest else { continue };
        if !images::is_digest(digest) {
            return Err(ApiError::Rejected("that is not a content address".into()));
        }
        let bytes = images::get(&images::directory(), digest)
            .await?
            .ok_or_else(|| ApiError::Rejected("nothing is kept at that address; store the pictures first".into()))?;
        let Some(mime) = images::sniff(&bytes) else {
            return Err(ApiError::Rejected("the bytes at that address are not an image".into()));
        };
        let (width, height) = images::dimensions(&bytes).map_or((None, None), |(w, h)| (Some(w), Some(h)));
        pictures.push((part, digest.clone(), mime, i64::try_from(bytes.len()).unwrap_or(i64::MAX), width, height));
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
                if !arms_exist(tx, &b.subject).await? {
                    return Err(ApiError::NotFound);
                }
                let known: i64 = tx
                    .query_one("SELECT count(*) FROM observation_image WHERE id = ANY($1)", &[&b.made_from])
                    .await?
                    .get(0);
                if known != b.made_from.len() as i64 {
                    return Err(ApiError::Rejected("a wrapping is made from photographs on file".into()));
                }
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    return Ok(());
                }
                let s = &b.subject;
                let wrap: Uuid = tx
                    .query_one(
                        "INSERT INTO round_wrap
                             (tenant_id, item_id, item_style_id, lot_id, item_part_id, packaging_level,
                              made_from, client_event_id, recorded_by_id)
                         VALUES ($1, $2, $3, $4, $5, $6::text::packaging_level, $7, $8, $9)
                         RETURNING id",
                        &[
                            &ev.tenant_id,
                            &s.item_id,
                            &s.item_style_id,
                            &s.lot_id,
                            &s.item_part_id,
                            &s.packaging_level,
                            &b.made_from,
                            &ev.client_event_id,
                            &ev.recorded_by_id,
                        ],
                    )
                    .await?
                    .get(0);
                for (part, digest, mime, bytes, width, height) in &pictures {
                    tx.execute(
                        "INSERT INTO round_wrap_picture
                             (tenant_id, round_wrap_id, part, digest, mime, byte_count, width_px, height_px)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                        &[&ev.tenant_id, &wrap, part, digest, mime, bytes, width, height],
                    )
                    .await?;
                }
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}
