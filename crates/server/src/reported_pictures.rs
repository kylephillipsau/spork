//! NetSuite's picture of an item, carried to Spork (D237).
//!
//! The item details name each item's picture file in NetSuite: the value of
//! the field read as its picture (D238, `reported_item_said`), NetSuite's
//! Transaction Image. The bytes come after, a few at a time:
//! Spork says which pictures named it doesn't hold yet, and the bridge brings
//! each one. So the feed of details stays one small file, a picture travels
//! once, and a new file named for an item is a new picture to bring.
//!
//! **NetSuite's, and only NetSuite's.** The picture is kept as its report
//! (`reported_item_picture`), beside Spork's photographs and never among them:
//! not a look, not a face to cut. Which picture an item shows, and whether
//! NetSuite's is the product at all, is Spork's word (`item_picture_said`).

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::images;
use crate::routes::machine;
use crate::tenancy::TenantScope;
use crate::AppState;

/// The most pictures asked for at once: a sync's worth, not the catalogue.
const MOST: i64 = 200;

#[derive(Deserialize, Debug)]
pub struct WantedQuery {
    /// The feed the item details came by.
    pub source: String,
    pub limit: Option<i64>,
}

/// A picture the item details name that Spork doesn't hold.
#[derive(Serialize, Debug)]
pub struct WantedPicture {
    /// The item, by its code, as the feed names it.
    pub item: String,
    /// NetSuite's reference to the picture's file, as the feed gave it.
    pub file: String,
}

/// The pictures the feed's item details name and Spork doesn't hold yet,
/// most wanted first: those of items in stock, then by code.
#[get("/import/item-pictures/wanted")]
pub async fn wanted(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<WantedQuery>,
) -> Result<HttpResponse, ApiError> {
    let caller = machine(&state, &req).await?;
    let source = query.source.clone();
    let limit = query.limit.unwrap_or(50).clamp(1, MOST);
    let mut scope = TenantScope::begin(&state.pool, caller.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        "SELECT DISTINCT ON (i.code) i.code, r.value,
                                NOT EXISTS (SELECT 1 FROM reported_stock s
                                             WHERE s.item_id = r.item_id AND s.on_hand > 0) AS out_of_stock
                           FROM reported_item_said r
                           JOIN item i ON i.id = r.item_id
                          WHERE r.source = $1 AND r.role = 'picture'
                            AND NOT EXISTS (SELECT 1 FROM reported_item_picture p
                                             WHERE p.item_id = r.item_id AND p.source = r.source
                                               AND p.file = r.value)
                          ORDER BY i.code, r.field",
                        &[&source],
                    )
                    .await?;
                let mut rows: Vec<_> = rows.iter().map(|r| (r.get::<_, bool>(2), r.get::<_, String>(0), r.get::<_, String>(1))).collect();
                rows.sort();
                rows.truncate(limit as usize);
                Ok(rows
                    .into_iter()
                    .map(|(_, item, file)| WantedPicture { item, file })
                    .collect::<Vec<_>>())
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

#[derive(Deserialize, Debug)]
pub struct PictureQuery {
    pub source: String,
    /// The item, by its code, and the file, as `wanted` gave them.
    pub item: String,
    pub file: String,
}

#[derive(Serialize, Debug)]
pub struct PictureLoaded {
    pub digest: String,
    pub byte_count: i64,
    /// False when Spork held this picture for the item already.
    pub changed: bool,
}

/// Keep NetSuite's picture of an item: the bytes of the file its item details
/// name. Only the file named is taken, so the picture is always the one the
/// details say; a file they no longer name is refused, and the next `wanted`
/// asks for the one they do.
#[post("/import/item-pictures")]
pub async fn load(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<PictureQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let caller = machine(&state, &req).await?;
    let query = query.into_inner();
    let stored = images::store(&body).await?;
    let mut scope = TenantScope::begin(&state.pool, caller.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let named = tx
                    .query_opt(
                        "SELECT r.item_id FROM reported_item_said r JOIN item i ON i.id = r.item_id
                          WHERE i.code = $1 AND r.source = $2 AND r.role = 'picture' AND r.value = $3
                          LIMIT 1",
                        &[&query.item, &query.source, &query.file],
                    )
                    .await?;
                let Some(named) = named else {
                    return Err(ApiError::Rejected(format!(
                        "the item details from {} don't name file {} as {}'s picture",
                        query.source, query.file, query.item
                    )));
                };
                let item: uuid::Uuid = named.get(0);
                let changed = tx
                    .execute(
                        "INSERT INTO reported_item_picture
                             (tenant_id, item_id, source, file, digest, mime, byte_count, width_px, height_px)
                         VALUES (current_tenant(), $1, $2, $3, $4, $5, $6, $7, $8)
                         ON CONFLICT (tenant_id, item_id, source) DO UPDATE
                            SET file = excluded.file, digest = excluded.digest, mime = excluded.mime,
                                byte_count = excluded.byte_count, width_px = excluded.width_px,
                                height_px = excluded.height_px, loaded_at = now()
                          WHERE (reported_item_picture.file, reported_item_picture.digest)
                                IS DISTINCT FROM (excluded.file, excluded.digest)",
                        &[
                            &item,
                            &query.source,
                            &query.file,
                            &stored.digest,
                            &stored.mime,
                            &stored.byte_count,
                            &stored.width_px,
                            &stored.height_px,
                        ],
                    )
                    .await?
                    > 0;
                Ok(PictureLoaded { digest: stored.digest.clone(), byte_count: stored.byte_count, changed })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}
