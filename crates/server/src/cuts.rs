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
/// The cuts of subjects packed in something without six sides (D191), as a
/// `FROM` clause: `SELECT 1 {NOT_A_BOX} WHERE x.digest = ...` asks of one.
pub const NOT_A_BOX: &str = "
      FROM observation_image_cut x
      JOIN observation_image oi ON oi.id = x.observation_image_id
      JOIN observation_event e ON e.id = oi.observation_event_id
      JOIN observable o ON o.id = e.observable_id
      JOIN LATERAL packed_in(o.item_id, o.item_style_id, o.lot_id, o.item_part_id, o.packaging_level) pk ON true
      JOIN packaging_type t ON t.code = pk.packaging_type AND NOT t.six_sided";

pub const UNCUT: &str = "
    WITH newest AS (
        SELECT DISTINCT ON (o.id, oi.face)
               oi.id AS image_id, oi.digest, oi.face, oi.captured_at, oi.same_as_id,
               oi.observation_event_id AS look_id, o.packaging_level::text AS level,
               o.packaging_level, o.item_id, o.item_style_id, o.item_part_id, o.lot_id
          FROM observable o
          JOIN observation_event e ON e.observable_id = o.id
          JOIN observation_image oi ON oi.observation_event_id = e.id
         WHERE (o.item_id IS NOT NULL OR o.item_style_id IS NOT NULL OR o.item_part_id IS NOT NULL
                OR o.lot_id IS NOT NULL)
           AND oi.face IN ('front', 'back', 'left', 'right', 'top', 'bottom', 'label')
           AND NOT EXISTS (SELECT 1 FROM observation_image_move mv WHERE mv.observation_image_id = oi.id)
         ORDER BY o.id, oi.face, oi.captured_at DESC, oi.id DESC
    )
    SELECT n.image_id, n.digest, n.face, n.captured_at, n.look_id, n.level,
           (SELECT st.code FROM item_style st WHERE st.id = n.item_style_id) AS family,
           (SELECT lt.code FROM lot lt WHERE lt.id = n.lot_id) AS variant,
           coalesce(n.item_id, p.item_id, (SELECT lt.item_id FROM lot lt WHERE lt.id = n.lot_id),
                    (SELECT v.id FROM item v WHERE v.style_id = n.item_style_id
                      ORDER BY v.code LIMIT 1)) AS open_item
      FROM newest n
      LEFT JOIN item_part p ON p.id = n.item_part_id
     WHERE NOT EXISTS (SELECT 1 FROM observation_image_cut c WHERE c.observation_image_id = n.image_id)
       -- A side said to look like another is cut with it (D183).
       AND n.same_as_id IS NULL
       -- A thing not packed as a box has no faces to cut it to (D191).
       AND NOT EXISTS (
           SELECT 1
             FROM packed_in(n.item_id, n.item_style_id, n.lot_id, n.item_part_id, n.packaging_level) pk
             JOIN packaging_type t ON t.code = pk.packaging_type
            WHERE NOT t.six_sided)";

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
    /// The look it was taken in: photographs of one box, moved together.
    pub look_id: Uuid,
    /// `carton`, `inner` or `each`; absent for a part or a variant.
    pub level: Option<String>,
    /// Its family's code when it is a photograph of the family's carton, not
    /// of one item's (D190); a variant's name when it is of a variant.
    pub family: Option<String>,
    pub variant: Option<String>,
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
                            "SELECT u.image_id, u.digest, u.face, u.captured_at, i.id, i.code, i.description,
                                    u.look_id, u.level, u.family, u.variant
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
                        look_id: r.get(7),
                        level: r.get(8),
                        family: r.get(9),
                        variant: r.get(10),
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

/// A photograph filed against the wrong subject, moved (D190).
#[derive(Deserialize, Debug)]
pub struct MoveRequest {
    /// The photograph filed again under the right subject, already made with
    /// the same bytes under a look of that subject.
    pub moved_to: Uuid,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// Say a photograph was moved to its right subject: it is shown nowhere now,
/// and the one it was moved to is (D190).
#[post("/observation-images/{id}/moved")]
pub async fn record_move(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<MoveRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let from = path.into_inner();
    let to = body.moved_to;
    if from == to {
        return Err(ApiError::Rejected("a photograph is not moved onto itself".into()));
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
                let digests = tx
                    .query("SELECT id, digest FROM observation_image WHERE id = ANY($1)", &[&vec![from, to]])
                    .await?;
                if digests.len() != 2 {
                    return Err(ApiError::NotFound);
                }
                if digests[0].get::<_, String>(1) != digests[1].get::<_, String>(1) {
                    return Err(ApiError::Rejected("a photograph is moved with its own bytes".into()));
                }
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    return Ok(());
                }
                tx.execute(
                    "INSERT INTO observation_image_move
                         (tenant_id, observation_image_id, moved_to_image_id, client_event_id, recorded_by_id)
                     VALUES ($1, $2, $3, $4, $5)
                     ON CONFLICT (observation_image_id) DO NOTHING",
                    &[&ev.tenant_id, &from, &to, &ev.client_event_id, &ev.recorded_by_id],
                )
                .await?;
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// An item drawn as its box from three cut faces (D186).
#[derive(Deserialize, Debug)]
pub struct BoxPictureRequest {
    /// Where `POST /images` kept the drawing.
    pub digest: String,
    /// The front, right and top cuts it was drawn from, by content address.
    pub made_from: Vec<String>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct BoxPictureSaid {
    pub box_picture_id: Uuid,
    pub digest: String,
}

/// Keep an item's box drawing (D186): the newest is its picture.
#[post("/items/{id}/box-picture")]
pub async fn record_box_picture(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<BoxPictureRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item_id = path.into_inner();
    let body = body.into_inner();
    if body.made_from.len() != 3 || !body.made_from.iter().all(|d| images::is_digest(d)) {
        return Err(ApiError::Rejected("a box is drawn from three cut faces: front, right and top".into()));
    }
    if !images::is_digest(&body.digest) {
        return Err(ApiError::Rejected("that is not a content address".into()));
    }
    let bytes = images::get(&images::directory(), &body.digest)
        .await?
        .ok_or_else(|| ApiError::Rejected("nothing is kept at that address; store the drawing first".into()))?;
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
                tx.query_opt("SELECT 1 FROM item WHERE id = $1", &[&item_id])
                    .await?
                    .ok_or(ApiError::NotFound)?;
                // Only a box is drawn: a thing in shrink-wrap is pictured by its photo (D191).
                let flat: bool = tx
                    .query_one(&format!("SELECT EXISTS (SELECT 1 {NOT_A_BOX} WHERE x.digest = $1)"), &[&body.made_from[0]])
                    .await?
                    .get(0);
                if flat {
                    return Err(ApiError::Rejected(
                        "only a box is drawn; this is packed in something without six sides".into(),
                    ));
                }
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    let prior = tx
                        .query_opt("SELECT id, digest FROM box_picture WHERE client_event_id = $1", &[&ev.client_event_id])
                        .await?
                        .ok_or_else(|| ApiError::Rejected("client_event exists but no drawing was kept; incomplete act".into()))?;
                    return Ok(BoxPictureSaid { box_picture_id: prior.get(0), digest: prior.get(1) });
                }
                let row = tx
                    .query_one(
                        "INSERT INTO box_picture
                             (tenant_id, item_id, digest, mime, byte_count, width_px, height_px,
                              made_from, client_event_id, recorded_by_id)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                         RETURNING id",
                        &[
                            &ev.tenant_id,
                            &item_id,
                            &body.digest,
                            &mime,
                            &byte_count,
                            &width,
                            &height,
                            &body.made_from,
                            &ev.client_event_id,
                            &ev.recorded_by_id,
                        ],
                    )
                    .await?;
                Ok(BoxPictureSaid { box_picture_id: row.get(0), digest: body.digest })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

/// A side said to look like another (D183).
#[derive(Deserialize, Debug)]
pub struct SameAsRequest {
    /// The side that looks like the photograph's.
    pub face: String,
}

/// The sides a box has: a label looks like nothing but itself.
const SIDES: [&str; 6] = ["front", "right", "back", "left", "top", "bottom"];

/// Say that another side of the same thing looks like this photograph: a
/// photograph of that side, in the same look, carrying these bytes and, once
/// it is cut, this cut (D183). Nothing is uploaded.
#[post("/observation-images/{id}/same-as")]
pub async fn same_as(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SameAsRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let source = path.into_inner();
    let face = body.face.clone();
    if !SIDES.contains(&face.as_str()) {
        return Err(ApiError::Rejected(format!("a side is one of {}, not {face}", SIDES.join(", "))));
    }
    let tenant = who.tenant_id;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let src = tx
                    .query_opt(
                        "SELECT observation_event_id, face, digest, mime, byte_count, width_px, height_px,
                                captured_at, same_as_id
                           FROM observation_image WHERE id = $1",
                        &[&source],
                    )
                    .await?
                    .ok_or(ApiError::NotFound)?;
                let src_face: String = src.get(1);
                if src_face == face {
                    return Err(ApiError::Rejected("a side looks like itself already".into()));
                }
                if !SIDES.contains(&src_face.as_str()) {
                    return Err(ApiError::Rejected("only a side of the box can stand for another".into()));
                }
                // Said of a side that was itself said: point at the photograph taken.
                let taken: Uuid = src.get::<_, Option<Uuid>>(8).unwrap_or(source);
                let row = tx
                    .query_one(
                        "INSERT INTO observation_image
                             (tenant_id, observation_event_id, face, digest, mime, byte_count,
                              width_px, height_px, captured_at, same_as_id)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                         RETURNING id",
                        &[
                            &tenant,
                            &src.get::<_, Uuid>(0),
                            &face,
                            &src.get::<_, String>(2),
                            &src.get::<_, String>(3),
                            &src.get::<_, i64>(4),
                            &src.get::<_, Option<i32>>(5),
                            &src.get::<_, Option<i32>>(6),
                            &src.get::<_, DateTime<Utc>>(7),
                            &taken,
                        ],
                    )
                    .await?;
                Ok(SameAsSaid { image_id: row.get(0), same_as_id: taken, face })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

#[derive(Serialize, Debug)]
pub struct SameAsSaid {
    pub image_id: Uuid,
    /// The photograph it stands for.
    pub same_as_id: Uuid,
    pub face: String,
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
