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
