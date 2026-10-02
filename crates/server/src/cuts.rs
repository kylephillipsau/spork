//! A photograph cut to the face it is of. D176.
//!
//! A photograph of a carton's side is the side and everything around it, and
//! the side leans away because the phone was above it. Somebody marks the
//! side's four corners, the client straightens what is inside them, and the
//! result is kept beside the photograph rather than over it.
//!
//! # Two requests
//!
//! The straightened face's bytes go up first, to `POST /images`, and come back
//! as a content address. Then the act, `POST /observation-images/{id}/cuts`,
//! names the photograph, the corners and that address. So the act is JSON with
//! a retry identity like every other (D5), and the bytes are raw, as a
//! photograph's are. Bytes stored and never named by an act are a file nobody
//! refers to, which is the reaper's shape (D132), and serving them needs a row
//! that names them, so stored is not readable.
//!
//! # Who cut it
//!
//! From the session, as for every act (D11): a cut is somebody's judgement of
//! where the corners are, and it can be made long after the photograph, by
//! somebody else, at a desk.
//!
//! # What is waiting to be cut
//!
//! A phone takes the photographs and a computer cuts them (D181): the phone
//! has not the memory to find a face, and the computer finds each one and a
//! person checks it. `GET /photos/uncut` is that queue: the photographs an
//! item's page would show, its newest of each face, that nobody has cut.

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_events::{self, NewClientEvent};
use crate::error::ApiError;
use crate::images::{self, StoredImage};
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// The photographs an item's page shows, the newest of each face of each
/// subject, that have no cut: the queue a computer works through (D181).
/// Answers the photograph, its face, when it was taken, and the item whose
/// page shows it (a family's photograph opens on its first variant, a part's
/// on its product). Every face of the seven; never a detail, which is evidence
/// and not a side of anything.
pub const UNCUT: &str = "
    WITH newest AS (
        SELECT DISTINCT ON (o.id, oi.face)
               oi.id AS image_id, oi.digest, oi.face, oi.captured_at,
               o.item_id, o.item_style_id, o.item_part_id, o.lot_id
          FROM observable o
          JOIN observation_event e ON e.observable_id = o.id
          JOIN observation_image oi ON oi.observation_event_id = e.id
         WHERE (o.item_id IS NOT NULL OR o.item_style_id IS NOT NULL OR o.item_part_id IS NOT NULL
                OR o.lot_id IS NOT NULL)
           AND oi.face IN ('front', 'back', 'left', 'right', 'top', 'bottom', 'label')
         ORDER BY o.id, oi.face, oi.captured_at DESC, oi.id DESC
    )
    SELECT n.image_id, n.digest, n.face, n.captured_at,
           coalesce(n.item_id, p.item_id, (SELECT lt.item_id FROM lot lt WHERE lt.id = n.lot_id),
                    (SELECT v.id FROM item v WHERE v.style_id = n.item_style_id
                      ORDER BY v.code LIMIT 1)) AS open_item
      FROM newest n
      LEFT JOIN item_part p ON p.id = n.item_part_id
     WHERE NOT EXISTS (SELECT 1 FROM observation_image_cut c WHERE c.observation_image_id = n.image_id)";

/// A photograph waiting to be cut.
#[derive(Serialize, Debug)]
pub struct UncutPhoto {
    pub image_id: Uuid,
    pub digest: String,
    pub face: String,
    pub captured_at: DateTime<Utc>,
    /// The item whose page shows it, and so whose subjects say what it is of.
    pub item_id: Uuid,
    pub code: String,
    pub description: String,
}

/// How many the queue takes at once: a morning's photographs, not a year's.
const QUEUE: i64 = 300;

/// The photographs waiting to be cut, oldest first, so a computer works
/// through them in the order the phone took them (D181).
#[get("/photos/uncut")]
pub async fn uncut_photos(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        &format!(
                            "SELECT u.image_id, u.digest, u.face, u.captured_at, i.id, i.code, i.description
                               FROM ({UNCUT}) u
                               JOIN item i ON i.id = u.open_item
                              ORDER BY u.captured_at, u.image_id
                              LIMIT $1"
                        ),
                        &[&QUEUE],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| UncutPhoto {
                        image_id: r.get(0),
                        digest: r.get(1),
                        face: r.get(2),
                        captured_at: r.get(3),
                        item_id: r.get(4),
                        code: r.get(5),
                        description: r.get(6),
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

/// Keep an image's bytes and say their address. Nothing names them yet: an
/// act does that, and until one does they cannot be read back.
#[post("/images")]
pub async fn store_image(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    // Signed in, so the store is not a public bucket.
    caller(&state, &req).await?;
    let stored: StoredImage = images::store(&body).await?;
    Ok(HttpResponse::Ok().json(stored))
}

/// The act of cutting a photograph to its face.
#[derive(Deserialize, Debug)]
pub struct RecordCutRequest {
    /// Where `POST /images` kept the straightened face.
    pub digest: String,
    /// Top-left, top-right, bottom-right, bottom-left of the face, x then y,
    /// as fractions of the photograph shown the right way up.
    pub corners: Vec<f64>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct RecordCutResponse {
    pub cut_id: Uuid,
    pub digest: String,
}

/// The smallest share of the photograph a face may cover. Below it the corners
/// were not placed, they were dropped.
const LEAST_AREA: f64 = 0.01;

/// Corners that make a face, or why they do not.
///
/// Eight fractions, four points in order round the face, clockwise as the
/// photograph is shown (its y runs down), convex, and covering a real share of
/// the picture. A mirrored order would be a face seen from behind, and no
/// person marking corners produces one.
pub fn check_corners(corners: &[f64]) -> Result<(), String> {
    if corners.len() != 8 {
        return Err(format!("a face has four corners, eight numbers; this is {}", corners.len()));
    }
    if corners.iter().any(|c| !c.is_finite() || *c < 0.0 || *c > 1.0) {
        return Err("each corner is a fraction of the photograph, from 0 to 1".into());
    }
    let p: Vec<(f64, f64)> = corners.chunks(2).map(|c| (c[0], c[1])).collect();
    // The shoelace: twice the signed area, positive when clockwise as shown.
    let twice: f64 = (0..4)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % 4]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum();
    if twice / 2.0 < LEAST_AREA {
        return Err("the corners enclose almost nothing, or run the wrong way round".into());
    }
    // Convex: every turn round the face is the same way.
    for i in 0..4 {
        let (a, b, c) = (p[i], p[(i + 1) % 4], p[(i + 2) % 4]);
        let turn = (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0);
        if turn <= 0.0 {
            return Err("the corners cross or fold in; a face's edges do not".into());
        }
    }
    Ok(())
}

/// Cut a photograph to its face: the corners somebody marked, and the
/// straightened picture they made. Cutting again is another row; the newest
/// is the one shown.
#[post("/observation-images/{id}/cuts")]
pub async fn record_cut(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<RecordCutRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let image_id = path.into_inner();
    let body = body.into_inner();

    check_corners(&body.corners).map_err(ApiError::Rejected)?;
    if !images::is_digest(&body.digest) {
        return Err(ApiError::Rejected("that is not a content address".into()));
    }
    // The bytes have to be there and be an image: read back rather than
    // believed, as an upload's type is.
    let bytes = images::get(&images::directory(), &body.digest)
        .await?
        .ok_or_else(|| ApiError::Rejected("nothing is kept at that address; store the cut first".into()))?;
    let Some(mime) = images::sniff(&bytes) else {
        return Err(ApiError::Rejected("the bytes at that address are not an image".into()));
    };
    let (width, height) = match images::dimensions(&bytes) {
        Some((w, h)) => (Some(w), Some(h)),
        None => (None, None),
    };
    let byte_count = i64::try_from(bytes.len()).unwrap_or(i64::MAX);

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
                // The photograph, in this tenant: another's is a 404 here.
                tx.query_opt("SELECT 1 FROM observation_image WHERE id = $1", &[&image_id])
                    .await?
                    .ok_or(ApiError::NotFound)?;

                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    let prior = tx
                        .query_opt(
                            "SELECT id, digest, observation_image_id
                               FROM observation_image_cut WHERE client_event_id = $1",
                            &[&ev.client_event_id],
                        )
                        .await?
                        .ok_or_else(|| {
                            ApiError::Rejected("client_event exists but no cut was recorded; incomplete act".into())
                        })?;
                    let (id, digest, of): (Uuid, String, Uuid) = (prior.get(0), prior.get(1), prior.get(2));
                    if of != image_id || digest != body.digest {
                        return Err(ApiError::Rejected(
                            "that act was a different cut; a new cut is a new act".into(),
                        ));
                    }
                    return Ok(RecordCutResponse { cut_id: id, digest });
                }

                let row = tx
                    .query_one(
                        "INSERT INTO observation_image_cut
                             (tenant_id, observation_image_id, client_event_id, recorded_by_id,
                              corners, digest, mime, byte_count, width_px, height_px)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                         RETURNING id",
                        &[
                            &ev.tenant_id,
                            &image_id,
                            &ev.client_event_id,
                            &ev.recorded_by_id,
                            &body.corners,
                            &body.digest,
                            &mime,
                            &byte_count,
                            &width,
                            &height,
                        ],
                    )
                    .await?;
                Ok(RecordCutResponse { cut_id: row.get(0), digest: body.digest })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHOLE: [f64; 8] = [0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];

    #[test]
    fn a_face_is_four_corners_clockwise_round_a_real_share_of_the_photograph() {
        assert_eq!(check_corners(&WHOLE), Ok(()));
        // A side leaning away: wider at the bottom, as from a phone above it.
        assert_eq!(check_corners(&[0.3, 0.2, 0.7, 0.2, 0.85, 0.8, 0.15, 0.8]), Ok(()));
        // Its corners named from another corner, which is how a face is turned.
        assert_eq!(check_corners(&[0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0]), Ok(()));
    }

    #[test]
    fn corners_that_are_not_a_face_are_refused_in_words() {
        assert!(check_corners(&WHOLE[..6]).is_err(), "three corners");
        assert!(check_corners(&[0.0, 0.0, 1.2, 0.0, 1.0, 1.0, 0.0, 1.0]).is_err(), "off the photograph");
        assert!(check_corners(&[f64::NAN, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]).is_err(), "not a number");
        // Anticlockwise: the face seen from behind.
        assert!(check_corners(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0]).is_err());
        // Two corners swapped: the edges cross.
        assert!(check_corners(&[0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0]).is_err());
        // Folded in: a dart, not a face.
        assert!(check_corners(&[0.0, 0.0, 1.0, 0.0, 0.3, 0.3, 0.0, 1.0]).is_err());
        // Dropped in a heap.
        assert!(check_corners(&[0.5, 0.5, 0.51, 0.5, 0.51, 0.51, 0.5, 0.51]).is_err());
    }
}
