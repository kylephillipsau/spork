//! Saying where you are working, and the session that says so.
//!
//! Every browser session in this system has had `site_id = NULL`, because the
//! sign-in page posts an email and a password and nothing else. `SignOnRequest`
//! has always accepted a site and `Caller::site_id` has always carried one; no
//! form ever sent it.
//!
//! It has been latent rather than active — the seed gives each tenant one site
//! and row-level security already narrows every read to the tenant — but it
//! becomes a live correctness bug the moment a tenant has a second warehouse,
//! and it blocks D112 outright: every badge and the landing screen are defined
//! as *"work waiting for you, at this site, now"*, and there was no site to
//! name.
//!
//! # A new session, not an update
//!
//! `session.site_id` is documented as "where they signed on". Mutating the row
//! under a live token would make one session name two places over its life with
//! nothing in the record saying when it changed. So the site question revokes
//! and reissues, through the same `issue_session` every other way in uses.

use actix_web::{test, web, App};
use spork_server::auth::token_digest;
use spork_server::{routes, AppState};
use serde_json::{json, Value};
use uuid::Uuid;

mod common;
use common::{pool, url};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const MEL: &str = "a5170000-0000-0000-0000-000000000001";
/// Beta's site. Another tenant's, which is the point of it being here.
const SYD: &str = "a5170000-0000-0000-0000-000000000002";
const KYLE: &str = "kyle@example.test";
const PASSWORD: &str = "dock-station-1";

macro_rules! app {
    ($u:expr) => {{
        let state = web::Data::new(AppState { pool: pool($u) });
        test::init_service(App::new().app_data(state).configure(routes::configure)).await
    }};
}

/// Sign on **without naming a site**, exactly as the browser has always done.
async fn sign_on_blind<S>(app: &S) -> String
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let resp = test::call_service(
        app,
        test::TestRequest::post()
            .uri("/sessions")
            .set_json(json!({ "email": KYLE, "password": PASSWORD }))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success(), "the seeded password");
    let body: Value = test::read_body_json(resp).await;
    body["token"].as_str().expect("a bearer").to_string()
}

async fn whoami<S>(app: &S, token: &str) -> Value
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let resp = test::call_service(
        app,
        test::TestRequest::get()
            .uri("/sessions/current")
            .insert_header(("authorization", format!("Bearer {token}")))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    test::read_body_json(resp).await
}

fn choose(token: &str, site: &str) -> actix_http::Request {
    test::TestRequest::post()
        .uri("/sessions/site")
        .insert_header(("authorization", format!("Bearer {token}")))
        .set_json(json!({ "site_id": site }))
        .to_request()
}

/// The whole point: a session that named no site comes to name one, and the old
/// one dies rather than lingering with a different answer.
#[actix_web::test]
async fn a_session_with_no_site_can_be_told_where_it_is_working() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);
    let blind = sign_on_blind(&app).await;

    // The state every browser session has been in.
    let before = whoami(&app, &blind).await;
    assert!(
        before["site_id"].is_null(),
        "signing on without naming a site leaves it null: {before}"
    );

    // The choices, and the one currently in force.
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/sites")
            .insert_header(("authorization", format!("Bearer {blind}")))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let sites: Value = test::read_body_json(resp).await;
    let list = sites.as_array().expect("a list");
    assert!(!list.is_empty(), "this tenant has somewhere to work");
    assert!(
        list.iter().all(|s| s["current"] == false),
        "nothing is current yet, because nothing was chosen: {sites}"
    );
    // Row-level security, not a WHERE clause the handler wrote: Beta's site is
    // simply not visible from Alpha's scope.
    assert!(
        list.iter().all(|s| s["id"] != SYD),
        "another tenant's warehouse is not on this list: {sites}"
    );

    let resp = test::call_service(&app, choose(&blind, MEL)).await;
    assert!(resp.status().is_success(), "choosing a site this tenant has");
    let issued: Value = test::read_body_json(resp).await;
    assert_eq!(issued["site_id"], MEL);
    assert_eq!(issued["tenant_id"], ALPHA);
    let after_token = issued["token"].as_str().expect("a fresh bearer").to_string();
    assert_ne!(after_token, blind, "a new session, not the same one edited");

    let after = whoami(&app, &after_token).await;
    assert_eq!(after["site_id"], MEL);
    assert_eq!(after["site_code"], "MEL", "and it can read the code back");
    assert_eq!(
        after["person_id"], before["person_id"],
        "same person throughout"
    );

    // **The old one is dead.** Two live sessions for one person disagreeing
    // about where they are standing is the state this must not leave behind.
    let stale = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/sessions/current")
            .insert_header(("authorization", format!("Bearer {blind}")))
            .to_request(),
    )
    .await;
    assert_eq!(
        stale.status().as_u16(),
        401,
        "the session that named no site is gone"
    );

    // Asking again marks the current one, which is what the screen draws.
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/sites")
            .insert_header(("authorization", format!("Bearer {after_token}")))
            .to_request(),
    )
    .await;
    let sites: Value = test::read_body_json(resp).await;
    let current: Vec<&Value> = sites
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["current"] == true)
        .collect();
    assert_eq!(current.len(), 1, "exactly one is where you are: {sites}");
    assert_eq!(current[0]["id"], MEL);

    cleanup(&u, &[&blind, &after_token]).await;
}

/// Another tenant's warehouse is refused with a sentence, and the caller keeps
/// the session they had.
///
/// The composite foreign key `(site_id, tenant_id)` would refuse it anyway —
/// but as a constraint violation, which reaches the operator as a 500. The
/// check exists so the answer is a sentence rather than a stack trace, and the
/// test exists so the check cannot quietly be removed on the grounds that the
/// database has it covered.
#[actix_web::test]
async fn another_tenants_site_is_refused_and_costs_nothing() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);
    let token = sign_on_blind(&app).await;

    let resp = test::call_service(&app, choose(&token, SYD)).await;
    assert_eq!(
        resp.status().as_u16(),
        400,
        "refused, and not with a constraint violation"
    );

    // Still signed in. A question answered wrongly must not sign somebody out.
    let after = whoami(&app, &token).await;
    assert!(after["site_id"].is_null(), "and nothing was changed");

    cleanup(&u, &[&token]).await;
}

/// No session, no sites, and no way to ask where anybody works.
#[actix_web::test]
async fn the_sites_are_not_readable_without_a_session() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);

    for req in [
        test::TestRequest::get().uri("/sites").to_request(),
        test::TestRequest::post()
            .uri("/sessions/site")
            .set_json(json!({ "site_id": MEL }))
            .to_request(),
    ] {
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401, "not signed in");
    }
}

/// Remove the sessions **this test** issued, and nothing else.
///
/// This deleted every session belonging to the seeded person, which is one row
/// while a test runs alone and is not what happens: two tests here sign in as
/// the same person, `cargo test` runs them on separate threads, and whichever
/// finished first deleted the other's live token. The other then failed on its
/// next `whoami` with a 401 — reported as `assertion failed:
/// resp.status().is_success()` at a line that had nothing to do with it.
///
/// It passed locally and on eighty-two CI runs before it did not, which is what
/// a race looks like from outside. Deleting by digest is narrower *and*
/// simpler than serialising the tests: a test that only removes what it made
/// has nothing to be serialised against.
async fn cleanup(u: &str, tokens: &[&str]) {
    let (client, connection) = tokio_postgres::connect(u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let digests: Vec<Vec<u8>> = tokens.iter().map(|t| token_digest(t)).collect();
    client
        .execute(
            "DELETE FROM session WHERE person_id = $1 AND token_sha256 = ANY($2)",
            &[
                &Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap(),
                &digests,
            ],
        )
        .await
        .expect("the test removes what it committed");
}
