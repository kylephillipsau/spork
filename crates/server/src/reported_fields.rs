//! What each of NetSuite's fields means to Spork (D238).
//!
//! The item details carry every field the Bridge sends, as NetSuite said it
//! (`reported_item_field`). Which of them is a weight, a barcode at a level,
//! a pack count, a picture, something shown beside the code or only kept, is
//! Spork's word, said here and newest first, over defaults for the names this
//! NetSuite uses (`reported_field_meaning`). Saying it changes how what
//! NetSuite said is read, never what it said.

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_events::{self, NewClientEvent};
use crate::error::ApiError;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// What a field can be read as.
pub const ROLES: &[&str] = &[
    "kept",
    "shown",
    "warning",
    "note",
    "art_no",
    "picture",
    "weight",
    "length",
    "width",
    "height",
    "barcode",
    "per_carton",
    "per_inner",
    "inners_per_carton",
];

/// One of NetSuite's fields, as the item details have carried it.
#[derive(Serialize, Debug)]
pub struct ReportedField {
    pub source: String,
    pub field: String,
    /// Items NetSuite says it of now.
    pub items: i64,
    /// One value it says, to know it by.
    pub example: String,
    pub role: String,
    pub unit: Option<String>,
    pub unit_field: Option<String>,
    pub level: Option<String>,
    /// Said here, rather than read by its default.
    pub said: bool,
}

/// Every field the item details carry now, and what each is read as.
#[get("/netsuite-fields")]
pub async fn fields(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        "WITH f AS (SELECT source, field, count(*) AS items, min(value) AS example
                                      FROM reported_item_field WHERE said_to IS NULL
                                     GROUP BY source, field)
                         SELECT f.source, f.field, f.items, f.example,
                                m.role, m.unit, m.unit_field, m.level::text, m.said
                           FROM f CROSS JOIN LATERAL reported_field_meaning(f.source, f.field) m
                          ORDER BY f.field, f.source",
                        &[],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| ReportedField {
                        source: r.get(0),
                        field: r.get(1),
                        items: r.get(2),
                        example: r.get(3),
                        role: r.get(4),
                        unit: r.get(5),
                        unit_field: r.get(6),
                        level: r.get(7),
                        said: r.get(8),
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

#[derive(Deserialize, Debug)]
pub struct SayFieldRequest {
    pub source: String,
    pub field: String,
    pub role: String,
    /// A weight's or a size's unit: fixed, or the name of the field holding it.
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub unit_field: Option<String>,
    /// A barcode's level; none for the level NetSuite counts.
    #[serde(default)]
    pub level: Option<String>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

/// Say what one of NetSuite's fields means (D238).
#[post("/netsuite-fields")]
pub async fn say_field(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<SayFieldRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let body = body.into_inner();
    if !ROLES.contains(&body.role.as_str()) {
        return Err(ApiError::Rejected(format!("a field is read as one of {}, not {}", ROLES.join(", "), body.role)));
    }
    let measured = matches!(body.role.as_str(), "weight" | "length" | "width" | "height");
    if !measured && (body.unit.is_some() || body.unit_field.is_some()) {
        return Err(ApiError::Rejected("only a weight or a size has a unit".into()));
    }
    if body.level.is_some() && body.role != "barcode" {
        return Err(ApiError::Rejected("only a barcode has a level".into()));
    }
    if let Some(level) = &body.level {
        if !matches!(level.as_str(), "each" | "inner" | "carton") {
            return Err(ApiError::Rejected("a barcode is the each's, a pack's or a carton's".into()));
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
    scope
        .run(move |tx| {
            Box::pin(async move {
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    return Ok(());
                }
                tx.execute(
                    "INSERT INTO reported_field_said
                         (tenant_id, source, field, role, unit, unit_field, level, client_event_id, recorded_by_id)
                     VALUES ($1, $2, $3, $4, $5, $6, $7::text::packaging_level, $8, $9)",
                    &[
                        &ev.tenant_id,
                        &body.source,
                        &body.field,
                        &body.role,
                        &body.unit,
                        &body.unit_field,
                        &body.level,
                        &ev.client_event_id,
                        &ev.recorded_by_id,
                    ],
                )
                .await?;
                Ok(())
            })
        })
        .await?;
    Ok(HttpResponse::NoContent().finish())
}
