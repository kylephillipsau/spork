//! What this deployment is, and which warehouses are in it.
//!
//! A read, so it owns its query and its shaping together (D160). The handler is
//! here rather than in `routes.rs` for the same reason `picking::record`'s is:
//! new work goes where it belongs rather than where the old work happens to be.
//!
//! # Counts, and why they are on this screen
//!
//! A site with no bins looks identical to a site with eight thousand until
//! something counts them, and the difference decides whether a capture worklist
//! has anything in it. The bin import creates a site the moment a warehouse
//! appears in the export, so "Perth, 0 bins" is a real and expected state that
//! ought to be legible rather than surprising.
//!
//! `pick_sequence` is counted separately because a bin without one is a bin no
//! walk can order. Melbourne has 2,190 of 2,190; Brisbane has 2,780 of 2,923.

use serde::Serialize;
use uuid::Uuid;

use actix_web::{get, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};

use crate::error::ApiError;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

#[derive(Serialize, Debug)]
pub struct Organisation {
    pub id: Uuid,
    pub name: String,
    /// The URL-safe form, fixed at setup. Renaming it would move addresses.
    pub slug: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct WorkspaceSite {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    /// Decides when a day starts for despatch and counting.
    pub timezone: String,
    pub active: bool,
    /// Bins on file here.
    pub locations: i64,
    /// Of those, how many carry a walking position.
    pub sequenced: i64,
    /// True for the one this session is working at.
    pub current: bool,
}

#[derive(Serialize, Debug)]
pub struct Workspace {
    pub organisation: Organisation,
    pub sites: Vec<WorkspaceSite>,
}

/// The organisation and its warehouses.
#[get("/workspace")]
pub async fn workspace(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let at = who.site_id;

    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                // `tenant` carries no `tenant_id` of its own — it *is* the
                // tenant — so this is the one read here that names an id
                // instead of leaning on the policy.
                let t = tx
                    .query_one(
                        "SELECT id, name, slug, active, created_at
                           FROM tenant WHERE id = $1",
                        &[&who.tenant_id],
                    )
                    .await?;

                // A LEFT JOIN rather than two queries: a site with no bins must
                // come back saying nought, and an inner join would drop it —
                // which is exactly the site somebody is looking for when they
                // open this screen.
                let rows = tx
                    .query(
                        "SELECT s.id, s.code, s.name, s.timezone, s.active,
                                count(l.id),
                                count(l.id) FILTER (WHERE l.pick_sequence IS NOT NULL)
                           FROM site s
                           LEFT JOIN location l
                             ON l.site_id = s.id AND l.active
                          GROUP BY s.id, s.code, s.name, s.timezone, s.active
                          ORDER BY s.code",
                        &[],
                    )
                    .await?;

                Ok(Workspace {
                    organisation: Organisation {
                        id: t.get(0),
                        name: t.get(1),
                        slug: t.get(2),
                        active: t.get(3),
                        created_at: t.get(4),
                    },
                    sites: rows
                        .iter()
                        .map(|r| {
                            let id: Uuid = r.get(0);
                            WorkspaceSite {
                                id,
                                code: r.get(1),
                                name: r.get(2),
                                timezone: r.get(3),
                                active: r.get(4),
                                locations: r.get(5),
                                sequenced: r.get(6),
                                current: at == Some(id),
                            }
                        })
                        .collect(),
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(out))
}
