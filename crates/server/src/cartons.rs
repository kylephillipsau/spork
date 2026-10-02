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
fn total(units_per_inner: Option<i32>, inners_per_carton: Option<i32>) -> Option<i64> {
    inners_per_carton.map(|n| i64::from(n) * i64::from(units_per_inner.unwrap_or(1)))
}

/// What saying a count does to the carton on file.
enum Change {
    Nothing,
    Make,
    FillIn(Uuid, i32),
    Version(i32),
}

/// The case pack in force on the day, by the rule the observation writer
/// names a carton by. `$2::timestamptz::date`, never `$2::date`: see
/// `observable_for`.
async fn in_force(
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
    if let Some(n) = body.holds {
        if !(1..=MOST).contains(&n) {
            return Err(ApiError::Rejected(format!(
                "a carton holds from 1 to {MOST} of an item, not {n}"
            )));
        }
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
                tx.query_opt("SELECT 1 FROM item WHERE id = $1", &[&item_id])
                    .await?
                    .ok_or(ApiError::NotFound)?;

                let now = in_force(tx, item_id, body.occurred_at).await?;
                // What the act would do, decided before it is claimed: saying
                // what is on file is no act at all, and a retried press of one
                // that changed something finds it on file and answers the same.
                let change = match &now {
                    None => Change::Make,
                    Some(c) => match (total(c.get(1), c.get(2)), body.holds) {
                        (_, None) => Change::Nothing,
                        (Some(was), Some(n)) if was == i64::from(n) => Change::Nothing,
                        (None, Some(n)) => Change::FillIn(c.get(0), n),
                        (Some(_), Some(n)) => Change::Version(n),
                    },
                };
                if let (Change::Nothing, Some(c)) = (&change, &now) {
                    return Ok(said(c, false));
                }
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    let row = in_force(tx, item_id, body.occurred_at).await?.ok_or_else(|| {
                        ApiError::Rejected("client_event exists but no carton is on file; incomplete act".into())
                    })?;
                    return Ok(said(&row, false));
                }

                const RETURNING: &str = "RETURNING id, units_per_inner, inners_per_carton, effective_from";
                let row = match change {
                    // The count, never said, now is: the same carton.
                    Change::FillIn(id, n) => {
                        tx.query_one(
                            &format!(
                                "UPDATE item_packing_config
                                    SET units_per_inner = 1, inners_per_carton = $2,
                                        client_event_id = $3, recorded_by_id = $4
                                  WHERE id = $1 {RETURNING}"
                            ),
                            &[&id, &n, &ev.client_event_id, &ev.recorded_by_id],
                        )
                        .await?
                    }
                    // No carton on file, or a different one from today (D23).
                    Change::Make | Change::Version(_) | Change::Nothing => {
                        let holds = match change {
                            Change::Version(n) => Some(n),
                            _ => body.holds,
                        };
                        tx.query_one(
                            &format!(
                                "INSERT INTO item_packing_config
                                     (tenant_id, item_id, units_per_inner, inners_per_carton,
                                      effective_from, client_event_id, recorded_by_id)
                                 VALUES ($1, $2, $3, $4, $5::timestamptz::date, $6, $7) {RETURNING}"
                            ),
                            &[
                                &ev.tenant_id,
                                &item_id,
                                &holds.map(|_| 1i32),
                                &holds,
                                &body.occurred_at,
                                &ev.client_event_id,
                                &ev.recorded_by_id,
                            ],
                        )
                        .await?
                    }
                };
                Ok(said(&row, true))
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
