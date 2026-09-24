//! Import tokens: minting them, listing them, withdrawing them, resolving them.
//!
//! The table and the reasoning live in migration 88. This is the application
//! side, and it holds one rule worth stating here because it is invisible in the
//! SQL: **`spork_app` has no privilege on `api_token` at all**, so everything
//! below goes through a `SECURITY DEFINER`. That is migration 70's arrangement
//! for the identity tables, and as of migration 87 it is the arrangement for all
//! of them — J73 asks Postgres whether the application can reach a table
//! carrying a `tenant_id`, and this one it cannot.
//!
//! # The secret is returned once
//!
//! [`mint`] is the only moment the token exists in a form anybody can use. The
//! row stores its SHA-256, so a lost token is re-minted rather than recovered —
//! the same bargain `session` makes, and the reason `credentials` can lock an
//! account without ever being able to read a password.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{self, Machine};
use crate::error::ApiError;
use crate::tenancy::TenantScope;

/// How long a token is good for when the request does not say.
///
/// Ninety days: long enough that a monthly load never trips over it, short
/// enough that a forgotten token stops working within a quarter. There is no
/// idle timeout — see `api_token_resolve` in migration 88 for why an abandoned
/// terminal and a monthly loader are not the same risk.
pub const DEFAULT_DAYS: i64 = 90;

/// The longest a token may be asked for.
///
/// A cap rather than a policy: without one, `days` is an integer somebody
/// eventually sets to thirty-six thousand, and the expiry column stops meaning
/// anything. A year is the outer edge of "I will remember this exists".
pub const MAX_DAYS: i64 = 365;

#[derive(Deserialize, Debug)]
pub struct MintRequest {
    /// What it is for, in words. Stored, and refused when blank.
    pub label: String,
    pub days: Option<i64>,
}

/// The one and only time the secret is readable.
#[derive(Serialize, Debug)]
pub struct Minted {
    pub id: Uuid,
    pub label: String,
    /// **Shown once.** Nothing can produce it again.
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct TokenRow {
    pub id: Uuid,
    pub label: String,
    pub created_by_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Mint a token for the caller's tenant, on their authority.
pub async fn mint(
    scope: &mut TenantScope,
    person_id: Uuid,
    body: MintRequest,
) -> Result<Minted, ApiError> {
    let label = body.label.trim().to_string();
    if label.is_empty() {
        return Err(ApiError::Rejected(
            "a token needs a label saying what it is for".into(),
        ));
    }
    let days = body.days.unwrap_or(DEFAULT_DAYS);
    if !(1..=MAX_DAYS).contains(&days) {
        return Err(ApiError::Rejected(format!(
            "a token lasts between 1 and {MAX_DAYS} days"
        )));
    }

    let minted = auth::mint_api_token();
    let tenant = scope.tenant();
    let digest = minted.sha256.clone();
    let for_label = label.clone();

    let (id, expires_at) = scope
        .run(move |tx| {
            Box::pin(async move {
                // `make_interval` from a typed integer rather than a formatted
                // string, for the reason `issue_session` gives: this is a value
                // arriving from a request, and building an interval by string is
                // where that goes wrong.
                let row = tx
                    .query_one(
                        "SELECT api_token_open($1, $2, $3, $4, make_interval(days => $5))",
                        &[&tenant, &person_id, &for_label, &digest, &(days as i32)],
                    )
                    .await?;
                let id: Uuid = row.get(0);
                let expires: DateTime<Utc> = tx
                    .query_one(
                        "SELECT expires_at FROM api_tokens_for_tenant($1) WHERE id = $2",
                        &[&tenant, &id],
                    )
                    .await?
                    .get(0);
                Ok((id, expires))
            })
        })
        .await?;

    Ok(Minted { id, label, token: minted.token, expires_at })
}

/// Every token this tenant has, spent ones included, without their digests.
pub async fn list(scope: &mut TenantScope) -> Result<Vec<TokenRow>, ApiError> {
    let tenant = scope.tenant();
    scope
        .run(move |tx| {
            Box::pin(async move {
                let rows = tx
                    .query("SELECT * FROM api_tokens_for_tenant($1)", &[&tenant])
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| TokenRow {
                        id: r.get(0),
                        label: r.get(1),
                        created_by_id: r.get(2),
                        created_at: r.get(3),
                        expires_at: r.get(4),
                        last_used_at: r.get(5),
                        revoked_at: r.get(6),
                    })
                    .collect())
            })
        })
        .await
}

/// Withdraw one. Scoped to the tenant, so holding an id is not enough.
pub async fn revoke(scope: &mut TenantScope, id: Uuid) -> Result<bool, ApiError> {
    let tenant = scope.tenant();
    scope
        .run(move |tx| {
            Box::pin(async move {
                let done: Option<bool> = tx
                    .query_opt("SELECT api_token_revoke($1, $2)", &[&id, &tenant])
                    .await?
                    .and_then(|r| r.get(0));
                Ok(done.unwrap_or(false))
            })
        })
        .await
}

/// Resolve a bearer that carries the import prefix.
///
/// Runs on a raw checkout as `spork_app` and reaches the definer, which is
/// the same shape `caller` uses for sessions — and for the same reason. There
/// is no tenant yet, because the token is what names one.
pub async fn machine(
    pool: &deadpool_postgres::Pool,
    token: &str,
) -> Result<Machine, ApiError> {
    let conn = pool.get().await?;
    crate::tenancy::ensure_app_role(&conn).await?;
    let row = conn
        .query_opt(
            "SELECT token_id, tenant_id, created_by_id, label FROM api_token_resolve($1)",
            &[&auth::token_digest(token)],
        )
        .await?
        .ok_or(ApiError::Unauthenticated)?;
    Ok(Machine {
        token_id: row.get(0),
        tenant_id: row.get(1),
        created_by_id: row.get(2),
        label: row.get(3),
    })
}
