//! What a carton of an item holds, said at the item. D178.
//!
//! An item is modelled as itself, its `each`, and its carton as a box of so
//! many of it. The relationship is `item_packing_config`: the carton, and how
//! many of the item are in one. A carton is only a definite thing to measure
//! relative to one (D23), and until this only the prepack loader wrote them,
//! so an item the prepack list never named could not have its carton weighed.
//!
//! # One act, three outcomes
//!
//! - **No carton on file**: one is made, holding what was said, or holding an
//!   unsaid count when nothing was, which is the loader's "there is a carton
//!   and nobody has said what is in it".
//! - **A carton whose count was never said**: the count is filled in. It is
//!   the same carton, now described, and what was measured of it stays its own.
//! - **A carton that holds a different count**: that is a different carton
//!   (D23), so it is a new version from the day it was said. The old one still
//!   explains what was measured against it.
//! - **A count said wrongly, corrected** (D229): a carton of 1,000 boxes typed
//!   for one of 10. It is the same carton, described right, so the count in
//!   force is put right where it is, and what was measured of it stays its own.
//!
//! Saying what is already on file changes nothing, and says so.

use actix_web::{post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_events::{self, NewClientEvent};
use crate::error::ApiError;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// The most a carton may be said to hold: a sanity bound, not a rule of
/// packing. Hairnets come a thousand to a carton; nothing comes a million.
const MOST: i32 = 100_000;

#[derive(Deserialize, Debug)]
pub struct SayCartonRequest {
    /// How many of the item one carton holds. Absent: there is a carton, and
    /// how many it holds is not being said.
    pub holds: Option<i32>,
    /// When the carton holds packs: how many of the item are in each pack, and
    /// `holds` counts the packs (D185). Absent: `holds` counts the item.
    pub per: Option<i32>,
    /// The count in force was said wrongly (D229): put it right, rather than
    /// say a different carton from today.
    #[serde(default)]
    pub correction: bool,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// The carton in force after the act.
#[derive(Serialize, Debug)]
pub struct CartonSaid {
    pub item_packing_config_id: Uuid,
    pub units_per_inner: Option<i32>,
    pub inners_per_carton: Option<i32>,
    pub effective_from: NaiveDate,
    /// Whether the act changed anything. Saying what was on file does not.
    pub changed: bool,
}

/// How many of the item a case pack's carton holds, when it says.
#[cfg_attr(not(test), allow(dead_code))]
fn total(units_per_inner: Option<i32>, inners_per_carton: Option<i32>) -> Option<i64> {
    inners_per_carton.map(|n| i64::from(n) * i64::from(units_per_inner.unwrap_or(1)))
}

/// What saying a count does to the carton on file.
enum Change {
    Nothing,
    Make,
    /// The carton on file, its count said now: never said, or said wrongly.
    Amend(Uuid),
    Version,
}

/// The case pack in force on the day, by the rule the observation writer
/// names a carton by. `$2::timestamptz::date`, never `$2::date`: see
/// `observable_for`.
pub(crate) async fn in_force(
    tx: &tokio_postgres::Transaction<'_>,
    item_id: Uuid,
    at: DateTime<Utc>,
) -> Result<Option<tokio_postgres::Row>, tokio_postgres::Error> {
    tx.query_opt(
        "SELECT id, units_per_inner, inners_per_carton, effective_from
           FROM item_packing_config
          WHERE item_id = $1 AND effective_from <= $2::timestamptz::date
          ORDER BY effective_from DESC, id DESC
          LIMIT 1",
        &[&item_id, &at],
    )
    .await
}

fn said(r: &tokio_postgres::Row, changed: bool) -> CartonSaid {
    CartonSaid {
        item_packing_config_id: r.get(0),
        units_per_inner: r.get(1),
        inners_per_carton: r.get(2),
        effective_from: r.get(3),
        changed,
    }
}

/// The case pack's two counts for what is said: packs of so many, and so many
/// packs, or so many of the item (packs of one). `holds` absent: there is a
/// carton, and how many it holds is not being said; with `per`, what a pack
/// of it holds is, a pair's two (D233).
pub(crate) fn counts(holds: Option<i32>, per: Option<i32>) -> Result<(Option<i32>, Option<i32>), ApiError> {
    if let Some(n) = holds {
        if !(1..=MOST).contains(&n) {
            return Err(ApiError::Rejected(format!(
                "a carton holds from 1 to {MOST} of an item, not {n}"
            )));
        }
    }
    if let Some(p) = per {
        if !(1..=MOST).contains(&p) {
            return Err(ApiError::Rejected(format!("a pack holds from 1 to {MOST} of an item, not {p}")));
        }
    }
    Ok((per.or(holds.map(|_| 1)), holds))
}

/// Say what a carton of the item holds, as the act `ev`: `wanted` is the case
/// pack's two counts, the second absent when no count is being said. Every
/// writer of a case pack at the item says it here: the carton's own card,
/// matching a sibling (D228), and moving a carton measured as the product onto
/// its carton (D232).
///
/// A carton made where none was is in force from `from`: the day it is said,
/// or, said by a move, the day what moves was measured, since the carton was
/// there to be measured then. A different count is a new version from the day
/// of the act (D23).
pub(crate) async fn say(
    tx: &tokio_postgres::Transaction<'_>,
    ev: &NewClientEvent,
    item_id: Uuid,
    wanted: (Option<i32>, Option<i32>),
    correction: bool,
    from: DateTime<Utc>,
) -> Result<CartonSaid, ApiError> {
    let at = ev.submitted_at;
    let now = in_force(tx, item_id, at).await?;
    // What the act would do, decided before it is claimed: saying what is on
    // file is no act at all, and a retried press of one that changed something
    // finds it on file and answers the same. What a pack holds said alone
    // keeps the carton's count on file (D233).
    let mut wanted = wanted;
    let change = match &now {
        None => Change::Make,
        Some(c) => {
            let on_file: (Option<i32>, Option<i32>) = (c.get(1), c.get(2));
            if wanted.1.is_none() && wanted.0.is_some() {
                // A carton counted in packs of another size would hold
                // another number of the item: say what it holds as well.
                // Put right (D229), the carton still holds as many packs.
                if !correction && on_file.1.is_some() && on_file.0.is_some_and(|p| Some(p) != wanted.0) {
                    return Err(ApiError::Rejected(
                        "a carton of it holds packs of another count: say how many it holds as well".into(),
                    ));
                }
                wanted.1 = on_file.1;
            }
            match on_file {
                was if was == wanted => Change::Nothing,
                _ if wanted == (None, None) => Change::Nothing,
                // A count never said, now said: the same carton, described.
                (_, None) => Change::Amend(c.get(0)),
                // What a pack holds, never said, now said of the same carton.
                (None, n) if n == wanted.1 => Change::Amend(c.get(0)),
                _ if correction => Change::Amend(c.get(0)),
                _ => Change::Version,
            }
        }
    };
    if let (Change::Nothing, Some(c)) = (&change, &now) {
        return Ok(said(c, false));
    }
    if client_events::claim_act(tx, ev).await?.is_replay() {
        let row = in_force(tx, item_id, at).await?.ok_or_else(|| {
            ApiError::Rejected("client_event exists but no carton is on file; incomplete act".into())
        })?;
        return Ok(said(&row, false));
    }

    const RETURNING: &str = "RETURNING id, units_per_inner, inners_per_carton, effective_from";
    let row = match change {
        // The count, never said or said wrongly, now is: the same carton.
        Change::Amend(id) => {
            tx.query_one(
                &format!(
                    "UPDATE item_packing_config
                        SET units_per_inner = $2, inners_per_carton = $3,
                            client_event_id = $4, recorded_by_id = $5
                      WHERE id = $1 {RETURNING}"
                ),
                &[&id, &wanted.0, &wanted.1, &ev.client_event_id, &ev.recorded_by_id],
            )
            .await?
        }
        // No carton on file, in force from when it was there; or a different
        // one, from the day it is said (D23).
        Change::Make | Change::Version | Change::Nothing => {
            let effective = if matches!(change, Change::Make) { from.min(at) } else { at };
            tx.query_one(
                &format!(
                    "INSERT INTO item_packing_config
                         (tenant_id, item_id, units_per_inner, inners_per_carton,
                          effective_from, client_event_id, recorded_by_id)
                     VALUES ($1, $2, $3, $4, $5::timestamptz::date, $6, $7) {RETURNING}"
                ),
                &[&ev.tenant_id, &item_id, &wanted.0, &wanted.1, &effective, &ev.client_event_id, &ev.recorded_by_id],
            )
            .await?
        }
    };
    Ok(said(&row, true))
}

/// Say what a carton of this item holds.
#[post("/items/{id}/carton")]
pub async fn say_carton(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SayCartonRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item_id = path.into_inner();
    let body = body.into_inner();
    let wanted = counts(body.holds, body.per)?;
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
                say(tx, &ev, item_id, wanted, body.correction, body.occurred_at).await
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

/// A run of an item that looks different from the others (D182).
#[derive(Deserialize, Debug)]
pub struct AddLotRequest {
    /// What it is known by: the order number on its carton, say, and where it
    /// was made.
    pub code: String,
}

#[derive(Serialize, Debug)]
pub struct LotAdded {
    pub lot_id: Uuid,
    pub code: String,
    /// False when the item already had a run of that name: it is that one.
    pub added: bool,
}

/// Name a run of the item that looks different, so its own photographs and
/// figures can be taken (D182). A name the item already has is that run.
#[post("/items/{id}/lots")]
pub async fn add_lot(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<AddLotRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item_id = path.into_inner();
    let code = body.code.trim().to_string();
    if code.is_empty() || code.chars().count() > 80 {
        return Err(ApiError::Rejected("a run needs a name, up to 80 characters".into()));
    }
    let tenant = who.tenant_id;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                tx.query_opt("SELECT 1 FROM item WHERE id = $1", &[&item_id])
                    .await?
                    .ok_or(ApiError::NotFound)?;
                let made = tx
                    .query_opt(
                        "INSERT INTO lot (tenant_id, item_id, code) VALUES ($1, $2, $3)
                         ON CONFLICT (tenant_id, item_id, code) DO NOTHING
                         RETURNING id",
                        &[&tenant, &item_id, &code],
                    )
                    .await?;
                let (lot_id, added) = match made {
                    Some(r) => (r.get(0), true),
                    None => (
                        tx.query_one("SELECT id FROM lot WHERE item_id = $1 AND code = $2", &[&item_id, &code])
                            .await?
                            .get(0),
                        false,
                    ),
                };
                Ok(LotAdded { lot_id, code, added })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

#[derive(Deserialize, Debug)]
pub struct DefaultLotRequest {
    /// The variant that stands for the item's carton; absent for none.
    pub lot_id: Option<Uuid>,
}

/// Say which variant is the item's carton, or that none is (D184).
#[post("/items/{id}/default-lot")]
pub async fn set_default_lot(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<DefaultLotRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item_id = path.into_inner();
    let lot = body.lot_id;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                tx.query_opt("SELECT 1 FROM item WHERE id = $1", &[&item_id])
                    .await?
                    .ok_or(ApiError::NotFound)?;
                if let Some(l) = lot {
                    tx.query_opt("SELECT 1 FROM lot WHERE id = $1 AND item_id = $2", &[&l, &item_id])
                        .await?
                        .ok_or_else(|| ApiError::Rejected("that is not a variant of this item".into()))?;
                }
                tx.execute("UPDATE item SET default_lot_id = $2 WHERE id = $1", &[&item_id, &lot]).await?;
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize, Debug)]
pub struct FamilyPictureRequest {
    /// True: this item pictures its family. False: none does.
    pub pictures: bool,
}

/// Say this item's picture stands for its family, or that none does (D188).
#[post("/items/{id}/family-picture")]
pub async fn set_family_picture(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<FamilyPictureRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item_id = path.into_inner();
    let pictures = body.pictures;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let style: Option<Uuid> = tx
                    .query_opt("SELECT style_id FROM item WHERE id = $1", &[&item_id])
                    .await?
                    .ok_or(ApiError::NotFound)?
                    .get(0);
                let style = style.ok_or_else(|| ApiError::Rejected("it is not part of a family".into()))?;
                let chosen = pictures.then_some(item_id);
                tx.execute("UPDATE item_style SET picture_item_id = $2 WHERE id = $1", &[&style, &chosen]).await?;
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_carton_holds_its_inners_times_what_is_in_each() {
        assert_eq!(total(Some(1), Some(16)), Some(16));
        assert_eq!(total(Some(50), Some(6)), Some(300));
        // Inner packs counted, their contents not: the packs are what it holds.
        assert_eq!(total(None, Some(6)), Some(6));
        // Never said.
        assert_eq!(total(None, None), None);
        assert_eq!(total(Some(50), None), None);
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct SayUnitRequest {
    /// `each`, `inner` or `carton`: which level of the item is one in NetSuite.
    pub level: String,
    /// How many of it: two of the each for a pair (D233). Absent, one.
    #[serde(default)]
    pub quantity: Option<i32>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// Say which level of an item is one in NetSuite (D218): what it is sold as.
/// The newest saying wins over NetSuite's Pack Unit; saying it again is the
/// same act.
#[post("/items/{id}/unit")]
pub async fn say_unit(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SayUnitRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item_id = path.into_inner();
    let body = body.into_inner();
    if !matches!(body.level.as_str(), "each" | "inner" | "carton") {
        return Err(ApiError::Rejected("an item is sold as its each, its pack or its carton".into()));
    }
    let quantity = body.quantity.unwrap_or(1);
    if quantity != 1 && !(body.level == "each" && (2..=1000).contains(&quantity)) {
        return Err(ApiError::Rejected("several of an item are said of its each: two for a pair".into()));
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
                tx.query_opt("SELECT 1 FROM item WHERE id = $1", &[&item_id]).await?.ok_or(ApiError::NotFound)?;
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    return Ok(());
                }
                tx.execute(
                    "INSERT INTO item_unit (tenant_id, item_id, level, quantity, client_event_id, recorded_by_id)
                     VALUES ($1, $2, $3::text::packaging_level, $4, $5, $6)",
                    &[&ev.tenant_id, &item_id, &body.level, &quantity, &ev.client_event_id, &ev.recorded_by_id],
                )
                .await?;
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}
