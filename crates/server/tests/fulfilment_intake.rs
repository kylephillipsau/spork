//! One item fulfilment, sent from the NetSuite page (Spork phase 1).
//!
//! What the sender relies on is that it can send again: the page gets opened
//! twice, and a device resends after a failover (D170). So the assertions that
//! matter here are the second and third sends — the same fulfilment writes
//! nothing, and a changed quantity is reported rather than written.
//!
//! Order numbers and the new item's code are random, so this file can run
//! against a database it has already written to.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

mod common;
use common::{pool, url};

#[actix_web::test]
async fn a_fulfilment_loads_once_and_a_resend_writes_nothing() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let signed: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/sessions")
                .set_json(json!({ "email": "kyle@example.test", "password": "dock-station-1" }))
                .to_request(),
        )
        .await;
        assert!(r.status().is_success(), "the fixture signs in");
        test::read_body_json(r).await
    };
    let session = format!("Bearer {}", signed["token"].as_str().unwrap());

    let run = &Uuid::new_v4().simple().to_string()[..8];
    let order = format!("SO-{run}");
    let new_item = format!("SPORK-{run}");
    let fulfilment = |glove_qty: f64| {
        json!({
            "order": format!("Sales Order #{order}"),
            "customer": "Acme Foods : Dandenong",
            "po_ref": "PO-77",
            "date": "15/9/2026",
            "lines": [
                { "line": 1, "item": "GLOVE-M", "description": "Nitrile glove, medium",
                  "location": "Melbourne Warehouse", "quantity": glove_qty, "remaining": 10 },
                { "line": 2, "item": format!("STYLE : {new_item}"), "description": "Made on the page",
                  "location": "Sydney Warehouse", "quantity": 2 },
                { "line": 3, "item": "GLOVE-M", "location": "Atlantis Warehouse", "quantity": 1 }
            ]
        })
    };
    let send = |body: Value, auth: String, apply: bool| {
        test::TestRequest::post()
            .uri(if apply { "/import/fulfilment?apply=true" } else { "/import/fulfilment" })
            .insert_header(("authorization", auth))
            .set_payload(body.to_string())
            .to_request()
    };

    // ── a session is the wrong kind of credential (D158) ────────────────
    let r = test::call_service(&app, send(fulfilment(4.0), session.clone(), true)).await;
    assert_eq!(r.status(), 401, "a session reached the import path");

    let minted: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/tokens")
                .insert_header(("authorization", session.clone()))
                .set_json(json!({ "label": "the fulfilment userscript, from a test" }))
                .to_request(),
        )
        .await;
        assert_eq!(r.status(), 201, "a session mints a token");
        test::read_body_json(r).await
    };
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());

    // ── refused before anything is read ─────────────────────────────────
    let r = test::call_service(&app, send(json!({ "order": order, "lines": [] }), token.clone(), true)).await;
    assert_eq!(r.status(), 400, "a fulfilment with no lines");
    let r = test::call_service(&app, send(json!({ "lines": [] }), token.clone(), true)).await;
    assert_eq!(r.status(), 400, "a body that is not a fulfilment");

    // ── the dry run says what applying would do, and writes nothing ─────
    let dry: Value = test::read_body_json(
        test::call_service(&app, send(fulfilment(4.0), token.clone(), false)).await,
    )
    .await;
    assert_eq!(dry["order"], json!(order), "`Sales Order #` is read off the number");
    assert_eq!(dry["loaded"]["applied"], json!(false));
    assert_eq!(dry["loaded"]["orders_created"], json!(1));

    // ── applied ─────────────────────────────────────────────────────────
    let first: Value = test::read_body_json(
        test::call_service(&app, send(fulfilment(4.0), token.clone(), true)).await,
    )
    .await;
    let l = &first["loaded"];
    assert_eq!(l["applied"], json!(true));
    assert_eq!(l["orders_created"], json!(1), "the dry run wrote nothing, so this made it");
    assert_eq!(l["items_created"], json!(1), "the page's item is created: {first}");
    assert_eq!(l["lines_loaded"], json!(2));
    assert_eq!(l["units_to_pick"], json!(6));
    assert_eq!(
        l["fulfilments_created"],
        json!(2),
        "Melbourne and Sydney are two fulfilments of one order: {first}"
    );
    assert_eq!(l["commitments_created"], json!(2));
    let skipped = l["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1, "{first}");
    assert_eq!(skipped[0]["reason"], json!("unknown_warehouse"));
    assert_eq!(skipped[0]["line"], json!(3));

    // ── the same fulfilment again ───────────────────────────────────────
    let again: Value = test::read_body_json(
        test::call_service(&app, send(fulfilment(4.0), token.clone(), true)).await,
    )
    .await;
    let l = &again["loaded"];
    for counted in [
        "orders_created",
        "lines_created",
        "fulfilments_created",
        "commitments_created",
        "items_created",
        "sites_created",
        "customers_created",
    ] {
        assert_eq!(l[counted], json!(0), "a resend wrote {counted}: {again}");
    }
    assert_eq!(l["lines_loaded"], json!(2), "a resend still finds its lines");
    assert!(l["differs"].as_array().unwrap().is_empty());

    // ── a quantity that moved is reported, not written ──────────────────
    let moved: Value = test::read_body_json(
        test::call_service(&app, send(fulfilment(3.0), token.clone(), true)).await,
    )
    .await;
    let differs = moved["loaded"]["differs"].as_array().unwrap();
    assert_eq!(differs.len(), 1, "{moved}");
    assert_eq!(differs[0]["field"], json!("to_pick"));
    assert_eq!(differs[0]["on_file"], json!(4));
    assert_eq!(differs[0]["arrived"], json!(3));
    assert_eq!(moved["loaded"]["commitments_created"], json!(0));
}

/// D172: each item fulfilment is its own fulfilment, keyed by its internal id.
///
/// The case the userscript's (order, site) key got wrong: two item fulfilments
/// of one order at one site. And the migration path: a fulfilment loaded the old
/// way, with no ids, is adopted by the first send that names its document
/// rather than duplicated beside it.
#[actix_web::test]
async fn each_item_fulfilment_is_its_own_fulfilment() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let session = common::bearer(&app).await;
    let minted: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/tokens")
            .insert_header(("authorization", session))
            .set_json(json!({ "label": "the bridge, from a test" }))
            .to_request(),
        "POST /tokens",
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());

    let run = &Uuid::new_v4().simple().to_string()[..8];
    let order = format!("S-{run}");
    let (so_id, if_a, if_b, if_c) =
        (format!("so-{run}"), format!("if-a-{run}"), format!("if-b-{run}"), format!("if-c-{run}"));
    let body = |ids: Option<(&str, &str)>, site: &str, qty: f64| {
        let mut b = json!({
            "order": format!("Sales Order #{order}"),
            // Its own customer: the other test here makes "Acme Foods" at the
            // same moment, and two first sightings of one new customer race.
            "customer": format!("Bridge Test {run}"),
            "lines": [{ "line": 1, "item": "GLOVE-M", "location": site, "quantity": qty,
                        "external_line": "1" }]
        });
        if let Some((id, number)) = ids {
            b["order_id"] = json!(so_id);
            b["fulfilment_id"] = json!(id);
            b["fulfilment_number"] = json!(number);
        }
        b
    };
    let send = |b: Value| {
        test::TestRequest::post()
            .uri("/import/fulfilment?apply=true")
            .insert_header(("authorization", token.clone()))
            .set_payload(b.to_string())
            .to_request()
    };
    let loaded = |v: &Value, k: &str| v["loaded"][k].as_u64().unwrap();

    // ── the old way: by order and site, no ids ──────────────────────────
    let legacy = common::ok_json(&app, send(body(None, "Melbourne Warehouse", 3.0)), "legacy send").await;
    assert_eq!(loaded(&legacy, "fulfilments_created"), 1, "{legacy}");

    // ── the first send naming the document adopts that row ─────────────
    let a = common::ok_json(&app, send(body(Some((&if_a, "IF-A")), "Melbourne Warehouse", 3.0)), "IF A").await;
    assert_eq!(loaded(&a, "fulfilments_created"), 0, "adopted, not duplicated: {a}");
    assert_eq!(loaded(&a, "fulfilments_adopted"), 1, "{a}");

    // ── a second item fulfilment at the same site is a second fulfilment ─
    let b = common::ok_json(&app, send(body(Some((&if_b, "IF-B")), "Melbourne Warehouse", 2.0)), "IF B").await;
    assert_eq!(loaded(&b, "fulfilments_created"), 1, "{b}");
    assert_eq!(loaded(&b, "fulfilments_adopted"), 0, "the legacy row is already claimed: {b}");
    assert_eq!(loaded(&b, "commitments_created"), 1, "its own line, against the same order line: {b}");

    // ── and the same one sent again is the same row ────────────────────
    let again = common::ok_json(&app, send(body(Some((&if_b, "IF-B")), "Melbourne Warehouse", 2.0)), "IF B again").await;
    for k in ["fulfilments_created", "fulfilments_adopted", "commitments_created", "lines_created"] {
        assert_eq!(loaded(&again, k), 0, "a resend wrote {k}: {again}");
    }

    // ── another site, another document ──────────────────────────────────
    let c = common::ok_json(&app, send(body(Some((&if_c, "IF-C")), "Sydney Warehouse", 1.0)), "IF C").await;
    assert_eq!(loaded(&c, "fulfilments_created"), 1, "{c}");

    // ── what landed where ────────────────────────────────────────────────
    let (db, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.expect("connect");
    tokio::spawn(async move { let _ = connection.await; });
    let rows = db
        .query(
            "SELECT f.external_id, f.reference, fl.external_line, o.external_id
               FROM fulfilment f
               JOIN \"order\" o ON o.id = f.order_id
               JOIN fulfilment_line fl ON fl.fulfilment_id = f.id
              WHERE o.confirmation_number = $1
              ORDER BY f.external_id",
            &[&order],
        )
        .await
        .expect("read back");
    let got: Vec<(Option<String>, Option<String>, Option<String>, Option<String>)> =
        rows.iter().map(|r| (r.get(0), r.get(1), r.get(2), r.get(3))).collect();
    assert_eq!(got.len(), 3, "three item fulfilments, three fulfilments: {got:?}");
    for (id, number) in [(&if_a, "IF-A"), (&if_b, "IF-B"), (&if_c, "IF-C")] {
        let row = got.iter().find(|r| r.0.as_deref() == Some(id.as_str())).expect("each document");
        assert_eq!(row.1.as_deref(), Some(number), "the number a person quotes is the reference");
        assert_eq!(row.2.as_deref(), Some("1"), "the line key");
        assert_eq!(row.3.as_deref(), Some(so_id.as_str()), "the order learnt its own id");
    }
}
