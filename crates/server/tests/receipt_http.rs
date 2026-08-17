//! `POST /receipts`, over HTTP, through the handler that serves it.
//!
//! **Five hundred and thirty-eight lines and nothing had ever executed them.**
//! The reasoning underneath is covered well — `receipt_disposition` drives
//! `resolve_receiving_policy` and `disposition`, `receipt_entered` drives
//! `resolve_count`, and `receipt_header` drives `ensure_goods_receipt` — but
//! all three call the pure modules a handler calls rather than the handler.
//! `pack_walk_http` records what that gap costs: three defects shipped in
//! endpoints whose reasoning was tested, because *a handler compiles whatever
//! its SQL says. It is a string until something runs it.*
//!
//! This one has more SQL than any other endpoint in the file — eleven
//! statements, a mediated dispose function, two casts and a projection call —
//! and it is the first of the six paths that write `stock_movement`.
//!
//! # The replay test is the point
//!
//! The architecture rests on one sentence: *"sending the same entry twice
//! changes nothing, and entries can arrive in any order without changing the
//! result."* `act_idempotency` asserts that of `client_events`, and its own
//! doc says it does so *"the same way handlers claim the envelope"* — it
//! re-implements the sequence rather than calling a handler. So the claim was
//! made of the helper and never of this endpoint, which is where a duplicate
//! submission actually arrives.
//!
//! # What it writes
//!
//! Handlers own their transactions, so like `pack_walk_http` this commits.
//! Every act id is minted and named up front so the cleanup can reach it, and
//! each test removes its own rows and re-folds the projection — a receipt left
//! behind would inflate `expected_supply.quantity_received` for every later
//! reader.

use actix_web::{test, web, App};
use chrono::Utc;
use nylonite_server::{routes, AppState};
use serde_json::{json, Value};
use uuid::Uuid;

mod common;
use common::{pool, url};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const DOCK: &str = "10c00000-0000-0000-0000-000000000003";
const ITEM: &str = "17e10000-0000-0000-0000-000000000001";
const LOT: &str = "10700000-0000-0000-0000-000000000001";
const OWNER: &str = "9a247000-0000-0000-0000-000000000001";
/// The packing config the fixture receives against: ten to an inner, one inner
/// to a carton, so a carton is ten base units.
const PACKING_CONFIG: &str = "9ac40000-0000-0000-0000-000000000001";
/// **The promise is found by its purchase order line, never by its own id.**
/// `expected_supply` is a projection and `projection_run_all` mints it a fresh
/// uuidv7 every time the fixture is rebuilt; the line that causes it is the
/// stable half. `receipt_header` learned this first.
const POL: &str = "901e0000-0000-0000-0000-000000000001";

/// Whether a response says it answered a replay rather than a fresh act.
///
/// Substring matching, because `warnings` is a list of sentences rather than a
/// list of codes — which is the one thing about this response shape that made
/// the test harder to write than the endpoint made it hard to use.
fn is_replay(response: &Value) -> bool {
    response["warnings"]
        .as_array()
        .expect("warnings is a list")
        .iter()
        .any(|w| w.as_str().unwrap_or("").contains("replay"))
}

/// A connection that is not the pool's, for reading facts back and removing them.
///
/// Deliberately the superuser rather than `nylonite_app`: this asserts what was
/// written rather than what a tenant may see, and the cleanup deletes from
/// append-only tables the application holds no DELETE on.
async fn raw(u: &str) -> tokio_postgres::Client {
    let (c, conn) = tokio_postgres::connect(u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = conn.await;
    });
    c
}

/// The promise this file receives against.
async fn promise(c: &tokio_postgres::Client) -> Uuid {
    c.query_one(
        "SELECT id FROM expected_supply WHERE purchase_order_line_id = $1",
        &[&Uuid::parse_str(POL).unwrap()],
    )
    .await
    .expect("the fixture's promise, folded from its purchase order line")
    .get(0)
}

/// Sign on and return the bearer the handlers read the caller from.
///
/// The tenant and `recorded_by_id` both come from the session since migration
/// 70 — there is no header to set and no person id to pass.
async fn bearer(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
) -> String {
    let signed: Value = common::ok_json(
        app,
        test::TestRequest::post()
            .uri("/sessions")
            .set_json(json!({
                "email": "kyle@example.test",
                "password": "dock-station-1",
                "site_id": SITE
            }))
            .to_request(),
        "POST /sessions",
    )
    .await;
    format!(
        "Bearer {}",
        signed["token"].as_str().expect("a session token")
    )
}

/// Remove what these acts recorded, then re-fold.
///
/// **The projection run is not optional.** `quantity_received` on the promise is
/// derived from the ledger, so a test that deletes its movement and stops there
/// leaves the fold reading a receipt that no longer exists — which is the exact
/// disagreement J1 is written to catch, seeded by the suite that checks it.
async fn cleanup(c: &tokio_postgres::Client, acts: &[Uuid]) {
    let list = acts
        .iter()
        .map(|a| format!("'{a}'"))
        .collect::<Vec<_>>()
        .join(", ");
    c.batch_execute(&format!(
        "DELETE FROM discrepancy WHERE goods_receipt_line_id IN
             (SELECT id FROM goods_receipt_line WHERE client_event_id IN ({list}));
         DELETE FROM stock_movement WHERE client_event_id IN ({list});
         DELETE FROM goods_receipt_line WHERE client_event_id IN ({list});
         DELETE FROM goods_receipt WHERE client_event_id IN ({list});
         DELETE FROM client_event WHERE client_event_id IN ({list});
         SELECT projection_run_all('{ALPHA}');"
    ))
    .await
    .expect("the test removes what it recorded");
}

/// One delivery, two lines, and the movement the accepted line writes.
///
/// Asserts the three things only a running handler can: that the entered count
/// is converted and *both* forms are stored (D92 / Q173), that the movement
/// names the receipt line that caused it (which is what makes J26's fold a
/// batch load rather than an N+1), and that a second line naming the same
/// delivery joins the header rather than opening another (D43 / Q172).
#[actix_web::test]
async fn a_receipt_writes_a_header_a_line_and_the_movement_that_names_it() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let c = raw(&u).await;
    let supply = promise(&c).await;

    let first = Uuid::now_v7();
    let second = Uuid::now_v7();
    let delivery = Uuid::now_v7();
    let now = Utc::now();
    let auth = bearer(&app).await;

    // Unauthenticated is refused rather than served, which is what an
    // RLS-only answer would look like from the outside.
    let anon = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/receipts")
            .set_json(json!({
                "expected_supply_id": supply,
                "to_location_id": DOCK,
                "quantity": 1,
                "client_event_id": Uuid::now_v7(),
                "occurred_at": now.to_rfc3339()
            }))
            .to_request(),
    )
    .await;
    assert_eq!(anon.status(), 401, "a receipt names a person or it is refused");

    // Two cartons, counted as cartons. The handler converts; the line stores
    // both forms.
    let one: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/receipts")
            .insert_header(("authorization", auth.clone()))
            .set_json(json!({
                "expected_supply_id": supply,
                "to_location_id": DOCK,
                "entered_quantity": 2,
                "entered_packaging_level": "carton",
                "item_packing_config_id": PACKING_CONFIG,
                "lot_id": LOT,
                "owner_id": OWNER,
                "goods_receipt_id": delivery,
                "client_event_id": first,
                "occurred_at": now.to_rfc3339()
            }))
            .to_request(),
        "POST /receipts (first line)",
    )
    .await;

    assert_eq!(one["header_created"], true, "the first line opens the delivery");
    assert_eq!(one["goods_receipt_id"], json!(delivery), "under the id it was given");
    assert_eq!(one["quantity"], 20, "two cartons of ten are twenty base units");
    assert_eq!(one["entered_quantity"], 2, "and the count as entered survives");
    assert_eq!(one["entered_packaging_level"], "carton");
    assert_eq!(one["item_packing_config_id"], json!(PACKING_CONFIG));
    assert_eq!(one["accepted"], true, "twenty against a promise of a hundred and thirty");
    assert!(
        one["receiving_policy_id"].is_string(),
        "the version that governed the disposition is stamped (J63)"
    );
    let movement_id = one["movement_id"].as_str().expect("an accepted line moves stock");
    let line_id = one["goods_receipt_line_id"].as_str().expect("a line id");

    // **The movement names its cause.** Nothing in the response says this and
    // the fold depends on it.
    let m = c
        .query_one(
            "SELECT quantity, reason::text, to_location_id, goods_receipt_line_id, item_id
               FROM stock_movement WHERE client_event_id = $1",
            &[&first],
        )
        .await
        .expect("exactly one movement for the act");
    assert_eq!(m.get::<_, i64>(0), 20, "the ledger carries base units");
    assert_eq!(m.get::<_, String>(1), "receipt");
    assert_eq!(
        m.get::<_, Uuid>(2),
        Uuid::parse_str(DOCK).unwrap(),
        "landed where the request said"
    );
    assert_eq!(
        m.get::<_, Option<Uuid>>(3),
        Some(Uuid::parse_str(line_id).unwrap()),
        "and names the receipt line that caused it"
    );
    assert_eq!(m.get::<_, Uuid>(4), Uuid::parse_str(ITEM).unwrap());
    assert_eq!(
        movement_id,
        c.query_one(
            "SELECT id FROM stock_movement WHERE client_event_id = $1",
            &[&first]
        )
        .await
        .unwrap()
        .get::<_, Uuid>(0)
        .to_string(),
        "the id the response reported is the row that exists"
    );

    // A second line on the same truck. D43: one header per delivery.
    let two: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/receipts")
            .insert_header(("authorization", auth))
            .set_json(json!({
                "expected_supply_id": supply,
                "to_location_id": DOCK,
                "quantity": 5,
                "lot_id": LOT,
                "owner_id": OWNER,
                "goods_receipt_id": delivery,
                "client_event_id": second,
                "occurred_at": now.to_rfc3339()
            }))
            .to_request(),
        "POST /receipts (second line)",
    )
    .await;
    assert_eq!(
        two["header_created"], false,
        "the second line joins the delivery rather than opening one"
    );
    assert_eq!(two["goods_receipt_id"], json!(delivery));
    assert_eq!(two["quantity"], 5, "counted in eaches, stored as five");
    assert_eq!(
        two["entered_packaging_level"], "each",
        "a quantity with no level named is a count of eaches"
    );
    assert_ne!(
        two["goods_receipt_line_id"], one["goods_receipt_line_id"],
        "two lines, not one"
    );

    let headers: i64 = c
        .query_one(
            "SELECT count(*) FROM goods_receipt WHERE id = $1",
            &[&delivery],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(headers, 1, "one delivery is one goods_receipt");

    cleanup(&c, &[first, second]).await;
}

/// The same act, sent twice, writes one movement.
///
/// This is the assertion the whole append-only design rests on and the one the
/// suite did not make of this endpoint. A dropped response is the ordinary case
/// on a handheld: the server commits, the reply never lands, and the device
/// sends the act again. What must not happen is a second receipt.
#[actix_web::test]
async fn the_same_receipt_sent_twice_writes_one_movement() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let c = raw(&u).await;
    let supply = promise(&c).await;

    let act = Uuid::now_v7();
    let delivery = Uuid::now_v7();
    let now = Utc::now();
    let auth = bearer(&app).await;

    let body = json!({
        "expected_supply_id": supply,
        "to_location_id": DOCK,
        "quantity": 7,
        "lot_id": LOT,
        "owner_id": OWNER,
        "goods_receipt_id": delivery,
        "client_event_id": act,
        "occurred_at": now.to_rfc3339()
    });

    let sent: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/receipts")
            .insert_header(("authorization", auth.clone()))
            .set_json(body.clone())
            .to_request(),
        "POST /receipts",
    )
    .await;
    // **Not "no warnings" — "not the replay warning".** The first draft asserted
    // the list was empty and failed roughly one run in three, because the
    // handler's `warnings` mixes two kinds: what the act meant, and what the
    // server happened to be doing at the time. `projection_refresh_tenant` is
    // rate-limited, so three tests receiving at once make one of them report a
    // deferred handover — true, useful, and nothing to do with idempotency.
    assert!(
        !is_replay(&sent),
        "a first send is not a replay: {}",
        sent["warnings"]
    );

    // The same bytes again, as a device with no answer would send them.
    let again: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/receipts")
            .insert_header(("authorization", auth))
            .set_json(body)
            .to_request(),
        "POST /receipts (again)",
    )
    .await;

    assert_eq!(
        again["goods_receipt_line_id"], sent["goods_receipt_line_id"],
        "the replay answers with the line the first send wrote"
    );
    assert_eq!(again["goods_receipt_id"], sent["goods_receipt_id"]);
    assert_eq!(again["movement_id"], sent["movement_id"], "and the same movement");
    assert_eq!(again["quantity"], sent["quantity"]);
    assert_eq!(again["accepted"], sent["accepted"]);
    // **The answer does not change on the second ask.** A retry of the line
    // that opened the delivery still opened it.
    assert_eq!(
        again["header_created"], true,
        "the replay answers for the act being replayed, not for this call"
    );
    assert!(is_replay(&again), "and says so: {}", again["warnings"]);

    // The half no response can show.
    let counts = c
        .query_one(
            "SELECT (SELECT count(*) FROM stock_movement WHERE client_event_id = $1),
                    (SELECT count(*) FROM goods_receipt_line WHERE client_event_id = $1),
                    (SELECT count(*) FROM goods_receipt WHERE client_event_id = $1)",
            &[&act],
        )
        .await
        .unwrap();
    assert_eq!(counts.get::<_, i64>(0), 1, "one movement, not two");
    assert_eq!(counts.get::<_, i64>(1), 1, "one receipt line, not two");
    assert_eq!(counts.get::<_, i64>(2), 1, "one delivery header, not two");

    cleanup(&c, &[act]).await;
}

/// A replay that disagrees with itself is refused.
///
/// The guard on the guard. An identifier reused for a different count is not a
/// retry of the same act — it is two acts wearing one name, and answering it
/// with the stored line would report a quantity the caller never sent.
#[actix_web::test]
async fn a_replay_that_disagrees_about_the_quantity_is_refused() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let c = raw(&u).await;
    let supply = promise(&c).await;

    let act = Uuid::now_v7();
    let now = Utc::now();
    let auth = bearer(&app).await;
    let at = |qty: i64| {
        json!({
            "expected_supply_id": supply,
            "to_location_id": DOCK,
            "quantity": qty,
            "lot_id": LOT,
            "owner_id": OWNER,
            "client_event_id": act,
            "occurred_at": now.to_rfc3339()
        })
    };

    common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/receipts")
            .insert_header(("authorization", auth.clone()))
            .set_json(at(3))
            .to_request(),
        "POST /receipts",
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/receipts")
            .insert_header(("authorization", auth))
            .set_json(at(4))
            .to_request(),
    )
    .await;
    assert_eq!(
        resp.status(),
        400,
        "the same act may not come back with a different count"
    );
    let text = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(
        text.contains("already recorded quantity 3"),
        "and says which figure it is holding: {text}"
    );

    let movements: i64 = c
        .query_one(
            "SELECT count(*) FROM stock_movement WHERE client_event_id = $1",
            &[&act],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(movements, 1, "the refused second send wrote nothing");

    cleanup(&c, &[act]).await;
}
