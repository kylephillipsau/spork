//! Where the system of record says a thing is, loaded over HTTP (D158).
//!
//! The load is the easy half. What this file is really about is the two
//! refusals: a report that cannot say when it was taken, and a file whose row
//! count disagrees with what it was said to be. The second is the one that
//! matters, because a truncated inventory export is the failure that reads as
//! success — a bin missing from it looks empty, and an empty bin is an
//! instruction to put something there.

use actix_web::{test, web, App};
use spork_server::{routes, AppState};
use serde_json::{json, Value};
use uuid::Uuid;

mod common;
use common::{pool, url};

/// The fixture's glove, its Melbourne pick face, and the warehouse name
/// `bins::site_name` matches onto the site the fixture already has.
const HEADER: &str = "Item,Location,Bin Number,On Hand,Available,Status\n";

fn csv(qty: &[&str]) -> String {
    let mut s = String::from(HEADER);
    for q in qty {
        s.push_str(&format!("GLOVE-M,Melbourne Warehouse,A-01-1,{q},{q},Good\n"));
    }
    s
}

async fn token<S>(app: &S) -> String
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let session = common::bearer_without_site(app).await;
    let minted: Value = common::ok_json(
        app,
        test::TestRequest::post()
            .uri("/tokens")
            .insert_header(("authorization", session))
            .set_json(json!({ "label": "the inventory balance, from a test" }))
            .to_request(),
        "minting an import token",
    )
    .await;
    format!("Bearer {}", minted["token"].as_str().unwrap())
}

#[actix_web::test]
async fn a_report_lands_in_reported_stock_and_never_in_stock() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let bearer = token(&app).await;

    // Unique per run so a second pass over the same database replaces its own
    // rows rather than reading the previous run's as this one's.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let source = format!("test-balance-{}", nonce % 1_000_000);
    let as_at = "2026-09-10T02:00:00Z";
    let base = format!("/import/stock?as_at={as_at}&source={source}");

    // ── neither date nor feed has a defensible default ──────────────────
    let r = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/import/stock?source={source}"))
            .insert_header(("authorization", bearer.clone()))
            .set_payload(csv(&["4"]))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 400, "a report with no as_at is refused");

    let r = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/import/stock?as_at={as_at}"))
            .insert_header(("authorization", bearer.clone()))
            .set_payload(csv(&["4"]))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 400, "a report with no source is refused");

    let held = |src: String| {
        let pool = pool(&u);
        async move {
            let conn = pool.get().await.unwrap();
            conn.query_one(
                "SELECT coalesce(sum(on_hand), 0)::text, count(*)
                   FROM reported_stock WHERE source = $1",
                &[&src],
            )
            .await
            .unwrap()
        }
    };

    // ── a dry run reports and keeps nothing ─────────────────────────────
    let dry: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri(&base)
            .insert_header(("authorization", bearer.clone()))
            .set_payload(csv(&["4"]))
            .to_request(),
        "a dry run",
    )
    .await;
    assert_eq!(dry["loaded"]["applied"], false);
    assert_eq!(dry["loaded"]["rows_written"], 1, "it says what it would write");
    assert_eq!(dry["survey"]["positioned"], 1, "the row names a shelf");
    assert!(dry["refused"].is_null(), "no count was given, so nothing was checked");
    assert_eq!(held(source.clone()).await.get::<_, i64>(1), 0, "and it wrote nothing");

    // ── applying keeps it ───────────────────────────────────────────────
    let wet: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}&apply=true"))
            .insert_header(("authorization", bearer.clone()))
            .set_payload(csv(&["4"]))
            .to_request(),
        "applying",
    )
    .await;
    assert_eq!(wet["loaded"]["rows_written"], 1);
    assert_eq!(wet["arrival"]["replay"], false, "the file is on file");
    let row = held(source.clone()).await;
    assert_eq!(row.get::<_, i64>(1), 1);
    assert_eq!(row.get::<_, &str>(0), "4");

    // **The arrival is `parsed`, and `stock` is untouched.** This report is not
    // a movement and migration 86 is emphatic about why: turning it into one
    // would have this system assert it holds goods it never recorded receiving.
    let conn = pool(&u).get().await.unwrap();
    let status: String = conn
        .query_one(
            "SELECT m.parse_status::text FROM party_message m
              ORDER BY m.recorded_at DESC LIMIT 1",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(status, "parsed");

    // ── a snapshot replaces, it does not accumulate ─────────────────────
    let again: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}&apply=true"))
            .insert_header(("authorization", bearer.clone()))
            .set_payload(csv(&["9"]))
            .to_request(),
        "a fresh export under the same source",
    )
    .await;
    assert_eq!(again["loaded"]["rows_replaced"], 1, "the previous load was cleared");
    let row = held(source.clone()).await;
    assert_eq!(row.get::<_, i64>(1), 1, "one row, not two");
    assert_eq!(row.get::<_, &str>(0), "9", "two copies of a snapshot is not twice the stock");
}

#[actix_web::test]
async fn a_file_that_is_not_the_size_it_was_said_to_be_loads_nothing() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let bearer = token(&app).await;

    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let source = format!("test-truncated-{}", nonce % 1_000_000);

    // Three rows, said to be four. This is what a truncated saved-search email
    // looks like from here: a well-formed file that is quietly missing the end.
    let report: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri(&format!(
                "/import/stock?as_at=2026-09-10T02:00:00Z&source={source}&expect=4&apply=true"
            ))
            .insert_header(("authorization", bearer.clone()))
            .set_payload(csv(&["1", "2", "3"]))
            .to_request(),
        "a short file",
    )
    .await;

    let refused = report["refused"].as_str().expect("it says why it refused");
    assert!(refused.contains("truncated"), "{refused}");
    assert_eq!(report["loaded"]["rows_written"], 0, "nothing was loaded");

    let conn = pool(&u).get().await.unwrap();
    let kept: i64 = conn
        .query_one(
            "SELECT count(*) FROM reported_stock WHERE source = $1",
            &[&source],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(kept, 0, "a partial inventory is worse than none: it reads as empty shelves");

    // **The bytes are kept and marked, which is what `partial` is for.** The
    // file arrived; we declined to act on it. That distinction is the reason
    // `party_message` carries a parse status at all.
    let id = Uuid::parse_str(report["arrival"]["party_message_id"].as_str().unwrap()).unwrap();
    let status: String = conn
        .query_one("SELECT parse_status::text FROM party_message WHERE id = $1", &[&id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(status, "partial");
}
