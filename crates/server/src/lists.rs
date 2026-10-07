//! Lists of items to work through. D179, D235.
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
//!
//! **Worked as the layout is** (D235): a list is renamed, has codes added to
//! it or items taken off it, and is put away, each change made in place and
//! kept beside it as it was and as it became (`item_list_change`), under the
//! act that made it. Put away, it is gone from every picker and kept.

use std::collections::{HashMap, HashSet};

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
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

/// A list, how many items are on it, and how many of those are done (D235).
#[derive(Serialize, Debug)]
pub struct ItemListRow {
    pub item_list_id: Uuid,
    pub name: String,
    pub items: i64,
    /// Items whose unit is weighed and measured (D219): the sheet's boxes filled.
    pub done: i64,
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

/// The codes, trimmed and once each, within a list's bounds.
fn checked(codes: &[String]) -> Result<Vec<String>, ApiError> {
    let codes = codes_in_order(codes);
    if codes.is_empty() {
        return Err(ApiError::Rejected("a list needs at least one item code".into()));
    }
    if codes.len() > LONGEST {
        return Err(ApiError::Rejected(format!("a list takes up to {LONGEST} items, not {}", codes.len())));
    }
    Ok(codes)
}

/// A list's name, trimmed, within its bounds.
fn named(name: &str) -> Result<String, ApiError> {
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(ApiError::Rejected("a list needs a name, up to 120 characters".into()));
    }
    Ok(name)
}

/// The items the codes name, in their order, each once. Every code has to be
/// an item: a list with a code quietly dropped is a sheet with a row missing,
/// so a code nobody knows refuses and is named. A code is matched as written
/// first; then, for one nobody wrote that way, ignoring case where that names
/// one item and no other.
async fn items_named(tx: &tokio_postgres::Transaction<'_>, codes: &[String]) -> Result<Vec<Uuid>, ApiError> {
    let rows = tx
        .query(
            "SELECT id, code FROM item WHERE code = ANY($1) OR lower(code) = ANY($2)",
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
    for code in codes {
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
        return Err(ApiError::Rejected(format!("no item has the code {}", unknown.join(", "))));
    }
    Ok(items)
}

/// Lists as rows: `$1` the caller's site (a list made at no site is shown
/// everywhere, and a caller at no site sees every list), `$2` one list or
/// any. Those not put away, with work left first, then newest first.
fn rows_sql() -> String {
    let weighed = crate::items::unit_measured("'gross_weight'");
    let sized = crate::items::unit_measured("'length', 'width', 'height'");
    format!(
        "WITH unit AS MATERIALIZED (SELECT item_id, level::text AS level FROM item_unit_level),
         c AS (
             SELECT e.item_list_id, i.id, i.style_id, coalesce(u.level, 'each') AS unit
               FROM item_list_entry e
               JOIN item_list l ON l.id = e.item_list_id
               JOIN item i ON i.id = e.item_id
               LEFT JOIN unit u ON u.item_id = i.id
              WHERE l.removed_at IS NULL AND ($2::uuid IS NULL OR l.id = $2)
         ),
         progress AS (
             SELECT c.item_list_id, count(*) AS items, count(*) FILTER (WHERE {weighed} AND {sized}) AS done
               FROM c GROUP BY 1
         )
         SELECT l.id, l.name, l.recorded_at, p.display_name,
                coalesce(g.items, 0), coalesce(g.done, 0)
           FROM item_list l
           LEFT JOIN person p ON p.id = l.recorded_by_id
           LEFT JOIN progress g ON g.item_list_id = l.id
          WHERE l.removed_at IS NULL
            AND ($1::uuid IS NULL OR l.site_id IS NULL OR l.site_id = $1)
            AND ($2::uuid IS NULL OR l.id = $2)
          ORDER BY coalesce(g.done, 0) < coalesce(g.items, 0) DESC, l.recorded_at DESC, l.id DESC"
    )
}

fn row_of(r: &tokio_postgres::Row) -> ItemListRow {
    ItemListRow {
        item_list_id: r.get(0),
        name: r.get(1),
        recorded_at: r.get(2),
        recorded_by_name: r.get(3),
        items: r.get(4),
        done: r.get(5),
    }
}

/// One list as a row, wherever it is worked; refused when put away.
async fn one(tx: &tokio_postgres::Transaction<'_>, list: Uuid) -> Result<ItemListRow, ApiError> {
    let none: Option<Uuid> = None;
    let row = tx.query_opt(&rows_sql(), &[&none, &list]).await?.ok_or(ApiError::NotFound)?;
    Ok(row_of(&row))
}

/// Make a list from item codes.
#[post("/item-lists")]
pub async fn make_list(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<MakeListRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let body = body.into_inner();
    let name = named(&body.name)?;
    let codes = checked(&body.codes)?;
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
                let items = items_named(tx, &codes).await?;
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    let prior: Uuid = tx
                        .query_opt("SELECT id FROM item_list WHERE client_event_id = $1", &[&ev.client_event_id])
                        .await?
                        .ok_or_else(|| {
                            ApiError::Rejected("client_event exists but no list was made; incomplete act".into())
                        })?
                        .get(0);
                    return one(tx, prior).await;
                }
                let list: Uuid = tx
                    .query_one(
                        "INSERT INTO item_list
                             (tenant_id, site_id, name, client_event_id, recorded_by_id)
                         VALUES ($1, $2, $3, $4, $5)
                         RETURNING id",
                        &[&ev.tenant_id, &ev.site_id, &name, &ev.client_event_id, &ev.recorded_by_id],
                    )
                    .await?
                    .get(0);
                put_on(tx, &ev, list, &items).await?;
                one(tx, list).await
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

/// Put items on a list after those on it, in their order; those on it already
/// stay where they are. The places they took.
async fn put_on(
    tx: &tokio_postgres::Transaction<'_>,
    ev: &NewClientEvent,
    list: Uuid,
    items: &[Uuid],
) -> Result<Vec<(Uuid, i32)>, ApiError> {
    let rows = tx
        .query(
            "INSERT INTO item_list_entry (item_list_id, tenant_id, item_id, position)
             SELECT $1, $2, t.item_id,
                    (SELECT coalesce(max(position), 0) FROM item_list_entry WHERE item_list_id = $1)
                    + row_number() OVER (ORDER BY t.ordinality)::int
               FROM unnest($3::uuid[]) WITH ORDINALITY AS t(item_id, ordinality)
              WHERE NOT EXISTS (SELECT 1 FROM item_list_entry e
                                 WHERE e.item_list_id = $1 AND e.item_id = t.item_id)
             RETURNING item_id, position",
            &[&list, &ev.tenant_id, &items],
        )
        .await?;
    Ok(rows.iter().map(|r| (r.get(0), r.get(1))).collect())
}

/// The lists worked at the caller's site, those with work left first.
#[get("/item-lists")]
pub async fn lists(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let site = who.site_id;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let any: Option<Uuid> = None;
                let rows = tx.query(&rows_sql(), &[&site, &any]).await?;
                Ok(rows.iter().map(row_of).collect::<Vec<_>>())
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

/// What is done to a list: its new name, codes to add, items to take off, or
/// put away. One of them a request.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ListChange {
    Rename { name: String },
    Add { codes: Vec<String> },
    TakeOff { item_ids: Vec<Uuid> },
    Remove,
}

#[derive(Deserialize, Debug)]
pub struct ChangeListRequest {
    pub change: ListChange,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// Change a list as it is now, and keep the change (D235): renamed, added to,
/// taken from, or put away. The list afterwards; put away, nothing.
///
/// Its items keep their places on the sheet: one taken off leaves its number,
/// as a row struck through on the paper does, and those added come after the
/// last. A change that changes nothing is no act.
#[post("/item-lists/{id}/changes")]
pub async fn change_list(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<ChangeListRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let list = path.into_inner();
    let body = body.into_inner();
    // Checked before anything is read: what was asked is in bounds.
    let change = match body.change {
        ListChange::Rename { name } => ListChange::Rename { name: named(&name)? },
        ListChange::Add { codes } => ListChange::Add { codes: checked(&codes)? },
        ListChange::TakeOff { item_ids } if item_ids.is_empty() => {
            return Err(ApiError::Rejected("say which items to take off the list".into()))
        }
        other => other,
    };
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
                let was: String = tx
                    .query_opt("SELECT name FROM item_list WHERE id = $1 AND removed_at IS NULL", &[&list])
                    .await?
                    .ok_or(ApiError::NotFound)?
                    .get(0);
                let added = match &change {
                    ListChange::Add { codes } => items_named(tx, codes).await?,
                    _ => vec![],
                };
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    return Ok(one(tx, list).await.ok());
                }
                let (kind, before, after) = match &change {
                    ListChange::Rename { name } if *name == was => return Ok(one(tx, list).await.ok()),
                    ListChange::Rename { name } => {
                        tx.execute("UPDATE item_list SET name = $2 WHERE id = $1", &[&list, &name]).await?;
                        ("renamed", Some(json!({ "name": was })), Some(json!({ "name": name })))
                    }
                    ListChange::Add { .. } => {
                        let put = put_on(tx, &ev, list, &added).await?;
                        if put.is_empty() {
                            return Err(ApiError::Rejected("every one of those is on the list already".into()));
                        }
                        let put: Vec<_> = put.iter().map(|(i, at)| json!({ "item_id": i, "position": at })).collect();
                        ("added", None, Some(json!({ "items": put })))
                    }
                    ListChange::TakeOff { item_ids } => {
                        let gone = tx
                            .query(
                                "DELETE FROM item_list_entry WHERE item_list_id = $1 AND item_id = ANY($2)
                                 RETURNING item_id, position",
                                &[&list, &item_ids],
                            )
                            .await?;
                        if gone.is_empty() {
                            return Err(ApiError::Rejected("none of those is on the list".into()));
                        }
                        let gone: Vec<_> = gone
                            .iter()
                            .map(|r| json!({ "item_id": r.get::<_, Uuid>(0), "position": r.get::<_, i32>(1) }))
                            .collect();
                        ("taken_off", Some(json!({ "items": gone })), None)
                    }
                    ListChange::Remove => {
                        tx.execute("UPDATE item_list SET removed_at = $2 WHERE id = $1", &[&list, &ev.submitted_at])
                            .await?;
                        ("removed", Some(json!({ "name": was })), None)
                    }
                };
                tx.execute(
                    "INSERT INTO item_list_change
                         (tenant_id, item_list_id, client_event_id, recorded_by_id, change, before, after)
                     VALUES ($1, $2, $3, $4, $5, $6::text::jsonb, $7::text::jsonb)",
                    &[
                        &ev.tenant_id,
                        &list,
                        &ev.client_event_id,
                        &ev.recorded_by_id,
                        &kind,
                        &before.map(|v| v.to_string()),
                        &after.map(|v| v.to_string()),
                    ],
                )
                .await?;
                Ok(one(tx, list).await.ok())
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
