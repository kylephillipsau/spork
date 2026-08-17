//! Tenant scoping, which is the one thing in this server that must not be got
//! wrong.
//!
//! Every tenant-scoped table carries an RLS policy reading `current_tenant()`,
//! which reads the `nylonite.tenant_id` setting. That makes the setting the
//! whole of the tenancy boundary: a query with the wrong value returns another
//! tenant's rows, and a query with no value returns none.
//!
//! # Why this is a transaction and not a connection setting
//!
//! The obvious implementation is `SET nylonite.tenant_id` after checking out a
//! pooled connection. It is also a cross-tenant leak, and a quiet one. A pooled
//! connection outlives the request that borrowed it, so a handler that returns
//! early, panics, or simply forgets to reset leaves the setting in place for
//! whoever borrows that connection next. The bug does not appear under test,
//! because a test uses one connection and one tenant. It appears in production
//! under concurrency, as one customer seeing another's stock.
//!
//! So the scope is a **transaction** and the setting is `SET LOCAL`, which
//! Postgres reverts at commit or rollback without anyone remembering to. A
//! connection cannot escape the pool carrying a tenant, because the only way to
//! set one is inside a transaction that ends.
//!
//! # Why the server connects as `nylonite_app`
//!
//! D25 gives the application role no UPDATE on a projection and no UPDATE or
//! DELETE on a fact. Connecting as the owner or a superuser would make those
//! grants decorative, and RLS is bypassed by superusers entirely, so the
//! tenancy boundary would be off as well. The role in the connection string is
//! part of the design rather than a deployment detail.
//!
//! Compose (and other deploys where `nylonite_app` is NOLOGIN) may connect as
//! `postgres` and then [`ensure_app_role`] — the same pattern as the scheduler's
//! `SET ROLE nylonite_scheduler`. After that, every request runs as the app.

use deadpool_postgres::{Object, Pool};
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::error::ApiError;

/// If the session bypasses RLS, assume `nylonite_app`. No-op when already the app.
///
/// Call on every pooled checkout that will serve tenant data: pool connections
/// start as the login role, and a recycled connection that never assumed the
/// app would serve with RLS off.
pub async fn ensure_app_role(conn: &Object) -> Result<(), ApiError> {
    let row = conn
        .query_one(
            "SELECT current_user::text, rolsuper, rolbypassrls
               FROM pg_roles WHERE rolname = current_user",
            &[],
        )
        .await?;
    let user: String = row.get(0);
    let superuser: bool = row.get(1);
    let bypass: bool = row.get(2);

    if superuser || bypass {
        conn.batch_execute("SET ROLE nylonite_app").await?;
        tracing::debug!(from = %user, "assumed nylonite_app");
    }

    let row = conn
        .query_one(
            "SELECT current_user::text, rolsuper, rolbypassrls
               FROM pg_roles WHERE rolname = current_user",
            &[],
        )
        .await?;
    let user: String = row.get(0);
    let superuser: bool = row.get(1);
    let bypass: bool = row.get(2);
    if superuser || bypass {
        return Err(ApiError::Configuration(format!(
            "connected as {user}, which still bypasses row level security after \
             SET ROLE nylonite_app. Connect as nylonite_app or a login that can \
             become it."
        )));
    }
    Ok(())
}

/// Return a pooled connection to the role it logged in as.
///
/// **The counterpart to [`ensure_app_role`], and the reason it has to exist.**
/// `SET ROLE` is session state, and deadpool's default recycling runs no
/// cleanup statement at all — not `DISCARD ALL`, not `RESET ROLE` — so a
/// connection that served one tenant request comes back out of the pool still
/// as `nylonite_app`. `assert_not_superuser` assumes the app role at boot, so
/// this is true of a connection before the first request as well.
///
/// Almost nothing needs this: every other raw checkout in the server calls
/// `ensure_app_role` and then reads through a `SECURITY DEFINER`, which is
/// migration 70's design and works whichever role it starts from. Setup is the
/// exception, because it writes `person`, `person_credential` and
/// `person_tenant` directly and the application role holds no grant on any of
/// them. Its comment claimed it ran "as `postgres`"; that was true of a virgin
/// connection and of no other, which made the failure depend on which
/// connection the pool happened to hand over.
///
/// Where the login role is itself `nylonite_app` — a deployment that grants it
/// LOGIN — this changes nothing and the writes are refused, which is correct:
/// that deployment cannot mint identities and should say so rather than half
/// succeed.
pub async fn ensure_login_role(conn: &Object) -> Result<(), ApiError> {
    conn.batch_execute("RESET ROLE").await?;
    Ok(())
}

/// A database handle scoped to one tenant for the life of one transaction.
///
/// Obtained only through [`TenantScope::begin`], so there is no way to reach a
/// tenant-scoped table without having declared which tenant you are.
pub struct TenantScope {
    conn: Object,
    tenant: Uuid,
}

impl TenantScope {
    pub async fn begin(pool: &Pool, tenant: Uuid) -> Result<Self, ApiError> {
        let conn = pool.get().await?;
        ensure_app_role(&conn).await?;
        Ok(Self { conn, tenant })
    }

    pub fn tenant(&self) -> Uuid {
        self.tenant
    }

    /// Runs `f` inside a transaction with the tenant setting applied locally.
    ///
    /// The transaction is committed if `f` returns `Ok` and rolled back
    /// otherwise. Either way the setting is gone when the connection returns to
    /// the pool, because `SET LOCAL` is scoped to the transaction rather than to
    /// the session.
    pub async fn run<T, F>(&mut self, f: F) -> Result<T, ApiError>
    where
        F: for<'a> FnOnce(
            &'a Transaction<'a>,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<T, ApiError>> + 'a>,
        >,
    {
        let tx = self.conn.transaction().await?;

        // set_config with is_local = true is SET LOCAL. Passing the tenant as a
        // parameter rather than formatting it into the statement is not
        // decoration: a tenant identifier arriving from a request is untrusted
        // input, and this is a SET, which is exactly where string building goes
        // wrong.
        tx.execute(
            "SELECT set_config('nylonite.tenant_id', $1::text, true)",
            &[&self.tenant.to_string()],
        )
        .await?;

        match f(&tx).await {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(e) => {
                // Explicit, though the drop would roll back anyway. A reader
                // should not have to know that to believe the setting is gone.
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
}

/// Asserts requests will run as a non-bypass role (after optional SET ROLE).
///
/// A superuser bypasses row level security entirely, so serving as one has no
/// tenancy boundary while every policy still reads as though it does. Compose
/// may log in as `postgres` and assume `nylonite_app`; that is checked here.
/// Loud at startup.
pub async fn assert_not_superuser(pool: &Pool) -> Result<(), ApiError> {
    let conn = pool.get().await?;
    ensure_app_role(&conn).await?;
    let row = conn
        .query_one(
            "SELECT current_user::text, rolsuper, rolbypassrls
               FROM pg_roles WHERE rolname = current_user",
            &[],
        )
        .await?;
    let user: String = row.get(0);
    tracing::info!(role = %user, "database role verified: row level security applies");
    Ok(())
}
