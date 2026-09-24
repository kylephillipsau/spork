//! The tenancy boundary, tested against a real database.
//!
//! These are not tests of the handlers. They are tests of the claim the whole
//! design rests on: that a query cannot see another tenant's rows, and that a
//! pooled connection cannot carry a tenant out of the request that set it.

use tokio_postgres::NoTls;

mod common;
use common::{connect, url_and_role};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const BETA: &str = "22222222-2222-2222-2222-222222222222";

#[tokio::test]
async fn a_tenant_sees_only_its_own_rows() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;

    for (tenant, expected) in [(ALPHA, "MEL"), (BETA, "SYD")] {
        let tx = client.transaction().await.unwrap();
        tx.execute(
            "SELECT set_config('spork.tenant_id', $1::text, true)",
            &[&tenant],
        )
        .await
        .unwrap();
        let rows = tx.query("SELECT code FROM site", &[]).await.unwrap();
        assert_eq!(rows.len(), 1, "{tenant} should see exactly one site");
        assert_eq!(rows[0].get::<_, String>(0), expected);
        tx.commit().await.unwrap();
    }
}

/// The leak this design is most exposed to.
///
/// A pooled connection outlives the request that borrowed it. If the tenant were
/// set with `SET` rather than `SET LOCAL`, it would still be set when the next
/// request borrowed the same connection, and that request would read the
/// previous tenant's rows while looking entirely correct in the handler.
///
/// So: set a tenant inside a transaction, end it, and confirm the setting is
/// gone on the very same connection.
#[tokio::test]
async fn a_tenant_cannot_outlive_its_transaction() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();
    assert_eq!(
        tx.query("SELECT code FROM site", &[]).await.unwrap().len(),
        1,
        "inside the transaction the tenant is set"
    );
    tx.commit().await.unwrap();

    // Same connection, no transaction, no tenant.
    let after: String = client
        .query_one("SELECT current_setting('spork.tenant_id', true)", &[])
        .await
        .unwrap()
        .get::<_, Option<String>>(0)
        .unwrap_or_default();
    assert!(
        after.is_empty(),
        "the tenant survived its transaction and is still {after} on a pooled \
         connection: the next request would read that tenant's rows"
    );

    let rows = client.query("SELECT code FROM site", &[]).await.unwrap();
    assert!(
        rows.is_empty(),
        "with no tenant the application role must see nothing, not everything"
    );
}

/// D25's grants are the second half of the boundary. RLS decides which rows; the
/// grants decide which verbs. A fact is what happened, and there is no verb for
/// changing what happened.
#[tokio::test]
async fn the_application_role_cannot_rewrite_history() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let updated = tx
        .execute("UPDATE stock_movement SET quantity = 1", &[])
        .await;
    assert!(
        updated.is_err(),
        "the application role updated the ledger, which D25 forbids by grant"
    );

    let deleted = tx.execute("DELETE FROM stock WHERE true", &[]).await;
    assert!(
        deleted.is_err(),
        "the application role deleted from a projection, which D25 forbids by grant"
    );
}

/// A pooled connection carries `SET ROLE` out of the request that set it.
///
/// **This is the assumption `setup.rs` documents and does not have.** Its
/// comment says the identity path runs on "a raw connection, as `postgres`",
/// and that is true only of a connection which has never served a tenant
/// request. `ensure_app_role` issues a session-level `SET ROLE spork_app`,
/// deadpool's default recycling is `Fast` — which runs no cleanup statement at
/// all, not `DISCARD ALL`, not `RESET ROLE` — so the role is still in place
/// when the next checkout gets that connection back.
///
/// The pool is capped at one so the reuse is certain rather than probable. That
/// is not stacking the deck: it is the steady state of any deployment whose
/// connections outnumber neither its cores nor its traffic, and the failure it
/// produces is intermittent precisely because reuse is ordinarily probable.
///
/// Only one path actually depends on the login role, which is why the fix is
/// `ensure_login_role` at that one site rather than a pool-wide recycling
/// change: every other raw checkout — `sign_on`, `caller`, the passkey
/// handlers — calls `ensure_app_role` first and reads through a definer, on
/// purpose. `setup` is the one that writes `person_credential` directly, and it
/// is the one this would have failed.
#[tokio::test]
async fn set_role_survives_the_pool() {
    let Some((u, _)) = url_and_role() else {
        eprintln!("DATABASE_URL not set; skipping");
        return;
    };
    let pg: tokio_postgres::Config = u.parse().expect("a usable connection string");
    let manager = deadpool_postgres::Manager::from_config(
        pg,
        NoTls,
        deadpool_postgres::ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Fast,
        },
    );
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(1)
        .build()
        .expect("a pool");

    let first: String = {
        let conn = pool.get().await.expect("a connection");
        conn.batch_execute("SET ROLE spork_app")
            .await
            .expect("become the application role");
        conn.query_one("SELECT current_user::text", &[])
            .await
            .unwrap()
            .get(0)
    };
    assert_eq!(first, "spork_app");

    // A different checkout, which believes it is the login role.
    let conn = pool.get().await.expect("a connection");
    let second: String = conn
        .query_one("SELECT current_user::text", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        second, "spork_app",
        "a recycled connection reset its role, so the identity paths really do \
         run as the login role and this note can be deleted"
    );

    // And the consequence, rather than only the mechanism: the setup path's
    // own writes are refused on a connection in this state.
    let refused = conn
        .execute(
            "INSERT INTO person_credential (person_id, kind, phc) \
             VALUES (gen_random_uuid(), 'password', 'x')",
            &[],
        )
        .await;
    assert!(
        refused.is_err(),
        "spork_app wrote person_credential, which D25 forbids by grant"
    );
}
