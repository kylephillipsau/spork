//! An item's own page: what it is, what it looks like, what it measures, and
//! where it is.
//!
//! **Where a scanned item lands.** D111's locator resolves an item code or
//! barcode to its id, and the question a person scans a product to ask is
//! *what is this and where does it live*. Until this, an item scan went to the
//! capture worklist or nowhere.
//!
//! # Two answers to "where", kept apart
//!
//! **Where this system holds it** is `stock`, the fold of its own ledger.
//! **Where NetSuite says it is** is `reported_stock`, a snapshot somebody
//! exported. Migration 86 is emphatic that the second is not the first, and a
//! page that merged them would be this system asserting it holds goods it never
//! recorded receiving. So they are two lists, the second carries its age, and
//! a screen says which is which.
//!
//! Nothing here writes.

use actix_web::{get, web, HttpRequest, HttpResponse};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::pictures::{self, Picture};
use crate::routes::{caller, measurements_of, ItemMeasurements};
use crate::tenancy::TenantScope;
use crate::AppState;

/// The style an item is a variant of (D108).
#[derive(Serialize, Debug)]
pub struct ItemStyleRef {
    pub code: String,
    pub description: Option<String>,
    /// Codes on file that are variants of it, this one included.
    pub variants: i64,
}

/// What a carton of it holds, as the case pack in force says.
///
/// Both counts are nullable because a carton can be known to exist before
/// anybody has said what is in it, which is what the prepack loader records
/// for a carton with no stated count.
#[derive(Serialize, Debug)]
pub struct ItemPacking {
    pub units_per_inner: Option<i32>,
    pub inners_per_carton: Option<i32>,
    pub effective_from: NaiveDate,
}

/// Some of it, on a shelf or at a warehouse, as this system's ledger holds it.
#[derive(Serialize, Debug)]
pub struct ItemHeld {
    pub site_code: Option<String>,
    /// The bin, when it is on one. Absent when it is inside a package.
    pub location_id: Option<Uuid>,
    pub bin_code: Option<String>,
    pub quantity: i64,
    pub allocated_quantity: i64,
}

/// Some of it, as NetSuite's inventory balance reported it.
#[derive(Serialize, Debug)]
pub struct ItemReported {
    pub site_code: String,
    /// Absent when the report named a warehouse and no shelf.
    pub location_id: Option<Uuid>,
    pub bin_code: Option<String>,
    /// Text, because `on_hand` is numeric and a quantity can be fractional.
    pub on_hand: String,
    pub available: Option<String>,
    pub status: Option<String>,
    /// When the report was taken, which is how old this is.
    pub as_at: DateTime<Utc>,
    /// Which feed it came from.
    pub source: String,
}

/// One item, with everything a person needs to find it and know it.
#[derive(Serialize, Debug)]
pub struct ItemView {
    pub item_id: Uuid,
    pub code: String,
    pub description: String,
    pub active: bool,
    pub style: Option<ItemStyleRef>,
    /// The front, own before its style's, and saying whose (D141).
    pub picture: Option<Picture>,
    pub measurements: Vec<ItemMeasurements>,
    pub packing: Option<ItemPacking>,
    /// This system's own record, in walking order.
    pub held: Vec<ItemHeld>,
    /// What NetSuite last reported, in walking order.
    pub reported: Vec<ItemReported>,
}

#[get("/items/{item_id}")]
pub async fn item_page(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let id = path.into_inner();
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let view = scope
        .run(|tx| {
            Box::pin(async move {
                let Some(r) = tx
                    .query_opt(
                        "SELECT i.code, i.description, i.active, s.code, s.description,
                                (SELECT count(*) FROM item v WHERE v.style_id = s.id)
                           FROM item i
                           LEFT JOIN item_style s ON s.id = i.style_id
                          WHERE i.id = $1",
                        &[&id],
                    )
                    .await?
                else {
                    return Err(ApiError::NotFound);
                };
                let style = r.get::<_, Option<String>>(3).map(|code| ItemStyleRef {
                    code,
                    description: r.get(4),
                    variants: r.get(5),
                });

                let picture = tx
                    .query_opt(
                        &format!(
                            "WITH {} SELECT digest, source FROM picture WHERE item_id = $1",
                            pictures::PICTURE_CTE
                        ),
                        &[&id],
                    )
                    .await?
                    .and_then(|p| pictures::from_row(p.get(0), p.get(1)));

                let measurements = measurements_of(tx, id).await?;

                // The newest case pack is the one in force, which is the rule
                // `receiving` reads it by.
                let packing = tx
                    .query_opt(
                        "SELECT units_per_inner, inners_per_carton, effective_from
                           FROM item_packing_config
                          WHERE item_id = $1 AND effective_from <= CURRENT_DATE
                          ORDER BY effective_from DESC, id DESC
                          LIMIT 1",
                        &[&id],
                    )
                    .await?
                    .map(|p| ItemPacking {
                        units_per_inner: p.get(0),
                        inners_per_carton: p.get(1),
                        effective_from: p.get(2),
                    });

                let held = tx
                    .query(
                        "SELECT si.code, l.id, l.code, sum(st.quantity)::bigint,
                                sum(st.allocated_quantity)::bigint
                           FROM stock st
                           LEFT JOIN location l ON l.id = st.resolved_location_id
                           LEFT JOIN site si ON si.id = st.site_id
                          WHERE st.item_id = $1 AND st.quantity <> 0
                          GROUP BY si.code, l.id, l.code, l.pick_sequence
                          ORDER BY si.code, l.pick_sequence NULLS LAST, l.code",
                        &[&id],
                    )
                    .await?
                    .iter()
                    .map(|h| ItemHeld {
                        site_code: h.get(0),
                        location_id: h.get(1),
                        bin_code: h.get(2),
                        quantity: h.get(3),
                        allocated_quantity: h.get(4),
                    })
                    .collect();

                let reported = tx
                    .query(
                        "SELECT si.code, l.id, l.code, rs.on_hand::text, rs.available::text,
                                rs.status, rs.as_at, rs.source
                           FROM reported_stock rs
                           JOIN site si ON si.id = rs.site_id
                           LEFT JOIN location l ON l.id = rs.location_id
                          WHERE rs.item_id = $1
                          ORDER BY si.code, l.pick_sequence NULLS LAST, l.code NULLS LAST",
                        &[&id],
                    )
                    .await?
                    .iter()
                    .map(|x| ItemReported {
                        site_code: x.get(0),
                        location_id: x.get(1),
                        bin_code: x.get(2),
                        on_hand: x.get(3),
                        available: x.get(4),
                        status: x.get(5),
                        as_at: x.get(6),
                        source: x.get(7),
                    })
                    .collect();

                Ok(ItemView {
                    item_id: id,
                    code: r.get(0),
                    description: r.get(1),
                    active: r.get(2),
                    style,
                    picture,
                    measurements,
                    packing,
                    held,
                    reported,
                })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(view))
}
