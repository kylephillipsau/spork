//! Creating the first administrator, once, on a deployment that has none.
//!
//! A migrations-only database has a schema and nothing else: no tenant, no
//! person, no site. Nothing can sign in, and until this existed the only way in
//! was loading `fixtures/seed.sql` — whose password is committed in this
//! repository — or hand-writing rows with the `hash_password` example. Neither
//! is a way to hand somebody a deployment.
//!
//! # This is not an exception to D11
//!
//! It looks like one and is not. D11 governs attribution on the ledger —
//! `stock_movement.recorded_by_id`, never null and never editable — and setup
//! writes no facts about goods. It writes a tenant, a site, a person, a
//! credential and a membership, none of which carries `recorded_by_id`. Sign-on
//! is what *establishes* attribution, so it cannot require it, and the same is
//! true one step earlier.
//!
//! Nor does it widen the privilege boundary. `nylonite_app` holds SELECT on
//! `tenant` and `person` and nothing at all on `person_credential`, which is
//! deliberate — the tenant-scoped role may not mint identities. But identity
//! acts already run on a raw pooled connection as `postgres`: that is how
//! `sign_on` writes a session and how passkey enrolment writes a credential.
//! Setup joins that established category rather than inventing one.
//!
//! # The gate is a token, because "no persons yet" is a race with strangers
//!
//! Gating only on an empty database hands the deployment to whoever finds it
//! first. That window is real — it was open on this project's own deployment
//! for hours — and it is the shape of CVE-2024-31218, the PocketBase installer
//! race. So the endpoint also requires a token that only somebody who can read
//! the server's log or its filesystem can have.
//!
//! The lifecycle is [`reconcile`]: at boot, if no person exists, mint a token
//! into a state directory and log it; if one does, delete any token lying
//! around. That second half matters more than it looks — restoring a backup
//! onto a box with a stale token file would otherwise leave a live setup
//! credential behind.
//!
//! # Once per deployment, and not once per tenant
//!
//! The predicate is zero persons *anywhere*, not zero persons in some tenant.
//! Adding a second tenant to a running deployment is provisioning, it has D19
//! and D41 to answer to, and it is deliberately not this.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;

/// Where the token lives.
///
/// **Not under the image directory, and that is the point.** `crate::images`
/// writes bytes a stranger uploaded; a path-traversal bug in that handler would
/// read anything beside them. The token gets its own directory so the worst
/// case in one is not a way into the other — the convention PocketBase and
/// GitLab arrived at the same way.
pub fn state_directory() -> PathBuf {
    std::env::var("NYLONITE_STATE_DIR")
        .unwrap_or_else(|_| "/var/lib/nylonite/state".to_string())
        .into()
}

fn token_path() -> PathBuf {
    state_directory().join("setup.token")
}

/// How long a minted token stays good.
///
/// Thirty minutes: long enough to finish setting up, short enough that a token
/// scraped from a log line goes stale before it is useful. Overridable for a
/// deployment where somebody has to walk to another building.
fn ttl() -> Duration {
    let secs = std::env::var("NYLONITE_SETUP_TOKEN_TTL_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(30 * 60);
    Duration::from_secs(secs)
}

/// Whether this deployment has anybody in it.
///
/// Zero persons is the whole predicate. It is read on a raw connection because
/// `nylonite_app` can see `person` but this runs before any tenant is set, and
/// `current_tenant()` has nothing to say about a table that is global (D19).
pub async fn deployment_is_empty(
    client: &tokio_postgres::Client,
) -> Result<bool, tokio_postgres::Error> {
    let n: i64 = client
        .query_one("SELECT count(*) FROM person", &[])
        .await?
        .get(0);
    Ok(n == 0)
}

/// Read the token on disk, if there is one and it has not expired.
///
/// An expired token is treated as absent rather than deleted here, so that a
/// read path never mutates state; [`reconcile`] does the deleting.
fn current_token() -> Option<String> {
    let path = token_path();
    let meta = fs::metadata(&path).ok()?;
    let age = SystemTime::now()
        .duration_since(meta.modified().ok()?)
        .unwrap_or(Duration::ZERO);
    if age > ttl() {
        return None;
    }
    let token = fs::read_to_string(&path).ok()?;
    let token = token.trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

fn mint() -> std::io::Result<String> {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let token = hex_of(&bytes);

    let dir = state_directory();
    fs::create_dir_all(&dir)?;
    let path = token_path();

    // 0600 before anything is written to it, rather than after: a token that is
    // world-readable for even a moment is a token that was world-readable.
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&path)?;
    f.write_all(token.as_bytes())?;
    f.flush()?;
    Ok(token)
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Bring the token file into line with the database, at boot.
///
/// Four cases, and the last is the one worth having:
///
/// * empty deployment, no usable token → mint one and log it
/// * empty deployment, token still good → leave it, so a token does not rotate
///   out from under somebody who is halfway through the form
/// * somebody exists, token present → **delete it**. A backup restored onto a
///   box that had been waiting for setup would otherwise leave a live
///   credential on disk for an endpoint that is now closed
/// * somebody exists, no token → nothing to do
pub async fn reconcile(client: &tokio_postgres::Client) {
    let empty = match deployment_is_empty(client).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, "could not tell whether this deployment has anybody in it; setup token left alone");
            return;
        }
    };

    if !empty {
        if fs::remove_file(token_path()).is_ok() {
            tracing::info!("setup is already done; removed the leftover setup token");
        }
        return;
    }

    if current_token().is_some() {
        tracing::info!(
            path = %token_path().display(),
            "this deployment has nobody in it; the existing setup token is still good"
        );
        return;
    }

    match mint() {
        Ok(token) => {
            // **Logged, because the log is the channel.** Whoever can read the
            // server's output is whoever is entitled to set it up; that is the
            // same trust boundary as being able to read the state directory.
            tracing::info!(
                "this deployment has nobody in it. Set it up at /setup with this token, \
                 good for {} minutes:\n\n    {}\n",
                ttl().as_secs() / 60,
                token
            );
        }
        Err(e) => tracing::error!(
            error = %e,
            path = %token_path().display(),
            "could not write a setup token; this deployment cannot be set up until it can"
        ),
    }
}

/// Whether a presented token matches the one on disk.
///
/// Constant-time over the bytes, because a token compared with `==` leaks its
/// prefix to anybody willing to make enough requests.
fn token_matches(presented: &str) -> bool {
    let Some(actual) = current_token() else {
        return false;
    };
    let a = actual.as_bytes();
    let b = presented.trim().as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Whether a usable token is on disk right now.
pub fn token_is_ready() -> bool {
    current_token().is_some()
}

/// What `GET /setup` answers.
#[derive(Serialize, Debug)]
pub struct SetupStatus {
    /// True when this deployment has nobody in it.
    pub required: bool,
    /// True when a usable token exists on disk. The screen uses it to say
    /// *"check the server log"* rather than *"paste a token"* into a void.
    pub token_ready: bool,
}

#[derive(Deserialize, Debug)]
pub struct SetupRequest {
    pub token: String,
    /// The organisation. `slug` is derived rather than asked for: it is a
    /// machine-facing name and one fewer field to explain.
    pub organisation: String,
    /// The first site. A deployment with no site cannot sign anybody on,
    /// because a session names one.
    pub site_name: String,
    pub site_code: String,
    pub timezone: String,
    pub display_name: String,
    pub email: String,
    pub password: String,
}

#[derive(Serialize, Debug)]
pub struct SetupDone {
    pub tenant_id: Uuid,
    pub site_id: Uuid,
    pub person_id: Uuid,
}

/// A machine-facing name from a human one.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// The rules a first credential has to clear, which are everyone's rules.
///
/// **One policy, in [`crate::credentials`], rather than a copy here.** The first
/// draft had its own twelve-character check and its own message saying *"a first
/// password"*; the moment change-password existed that was two policies, one of
/// which would drift, and a message that read wrong on the other path.
pub use crate::credentials::check_password;

/// Create the tenant, the site, the person, their credential and the membership
/// that ties them together — or refuse.
///
/// **The lock, then the count, then the inserts, all in one transaction.** Two
/// requests arriving together must not both find an empty deployment and both
/// create an administrator. `pg_advisory_xact_lock` is held until the
/// transaction ends, so the second waits, then sees the first one's person and
/// is refused.
///
/// Worth saying plainly: `scripts/migrate.sh` had a lock that did not lock —
/// backgrounded, never waited on, and the work started anyway. This is the same
/// idea done properly, and the difference is that the lock and the thing it
/// guards are in one transaction rather than in two processes.
pub async fn create_first_administrator(
    pool: &deadpool_postgres::Pool,
    body: SetupRequest,
) -> Result<SetupDone, ApiError> {
    if !token_matches(&body.token) {
        // One message for a wrong token and for no token at all: which of the
        // two it is tells a stranger whether setup is still open.
        return Err(ApiError::Rejected(
            "that setup token is not valid. It is printed in the server's log when a \
             deployment has nobody in it, and it expires."
                .into(),
        ));
    }
    check_password(&body.password).map_err(|m| ApiError::Rejected(m.into()))?;

    let organisation = body.organisation.trim();
    let email = body.email.trim().to_lowercase();
    if organisation.is_empty() || body.display_name.trim().is_empty() || email.is_empty() {
        return Err(ApiError::Rejected(
            "an organisation, a name and an email address are all needed".into(),
        ));
    }
    let slug = slugify(organisation);
    if slug.is_empty() {
        return Err(ApiError::Rejected(
            "that organisation name has no letters or digits in it".into(),
        ));
    }
    let phc = crate::auth::hash_password(&body.password)
        .map_err(|_| ApiError::Rejected("that password could not be stored".into()))?;

    // **A raw connection, as the login role.** Identity acts do not run as
    // `nylonite_app`, which holds no grant on `person_credential` at all; this
    // is the same connection shape `sign_on` uses to write a session.
    //
    // The reset is not ceremony. A pooled connection keeps its `SET ROLE`, and
    // one has been assumed on at least one connection since boot, so "as
    // `postgres`" was a property of which connection the pool handed over
    // rather than of this code. See `tenancy::ensure_login_role`.
    let mut conn = pool.get().await?;
    crate::tenancy::ensure_login_role(&conn).await?;
    let tx = conn.transaction().await?;

    // 0x4E594C5F535450 is "NYL_STP".
    tx.execute("SELECT pg_advisory_xact_lock($1)", &[&0x4E594C5F535450i64])
        .await?;

    let already: i64 = tx.query_one("SELECT count(*) FROM person", &[]).await?.get(0);
    if already > 0 {
        return Err(ApiError::Rejected(
            "this deployment already has somebody in it, so it is already set up".into(),
        ));
    }

    let tenant_id: Uuid = tx
        .query_one(
            "INSERT INTO tenant (name, slug) VALUES ($1, $2) RETURNING id",
            &[&organisation, &slug],
        )
        .await?
        .get(0);

    let site_id: Uuid = tx
        .query_one(
            "INSERT INTO site (tenant_id, name, code, timezone)
             VALUES ($1, $2, $3, $4) RETURNING id",
            &[
                &tenant_id,
                &body.site_name.trim(),
                &body.site_code.trim(),
                &body.timezone.trim(),
            ],
        )
        .await?
        .get(0);

    let person_id: Uuid = tx
        .query_one(
            "INSERT INTO person (display_name, email) VALUES ($1, $2) RETURNING id",
            &[&body.display_name.trim(), &email],
        )
        .await?
        .get(0);

    tx.execute(
        "INSERT INTO person_credential (person_id, kind, phc) VALUES ($1, 'password', $2)",
        &[&person_id, &phc],
    )
    .await?;

    tx.execute(
        "INSERT INTO person_tenant (person_id, tenant_id, role) VALUES ($1, $2, 'operator')",
        &[&person_id, &tenant_id],
    )
    .await?;

    tx.commit().await?;

    // **The token dies with the act it authorised.** The count check above
    // already closes the endpoint; removing the file means a token scraped from
    // a log is inert as well, rather than merely useless.
    let _ = fs::remove_file(token_path());
    tracing::info!(%tenant_id, %site_id, %person_id, "deployment set up; setup is now closed");

    Ok(SetupDone { tenant_id, site_id, person_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slug_is_a_machine_name_for_a_human_one() {
        assert_eq!(slugify("Nylonite Pty Ltd"), "nylonite-pty-ltd");
        assert_eq!(slugify("  Acme   &   Co.  "), "acme-co");
        assert_eq!(slugify("Ångström"), "ngstr-m");
        // Nothing usable in it at all, which the caller refuses rather than
        // storing an empty slug.
        assert_eq!(slugify("!!!"), "");
        assert_eq!(slugify(""), "");
    }
}
