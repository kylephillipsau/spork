//! Confirming a weight, and what happens when the scale disagrees.
//!
//! **The act that makes an interval mean something.** Until a value has been
//! measured once, its `observed_at` is the date of the import that carried it,
//! so no schedule can be built on it. These cover the loop: what the worklist
//! offers, what recording does, and that a disagreement becomes a finding rather
//! than an overwrite.

use actix_web::{test, web, App};
use spork_server::{routes, AppState};
use serde_json::{json, Value};
use uuid::Uuid;

mod common;
use common::{pool, url};

const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const GUMBOOT: &str = "17e10000-0000-0000-0000-000000000002";

async fn sign_in(app: &impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
>) -> (String, String) {
    let r = test::call_service(
        app,
        test::TestRequest::post().uri("/sessions").set_json(json!({
            "email": "kyle@example.test", "password": "dock-station-1", "site_id": SITE
        })).to_request(),
    ).await;
    assert!(r.status().is_success());
    let b: Value = test::read_body_json(r).await;
    let token = b["token"].as_str().unwrap().to_string();
    (format!("Bearer {token}"), token)
}

async fn cleanup(u: &str, acts: &[Uuid], token: &str) {
    let (c, conn) = tokio_postgres::connect(u, tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(async move { let _ = conn.await; });
    // **By act, never by a predicate.** The first version also deleted every
    // recent `identity_mismatch` whose detail looked like a weighing, which
    // reached into the finding another test in this binary had just raised and
    // was still asserting on. Third time that shape has bitten in this codebase;
    // a test may remove what it created and nothing else.
    c.batch_execute(&format!(
        "DELETE FROM observation WHERE client_event_id = ANY(ARRAY[{acts}]::uuid[]);
         DELETE FROM observation_event WHERE client_event_id = ANY(ARRAY[{acts}]::uuid[]);
         DELETE FROM client_event WHERE client_event_id = ANY(ARRAY[{acts}]::uuid[]);",
        acts = acts.iter().map(|a| format!("'{a}'")).collect::<Vec<_>>().join(",")
    )).await.expect("the test removes what it recorded");
    c.execute(
        "DELETE FROM session WHERE token_sha256 = $1",
        &[&spork_server::auth::token_digest(token)],
    ).await.expect("and the session it opened");
}

/// The worklist offers what has never been measured, ahead of what has.
#[actix_web::test]
async fn the_worklist_puts_the_unmeasured_first() {
    let Some(u) = url() else { eprintln!("no DATABASE_URL: skipping"); return };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)
            .configure(spork_server::web::configure)).await;
    let (bearer, token) = sign_in(&app).await;

    let r = test::call_service(&app, test::TestRequest::get()
        .uri("/revalidation?limit=50").insert_header(("authorization", bearer)).to_request()).await;
    assert!(r.status().is_success());
    let list: Value = test::read_body_json(r).await;
    let rows = list.as_array().expect("a worklist");
    assert!(!rows.is_empty(), "the fixture holds transcribed weights, so there is work");

    // Never-measured sort ahead of overdue, and every row says which it is.
    let mut seen_overdue = false;
    for r in rows {
        let because = r["because"].as_str().unwrap();
        assert!(matches!(because, "never" | "overdue"), "unexpected reason {because}");
        if because == "overdue" { seen_overdue = true; }
        else { assert!(!seen_overdue, "a never-measured row came after an overdue one"); }
        // And each carries what it is replacing, so a person can see the claim
        // before they weigh anything.
        assert!(r["code"].is_string());
        assert!(r["packaging_level"].is_string());
    }
    // The fixture's one instrument reading is not offered: it is neither
    // unmeasured nor old.
    assert!(
        rows.iter().all(|r| r["held_method"] != "instrument"),
        "a fresh scale reading should not be on the list"
    );
    cleanup(&u, &[], &token).await;
}

/// A first weighing establishes, and cannot contradict.
#[actix_web::test]
async fn a_first_weighing_raises_nothing() {
    let Some(u) = url() else { eprintln!("no DATABASE_URL: skipping"); return };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)
            .configure(spork_server::web::configure)).await;
    let (bearer, token) = sign_in(&app).await;

    // The gumboot's `each` level carries one weight in the fixture, 1.9 kg.
    // Weighing it at very nearly that is a confirmation rather than a
    // contradiction.
    let act = Uuid::now_v7();
    let r = test::call_service(&app, test::TestRequest::post().uri("/weighings")
        .insert_header(("authorization", bearer)).set_json(json!({
            "item_id": GUMBOOT, "packaging_level": "each",
            "entered_value": "1.91", "unit": "kg",
            "client_event_id": act, "occurred_at": chrono::Utc::now().to_rfc3339()
        })).to_request()).await;
    assert!(r.status().is_success(), "a weighing is recordable");
    let out: Value = test::read_body_json(r).await;
    assert_eq!(out["recorded_g"], 1910);
    assert_eq!(out["previous_g"], 1900, "and it says what it replaced");
    assert_eq!(out["disagreed"], false, "ten grams on 1.9 kg is not a finding");
    assert!(out["discrepancy_id"].is_null());

    cleanup(&u, &[act], &token).await;
}

/// **The point of the exercise.** A scale that disagrees with a spreadsheet
/// produces a finding carrying both, rather than quietly replacing one with the
/// other.
#[actix_web::test]
async fn a_disagreeing_scale_raises_a_finding_and_keeps_both() {
    let Some(u) = url() else { eprintln!("no DATABASE_URL: skipping"); return };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)
            .configure(spork_server::web::configure)).await;
    let (bearer, token) = sign_in(&app).await;

    let act = Uuid::now_v7();
    let r = test::call_service(&app, test::TestRequest::post().uri("/weighings")
        .insert_header(("authorization", bearer.clone())).set_json(json!({
            "item_id": GUMBOOT, "packaging_level": "each",
            "entered_value": "3.10", "unit": "kg",
            "client_event_id": act, "occurred_at": chrono::Utc::now().to_rfc3339()
        })).to_request()).await;
    assert!(r.status().is_success());
    let out: Value = test::read_body_json(r).await;
    assert_eq!(out["recorded_g"], 3100);
    assert_eq!(out["disagreed"], true, "1.9 kg against 3.1 kg is worth a look");
    let finding = out["discrepancy_id"].as_str().expect("a finding was raised");

    let (c, conn) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(async move { let _ = conn.await; });
    let row = c.query_one(
        "SELECT expected_quantity::text, observed_quantity::text, variance::text, detail
           FROM discrepancy WHERE id = $1::text::uuid",
        &[&finding],
    ).await.unwrap();
    // Both numbers are on the finding, and the difference is computed rather
    // than supplied: `variance` is a generated column, so it cannot disagree
    // with the two values it comes from.
    assert_eq!(row.get::<_, String>(0), "1900");
    assert_eq!(row.get::<_, String>(1), "3100");
    assert_eq!(row.get::<_, String>(2), "1200");
    // The finding names whatever it disagreed with, which for this fixture row
    // is another instrument reading — two scales disagreeing is as much a
    // finding as a scale disagreeing with a spreadsheet.
    let previous_method = out["previous_method"].as_str().expect("a previous method");
    assert!(
        row.get::<_, String>(3).contains(previous_method),
        "the finding should name {previous_method}"
    );

    // And the earlier value is still there: a weighing adds a fact, it does not
    // erase one.
    let kept: i64 = c.query_one(
        "SELECT count(*) FROM observation o
           JOIN observable ob ON ob.id = o.observable_id
           JOIN metric m ON m.id = o.metric_id
          WHERE ob.item_id = $1::text::uuid AND ob.packaging_level = 'each'
            AND m.code = 'gross_weight'",
        &[&GUMBOOT],
    ).await.unwrap().get(0);
    assert!(kept >= 2, "both the old value and the new one are on file");

    c.execute("DELETE FROM discrepancy WHERE id = $1::text::uuid", &[&finding]).await.unwrap();
    cleanup(&u, &[act], &token).await;
}

/// Naming both subjects, or neither, is refused rather than guessed at.
#[actix_web::test]
async fn a_weighing_names_exactly_one_subject() {
    let Some(u) = url() else { eprintln!("no DATABASE_URL: skipping"); return };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)
            .configure(spork_server::web::configure)).await;
    let (bearer, token) = sign_in(&app).await;

    for body in [
        json!({ "packaging_level": "each", "entered_value": "1", "unit": "kg",
                "client_event_id": Uuid::now_v7(), "occurred_at": chrono::Utc::now().to_rfc3339() }),
        json!({ "item_id": GUMBOOT, "item_style_id": "57110000-0000-0000-0000-000000000001",
                "packaging_level": "each", "entered_value": "1", "unit": "kg",
                "client_event_id": Uuid::now_v7(), "occurred_at": chrono::Utc::now().to_rfc3339() }),
    ] {
        let r = test::call_service(&app, test::TestRequest::post().uri("/weighings")
            .insert_header(("authorization", bearer.clone())).set_json(body).to_request()).await;
        assert_eq!(r.status(), 400, "neither and both are equally refused");
    }
    cleanup(&u, &[], &token).await;
}
