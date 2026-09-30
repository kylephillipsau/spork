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

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use actix_web::{get, post, web, HttpRequest, HttpResponse};
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
    /// The code of the location it packs at (migration 97), when it has said.
    pub pack_location: Option<String>,
    /// Whose goods it holds, by name, when it has said (migration 95).
    pub owner: Option<String>,
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
                                count(l.id) FILTER (WHERE l.pick_sequence IS NOT NULL),
                                pl.code, o.name
                           FROM site s
                           LEFT JOIN location l
                             ON l.site_id = s.id AND l.active
                           LEFT JOIN location pl ON pl.id = s.pack_location_id
                           LEFT JOIN party o ON o.id = s.owner_party_id
                          GROUP BY s.id, s.code, s.name, s.timezone, s.active, pl.code, o.name
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
                                pack_location: r.get(7),
                                owner: r.get(8),
                            }
                        })
                        .collect(),
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(out))
}

/// Where a site packs, by the code of a location there.
#[derive(Deserialize, Debug)]
pub struct SetPackLocation {
    pub code: String,
}

/// What setting it did.
#[derive(Serialize, Debug)]
pub struct PackLocationSet {
    pub code: String,
    /// True when no location had that code, and one was made.
    pub created: bool,
}

/// Say where a site packs (migration 97).
///
/// **A code the site has is used, and one it has not is made.** The packing
/// station is usually not in the bin list somebody imported, because the other
/// system never stored stock there: it is a bench, not a shelf. So the person
/// names it, and the location is created as `staging`, which is what migration 1
/// calls somewhere goods wait on their way out. Nothing here moves stock.
#[post("/workspace/sites/{site_id}/pack-location")]
pub async fn set_pack_location(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SetPackLocation>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let site_id = path.into_inner();
    let code = body.code.trim().to_string();
    if code.is_empty() {
        return Err(ApiError::Rejected("name the location this site packs at".into()));
    }
    let tenant = who.tenant_id;
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                if tx
                    .query_opt("SELECT 1 FROM site WHERE id = $1", &[&site_id])
                    .await?
                    .is_none()
                {
                    return Err(ApiError::NotFound);
                }
                let found: Option<Uuid> = tx
                    .query_opt(
                        "SELECT id FROM location WHERE site_id = $1 AND code = $2",
                        &[&site_id, &code],
                    )
                    .await?
                    .map(|r| r.get(0));
                let (location, created) = match found {
                    Some(id) => (id, false),
                    None => {
                        let id: Uuid = tx
                            .query_one(
                                "INSERT INTO location (tenant_id, site_id, code, kind, active)
                                 VALUES ($1, $2, $3, 'staging', true) RETURNING id",
                                &[&tenant, &site_id, &code],
                            )
                            .await?
                            .get(0);
                        (id, true)
                    }
                };
                tx.execute(
                    "UPDATE site SET pack_location_id = $2 WHERE id = $1",
                    &[&site_id, &location],
                )
                .await?;
                Ok(PackLocationSet { code, created })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

/// Whose goods a site holds. `business` is the only answer taken so far: the
/// organisation itself.
#[derive(Deserialize, Debug)]
pub struct SetOwner {
    pub owner: String,
}

/// What setting it did.
#[derive(Serialize, Debug)]
pub struct OwnerSet {
    pub owner: String,
    /// True when the organisation had no party of its own, and one was made.
    pub created: bool,
}

/// Say whose goods a site holds (migration 95's `owner_party_id`).
///
/// **The organisation, as a party of its own**, found by the organisation's
/// slug as its code, or made with its name and that code. A handover records
/// this owner on the goods, and migration 95 refuses to guess it; this is the
/// person saying. Stock another company owns at the site, such as a customer's
/// own goods, is an answer this does not take yet, and says so.
#[post("/workspace/sites/{site_id}/owner")]
pub async fn set_owner(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SetOwner>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let site_id = path.into_inner();
    if body.owner != "business" {
        return Err(ApiError::Rejected(
            "a site's owner can only be this business so far".into(),
        ));
    }
    let tenant = who.tenant_id;
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                if tx
                    .query_opt("SELECT 1 FROM site WHERE id = $1", &[&site_id])
                    .await?
                    .is_none()
                {
                    return Err(ApiError::NotFound);
                }
                let t = tx
                    .query_one("SELECT name, upper(slug) FROM tenant WHERE id = $1", &[&tenant])
                    .await?;
                let (name, code): (String, String) = (t.get(0), t.get(1));
                let found: Option<(Uuid, String)> = tx
                    .query_opt("SELECT id, name FROM party WHERE code = $1", &[&code])
                    .await?
                    .map(|r| (r.get(0), r.get(1)));
                let (party, owner, created) = match found {
                    Some((id, named)) => (id, named, false),
                    None => {
                        let id: Uuid = tx
                            .query_one(
                                "INSERT INTO party (tenant_id, name, code) VALUES ($1, $2, $3)
                                 RETURNING id",
                                &[&tenant, &name, &code],
                            )
                            .await?
                            .get(0);
                        (id, name, true)
                    }
                };
                tx.execute(
                    "UPDATE site SET owner_party_id = $2 WHERE id = $1",
                    &[&site_id, &party],
                )
                .await?;
                Ok(OwnerSet { owner, created })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}
