//! The people of a workspace: who can sign in to it, and as what. D205.
//!
//! Until now the only way a person came to exist was setting a deployment up,
//! so a workspace had one person and everybody else worked as a crew name.
//! Picking in Spork needs each picker to sign in as themselves
//! (docs/picking-plan.md), so an administrator adds them here.
//!
//! **Identity is read and written as the login role**, as setup and sign-on do:
//! `spork_app` holds no grant on sign-ins, and every query names the workspace
//! explicitly. The administrator check runs first, inside the workspace's scope
//! (D192).
//!
//! **A person who leaves keeps their history.** Leaving sets `left_at`, which
//! `session_resolve` and `role_in_tenant` already read: their sessions stop
//! resolving at once, and everything they recorded still names them.

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::routes::{administrator, caller};
use crate::AppState;

/// What a member can be. An administrator also sees Backup and People (D192).
const ROLES: &[&str] = &["administrator", "operator"];

/// One person of the workspace, current or past.
#[derive(Serialize, Debug)]
pub struct WorkspacePerson {
    pub person_id: Uuid,
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
    pub joined_at: DateTime<Utc>,
    /// When they stopped being a member; absent while they are one.
    pub left_at: Option<DateTime<Utc>>,
    /// Whether they can sign in with a password, and with how many passkeys.
    pub password: bool,
    pub passkeys: i64,
    /// The administrator looking at the list.
    pub you: bool,
}

/// The workspace's people, current first.
#[get("/workspace/people")]
pub async fn list_people(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    administrator(&state, &who).await?;
    let conn = state.pool.get().await?;
    crate::tenancy::ensure_login_role(&conn).await?;
    let rows = conn
        .query(
            "SELECT p.id, p.display_name, p.email, pt.role, pt.joined_at, pt.left_at,
                    EXISTS (SELECT 1 FROM person_credential c
                             WHERE c.person_id = p.id AND c.kind = 'password'),
                    (SELECT count(*) FROM person_passkey k WHERE k.person_id = p.id)
               FROM person_tenant pt
               JOIN person p ON p.id = pt.person_id
              WHERE pt.tenant_id = $1
              ORDER BY pt.left_at IS NOT NULL, lower(p.display_name)",
            &[&who.tenant_id],
        )
        .await?;
    let people: Vec<WorkspacePerson> = rows
        .iter()
        .map(|r| WorkspacePerson {
            person_id: r.get(0),
            display_name: r.get(1),
            email: r.get(2),
            role: r.get(3),
            joined_at: r.get(4),
            left_at: r.get(5),
            password: r.get(6),
            passkeys: r.get(7),
            you: r.get::<_, Uuid>(0) == who.person_id,
        })
        .collect();
    Ok(HttpResponse::Ok().json(people))
}

#[derive(Deserialize, Debug)]
pub struct AddPerson {
    pub display_name: String,
    pub email: String,
    /// Their first password, at least twelve characters; they change it under
    /// Account. Not used when the email already signs in to another workspace,
    /// because that person keeps the sign-in they have.
    #[serde(default)]
    pub password: Option<String>,
    pub role: String,
}

#[derive(Serialize, Debug)]
pub struct PersonAdded {
    pub person_id: Uuid,
    /// True when the email already signed in elsewhere: they were added with
    /// the sign-in they have, and no password was set.
    pub existing: bool,
}

/// Add a person to the workspace, or bring back one who left.
#[post("/workspace/people")]
pub async fn add_person(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<AddPerson>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    administrator(&state, &who).await?;
    let body = body.into_inner();
    let name = body.display_name.trim().to_string();
    let email = body.email.trim().to_lowercase();
    if name.is_empty() || !email.contains('@') {
        return Err(ApiError::Rejected("a name and an email address are both needed".into()));
    }
    if !ROLES.contains(&body.role.as_str()) {
        return Err(ApiError::Rejected("the role is administrator or operator".into()));
    }

    let mut conn = state.pool.get().await?;
    crate::tenancy::ensure_login_role(&conn).await?;
    let tx = conn.transaction().await?;
    let found: Option<Uuid> = tx
        .query_opt("SELECT id FROM person WHERE lower(email) = $1", &[&email])
        .await?
        .map(|r| r.get(0));

    let added = match found {
        Some(person_id) => {
            let membership: Option<Option<DateTime<Utc>>> = tx
                .query_opt(
                    "SELECT left_at FROM person_tenant WHERE person_id = $1 AND tenant_id = $2",
                    &[&person_id, &who.tenant_id],
                )
                .await?
                .map(|r| r.get(0));
            match membership {
                Some(None) => {
                    return Err(ApiError::Rejected(format!("{email} is already in this workspace")));
                }
                // Back again: the same membership, from now, as what they are now.
                Some(Some(_)) => {
                    tx.execute(
                        "UPDATE person_tenant SET role = $3, joined_at = now(), left_at = NULL
                          WHERE person_id = $1 AND tenant_id = $2",
                        &[&person_id, &who.tenant_id, &body.role],
                    )
                    .await?;
                }
                None => {
                    tx.execute(
                        "INSERT INTO person_tenant (person_id, tenant_id, role) VALUES ($1, $2, $3)",
                        &[&person_id, &who.tenant_id, &body.role],
                    )
                    .await?;
                }
            }
            PersonAdded { person_id, existing: true }
        }
        None => {
            let password = body.password.unwrap_or_default();
            crate::credentials::check_password(&password).map_err(|m| ApiError::Rejected(m.into()))?;
            let phc = crate::auth::hash_password(&password)
                .map_err(|_| ApiError::Rejected("that password could not be stored".into()))?;
            let person_id: Uuid = tx
                .query_one(
                    "INSERT INTO person (display_name, email) VALUES ($1, $2) RETURNING id",
                    &[&name, &email],
                )
                .await?
                .get(0);
            tx.execute(
                "INSERT INTO person_credential (person_id, kind, phc) VALUES ($1, 'password', $2)",
                &[&person_id, &phc],
            )
            .await?;
            tx.execute(
                "INSERT INTO person_tenant (person_id, tenant_id, role) VALUES ($1, $2, $3)",
                &[&person_id, &who.tenant_id, &body.role],
            )
            .await?;
            PersonAdded { person_id, existing: false }
        }
    };
    tx.commit().await?;
    tracing::info!(tenant_id = %who.tenant_id, person_id = %added.person_id, by = %who.person_id, "a person was added to the workspace");
    Ok(HttpResponse::Ok().json(added))
}

#[derive(Deserialize, Debug)]
pub struct SetRole {
    pub role: String,
}

/// The workspace's other current administrators, besides `person`.
async fn other_administrators(
    tx: &tokio_postgres::Transaction<'_>,
    tenant: Uuid,
    person: Uuid,
) -> Result<i64, ApiError> {
    Ok(tx
        .query_one(
            "SELECT count(*) FROM person_tenant
              WHERE tenant_id = $1 AND role = 'administrator' AND left_at IS NULL
                AND person_id <> $2",
            &[&tenant, &person],
        )
        .await?
        .get(0))
}

/// Say what a member is (D205, answering Q176). The last administrator stays one.
#[post("/workspace/people/{id}/role")]
pub async fn set_role(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SetRole>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    administrator(&state, &who).await?;
    let person = path.into_inner();
    if !ROLES.contains(&body.role.as_str()) {
        return Err(ApiError::Rejected("the role is administrator or operator".into()));
    }
    let mut conn = state.pool.get().await?;
    crate::tenancy::ensure_login_role(&conn).await?;
    let tx = conn.transaction().await?;
    if body.role != "administrator" && other_administrators(&tx, who.tenant_id, person).await? == 0 {
        return Err(ApiError::Rejected("a workspace needs an administrator; make somebody else one first".into()));
    }
    let changed = tx
        .execute(
            "UPDATE person_tenant SET role = $3
              WHERE person_id = $1 AND tenant_id = $2 AND left_at IS NULL",
            &[&person, &who.tenant_id, &body.role],
        )
        .await?;
    if changed == 0 {
        return Err(ApiError::NotFound);
    }
    tx.commit().await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Take somebody out of the workspace. Their sessions stop at once and their
/// history stays theirs. Nobody removes themselves, and the last administrator
/// stays.
#[post("/workspace/people/{id}/leave")]
pub async fn remove_person(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    administrator(&state, &who).await?;
    let person = path.into_inner();
    if person == who.person_id {
        return Err(ApiError::Rejected("you can't take yourself out of the workspace".into()));
    }
    let mut conn = state.pool.get().await?;
    crate::tenancy::ensure_login_role(&conn).await?;
    let tx = conn.transaction().await?;
    let is_admin: bool = tx
        .query_opt(
            "SELECT role = 'administrator' FROM person_tenant
              WHERE person_id = $1 AND tenant_id = $2 AND left_at IS NULL",
            &[&person, &who.tenant_id],
        )
        .await?
        .ok_or(ApiError::NotFound)?
        .get(0);
    if is_admin && other_administrators(&tx, who.tenant_id, person).await? == 0 {
        return Err(ApiError::Rejected("a workspace needs an administrator; make somebody else one first".into()));
    }
    tx.execute(
        "UPDATE person_tenant SET left_at = now()
          WHERE person_id = $1 AND tenant_id = $2 AND left_at IS NULL",
        &[&person, &who.tenant_id],
    )
    .await?;
    tx.execute(
        "UPDATE session SET revoked_at = now()
          WHERE person_id = $1 AND tenant_id = $2 AND revoked_at IS NULL",
        &[&person, &who.tenant_id],
    )
    .await?;
    tx.commit().await?;
    tracing::info!(tenant_id = %who.tenant_id, person_id = %person, by = %who.person_id, "a person left the workspace");
    Ok(HttpResponse::NoContent().finish())
}
