//! Setting a deployment up, once, and the four ways that must not work.
//!
//! The interesting assertions here are all refusals. Creating an administrator
//! on an empty deployment is the easy half; what the design is actually made of
//! is that a second one cannot be created, that a racing request loses, that a
//! stale token is not a key, and that the token stops being one the moment it
//! has been used.
//!
//! **The race is the reason this file exists.** `pg_advisory_xact_lock` then the
//! count then the inserts, all in one transaction, is the shape — and the shape
//! is only worth anything if something proves two concurrent requests do not
//! both succeed. `scripts/migrate.sh` shipped a lock that did not lock because
//! nothing tested it.

use std::fs;

use actix_web::{test, web, App};
use spork_server::{routes, AppState};
use serde_json::{json, Value};
use std::str::FromStr;

mod common;
use common::{pool, url};

fn body(token: &str) -> Value {
    json!({
        "token": token,
        "organisation": "Spork Test Co",
        "site_name": "Melbourne",
        "site_code": "MEL2",
        "timezone": "Australia/Melbourne",
        "display_name": "First Administrator",
        "email": "first@example.test",
        "password": "a-long-enough-password"
    })
}

/// The same connection string, pointing at a different database.
///
/// **The last path segment, not every occurrence of it**, and the difference is
/// the reason CI sat red for three commits. This was
/// `u.replace(u.rsplit('/').next().unwrap(), &name)`, which replaces the
/// database name *everywhere it appears in the URL* — and on the CI runner the
/// URL is `postgres://postgres:spork@postgres:5432/spork`, where it also
/// appears as the password. The scoped URL came out with the generated database
/// name in the password field, `migrate.sh` failed with `password
/// authentication failed for user "postgres"`, `check` failed, `images` was
/// skipped, and the registry's `latest` stayed three commits behind while every
/// redeploy looked like it worked.
///
/// It never failed locally because the development database is called
/// `spork` and the password happens to be the same word — so the URL is
/// `.../spork` with `spork` as the password, and the test only passes when
/// the two differ, which is the case a scratch database called
/// `spork_check` accidentally created.
fn with_database(url: &str, name: &str) -> String {
    let (prefix, _) = url.rsplit_once('/').expect("a URL with a database in it");
    format!("{prefix}/{name}")
}

/// A state directory of this test's own, so a real one is never touched.
///
/// **The directory is per test and the variable naming it is not**, because an
/// environment variable belongs to the process. Two tests in this binary run on
/// two threads, and the second `set_var` redirects the first test's token
/// lookup to the second test's directory — which reads as "a token exists
/// before anything minted one", a failure with nothing to do with either test.
/// So the tests take a gate, and it is returned here rather than left to be
/// remembered separately from the thing it guards. It is a `tokio` mutex rather
/// than a `std` one because the guard is held for the length of a test, which
/// is to say across every await in it.
async fn isolate_state() -> (std::path::PathBuf, tokio::sync::MutexGuard<'static, ()>) {
    static GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let held = GATE.lock().await;
    let dir = std::env::temp_dir().join(format!("spork-setup-{}", uuid::Uuid::new_v4()));
    std::env::set_var("SPORK_STATE_DIR", &dir);
    (dir, held)
}

#[actix_web::test]
async fn a_deployment_is_set_up_once_and_refuses_every_other_attempt() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let (state_dir, _gate) = isolate_state().await;

    // **A database of this test's own.** Setup is a statement about the whole
    // deployment — zero persons anywhere — so it cannot be tested against the
    // shared fixture, which has two.
    let admin = pool(&u);
    let name = format!("spork_setup_{}", uuid::Uuid::new_v4().simple());
    {
        let c = admin.get().await.expect("connect");
        c.execute(&format!("CREATE DATABASE {name}"), &[]).await.expect("create");
    }
    let scoped = with_database(&u, &name);
    let pool = pool(&scoped);

    // **Build the schema with the real migrator**, rather than replaying the
    // files here. Doing it in Rust with `batch_execute` puts every statement in
    // an implicit transaction, and migration 30 adds an enum value and then
    // uses it — which Postgres refuses inside one. `scripts/migrate.sh` already
    // knows that, per file, and testing against what actually runs beats
    // testing against a second implementation of it.
    {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out = std::process::Command::new("sh")
            .arg("scripts/migrate.sh")
            .current_dir(&root)
            .env("DATABASE_URL", &scoped)
            .output()
            .expect("run migrate.sh");
        assert!(
            out.status.success(),
            "migrate.sh failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let state = web::Data::new(AppState { pool: pool.clone() });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    // ── nobody is in it, and it says so ─────────────────────────────────
    let status: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::get().uri("/setup").to_request(),
    )
    .await;
    assert_eq!(status["required"], true, "an empty deployment needs setting up");
    assert_eq!(
        status["token_ready"], false,
        "and no token exists until something mints one"
    );

    // ── a wrong token is refused before anything else is looked at ──────
    let refused = test::call_service(
        &app,
        test::TestRequest::post().uri("/setup").set_json(body("not-the-token")).to_request(),
    )
    .await;
    assert_eq!(refused.status(), 400, "a stranger with no token gets nowhere");

    // Mint one the way boot does.
    {
        let c = pool.get().await.expect("connect");
        spork_server::setup::reconcile(&c).await;
    }
    let token = fs::read_to_string(state_dir.join("setup.token")).expect("a token was minted");
    let token = token.trim().to_string();
    assert_eq!(token.len(), 64, "32 random bytes, in hex");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(state_dir.join("setup.token")).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "the token is not readable by anybody else");
    }

    // ── a short password is refused even with the right token ───────────
    let mut weak = body(&token);
    weak["password"] = json!("short");
    let r = test::call_service(
        &app,
        test::TestRequest::post().uri("/setup").set_json(weak).to_request(),
    )
    .await;
    assert_eq!(r.status(), 400, "twelve characters is the one rule and it is enforced");

    // ── two at once, and exactly one wins ───────────────────────────────
    //
    // **This is what the transaction is for.** Both requests find an empty
    // deployment; without `pg_advisory_xact_lock` held across the count and the
    // inserts, both would create an administrator and the loser would be
    // whoever looked second. Two independent app instances over one pool,
    // because that is the shape a second worker or a second replica has.
    let other = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { pool: pool.clone() }))
            .configure(routes::configure),
    )
    .await;

    let (a, b) = futures_util::join!(
        test::call_service(
            &app,
            test::TestRequest::post().uri("/setup").set_json(body(&token)).to_request()
        ),
        test::call_service(
            &other,
            test::TestRequest::post().uri("/setup").set_json(body(&token)).to_request()
        ),
    );

    let codes = [a.status().as_u16(), b.status().as_u16()];
    let winners = codes.iter().filter(|c| **c == 200).count();
    assert_eq!(
        winners, 1,
        "exactly one of two concurrent setups may succeed, got {codes:?}"
    );
    assert!(
        codes.contains(&400),
        "and the other is refused rather than erroring, got {codes:?}"
    );

    let done: Value = test::read_body_json(if a.status() == 200 { a } else { b }).await;
    assert!(done["tenant_id"].is_string(), "a tenant was created: {done}");
    assert!(done["site_id"].is_string());
    assert!(done["person_id"].is_string());

    // One administrator, not two.
    {
        let c = pool.get().await.expect("connect");
        let n: i64 = c.query_one("SELECT count(*) FROM person", &[]).await.unwrap().get(0);
        assert_eq!(n, 1, "the race left one person, not two");
        let t: i64 = c.query_one("SELECT count(*) FROM tenant", &[]).await.unwrap().get(0);
        assert_eq!(t, 1, "and one tenant");
    }

    // ── the gate is shut, by the database and by the token both ─────────
    let status: Value = test::call_and_read_body_json(
        &app,
        test::TestRequest::get().uri("/setup").to_request(),
    )
    .await;
    assert_eq!(status["required"], false, "it does not need setting up twice");

    assert!(
        !state_dir.join("setup.token").exists(),
        "the token dies with the act it authorised, so one scraped from a log is inert"
    );

    let again = test::call_service(
        &app,
        test::TestRequest::post().uri("/setup").set_json(body(&token)).to_request(),
    )
    .await;
    assert_eq!(again.status(), 400, "the same token cannot be used twice");

    // ── the administrator can actually sign in ──────────────────────────
    //
    // The point of the whole exercise. A setup that creates rows nobody can
    // authenticate against has done nothing.
    let signed = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/sessions")
            .set_json(json!({
                "email": "first@example.test",
                "password": "a-long-enough-password",
                "site_id": done["site_id"]
            }))
            .to_request(),
    )
    .await;
    assert!(
        signed.status().is_success(),
        "the administrator setup created can sign on: {}",
        signed.status()
    );

    // ── a boot against a deployment that has somebody removes a token ───
    //
    // The restored-backup case: a box that had been waiting for setup, then had
    // a populated database put under it, must not keep a live credential.
    fs::create_dir_all(&state_dir).ok();
    fs::write(state_dir.join("setup.token"), "left-over-from-before").expect("write");
    {
        let c = pool.get().await.expect("connect");
        spork_server::setup::reconcile(&c).await;
    }
    assert!(
        !state_dir.join("setup.token").exists(),
        "a leftover token is deleted at boot once somebody exists"
    );

    // Clean up the database this test made.
    drop(pool);
    let c = admin.get().await.expect("connect");
    let _ = c.execute(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"), &[]).await;
    let _ = fs::remove_dir_all(&state_dir);
}

/// Setup works on a connection that has already served as the application role.
///
/// **The bug this guards was reachable on a real deployment and depended on
/// nothing the operator could see.** `SET ROLE` is session state and deadpool's
/// default recycling issues no cleanup statement, so a connection comes back
/// out of the pool as whatever role it was left as. `assert_not_superuser`
/// assumes `spork_app` at boot, which means the very first connection is in
/// that state before any request arrives — and setup writes `person_credential`
/// directly, which that role holds no grant on at all. Whether setup worked was
/// decided by which connection the pool happened to hand over.
///
/// The pool is capped at one so there is no happening about it. Boot is
/// reproduced with the real `assert_not_superuser` rather than a bare
/// `SET ROLE`, because the claim under test is about what boot leaves behind.
#[actix_web::test]
async fn setup_survives_a_connection_that_already_served() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let (state_dir, _gate) = isolate_state().await;

    let admin = pool(&u);
    let name = format!("spork_setup_{}", uuid::Uuid::new_v4().simple());
    {
        let c = admin.get().await.expect("connect");
        c.execute(&format!("CREATE DATABASE {name}"), &[]).await.expect("create");
    }
    let scoped = with_database(&u, &name);
    {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out = std::process::Command::new("sh")
            .arg("scripts/migrate.sh")
            .current_dir(&root)
            .env("DATABASE_URL", &scoped)
            .output()
            .expect("run migrate.sh");
        assert!(out.status.success(), "migrate.sh failed: {}",
            String::from_utf8_lossy(&out.stderr));
    }

    let pg = tokio_postgres::Config::from_str(&scoped).expect("url");
    let manager = deadpool_postgres::Manager::from_config(
        pg,
        tokio_postgres::NoTls,
        deadpool_postgres::ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Fast,
        },
    );
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(1)
        .build()
        .expect("pool");

    // Exactly what `main` does before it serves anything.
    spork_server::tenancy::assert_not_superuser(&pool)
        .await
        .expect("the boot check passes");
    {
        let c = pool.get().await.expect("connect");
        let role: String = c.query_one("SELECT current_user::text", &[]).await.unwrap().get(0);
        assert_eq!(role, "spork_app", "boot left the connection as the app role");
    }

    let state = web::Data::new(AppState { pool: pool.clone() });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    {
        let c = pool.get().await.expect("connect");
        spork_server::setup::reconcile(&c).await;
    }
    let token = fs::read_to_string(state_dir.join("setup.token")).expect("a token");
    let token = token.trim().to_string();

    let created = test::call_service(
        &app,
        test::TestRequest::post().uri("/setup").set_json(body(&token)).to_request(),
    )
    .await;
    assert_eq!(
        created.status(),
        200,
        "setup was refused on a connection the pool had already lent to a tenant \
         request, which is every connection after the first request"
    );

    drop(pool);
    let c = admin.get().await.expect("connect");
    let _ = c.execute(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"), &[]).await;
}
