//! Lists of items to work through. D179.
//!
//! Work arrives as a list somebody else drew up, a printed sheet of items to
//! weigh and measure, say, in bin order with boxes to fill in. The item list
//! narrows itself to what needs measuring here, which is every item this
//! system has not measured, and not the thirty on the sheet. So the sheet is
//! kept: its codes, in its order, under its name, at the site it is worked at.
//!
//! A list is a filter on the item list (`GET /items?list=`), so it is worked
//! with everything the item list already does: walking order, what each item
//! still needs, and the item open beside it with Previous and Next.

use std::collections::{HashMap, HashSet};

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_events::{self, NewClientEvent};
use crate::error::ApiError;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// The longest list taken in one act: several sheets, not a catalogue.
const LONGEST: usize = 500;

#[derive(Deserialize, Debug)]
pub struct MakeListRequest {
    pub name: String,
    /// Item codes, in the order on the paper. A code given twice is listed
    /// once, at its first place.
    pub codes: Vec<String>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// A list, and how many items are on it.
#[derive(Serialize, Debug)]
pub struct ItemListRow {
    pub item_list_id: Uuid,
    pub name: String,
    pub items: i64,
    pub recorded_at: DateTime<Utc>,
    pub recorded_by_name: Option<String>,
}

/// The codes as given: trimmed, blanks dropped, each once, in order.
pub fn codes_in_order(codes: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    codes
        .iter()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty() && seen.insert(c.clone()))
        .collect()
}

/// Make a list from item codes. Every code has to be an item: a list with a
/// code quietly dropped is a sheet with a row missing, so a code nobody knows
/// refuses the list and says which.
#[post("/item-lists")]
pub async fn make_list(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<MakeListRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let body = body.into_inner();
    let name = body.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(ApiError::Rejected("a list needs a name, up to 120 characters".into()));
    }
    let codes = codes_in_order(&body.codes);
    if codes.is_empty() {
        return Err(ApiError::Rejected("a list needs at least one item code".into()));
    }
    if codes.len() > LONGEST {
        return Err(ApiError::Rejected(format!("a list takes up to {LONGEST} items, not {}", codes.len())));
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
                // Exactly as written first; then, for a code nobody wrote that
                // way, ignoring case where that names one item and no other.
                let rows = tx
                    .query(
                        "SELECT id, code FROM item
                          WHERE code = ANY($1) OR lower(code) = ANY($2)",
                        &[&codes, &codes.iter().map(|c| c.to_lowercase()).collect::<Vec<_>>()],
                    )
                    .await?;
                let exact: HashMap<String, Uuid> = rows.iter().map(|r| (r.get(1), r.get(0))).collect();
                let mut folded: HashMap<String, Vec<Uuid>> = HashMap::new();
                for r in &rows {
                    folded.entry(r.get::<_, String>(1).to_lowercase()).or_default().push(r.get(0));
                }
                let mut unknown = vec![];
                let mut items: Vec<Uuid> = vec![];
                for code in &codes {
                    let found = exact.get(code).copied().or_else(|| match folded.get(&code.to_lowercase()) {
                        Some(ids) if ids.len() == 1 => Some(ids[0]),
                        _ => None,
                    });
                    match found {
                        Some(id) if !items.contains(&id) => items.push(id),
                        Some(_) => {}
                        None => unknown.push(code.clone()),
                    }
                }
                if !unknown.is_empty() {
                    return Err(ApiError::Rejected(format!(
                        "no item has the code {}",
                        unknown.join(", ")
                    )));
                }

                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    let prior = tx
                        .query_opt(
                            "SELECT l.id, l.name, l.recorded_at,
                                    (SELECT count(*) FROM item_list_entry e WHERE e.item_list_id = l.id)
                               FROM item_list l WHERE l.client_event_id = $1",
                            &[&ev.client_event_id],
                        )
                        .await?
                        .ok_or_else(|| {
                            ApiError::Rejected("client_event exists but no list was made; incomplete act".into())
                        })?;
                    return Ok(ItemListRow {
                        item_list_id: prior.get(0),
                        name: prior.get(1),
                        recorded_at: prior.get(2),
                        items: prior.get(3),
                        recorded_by_name: None,
                    });
                }

                let list = tx
                    .query_one(
                        "INSERT INTO item_list
                             (tenant_id, site_id, name, client_event_id, recorded_by_id)
                         VALUES ($1, $2, $3, $4, $5)
                         RETURNING id, recorded_at",
                        &[&ev.tenant_id, &ev.site_id, &name, &ev.client_event_id, &ev.recorded_by_id],
                    )
                    .await?;
                let list_id: Uuid = list.get(0);
                tx.execute(
                    "INSERT INTO item_list_entry (item_list_id, tenant_id, item_id, position)
                     SELECT $1, $2, item_id, position::int
                       FROM unnest($3::uuid[]) WITH ORDINALITY AS t(item_id, position)",
                    &[&list_id, &ev.tenant_id, &items],
                )
                .await?;
                Ok(ItemListRow {
                    item_list_id: list_id,
                    name,
                    items: items.len() as i64,
                    recorded_at: list.get(1),
                    recorded_by_name: None,
                })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

/// The lists worked at the caller's site, newest first. A list made at no
/// site is shown everywhere, and a caller at no site sees every list.
#[get("/item-lists")]
pub async fn lists(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let site = who.site_id;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        "SELECT l.id, l.name, l.recorded_at, p.display_name,
                                (SELECT count(*) FROM item_list_entry e WHERE e.item_list_id = l.id)
                           FROM item_list l
                           LEFT JOIN person p ON p.id = l.recorded_by_id
                          WHERE $1::uuid IS NULL OR l.site_id IS NULL OR l.site_id = $1
                          ORDER BY l.recorded_at DESC, l.id DESC",
                        &[&site],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| ItemListRow {
                        item_list_id: r.get(0),
                        name: r.get(1),
                        recorded_at: r.get(2),
                        recorded_by_name: r.get(3),
                        items: r.get(4),
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_trimmed_and_listed_once_at_their_first_place() {
        let given: Vec<String> = [" ABC-1234 ", "", "XYZ-9", "ABC-1234", "  ", "QRS-5"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(codes_in_order(&given), ["ABC-1234", "XYZ-9", "QRS-5"]);
    }
}
