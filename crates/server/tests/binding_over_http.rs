//! Binding a barcode to a box, over HTTP (D164).
//!
//! `crate::binding` decides what a proposal amounts to and proves it over every
//! branch without a database. What a unit test cannot reach is the half that
//! only exists once a row is written: that the route is wired, that the level
//! lands in the column migration 90 added, that the person's name lands beside
//! it, and that a string already meaning something else is refused rather than
//! quietly rebound.
//!
//! That last one is the assertion worth having. `item_barcode`'s exclusion
//! constraint would refuse the second write anyway — but a constraint violation
//! surfacing as a 500 and a sentence naming what the string already means are
//! very different things to somebody holding a scanner, and only one of them is
//! testable from outside.
//!
//! **This test writes**, and cleans up after itself: the suite shares one
//! database and a barcode left bound is a barcode the next run cannot bind.

use actix_web::{http::StatusCode, test, web, App};
use nylonite_server::{routes, AppState};
use serde_json::{json, Value};

mod common;
use common::{pool, url};

/// A code nothing else in the fixture uses, and a GTIN that checks.
const ITEM_CODE: &str = "TEST-D164-BOX";
const OTHER_CODE: &str = "TEST-D164-OTHER";
const GTIN: &str = "09312345678891";
const TENANT: &str = "11111111-1111-1111-1111-111111111111";

async fn forget(db: &tokio_postgres::Client) {
    db.execute(
        "DELETE FROM item_barcode WHERE barcode = $1 OR item_id IN
             (SELECT id FROM item WHERE code = ANY($2))",
        &[&GTIN, &vec![ITEM_CODE.to_string(), OTHER_CODE.to_string()]],
    )
    .await
    .expect("clear bindings");
    db.execute(
        "DELETE FROM item WHERE code = ANY($1)",
        &[&vec![ITEM_CODE.to_string(), OTHER_CODE.to_string()]],
    )
    .await
    .expect("clear items");
}

#[actix_web::test]
async fn a_barcode_says_which_box_it_is_on_and_who_said_so() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state.clone()).configure(routes::configure)).await;
    let bearer = common::bearer(&app).await;

    // **The tenant, or nothing here works and most of it looks like it did.**
    // These tables force row-level security and their policies read
    // `current_tenant()`, which is unset on a fresh connection: the inserts are
    // refused outright and the deletes match nothing while reporting success.
    // The setting's name is `nylonite.tenant_id`, which is a thing to be told
    // rather than guessed.
    let db = state.pool.get().await.expect("a connection");
    db.execute(
        "SELECT set_config('nylonite.tenant_id', $1, false)",
        &[&TENANT],
    )
    .await
    .expect("the tenant scope");
    forget(&db).await;

    let mut ids = vec![];
    for code in [ITEM_CODE, OTHER_CODE] {
        let id: uuid::Uuid = db
            .query_one(
                "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
                 SELECT current_tenant(), $1, 'For D164', u.id, 'none'
                   FROM unit u WHERE u.code = 'ea'
                 RETURNING id",
                &[&code],
            )
            .await
            .expect("the item")
            .get(0);
        ids.push(id);
    }
    let (item, other) = (ids[0], ids[1]);

    let bind = |id: uuid::Uuid, body: Value| {
        let bearer = bearer.clone();
        async move {
            test::TestRequest::post()
                .uri(&format!("/items/{id}/barcodes"))
                .insert_header(("authorization", bearer))
                .set_json(body)
                .to_request()
        }
    };

    // ── the level is recorded, and so is the person ─────────────────────
    let resp = test::call_service(
        &app,
        bind(item, json!({ "scan": GTIN, "packaging_level": "carton", "quantity": 12 })).await,
    )
    .await;
    let status = resp.status();
    let text = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert_eq!(status, StatusCode::OK, "the binding is accepted: {text}");
    let bound: Value = serde_json::from_str(&text).expect("a bound barcode");
    assert_eq!(bound["packaging_level"], "carton", "{bound}");
    assert_eq!(bound["scheme"], "gtin", "{bound}");
    assert_eq!(bound["quantity"], 12, "{bound}");
    assert_eq!(bound["already"], false, "{bound}");
    // **D11's floor, and migration 79's own objection answered.** A binding
    // with nobody behind it is the anonymous write that file refuses.
    assert!(
        bound["bound_by_name"].is_string(),
        "the binding does not say who made it: {bound}"
    );

    // ── and it reached the column, not just the response ────────────────
    let stored: String = db
        .query_one(
            "SELECT packaging_level::text FROM item_barcode WHERE barcode = $1",
            &[&GTIN],
        )
        .await
        .expect("the row")
        .get(0);
    assert_eq!(stored, "carton", "migration 90's column is not what was written");

    // ── the same binding twice is not a second question ─────────────────
    //
    // A trigger under a glove fires twice.
    let resp = test::call_service(
        &app,
        bind(item, json!({ "scan": GTIN, "packaging_level": "carton", "quantity": 12 })).await,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK, "a repeat is answered");
    let again: Value = test::read_body_json(resp).await;
    assert_eq!(again["already"], true, "{again}");
    assert_eq!(again["id"], bound["id"], "a repeat minted a second row: {again}");

    // ── one identifier means one thing at a time ────────────────────────
    //
    // The refusal names what the string already means, because the person
    // scanning it is holding something and that is the only answer that helps.
    let resp = test::call_service(
        &app,
        bind(other, json!({ "scan": GTIN, "packaging_level": "each", "quantity": 1 })).await,
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a bound barcode was rebound to another item"
    );
    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(
        body.contains(ITEM_CODE),
        "the refusal does not say what the barcode already means: {body}"
    );

    // Nor at another level on the same item: one label is on one box.
    let resp = test::call_service(
        &app,
        bind(item, json!({ "scan": GTIN, "packaging_level": "each", "quantity": 1 })).await,
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "one barcode was made to mean two levels of one item"
    );

    // ── a misread GTIN is refused rather than bound as a code ───────────
    let resp = test::call_service(
        &app,
        bind(other, json!({ "scan": "09312345678890", "packaging_level": "each" })).await,
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a failed check digit was bound as an internal code, which makes a typo permanent"
    );

    // ── a shared binding is a different refusal, and has no way out ─────
    //
    // **Two live rows for one string is a legal state**, because the exclusion
    // constraint COALESCEs the tenant and the read policy shows this tenant the
    // shared catalogue as well as its own. A handler reading that with
    // `query_opt` answers it by erroring, which is a 500 for a state the schema
    // permits on purpose.
    let shared_gtin = "09312345678884";
    db.execute("DELETE FROM item_barcode WHERE barcode = $1", &[&shared_gtin])
        .await
        .expect("clear");
    // **`RESET ROLE` first, and it is not ceremony.** `item_barcode_own_write`
    // is `FOR ALL` with a `WITH CHECK`, so it constrains INSERT — and a shared
    // row needs `is_platform()`. The pooled connection is carrying whatever
    // role the last request left on it, which the handover records as a thing
    // that bites; resetting it puts this back on the superuser, who bypasses
    // RLS. `nylonite.tenant_id` is session-level and survives.
    //
    // That the app role *cannot* write a shared row is the property being
    // relied on rather than worked around: it is why a tenant binding can
    // never quietly become everybody's.
    db.execute("RESET ROLE", &[]).await.expect("the superuser back");
    // Written as the platform would: no tenant, so it means this globally.
    db.execute(
        "INSERT INTO item_barcode (tenant_id, item_id, barcode, scheme, unit_id, quantity)
         SELECT NULL, $1, $2, 'gtin', u.id, 1 FROM unit u WHERE u.code = 'ea'",
        &[&other, &shared_gtin],
    )
    .await
    .expect("a shared binding");

    let resp = test::call_service(
        &app,
        bind(item, json!({ "scan": shared_gtin, "packaging_level": "each", "quantity": 1 })).await,
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a shared binding was overwritten from the floor"
    );
    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(
        body.contains("shared catalogue"),
        "the refusal reads as an ordinary clash rather than one with no way out: {body}"
    );
    db.execute("DELETE FROM item_barcode WHERE barcode = $1", &[&shared_gtin])
        .await
        .expect("clear");

    // ── and the list read answers what the item holds ───────────────────
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/items/{item}/barcodes"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let rows: Value = test::read_body_json(resp).await;
    let rows = rows.as_array().expect("a list");
    assert_eq!(rows.len(), 1, "the item answers to one thing: {rows:?}");
    assert_eq!(rows[0]["barcode"], GTIN);

    // ── the scan finds it, which is the point of writing it ─────────────
    //
    // The binding is only worth anything if the resolver reads it back. This is
    // the join between D164's write and D34's read, and neither half proves it.
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/resolve?scan={GTIN}"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let found: Value = test::read_body_json(resp).await;
    let subjects = found["subjects"].as_array().expect("subjects");
    assert!(
        subjects.iter().any(|s| s["code"] == ITEM_CODE),
        "a barcode bound a moment ago does not resolve: {found}"
    );

    forget(&db).await;
}
