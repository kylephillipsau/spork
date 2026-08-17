//! The four ledger writers that had never been executed over HTTP.
//!
//! `POST /adjustments`, `POST /moves`, `POST /counts` and
//! `POST /packages/{id}/despatch`. `handoff-the-write-paths.md` named them, in
//! this order, as what remains after `POST /receipts` got the first coverage a
//! writing endpoint in this repository has ever had:
//!
//! > *the reasoning underneath is covered well … but all three call the pure
//! > modules a handler calls rather than the handler.*
//!
//! `pack_walk_http` records what that gap costs: three defects shipped in
//! endpoints whose reasoning was tested, because *a handler compiles whatever
//! its SQL says. It is a string until something runs it.*
//!
//! # Two assertions each, and the second is the point
//!
//! One happy path, asserting what only a running handler can — that the SQL
//! parses, that the act is claimed, that the ledger and the projection agree
//! afterwards. Then one duplicate submission, because the architecture rests on
//! a sentence: *"sending the same entry twice changes nothing."*
//! `act_idempotency` asserts that of `client_events` and its own doc says it
//! does so *"the same way handlers claim the envelope"* — it re-implements the
//! sequence rather than calling a handler. The claim was made of the helper and
//! never of the endpoints where a duplicate actually arrives, which as of
//! today's client is a thing that happens: until `domain/acts.ts`, a retry
//! carried a fresh id and was not a duplicate at all.
//!
//! # Nothing here is hard-coded that the fixture mints
//!
//! `stock` rows carry a uuidv7 the projection mints on every rebuild, and
//! `adjustment_reason` ids are `gen_random_uuid()`. Both are looked up by the
//! stable half — the cell's (item, location) and the reason's `code` — which is
//! the lesson `receipt_header` learned about `expected_supply` and paid for
//! once already.
//!
//! # What it writes
//!
//! Handlers own their transactions, so this commits. Every act id is minted and
//! named up front so the cleanup can reach it, and each test removes its own
//! rows and re-folds — a movement left behind moves stock for every later
//! reader in a suite that shares one database.

use actix_web::{http::StatusCode, test, web, App};
use chrono::Utc;
use nylonite_server::{routes, AppState};
use serde_json::{json, Value};
use std::sync::OnceLock;
use tokio::sync::{Mutex, MutexGuard};
use uuid::Uuid;

mod common;
use common::{pool, url};

/// **These four run one at a time, and finding that out cost the first run.**
///
/// Cargo runs a file's tests on threads. Three of these write the ledger and
/// then rebuild the projection to check the fold agrees — so run together they
/// read each other's movements and each other's half-finished rebuilds. The
/// first version shared one cell and all three saw 55 where they expected 58,
/// 57 and 60: three correct handlers, three failing tests, and nothing wrong
/// with the code.
///
/// They also take a distinct cell each, which is not redundant with the lock:
/// the lock orders them, the distinct cells mean a *later file* in the same
/// suite is not reading a number this one moved. Both, because the handover's
/// standing warning is that this suite shares one database.
static LEDGER: OnceLock<Mutex<()>> = OnceLock::new();
async fn one_at_a_time() -> MutexGuard<'static, ()> {
    LEDGER.get_or_init(|| Mutex::new(())).lock().await
}

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
/// The two bins the fixture's stock sits in.
const BIN_A: &str = "10c00000-0000-0000-0000-000000000001";
const BIN_B: &str = "10c00000-0000-0000-0000-000000000002";
const DOCK: &str = "10c00000-0000-0000-0000-000000000003";
/// The gumboot variant: 60 in bin A, none of it allocated, so a test can move
/// and adjust it without disturbing the pack queue's coverage arithmetic.
const QUIET_ITEM: &str = "17e10000-0000-0000-0000-000000000002";
/// The glove: 100 in bin A, unallocated, and 40 in bin B.
const GLOVE: &str = "17e10000-0000-0000-0000-000000000001";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const SMALL_BOX: &str = "9a7e0000-0000-0000-0000-0000000000b1";

/// A connection that is not the pool's, for reading facts back and removing them.
///
/// The superuser rather than `nylonite_app`: this asserts what was *written*
/// rather than what a tenant may see, and the cleanup deletes from append-only
/// tables the application holds no DELETE on.
async fn raw(u: &str) -> tokio_postgres::Client {
    let (c, conn) = tokio_postgres::connect(u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = conn.await;
    });
    c
}

/// The cell holding this item in this bin, by the half of its identity that is
/// stable. Returns the id and what the projection currently says is in it.
async fn cell(c: &tokio_postgres::Client, item: &str, bin: &str) -> (Uuid, i64) {
    let row = c
        .query_one(
            "SELECT id, quantity FROM stock
              WHERE item_id = $1 AND holder_location_id = $2 AND quantity > 0
              ORDER BY quantity DESC LIMIT 1",
            &[
                &Uuid::parse_str(item).unwrap(),
                &Uuid::parse_str(bin).unwrap(),
            ],
        )
        .await
        .expect("the fixture holds this item in this bin");
    (row.get(0), row.get(1))
}

/// An adjustment reason by its code. `world_event` is the class `/adjustments`
/// accepts; a `record_error` belongs to `/corrections` and the handler says so.
async fn reason(c: &tokio_postgres::Client, code: &str) -> Uuid {
    c.query_one("SELECT id FROM adjustment_reason WHERE code = $1", &[&code])
        .await
        .expect("the fixture's reasons")
        .get(0)
}

/// Remove what a test recorded, and re-fold.
///
/// **The projection run is not optional.** `stock.quantity` is derived from the
/// ledger, so a test that deletes its movement and stops there leaves the fold
/// reading a movement that no longer exists — the exact disagreement J1 exists
/// to catch, seeded by the suite that checks it.
async fn cleanup(c: &tokio_postgres::Client, acts: &[Uuid]) {
    let list = acts
        .iter()
        .map(|a| format!("'{a}'"))
        .collect::<Vec<_>>()
        .join(", ");
    c.batch_execute(&format!(
        "DELETE FROM discrepancy WHERE stock_count_id IN
             (SELECT id FROM stock_count WHERE client_event_id IN ({list}));
         DELETE FROM discrepancy WHERE stock_movement_id IN
             (SELECT id FROM stock_movement WHERE client_event_id IN ({list}));
         DELETE FROM stock_count WHERE client_event_id IN ({list});
         DELETE FROM package_event WHERE client_event_id IN ({list});
         DELETE FROM stock_movement WHERE client_event_id IN ({list});
         DELETE FROM client_event WHERE client_event_id IN ({list});
         SELECT projection_run_all('{ALPHA}');"
    ))
    .await
    .expect("the test removes what it recorded");
}

/// What the projection says is in a cell now.
async fn held(c: &tokio_postgres::Client, id: Uuid) -> i64 {
    c.query_one("SELECT quantity FROM stock WHERE id = $1", &[&id])
        .await
        .map(|r| r.get::<_, i64>(0))
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------

/// A count below system writes one movement, and sending it twice writes one.
///
/// **What only a running handler proves.** `adjusting::check` is covered over
/// every branch of its own logic and has never seen the SQL underneath it: that
/// the reason's class is read, that the delta is signed the right way round,
/// that the movement names the cell it came out of, and that the fold afterwards
/// agrees with the number the operator counted.
#[actix_web::test]
async fn an_adjustment_moves_the_ledger_once_however_many_times_it_is_sent() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let _order = one_at_a_time().await;
    let bearer = common::bearer(&app).await;
    let db = raw(&u).await;

    // The gumboot in bin A: this test's cell and no other's.
    let (stock_id, before) = cell(&db, QUIET_ITEM, BIN_A).await;
    let damaged = reason(&db, "damaged").await;
    let act = Uuid::new_v4();
    let counted = before - 2;

    let body = json!({
        "stock_id": stock_id,
        "counted_quantity": counted,
        "adjustment_reason_id": damaged,
        "client_event_id": act,
        "occurred_at": Utc::now(),
    });

    let first: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/adjustments")
            .insert_header(("authorization", bearer.clone()))
            .set_json(&body)
            .to_request(),
        "POST /adjustments",
    )
    .await;

    assert_eq!(first["system_quantity"], before, "{first}");
    assert_eq!(first["counted_quantity"], counted, "{first}");
    assert_eq!(first["delta"], -2, "a count below system is a negative delta: {first}");
    assert!(
        first["movement_id"].is_string(),
        "a variance wrote no movement: {first}"
    );

    // **The fold, not the response.** A handler can answer correctly and write
    // a movement the projection cannot read — the arm that goes wrong is the
    // one naming the cell, and only the rebuild sees it.
    db.execute("SELECT projection_run_all($1)", &[&Uuid::parse_str(ALPHA).unwrap()])
        .await
        .expect("fold");
    assert_eq!(
        held(&db, stock_id).await,
        counted,
        "the ledger and the count disagree after the fold"
    );

    // ── the same act again ──────────────────────────────────────────────
    let again: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/adjustments")
            .insert_header(("authorization", bearer.clone()))
            .set_json(&body)
            .to_request(),
        "POST /adjustments (replay)",
    )
    .await;
    assert_eq!(
        again["movement_id"], first["movement_id"],
        "a replay minted a second movement: {again}"
    );

    let movements: i64 = db
        .query_one(
            "SELECT count(*) FROM stock_movement WHERE client_event_id = $1",
            &[&act],
        )
        .await
        .expect("count")
        .get(0);
    assert_eq!(movements, 1, "one act, one movement");

    cleanup(&db, &[act]).await;
    assert_eq!(held(&db, stock_id).await, before, "the test put it back");
}

/// A move takes stock out of one bin and puts it in another, once.
#[actix_web::test]
async fn a_move_is_one_movement_between_two_bins_however_many_times_it_is_sent() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let _order = one_at_a_time().await;
    let bearer = common::bearer(&app).await;
    let db = raw(&u).await;

    // The glove out of bin A and onto the dock: neither end is a cell another
    // test in this file reads.
    let (from, before) = cell(&db, GLOVE, BIN_A).await;
    let act = Uuid::new_v4();
    let body = json!({
        "from_stock_id": from,
        "to_location_id": DOCK,
        "quantity": 3,
        "client_event_id": act,
        "occurred_at": Utc::now(),
    });

    let first: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/moves")
            .insert_header(("authorization", bearer.clone()))
            .set_json(&body)
            .to_request(),
        "POST /moves",
    )
    .await;
    assert!(first["movement_id"].is_string(), "{first}");

    // **Both ends, because a move is one row with two sides** and a handler
    // that named only one would still answer 200. The fold is what reads them.
    db.execute("SELECT projection_run_all($1)", &[&Uuid::parse_str(ALPHA).unwrap()])
        .await
        .expect("fold");
    assert_eq!(held(&db, from).await, before - 3, "it did not leave the first bin");
    let (_, arrived) = cell(&db, GLOVE, DOCK).await;
    assert!(arrived >= 3, "it did not arrive on the dock");

    let again: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/moves")
            .insert_header(("authorization", bearer.clone()))
            .set_json(&body)
            .to_request(),
        "POST /moves (replay)",
    )
    .await;
    assert_eq!(
        again["movement_id"], first["movement_id"],
        "a replay moved the stock twice: {again}"
    );

    cleanup(&db, &[act]).await;
    assert_eq!(held(&db, from).await, before, "the test put it back");
}

/// A count asserts and never moves the ledger, and raises the finding.
///
/// **The shape that is different from the other three.** D8: a count writes an
/// assertion, and a variance raises a `count_variance` finding rather than
/// moving stock — *"resolve later with /adjustments if the floor decides the
/// ledger should move."* A handler that quietly wrote a movement here would
/// pass every test the pure module has.
#[actix_web::test]
async fn a_count_asserts_and_raises_a_finding_and_never_moves_stock() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let _order = one_at_a_time().await;
    let bearer = common::bearer(&app).await;
    let db = raw(&u).await;

    // The glove in bin B, which nothing else here touches.
    let (stock_id, before) = cell(&db, GLOVE, BIN_B).await;
    let act = Uuid::new_v4();
    let body = json!({
        "stock_id": stock_id,
        "counted_quantity": before - 4,
        "client_event_id": act,
        "counted_at": Utc::now(),
    });

    let first: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/counts")
            .insert_header(("authorization", bearer.clone()))
            .set_json(&body)
            .to_request(),
        "POST /counts",
    )
    .await;
    assert_eq!(first["variance"], -4, "{first}");
    assert!(
        first["discrepancy_id"].is_string(),
        "a variance raised no finding: {first}"
    );

    // The whole of D8 in one assertion.
    let moved: i64 = db
        .query_one(
            "SELECT count(*) FROM stock_movement WHERE client_event_id = $1",
            &[&act],
        )
        .await
        .expect("count")
        .get(0);
    assert_eq!(moved, 0, "a count moved the ledger");
    db.execute("SELECT projection_run_all($1)", &[&Uuid::parse_str(ALPHA).unwrap()])
        .await
        .expect("fold");
    assert_eq!(held(&db, stock_id).await, before, "a count changed the stock");

    let again: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/counts")
            .insert_header(("authorization", bearer.clone()))
            .set_json(&body)
            .to_request(),
        "POST /counts (replay)",
    )
    .await;
    assert_eq!(
        again["stock_count_id"], first["stock_count_id"],
        "a replay counted twice: {again}"
    );
    let counts: i64 = db
        .query_one(
            "SELECT count(*) FROM stock_count WHERE client_event_id = $1",
            &[&act],
        )
        .await
        .expect("count")
        .get(0);
    assert_eq!(counts, 1, "one act, one count");

    cleanup(&db, &[act]).await;
}

/// Despatching a carton writes the ledger *and* the carton's own history.
///
/// **Two tables, which is what makes this one different.** The stock leaves — a
/// movement with a `from` side and no `to`, which the whole-key CHECKs permit on
/// purpose — and the carton's own history gains a row. A handler that wrote one
/// and not the other leaves a despatched carton the ledger has never heard of,
/// or stock gone from a carton that says it is still on the dock.
///
/// # It builds its own carton, and the fixture is why
///
/// The seed holds four packages: one despatched with stock in it, and three
/// with neither a status nor contents. So there is nothing to despatch, and a
/// test that went looking would have found the already-despatched one and
/// asserted something about a carton that had already gone.
///
/// It is built through the handlers rather than by INSERT — allocate, raise the
/// carton, pick into it, seal it — which costs four requests and buys a subject
/// that exists the way a real one does. Three of those four already have HTTP
/// coverage in `pack_walk_http`; this reuses the sequence rather than the file.
#[actix_web::test]
async fn despatching_a_carton_writes_the_ledger_and_the_cartons_own_history() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let _order = one_at_a_time().await;
    let bearer = common::bearer(&app).await;
    let db = raw(&u).await;
    let now = Utc::now();

    // ── a line with room, and a cell that can serve it ──────────────────
    let open: Value = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/open-lines"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /sites/{id}/open-lines",
    )
    .await;
    let line = open
        .as_array()
        .expect("open work")
        .iter()
        .find(|l| {
            l["quantity"].as_i64().unwrap_or(0) - l["covered_quantity"].as_i64().unwrap_or(0) > 0
        })
        .expect("a line with room left to claim");
    let line_id = line["fulfilment_line_id"].as_str().unwrap().to_string();
    let fulfilment = line["fulfilment_id"].as_str().unwrap().to_string();
    // The read answers with the item's *code*, not its id — it is a screen's
    // read and a code is what a screen draws.
    let item_code = line["item_code"].as_str().unwrap().to_string();

    let source: Uuid = db
        .query_one(
            "SELECT s.id FROM stock s
               JOIN item i ON i.id = s.item_id
              WHERE i.code = $1 AND s.holder_location_id IS NOT NULL
                AND s.quantity - s.allocated_quantity > 0
              ORDER BY s.quantity - s.allocated_quantity DESC LIMIT 1",
            &[&item_code],
        )
        .await
        .expect("a cell with something spare in it")
        .get(0);

    let carton = Uuid::new_v4();
    let allocation = Uuid::new_v4();
    let act_carton = Uuid::new_v4();
    let act_pick = Uuid::new_v4();
    let act_seal = Uuid::new_v4();
    let act_gone = Uuid::new_v4();

    let post = |uri: String, body: Value, bearer: String| {
        test::TestRequest::post()
            .uri(&uri)
            .insert_header(("authorization", bearer))
            .set_json(body)
            .to_request()
    };

    common::ok_json(
        &app,
        post(
            "/allocations".into(),
            json!({ "id": allocation, "fulfilment_line_id": line_id,
                    "stock_id": source, "quantity": 1 }),
            bearer.clone(),
        ),
        "POST /allocations",
    )
    .await;
    common::ok_json(
        &app,
        post(
            "/packages".into(),
            json!({ "id": carton, "fulfilment_id": fulfilment,
                    "package_type_id": SMALL_BOX, "location_id": DOCK,
                    "client_event_id": act_carton, "occurred_at": now }),
            bearer.clone(),
        ),
        "POST /packages",
    )
    .await;
    common::ok_json(
        &app,
        post(
            "/picks".into(),
            json!({ "from_stock_id": source, "to_package_id": carton,
                    "fulfilment_line_id": line_id, "quantity": 1,
                    "client_event_id": act_pick, "occurred_at": now }),
            bearer.clone(),
        ),
        "POST /picks",
    )
    .await;

    // **The fold, before the endpoint under test can see the carton.** The pick
    // writes a movement; `stock` is folded from movements, and the despatch
    // handler finds what is in a carton by reading `stock WHERE
    // holder_package_id = …`. Without this it refuses with *"no stock cell in
    // the package matches the line's item"* — correctly, because as far as the
    // projection is concerned the carton is empty. In a deployment the
    // scheduler does this; in a test it has to be asked for.
    db.execute("SELECT projection_run_all($1)", &[&Uuid::parse_str(ALPHA).unwrap()])
        .await
        .expect("fold");

    common::ok_json(
        &app,
        post(
            format!("/packages/{carton}/seal"),
            json!({ "client_event_id": act_seal, "occurred_at": now }),
            bearer.clone(),
        ),
        "POST /packages/{id}/seal",
    )
    .await;

    // ── the endpoint under test ─────────────────────────────────────────
    let body = json!({
        "fulfilment_line_id": line_id,
        "quantity": 1,
        "client_event_id": act_gone,
        "occurred_at": now,
    });
    let sent = test::call_service(
        &app,
        post(format!("/packages/{carton}/despatch"), body.clone(), bearer.clone()),
    )
    .await;
    let status = sent.status();
    let text = String::from_utf8_lossy(&test::read_body(sent).await).to_string();
    assert_eq!(status, StatusCode::OK, "POST /packages/{{id}}/despatch: {text}");

    let moved: i64 = db
        .query_one(
            "SELECT count(*) FROM stock_movement WHERE client_event_id = $1",
            &[&act_gone],
        )
        .await
        .expect("count")
        .get(0);
    assert_eq!(moved, 1, "the stock did not leave");

    // **A `from` side and no `to`.** Goods leaving the building is the one
    // shape the whole-key CHECKs permit to be one-sided, and a handler that
    // filled in a destination would be recording a move rather than a despatch.
    let one_sided: bool = db
        .query_one(
            "SELECT to_location_id IS NULL AND to_package_id IS NULL
               FROM stock_movement WHERE client_event_id = $1",
            &[&act_gone],
        )
        .await
        .expect("the movement")
        .get(0);
    assert!(one_sided, "the goods left and arrived somewhere");

    let events: i64 = db
        .query_one(
            "SELECT count(*) FROM package_event WHERE client_event_id = $1",
            &[&act_gone],
        )
        .await
        .expect("count")
        .get(0);
    assert_eq!(events, 1, "the carton's own history says nothing about it");

    // ── and once ────────────────────────────────────────────────────────
    let again = test::call_service(
        &app,
        post(format!("/packages/{carton}/despatch"), body, bearer.clone()),
    )
    .await;
    let status = again.status();
    let text = String::from_utf8_lossy(&test::read_body(again).await).to_string();
    assert_eq!(status, StatusCode::OK, "a replay was refused: {text}");
    let moved: i64 = db
        .query_one(
            "SELECT count(*) FROM stock_movement WHERE client_event_id = $1",
            &[&act_gone],
        )
        .await
        .expect("count")
        .get(0);
    assert_eq!(moved, 1, "a replay despatched the goods twice");

    // The carton and everything raised to make it, in the order the keys allow.
    db.batch_execute(&format!(
        "DELETE FROM stock_movement WHERE to_package_id = '{carton}'
                                       OR from_package_id = '{carton}';
         DELETE FROM stock_allocation WHERE id = '{allocation}';
         UPDATE package SET placement_event_id = NULL, placement_occurred_at = NULL
          WHERE id = '{carton}';
         DELETE FROM package_containment WHERE package_id = '{carton}'
                                            OR parent_package_id = '{carton}';
         DELETE FROM stock WHERE holder_package_id = '{carton}';
         DELETE FROM package_event WHERE package_id = '{carton}';
         DELETE FROM package WHERE id = '{carton}';"
    ))
    .await
    .expect("the test removes the carton it built");
    cleanup(&db, &[act_carton, act_pick, act_seal, act_gone]).await;
}
