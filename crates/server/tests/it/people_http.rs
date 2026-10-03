//! The people of a workspace, over HTTP (D205): an administrator adds a
//! person who then signs in as themselves, says what they are, and takes them
//! out, which ends their sessions at once; and a workspace always keeps an
//! administrator.
//!
//! **The test makes its own workspace.** "The last administrator stays one" is
//! only testable where there is exactly one, and the seeded workspace has its
//! own administrator that the rest of the suite signs on as. A tenant is a
//! tenant row and a site, as setup makes them (setup.rs).

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{auth, routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

const PASSWORD: &str = "the-administrators-password";
const FIRST: &str = "a-first-password-to-hand-over";

macro_rules! app {
    ($u:expr) => {{
        let state = web::Data::new(AppState { pool: pool($u) });
        test::init_service(App::new().app_data(state).configure(routes::configure)).await
    }};
}

/// A raw connection, as the owner: making a workspace is the fixture here.
async fn raw(u: &str) -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(u, tokio_postgres::NoTls).await.expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

/// Sign on at `site` and return the bearer, or `None` when it is refused.
async fn sign_on<S>(app: &S, email: &str, password: &str, site: Uuid) -> Option<String>
where
    S: actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error>,
{
    let resp = test::call_service(
        app,
        test::TestRequest::post()
            .uri("/sessions")
            .set_json(json!({ "email": email, "password": password, "site_id": site }))
            .to_request(),
    )
    .await;
    if !resp.status().is_success() {
        return None;
    }
    let body: Value = test::read_body_json(resp).await;
    Some(body["token"].as_str().expect("a bearer").to_string())
}

fn get(token: &str, uri: &str) -> actix_http::Request {
    test::TestRequest::get()
        .uri(uri)
        .insert_header(("authorization", format!("Bearer {token}")))
        .to_request()
}

fn post(token: &str, uri: &str, body: Value) -> actix_http::Request {
    test::TestRequest::post()
        .uri(uri)
        .insert_header(("authorization", format!("Bearer {token}")))
        .set_json(body)
        .to_request()
}

/// Everybody named in the list, by email.
fn find<'a>(people: &'a Value, email: &str) -> &'a Value {
    people
        .as_array()
        .expect("a list")
        .iter()
        .find(|p| p["email"] == email)
        .unwrap_or_else(|| panic!("{email} is in the list: {people}"))
}

#[actix_web::test]
async fn an_administrator_adds_a_picker_who_signs_in_as_themselves() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);
    let db = raw(&u).await;

    // ── a workspace of its own, with one administrator ──────────────────
    let slug = format!("people-{}", Uuid::new_v4().simple());
    let tenant: Uuid = db
        .query_one("INSERT INTO tenant (name, slug) VALUES ('People test', $1) RETURNING id", &[&slug])
        .await
        .expect("a tenant")
        .get(0);
    let site: Uuid = db
        .query_one(
            "INSERT INTO site (tenant_id, name, code, timezone) VALUES ($1, 'Floor', 'PPL', 'Australia/Melbourne') RETURNING id",
            &[&tenant],
        )
        .await
        .expect("a site")
        .get(0);
    let admin_email = format!("admin@{slug}.test");
    let admin: Uuid = db
        .query_one("INSERT INTO person (display_name, email) VALUES ('Ada Admin', $1) RETURNING id", &[&admin_email])
        .await
        .expect("a person")
        .get(0);
    db.execute(
        "INSERT INTO person_credential (person_id, kind, phc) VALUES ($1, 'password', $2)",
        &[&admin, &auth::hash_password(PASSWORD).expect("hash")],
    )
    .await
    .expect("a credential");
    db.execute(
        "INSERT INTO person_tenant (person_id, tenant_id, role) VALUES ($1, $2, 'administrator')",
        &[&admin, &tenant],
    )
    .await
    .expect("a membership");
    let boss = sign_on(&app, &admin_email, PASSWORD, site).await.expect("the administrator signs on");

    // ── the list starts with them, marked as the one looking ────────────
    let people = common::ok_json(&app, get(&boss, "/workspace/people"), "the people").await;
    assert_eq!(people.as_array().map(Vec::len), Some(1), "{people}");
    assert_eq!(people[0]["you"], true, "{people}");
    assert_eq!(people[0]["role"], "administrator");
    assert_eq!(people[0]["password"], true);

    // ── adding a picker ─────────────────────────────────────────────────
    let picker_email = format!("Sam@{slug}.test");
    let short = test::call_service(
        &app,
        post(&boss, "/workspace/people", json!({ "display_name": "Sam", "email": picker_email, "password": "short", "role": "operator" })),
    )
    .await;
    assert_eq!(short.status().as_u16(), 400, "a first password is at least twelve characters");
    let odd = test::call_service(
        &app,
        post(&boss, "/workspace/people", json!({ "display_name": "Sam", "email": picker_email, "password": FIRST, "role": "picker" })),
    )
    .await;
    assert_eq!(odd.status().as_u16(), 400, "a role is administrator or operator");

    let added = common::ok_json(
        &app,
        post(&boss, "/workspace/people", json!({ "display_name": " Sam Rivera ", "email": picker_email, "password": FIRST, "role": "operator" })),
        "adding Sam",
    )
    .await;
    assert_eq!(added["existing"], false, "{added}");
    let sam: Uuid = added["person_id"].as_str().and_then(|s| s.parse().ok()).expect("an id");
    // The email is kept as typed in lower case, so signing on finds it either way.
    let sam_email = picker_email.to_lowercase();
    let again = test::call_service(
        &app,
        post(&boss, "/workspace/people", json!({ "display_name": "Sam", "email": sam_email, "password": FIRST, "role": "operator" })),
    )
    .await;
    assert_eq!(again.status().as_u16(), 400, "nobody is added twice");

    let picking = sign_on(&app, &sam_email, FIRST, site).await.expect("Sam signs on as themselves");
    let me = common::ok_json(&app, get(&picking, "/sessions/current"), "who Sam is").await;
    assert_eq!(me["administrator"], false, "{me}");
    let refused = test::call_service(&app, get(&picking, "/workspace/people")).await;
    assert_eq!(refused.status().as_u16(), 403, "an operator does not see People");

    // ── what they are ───────────────────────────────────────────────────
    let promoted = test::call_service(&app, post(&boss, &format!("/workspace/people/{sam}/role"), json!({ "role": "administrator" }))).await;
    assert!(promoted.status().is_success(), "{}", promoted.status());
    let people = common::ok_json(&app, get(&boss, "/workspace/people"), "the people").await;
    assert_eq!(find(&people, &sam_email)["role"], "administrator");
    let demoted = test::call_service(&app, post(&boss, &format!("/workspace/people/{sam}/role"), json!({ "role": "operator" }))).await;
    assert!(demoted.status().is_success());

    // ── the workspace keeps an administrator, and nobody removes themselves
    let alone = test::call_service(&app, post(&boss, &format!("/workspace/people/{admin}/role"), json!({ "role": "operator" }))).await;
    assert_eq!(alone.status().as_u16(), 400, "the last administrator stays one");
    let myself = test::call_service(&app, post(&boss, &format!("/workspace/people/{admin}/leave"), json!({}))).await;
    assert_eq!(myself.status().as_u16(), 400, "nobody takes themselves out");
    let stranger = test::call_service(&app, post(&boss, &format!("/workspace/people/{}/leave", Uuid::new_v4()), json!({}))).await;
    assert_eq!(stranger.status().as_u16(), 404);

    // ── taking Sam out ends their session at once ───────────────────────
    let gone = test::call_service(&app, post(&boss, &format!("/workspace/people/{sam}/leave"), json!({}))).await;
    assert!(gone.status().is_success(), "{}", gone.status());
    let stale = test::call_service(&app, get(&picking, "/sessions/current")).await;
    assert_eq!(stale.status().as_u16(), 401, "their session died when they left");
    assert!(sign_on(&app, &sam_email, FIRST, site).await.is_none(), "and they can't sign in again");
    let people = common::ok_json(&app, get(&boss, "/workspace/people"), "the people").await;
    assert!(!find(&people, &sam_email)["left_at"].is_null(), "they are listed as having left: {people}");
    assert_eq!(people[0]["email"], admin_email.as_str(), "current people first: {people}");

    // ── and back, with the sign-in they had ─────────────────────────────
    let back = common::ok_json(
        &app,
        post(&boss, "/workspace/people", json!({ "display_name": "Sam Rivera", "email": sam_email, "role": "operator" })),
        "bringing Sam back",
    )
    .await;
    assert_eq!(back["existing"], true, "{back}");
    assert_eq!(back["person_id"], sam.to_string().as_str());
    assert!(sign_on(&app, &sam_email, FIRST, site).await.is_some(), "Sam signs in again with their own password");

    // ── tidy up, sessions first because they name the person ────────────
    for person in [sam, admin] {
        for sql in [
            "DELETE FROM session WHERE person_id = $1",
            "DELETE FROM sign_on_attempt WHERE person_id = $1",
            "DELETE FROM person_credential WHERE person_id = $1",
            "DELETE FROM person_tenant WHERE person_id = $1",
            "DELETE FROM person WHERE id = $1",
        ] {
            let _ = db.execute(sql, &[&person]).await;
        }
    }
    let _ = db.execute("DELETE FROM site WHERE id = $1", &[&site]).await;
    let _ = db.execute("DELETE FROM tenant WHERE id = $1", &[&tenant]).await;
}
