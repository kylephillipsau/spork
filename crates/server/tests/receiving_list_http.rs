//! What is expected here, and receiving a line by the lot code on the carton.
//!
//! Two things this file exists for.
//!
//! # The read the write path had been waiting for
//!
//! `POST /receipts` takes an `expected_supply_id` and nothing said what to call
//! it about. `requires_lot` is on the wire because
//! [`nylonite_server::receiving::disposition`] **refuses** a line whose policy
//! wants a lot and has none — a screen that did not know would find out from a
//! refusal with the pallet already broken down.
//!
//! # A lot by its code, and an expiry that does not overwrite
//!
//! Nothing creates a `lot` row. The table has existed since migration 2 with
//! `expiry_date` on it, `stock.lot_id` points at it, the receiving policy can
//! demand one — and the only way to get one was a fixture INSERT. So an item
//! under a lot-requiring policy could not be received at all.
//!
//! The expiry rule is the half worth testing hardest: a blank on file is filled,
//! and a date that disagrees is kept while the conflict comes back as a warning.
//! Letting whoever received last decide is exactly the silent resolution D8
//! built the findings queue to prevent.
//!
//! # Its own promise, not the fixture's
//!
//! Both tests make an `expected_supply` of their own and remove it. The first
//! draft used the seeded one and passed alone and failed in the suite: another
//! file closes that promise with `close_promise`, and `closed_at IS NULL` is
//! part of what "still expected" means — so the list came back empty and an
//! `.expect` four lines down reported the wrong thing.
//!
//! That is the third time this session, and `change_password.rs` wrote the
//! lesson down first: *"a test whose subject is a credential cannot borrow a
//! credential the rest of the suite signs on with."* A promise is the same.
//!
//! Everything written here is removed.

use actix_web::{test, web, App};
use chrono::Utc;
use nylonite_server::{routes, AppState};
use serde_json::json;
use uuid::Uuid;

mod common;
use common::{pool, url};

const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const DOCK: &str = "10c00000-0000-0000-0000-000000000003";
const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const PO: &str = "90000000-0000-0000-0000-000000000001";
const ITEM: &str = "17e10000-0000-0000-0000-000000000001";

/// A promise this test owns, on the fixture's purchase order.
///
/// Returns `(expected_supply_id, purchase_order_line_id)` so the teardown can
/// take both away again.
async fn a_promise(db: &tokio_postgres::Client, quantity: i64) -> (Uuid, Uuid) {
    let line = Uuid::now_v7();
    let supply = Uuid::now_v7();
    db.execute(
        "INSERT INTO purchase_order_line
             (id, tenant_id, purchase_order_id, item_id, quantity_ordered)
         VALUES ($1, $2, $3, $4, $5)",
        &[
            &line,
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(PO).unwrap(),
            &Uuid::parse_str(ITEM).unwrap(),
            &quantity,
        ],
    )
    .await
    .expect("a line on the fixture's order");
    db.execute(
        "INSERT INTO expected_supply
             (id, tenant_id, site_id, item_id, purchase_order_line_id, quantity_expected)
         VALUES ($1, $2, $3, $4, $5, $6)",
        &[
            &supply,
            &Uuid::parse_str(TENANT).unwrap(),
            &Uuid::parse_str(SITE).unwrap(),
            &Uuid::parse_str(ITEM).unwrap(),
            &line,
            &quantity,
        ],
    )
    .await
    .expect("a promise of this test's own");
    (supply, line)
}

async fn forget_promise(db: &tokio_postgres::Client, supply: Uuid, line: Uuid) {
    db.execute("DELETE FROM expected_supply WHERE id = $1", &[&supply])
        .await
        .expect("the promise goes");
    db.execute("DELETE FROM purchase_order_line WHERE id = $1", &[&line])
        .await
        .expect("and its line");
}

macro_rules! app {
    ($u:expr) => {{
        let state = web::Data::new(AppState { pool: pool($u) });
        test::init_service(App::new().app_data(state).configure(routes::configure)).await
    }};
}

async fn raw(u: &str) -> tokio_postgres::Client {
    let (c, conn) = tokio_postgres::connect(u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = conn.await;
    });
    c
}

#[actix_web::test]
async fn the_expected_list_says_what_a_line_needs_before_the_pallet_is_broken_down() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);
    let db = raw(&u).await;
    let (mine, its_line) = a_promise(&db, 40).await;

    let anon = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/receiving"))
            .to_request(),
    )
    .await;
    assert_eq!(anon.status(), 401, "a machine is told plainly");

    let bearer = common::bearer(&app).await;
    let screen = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/receiving"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET what is expected",
    )
    .await;

    let lines = screen["lines"].as_array().expect("lines");
    // The promise this test made is on the list, by name. Asserting "not empty"
    // would pass on somebody else's row and fail when they close it.
    assert!(
        lines
            .iter()
            .any(|l| l["expected_supply_id"].as_str() == Some(&mine.to_string())),
        "this test's own promise is expected: {screen}"
    );

    for line in lines {
        let id = Uuid::parse_str(line["expected_supply_id"].as_str().unwrap()).unwrap();
        let row = db
            .query_one(
                "SELECT quantity_outstanding::bigint, closed_at IS NULL
                   FROM expected_supply WHERE id = $1",
                &[&id],
            )
            .await
            .expect("the promise");
        let outstanding: i64 = row.get(0);
        let open: bool = row.get(1);
        assert!(open, "a closed promise is not still expected: {line}");
        assert!(outstanding > 0, "nothing outstanding is not expected: {line}");
        assert_eq!(line["outstanding"].as_i64(), Some(outstanding), "{line}");

        // `each` is always answerable and needs no config; anything above it
        // names the config `POST /receipts` will demand (J57).
        let levels = line["levels"].as_array().expect("levels");
        assert_eq!(levels[0]["level"].as_str(), Some("each"), "{line}");
        assert_eq!(levels[0]["units"].as_i64(), Some(1), "{line}");
        for level in levels.iter().skip(1) {
            assert!(
                level["item_packing_config_id"].is_string(),
                "a level above each names its config: {level}"
            );
            assert!(
                level["units"].as_i64().unwrap_or(0) > 1,
                "a rung converts to more than one each: {level}"
            );
        }
        assert!(line["requires_lot"].is_boolean(), "{line}");
    }

    forget_promise(&db, mine, its_line).await;
}

#[actix_web::test]
async fn a_lot_arrives_by_its_code_and_a_stated_expiry_never_overwrites() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);
    let db = raw(&u).await;
    let bearer = common::bearer(&app).await;
    let (mine, its_line) = a_promise(&db, 40).await;

    let screen = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/receiving"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET what is expected",
    )
    .await;
    let line = screen["lines"]
        .as_array()
        .and_then(|l| {
            l.iter()
                .find(|l| l["expected_supply_id"].as_str() == Some(&mine.to_string()))
        })
        .cloned()
        .expect("this test's own promise, which it just made");
    let supply = line["expected_supply_id"].as_str().unwrap().to_string();
    let item = Uuid::parse_str(line["item_id"].as_str().unwrap()).unwrap();

    // A code nothing in the fixture uses, so the first receipt has to make it.
    let code = format!("TEST-LOT-{}", Uuid::new_v4().simple());
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let third = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();

    // The promise names no owner, so the receipt has to. The read says which
    // parties already own this item here, and the screen will offer exactly
    // that list — this test takes the same answer from the same place.
    let owner = line["owner_id"]
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            line["owners"]
                .as_array()
                .and_then(|o| o.first())
                .and_then(|o| o["owner_id"].as_str())
                .map(str::to_string)
        })
        .expect("the promise names an owner, or something here already does");

    let send = |event: Uuid, expiry: Option<&'static str>, code: String| {
        let bearer = bearer.clone();
        let supply = supply.clone();
        let now = now.clone();
        let owner = owner.clone();
        test::TestRequest::post()
            .uri("/receipts")
            .insert_header(("authorization", bearer))
            .set_json(json!({
                "expected_supply_id": supply,
                "to_location_id": DOCK,
                "quantity": 1,
                "owner_id": owner,
                "lot_code": code,
                "lot_expiry": expiry,
                "client_event_id": event,
                "occurred_at": now,
            }))
            .to_request()
    };

    let expiry_of = |db_code: &str| {
        let db_code = db_code.to_string();
        async move {
            let row: Option<chrono::NaiveDate> = raw(&url().unwrap())
                .await
                .query_one(
                    "SELECT expiry_date FROM lot WHERE item_id = $1 AND code = $2",
                    &[&item, &db_code],
                )
                .await
                .expect("the lot")
                .get(0);
            row.map(|d| d.to_string())
        }
    };

    // ── first delivery: no date on the label, so none on the lot ────────
    //
    // This is the case that makes the branch below reachable. Creating the lot
    // *with* a date would mean nothing ever fills a blank, and a test that only
    // ever creates cannot tell the difference.
    let made = common::ok_json(&app, send(first, None, code.clone()), "first receipt").await;
    assert_eq!(made["accepted"].as_bool(), Some(true), "{made}");
    assert_eq!(expiry_of(&code).await, None, "nobody stated a date, so none is held");

    // ── second delivery: a date, filling the blank ──────────────────────
    let filled = common::ok_json(
        &app,
        send(second, Some("2027-01-01"), code.clone()),
        "second receipt",
    )
    .await;
    assert_eq!(filled["accepted"].as_bool(), Some(true), "{filled}");
    assert_eq!(
        expiry_of(&code).await,
        Some("2027-01-01".to_string()),
        "the first date anybody states fills the blank — a lot with no date is a \
         lot no shelf-life rule can be applied to"
    );
    assert!(
        !filled["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap_or("").contains("expiry")),
        "filling a blank is not a conflict: {filled}"
    );

    // ── third delivery, same lot, a different date ──────────────────────
    let again = common::ok_json(&app, send(third, Some("2027-06-30"), code.clone()), "third receipt").await;
    assert_eq!(
        again["accepted"].as_bool(),
        Some(true),
        "a disagreement about a date does not stop goods coming in: {again}"
    );
    let warnings = again["warnings"].as_array().expect("warnings");
    assert!(
        warnings.iter().any(|w| {
            let w = w.as_str().unwrap_or("");
            w.contains("2027-01-01") && w.contains("2027-06-30")
        }),
        "the conflict is said, with both dates: {again}"
    );
    assert_eq!(
        expiry_of(&code).await,
        Some("2027-01-01".to_string()),
        "the held date stands; whoever received last does not get to decide"
    );

    // One lot, not two. The unique constraint is what makes the retry safe and
    // this is the assertion that says the code relies on it.
    let count: i64 = db
        .query_one(
            "SELECT count(*) FROM lot WHERE item_id = $1 AND code = $2",
            &[&item, &code],
        )
        .await
        .expect("count")
        .get(0);
    assert_eq!(count, 1, "three receipts of one lot code made one lot");

    let events = format!("'{first}','{second}','{third}'");
    db.batch_execute(&format!(
        "DELETE FROM stock_movement WHERE client_event_id IN ({events});
         DELETE FROM goods_receipt_line WHERE client_event_id IN ({events});
         DELETE FROM goods_receipt WHERE client_event_id IN ({events});
         DELETE FROM client_event WHERE client_event_id IN ({events});
         DELETE FROM stock WHERE lot_id IN
             (SELECT id FROM lot WHERE item_id = '{item}' AND code = '{code}');
         DELETE FROM lot WHERE item_id = '{item}' AND code = '{code}';"
    ))
    .await
    .expect("the test removes what it committed");

    forget_promise(&db, mine, its_line).await;
}
