//! Signing on, over HTTP, against the properties that make it worth having.
//!
//! D11 decided in 2026-08-02 that `recorded_by_id` *"comes from the
//! authenticated session, is never client-supplied, and is never editable"*, and
//! nothing built the session. These tests are for the half that is easy to get
//! subtly wrong: what the server says when the answer is no.

use actix_web::{test, web, App};
use spork_server::{auth, routes, AppState};
use serde_json::{json, Value};
use std::sync::OnceLock;
use tokio::sync::{Mutex, MutexGuard};
use uuid::Uuid;

mod common;
use common::{pool, url};

/// **These share the seeded accounts, so they run one at a time.**
///
/// Four of them sign on as `kyle@example.test` and finish by deleting that
/// person's sessions and resetting the lockout counter — which is correct for a
/// test running alone and wrong for four on threads: whichever finishes first
/// revokes the tokens the others are still using, and the reset erases the
/// failed attempt `a_wrong_password_…` had just recorded.
///
/// `where_you_are_working.rs` had the same shape and it went red on CI run 83,
/// having passed eighty-two times. Here the state is a lockout counter rather
/// than a session, and a counter cannot be scoped by token — so this file takes
/// the lock `ledger_http.rs` takes, for the same reason.
static SEEDED: OnceLock<Mutex<()>> = OnceLock::new();
async fn one_at_a_time() -> MutexGuard<'static, ()> {
    SEEDED.get_or_init(|| Mutex::new(())).lock().await
}

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const BETA: &str = "22222222-2222-2222-2222-222222222222";
const KYLE: &str = "kyle@example.test";
const DANA: &str = "dana@example.test";
const PASSWORD: &str = "dock-station-1";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";

macro_rules! app {
    ($u:expr) => {{
        let state = web::Data::new(AppState { pool: pool($u) });
        test::init_service(App::new().app_data(state).configure(routes::configure)).await
    }};
}

fn sign_on_req(email: &str, password: &str) -> actix_http::Request {
    test::TestRequest::post()
        .uri("/sessions")
        .set_json(json!({ "email": email, "password": password, "site_id": SITE }))
        .to_request()
}

#[actix_web::test]
async fn a_correct_password_opens_a_session_and_sets_a_hardened_cookie() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let _seeded = one_at_a_time().await;
    let app = app!(&u);

    let resp = test::call_service(&app, sign_on_req(KYLE, PASSWORD)).await;
    assert!(resp.status().is_success(), "signing on with the right password");

    let cookie = resp
        .headers()
        .get("set-cookie")
        .expect("a session cookie")
        .to_str()
        .unwrap()
        .to_string();
    // Every flag the `__Host-` prefix requires, and the two that matter beyond
    // it. `SameSite=Strict` is what stands in for a CSRF token: the browser
    // simply does not send the cookie on a cross-site request.
    for flag in ["HttpOnly", "SameSite=Strict", "Path=/"] {
        assert!(cookie.contains(flag), "{flag} missing from {cookie}");
    }
    assert!(!cookie.contains("Domain"), "a __Host- cookie forbids Domain");

    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["tenant_id"], ALPHA, "the one tenant this person belongs to");
    assert_eq!(body["display_name"], "Kyle Phillips");
    let token = body["token"].as_str().expect("a bearer for non-browser clients");
    assert_eq!(token.len(), 64, "256 bits, against OWASP's 64-bit floor");

    // The session answers for itself, through the bearer transport.
    let who = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/sessions/current")
            .insert_header(("authorization", format!("Bearer {token}")))
            .to_request(),
    )
    .await;
    assert!(who.status().is_success());
    let who: Value = test::read_body_json(who).await;
    assert_eq!(who["person_id"], body["person_id"]);
    assert!(who["site_code"].is_string(), "the site they signed on at");

    // And signing off kills it everywhere at once, which is the thing a
    // server-side record buys over a signed claim the client carries.
    let off = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/sessions/current")
            .insert_header(("authorization", format!("Bearer {token}")))
            .to_request(),
    )
    .await;
    assert!(off.status().is_success());

    let after = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/sessions/current")
            .insert_header(("authorization", format!("Bearer {token}")))
            .to_request(),
    )
    .await;
    assert_eq!(after.status(), 401, "a revoked token is nobody");

    cleanup(&u).await;
}

/// Every way of being refused looks the same from outside.
#[actix_web::test]
async fn a_wrong_password_and_an_unknown_address_are_indistinguishable() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let _seeded = one_at_a_time().await;
    let app = app!(&u);

    let wrong = test::call_service(&app, sign_on_req(KYLE, "not the password")).await;
    let missing = test::call_service(&app, sign_on_req("nobody@example.test", PASSWORD)).await;

    assert_eq!(wrong.status(), 401);
    assert_eq!(missing.status(), 401);
    let a: Value = test::read_body_json(wrong).await;
    let b: Value = test::read_body_json(missing).await;
    assert_eq!(
        a, b,
        "OWASP: one generic message for wrong password, missing account and \
         locked account alike — any difference is an oracle for who works here"
    );
    assert!(
        !serde_json::to_string(&a).unwrap().contains("password"),
        "and it does not say which half was wrong: {a}"
    );

    cleanup(&u).await;
}

/// The session decides the tenant, so a token cannot reach another one.
#[actix_web::test]
async fn a_session_reaches_only_its_own_tenants_data() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let _seeded = one_at_a_time().await;
    let app = app!(&u);

    // Dana belongs to Beta, not Alpha. D19 makes `person` global, so this is
    // exactly the case the session exists to disambiguate.
    //
    // **No site.** `session_site_fk` is composite through `tenant_id`, so
    // signing Dana on at Alpha's dock is refused by the key rather than by a
    // handler — S37's rule, one table further out, and the first draft of this
    // test walked straight into it.
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/sessions")
            .set_json(json!({ "email": DANA, "password": PASSWORD }))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["tenant_id"], BETA);

    // Naming a tenant they do not belong to is refused, and refused as the same
    // 401 — not as a 403, which would confirm the tenant exists.
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/sessions")
            .set_json(json!({
                "email": DANA, "password": PASSWORD, "tenant_id": ALPHA
            }))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), 401, "membership is not a request parameter");

    cleanup(&u).await;
}

/// A token that was never issued is nobody, and so is no token at all.
#[actix_web::test]
async fn an_invented_token_is_refused() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);

    for header in [
        Some(format!("Bearer {}", "0".repeat(64))),
        Some("Bearer nonsense".to_string()),
        None,
    ] {
        let mut req = test::TestRequest::get().uri("/sessions/current");
        if let Some(h) = header {
            req = req.insert_header(("authorization", h));
        }
        let resp = test::call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), 401, "only a token we issued is a session");
    }
}

/// What is stored is not what is presented.
#[actix_web::test]
async fn the_database_never_holds_the_token() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let _seeded = one_at_a_time().await;
    let app = app!(&u);
    let resp = test::call_service(&app, sign_on_req(KYLE, PASSWORD)).await;
    let body: Value = test::read_body_json(resp).await;
    let token = body["token"].as_str().unwrap().to_string();

    let (client, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });

    // The row is found by the digest and by nothing else, so a leaked backup
    // does not hand over live sessions.
    let by_digest: i64 = client
        .query_one(
            "SELECT count(*) FROM session WHERE token_sha256 = $1",
            &[&auth::token_digest(&token)],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(by_digest, 1);

    let plaintext: i64 = client
        .query_one(
            "SELECT count(*) FROM session WHERE encode(token_sha256, 'hex') = $1",
            &[&token],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(plaintext, 0, "the token itself appears nowhere");

    cleanup(&u).await;
}

/// The application cannot read a credential or enumerate a session.
#[actix_web::test]
async fn the_app_role_cannot_read_the_tables_behind_the_functions() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let (client, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client.batch_execute("SET ROLE spork_app").await.unwrap();

    for table in ["person_credential", "session"] {
        let err = client
            .query(&format!("SELECT * FROM {table}"), &[])
            .await
            .expect_err("the app has no business reading this");
        assert_eq!(
            err.code(),
            Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE),
            "{table} should be unreachable; the definer functions are the interface"
        );
    }

    // And the functions it may call are the whole of that interface.
    for f in [
        "credential_for_login",
        "credential_record_attempt",
        "session_open",
        "session_resolve",
        "session_revoke",
    ] {
        let ok: bool = client
            .query_one(
                "SELECT has_function_privilege('spork_app', p.oid, 'EXECUTE')
                   FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
                  WHERE p.proname = $1 AND n.nspname = 'public'",
                &[&f],
            )
            .await
            .unwrap()
            .get(0);
        assert!(ok, "{f} is how the app gets in");
    }
}

/// Sessions are committed, so the tests remove their own.
async fn cleanup(u: &str) {
    let (client, connection) = tokio_postgres::connect(u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
        .execute(
            "DELETE FROM session WHERE person_id = ANY($1)",
            &[&vec![
                Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap(),
                Uuid::parse_str("77770000-0000-0000-0000-000000000002").unwrap(),
            ]],
        )
        .await
        .expect("the test removes what it committed");
    // The lockout counter is state too: a test that leaves it advanced makes the
    // next run's tenth attempt lock an account nobody attacked.
    client
        .execute(
            "UPDATE person_credential SET failed_attempts = 0, locked_until = NULL",
            &[],
        )
        .await
        .expect("reset the attempt counters");
}
