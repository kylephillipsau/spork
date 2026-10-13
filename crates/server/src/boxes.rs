//! Boxes cut down to set heights, and box types photographed. D252.
//!
//! The warehouse's boxes are cut at the corners and folded down to set
//! heights. Each height is a box type of its own, cut from a full box
//! (`package_type.cut_from_id`, migration 141): the full box's length and
//! width, its own height, and the name the booking gives it. So the
//! suggestion, the bench and the booking read a cut as they read any box, and
//! `package_type_effective` fills a cut's empty weight and limit from its full
//! box unless it says its own.
//!
//! A box type is photographed as an item is (`observable.package_type_id`): a
//! look with photographs and nothing measured, each side cut to its face as an
//! item's is (D176). A cut wears its full box's photographs, from the bottom
//! edge up to its height, so it is drawn at true scale rather than squashed.

use std::collections::{BTreeMap, HashMap};

use actix_web::{delete, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::items::PhotoCut;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// The full box a cut box is cut from.
#[derive(Serialize, Debug, Clone)]
pub struct BoxCut {
    pub from_id: Uuid,
    pub from_name: String,
    /// The full box's height. A cut wears its full box's photographs, so the
    /// bottom `height / full_height_mm` of each side is what shows.
    pub full_height_mm: i32,
}

impl BoxCut {
    /// From `package_type_effective`'s `cut_from_id`, `cut_from_name` and
    /// `full_height_mm`, when the type is a cut.
    pub fn of(from_id: Option<Uuid>, from_name: Option<String>, full_height_mm: Option<i32>) -> Option<BoxCut> {
        match (from_id, from_name, full_height_mm) {
            (Some(from_id), Some(from_name), Some(full_height_mm)) => Some(BoxCut { from_id, from_name, full_height_mm }),
            _ => None,
        }
    }
}

/// One side of a box type, as last photographed.
#[derive(Serialize, Debug)]
pub struct BoxPhoto {
    /// `front`, `back`, `left`, `right`, `top` or `bottom`.
    pub face: String,
    /// The photograph, which a cut points at (D176).
    pub image_id: Uuid,
    /// `GET /images/{digest}` serves it.
    pub digest: String,
    pub captured_at: DateTime<Utc>,
    /// Its newest cut to the face, when somebody has marked one.
    pub cut: Option<PhotoCut>,
}

/// The newest photograph of each side of these box types, not one moved to
/// another subject.
pub async fn photos(tx: &tokio_postgres::Transaction<'_>, ids: &[Uuid]) -> Result<HashMap<Uuid, Vec<BoxPhoto>>, ApiError> {
    let mut out: HashMap<Uuid, Vec<BoxPhoto>> = HashMap::new();
    for r in tx
        .query(
            "SELECT DISTINCT ON (o.package_type_id, oi.face)
                    o.package_type_id, oi.face, oi.id, oi.digest, oi.captured_at,
                    cut.digest, cut.corners
               FROM observable o
               JOIN observation_event e ON e.observable_id = o.id
               JOIN observation_image oi ON oi.observation_event_id = e.id
               LEFT JOIN LATERAL cut_of(oi.id) cut ON true
              WHERE o.package_type_id = ANY($1)
                AND oi.face IN ('front', 'back', 'left', 'right', 'top', 'bottom')
                AND NOT EXISTS (SELECT 1 FROM observation_image_move mv
                                 WHERE mv.observation_image_id = oi.id)
              ORDER BY o.package_type_id, oi.face, oi.captured_at DESC, oi.id DESC",
            &[&ids],
        )
        .await?
    {
        out.entry(r.get(0)).or_default().push(BoxPhoto {
            face: r.get(1),
            image_id: r.get(2),
            digest: r.get(3),
            captured_at: r.get(4),
            cut: r.get::<_, Option<String>>(5).map(|digest| PhotoCut { digest, corners: r.get(6) }),
        });
    }
    Ok(out)
}

/// Each box type's sides to draw it with: the straightened picture of each
/// side cut to its face. An uncut photograph is the bench behind the box as
/// much as the box, so it is left out, as an item's is.
pub async fn faces(
    tx: &tokio_postgres::Transaction<'_>,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, BTreeMap<String, String>>, ApiError> {
    Ok(photos(tx, ids)
        .await?
        .into_iter()
        .map(|(id, sides)| (id, sides.into_iter().filter_map(|p| Some((p.face, p.cut?.digest))).collect()))
        .collect())
}

/// Centimetres as a box's heights are said: "28", "34.5".
pub fn cm(mm: i32) -> String {
    if mm % 10 == 0 {
        format!("{}", mm / 10)
    } else {
        format!("{}.{}", mm / 10, mm % 10)
    }
}

/// The full box a height is cut from, as the check needs it.
pub struct FullBox {
    pub name: String,
    pub fixed: bool,
    /// A pallet or a skid.
    pub carries: bool,
    pub height_mm: Option<i32>,
    /// Itself cut from another.
    pub cut: bool,
}

/// Whether a box can be cut down to this height, or why not.
pub fn check_height(full: &FullBox, height_mm: i32) -> Result<(), String> {
    if full.cut {
        return Err(format!("{} is itself cut down from a box; add the height to the full box", full.name));
    }
    if full.carries {
        return Err(format!("{} carries cartons; only a box is cut down", full.name));
    }
    let Some(full_height) = full.height_mm.filter(|_| full.fixed) else {
        return Err(format!("{} has no fixed size to cut down from", full.name));
    };
    if height_mm <= 0 {
        return Err("a height is more than nothing".into());
    }
    if height_mm >= full_height {
        return Err(format!("a cut is lower than the full box, {} cm", cm(full_height)));
    }
    Ok(())
}

#[derive(Deserialize, Debug)]
pub struct AddHeightRequest {
    pub height_mm: i32,
    /// The name it is booked as, when not "Medium Box, cut to 28". A box
    /// already on file by that name, this footprint and this height, becomes
    /// the cut: the half box imported as a box of its own.
    pub name: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct AddedHeight {
    pub id: Uuid,
    pub name: String,
    /// A box already on file, linked, rather than a new one made.
    pub linked: bool,
}

/// Add a height a box is cut down to (D252). The workspace's own boxes only,
/// as with its other settings: one the platform ships is refused as not found.
#[post("/package-types/{id}/heights")]
pub async fn add_height(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<AddHeightRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let full_id = path.into_inner();
    let body = body.into_inner();
    let named = body.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_owned);
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let added = scope
        .run(move |tx| {
            Box::pin(async move {
                let row = tx
                    .query_opt(
                        "SELECT name, dimensions_fixed, carries, height_mm, cut_from_id IS NOT NULL,
                                carrier_package_code, length_mm, width_mm, suggested
                           FROM package_type_effective
                          WHERE id = $1 AND tenant_id IS NOT NULL",
                        &[&full_id],
                    )
                    .await?
                    .ok_or(ApiError::NotFound)?;
                let full = FullBox {
                    name: row.get(0),
                    fixed: row.get(1),
                    carries: row.get(2),
                    height_mm: row.get(3),
                    cut: row.get(4),
                };
                check_height(&full, body.height_mm).map_err(ApiError::Rejected)?;
                if let Some(had) = tx
                    .query_opt(
                        "SELECT name FROM package_type WHERE cut_from_id = $1 AND height_mm = $2",
                        &[&full_id, &body.height_mm],
                    )
                    .await?
                {
                    let had: String = had.get(0);
                    return Err(ApiError::Rejected(format!(
                        "{} is already cut to {} cm, as {had}",
                        full.name,
                        cm(body.height_mm)
                    )));
                }
                let (length, width): (Option<i32>, Option<i32>) = (row.get(6), row.get(7));
                let name = named.clone().unwrap_or_else(|| format!("{}, cut to {}", full.name, cm(body.height_mm)));
                // A box on file by that name is the cut when it is one: the
                // same footprint, this height, a box of its own with no cuts.
                if let Some(same) = tx
                    .query_opt(
                        "SELECT id, length_mm, width_mm, height_mm, dimensions_fixed,
                                cut_from_id IS NOT NULL OR height_is_set, carries
                           FROM package_type_effective
                          WHERE name = $1 AND tenant_id IS NOT NULL",
                        &[&name],
                    )
                    .await?
                {
                    let fits = same.get::<_, Option<i32>>(1) == length
                        && same.get::<_, Option<i32>>(2) == width
                        && same.get::<_, Option<i32>>(3) == Some(body.height_mm)
                        && same.get::<_, bool>(4)
                        && !same.get::<_, bool>(5)
                        && !same.get::<_, bool>(6);
                    if !fits {
                        return Err(ApiError::Rejected(format!(
                            "{name} is already a box of another size, or one with heights of its own; \
                             give this height another name"
                        )));
                    }
                    let id: Uuid = same.get(0);
                    tx.execute("UPDATE package_type SET cut_from_id = $2 WHERE id = $1", &[&id, &full_id])
                        .await?;
                    return Ok(AddedHeight { id, name, linked: true });
                }
                let code: Option<String> = row.get(5);
                let suggested: bool = row.get(8);
                let id: Uuid = tx
                    .query_one(
                        "INSERT INTO package_type
                             (tenant_id, name, carrier_package_code, dimensions_fixed,
                              length_mm, width_mm, height_mm, suggested, cut_from_id)
                         VALUES ($1, $2, $3, true, $4, $5, $6, $7, $8)
                         RETURNING id",
                        &[&who.tenant_id, &name, &code, &length, &width, &body.height_mm, &suggested, &full_id],
                    )
                    .await?
                    .get(0);
                Ok(AddedHeight { id, name, linked: false })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(added))
}

/// Take a height away (D252). Only a cut, and only one nothing was packed in:
/// a carton packed in it still names it, so it stays, left out of the
/// suggestion instead.
#[delete("/package-types/{id}")]
pub async fn remove_height(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let id = path.into_inner();
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let row = tx
                    .query_opt(
                        "SELECT name, cut_from_id IS NOT NULL,
                                (SELECT count(*) FROM package p WHERE p.package_type_id = t.id),
                                EXISTS (SELECT 1 FROM observable o WHERE o.package_type_id = t.id)
                                 OR EXISTS (SELECT 1 FROM asserted_unit a WHERE a.resolved_package_type_id = t.id)
                           FROM package_type t
                          WHERE id = $1 AND tenant_id IS NOT NULL
                          FOR UPDATE",
                        &[&id],
                    )
                    .await?
                    .ok_or(ApiError::NotFound)?;
                let (name, cut, packed, named): (String, bool, i64, bool) = (row.get(0), row.get(1), row.get(2), row.get(3));
                if !cut {
                    return Err(ApiError::Rejected(format!(
                        "{name} is a full box, from the presets import; only a height cut from one is taken away"
                    )));
                }
                if packed > 0 || named {
                    return Err(ApiError::Rejected(format!(
                        "{name} has been packed in or recorded, so it stays on file: untick Suggest to stop the bench suggesting it"
                    )));
                }
                tx.execute("DELETE FROM package_type WHERE id = $1", &[&id]).await?;
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn medium() -> FullBox {
        FullBox { name: "Medium Box".into(), fixed: true, carries: false, height_mm: Some(410), cut: false }
    }

    #[test]
    fn a_cut_is_lower_than_its_full_box_and_more_than_nothing() {
        assert!(check_height(&medium(), 280).is_ok());
        assert!(check_height(&medium(), 345).is_ok());
        assert!(check_height(&medium(), 410).unwrap_err().contains("41 cm"));
        assert!(check_height(&medium(), 0).is_err());
    }

    #[test]
    fn a_pallet_a_cut_and_a_box_of_no_size_are_not_cut() {
        let pallet = FullBox { carries: true, ..medium() };
        assert!(check_height(&pallet, 500).unwrap_err().contains("carries cartons"));
        let cut = FullBox { cut: true, ..medium() };
        assert!(check_height(&cut, 200).unwrap_err().contains("full box"));
        let satchel = FullBox { fixed: false, height_mm: None, ..medium() };
        assert!(check_height(&satchel, 100).unwrap_err().contains("no fixed size"));
    }

    #[test]
    fn heights_are_said_in_centimetres() {
        assert_eq!(cm(280), "28");
        assert_eq!(cm(345), "34.5");
        assert_eq!(cm(80), "8");
    }
}
