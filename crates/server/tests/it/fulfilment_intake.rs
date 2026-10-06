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

use super::common;
use common::{pool, url};

#[actix_web::test]
async fn a_fulfilment_loads_once_and_a_resend_writes_nothing() {
    let _file = common::file_gate(module_path!());
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
    let _file = common::file_gate(module_path!());
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

/// D172: a pick made elsewhere is a level, reported, and never on the ledger.
///
/// What NetSuite says is picked lands in `external_picked_quantity`, and
/// `picked_quantity` (this system's own ledger, J68) stays at nought. Reports
/// are ordered by when NetSuite says they were so, not by when they arrived.
#[actix_web::test]
async fn a_pick_made_elsewhere_is_reported_as_a_level() {
    let _file = common::file_gate(module_path!());
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
            .set_json(json!({ "label": "the bridge, picks, from a test" }))
            .to_request(),
        "POST /tokens",
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());

    let run = &Uuid::new_v4().simple().to_string()[..8];
    let order = format!("S-pk-{run}");
    let if_id = format!("if-pk-{run}");
    let body = |picked: f64, at: &str| {
        json!({
            "order": order,
            "customer": format!("Pick Test {run}"),
            "order_id": format!("so-pk-{run}"),
            "fulfilment_id": if_id,
            "fulfilment_number": "IF-PK",
            "status": "Picked",
            "observed_at": at,
            "picked_by": "Casual Melbourne",
            "lines": [{ "line": 1, "item": "GLOVE-M", "location": "Melbourne Warehouse",
                        "quantity": 4, "picked": picked, "external_line": "1" }]
        })
    };
    let send = |b: Value, apply: bool| {
        test::TestRequest::post()
            .uri(if apply { "/import/fulfilment?apply=true" } else { "/import/fulfilment" })
            .insert_header(("authorization", token.clone()))
            .set_payload(b.to_string())
            .to_request()
    };

    let (db, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.expect("connect");
    tokio::spawn(async move { let _ = connection.await; });
    // Fold this tenant now rather than wait for the scheduler, then read the line.
    let levels = || async {
        let tenant: Uuid = db
            .query_one("SELECT tenant_id FROM fulfilment WHERE external_id = $1", &[&if_id])
            .await
            .expect("the fulfilment")
            .get(0);
        db.execute("SELECT projection_fulfilment_rebuild($1)", &[&tenant]).await.expect("fold");
        let row = db
            .query_one(
                "SELECT fl.external_picked_quantity, fl.picked_quantity
                   FROM fulfilment_line fl JOIN fulfilment f ON f.id = fl.fulfilment_id
                  WHERE f.external_id = $1",
                &[&if_id],
            )
            .await
            .expect("the line");
        (row.get::<_, i64>(0), row.get::<_, i64>(1))
    };

    // ── a dry run records nothing ────────────────────────────────────────
    let dry = common::ok_json(&app, send(body(4.0, "2026-09-28T09:00:00Z"), false), "dry").await;
    assert!(dry["picks"].is_null(), "a dry run reported picks: {dry}");

    // ── picked in NetSuite: a level, beside the ledger and not in it ─────
    let first = common::ok_json(&app, send(body(4.0, "2026-09-28T09:00:00Z"), true), "first").await;
    assert_eq!(first["picks"]["recorded"], json!(1), "{first}");
    assert_eq!(levels().await, (4, 0), "reported as picked elsewhere; the ledger saw nothing");

    // ── the same bytes again are the same arrival, and record nothing ────
    let replay = common::ok_json(&app, send(body(4.0, "2026-09-28T09:00:00Z"), true), "replay").await;
    assert!(replay["picks"].is_null(), "a replay recorded picks: {replay}");

    // ── a later report replaces the level: an un-pick of two ─────────────
    let later = common::ok_json(&app, send(body(2.0, "2026-09-28T10:00:00Z"), true), "later").await;
    assert_eq!(later["picks"]["recorded"], json!(1), "{later}");
    assert_eq!(levels().await, (2, 0), "the newest report is the level");

    // ── an older report arriving late does not ────────────────────────────
    let late = common::ok_json(&app, send(body(3.0, "2026-09-28T08:00:00Z"), true), "late").await;
    assert_eq!(late["picks"]["recorded"], json!(1), "still recorded: it happened: {late}");
    assert_eq!(levels().await, (2, 0), "but ordered by when NetSuite says, not when it arrived");

    // ── a report for a line the fulfilment does not have ─────────────────
    let mut stray = body(1.0, "2026-09-28T11:00:00Z");
    stray["lines"][0]["external_line"] = json!("99");
    stray["lines"][0]["line"] = json!(2);
    let s = common::ok_json(&app, send(stray, true), "stray").await;
    assert_eq!(s["picks"]["recorded"], json!(1), "line 99 was loaded by the same send, so it matches: {s}");
}

/// Packed or shipped in NetSuite, it is not work here; Picked again, it is (D187).
#[actix_web::test]
async fn a_fulfilment_finished_elsewhere_is_closed_here() {
    let _file = common::file_gate(module_path!());
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
            .insert_header(("authorization", session.clone()))
            .set_json(json!({ "label": "the bridge, closing, from a test" }))
            .to_request(),
        "POST /tokens",
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());
    let run = &Uuid::new_v4().simple().to_string()[..8];
    let if_id = format!("if-cl-{run}");
    let picked = json!({
        "order": format!("S-cl-{run}"), "customer": format!("Close Test {run}"),
        "order_id": format!("so-cl-{run}"), "fulfilment_id": if_id, "fulfilment_number": format!("IF-CL-{run}"),
        "status": "Picked", "observed_at": "2026-10-02T09:00:00+10:00",
        "lines": [{ "line": 1, "item": "GLOVE-M", "location": "Melbourne Warehouse", "quantity": 2, "picked": 2, "external_line": "1" }]
    });
    let call = |method: &str, uri: &str, body: Option<Value>| {
        let mut r = match method {
            "GET" => test::TestRequest::get(),
            _ => test::TestRequest::post(),
        }
        .uri(uri)
        .insert_header(("authorization", token.clone()));
        if let Some(b) = body {
            r = r.set_payload(b.to_string()).insert_header(("content-type", "application/json"));
        }
        r.to_request()
    };
    let open = |app| {
        let req = call("GET", "/import/fulfilments/open", None);
        async move { common::ok_json(app, req, "the open fulfilments").await }
    };
    let listed = |v: &Value| v.as_array().unwrap().iter().any(|f| f["fulfilment_id"] == if_id.as_str());

    common::ok_json(&app, call("POST", "/import/fulfilment?apply=true", Some(picked.clone())), "picked").await;
    assert!(listed(&open(&app).await), "a picked fulfilment is open");

    let said = common::ok_json(
        &app,
        call("POST", "/import/fulfilment/status", Some(json!({ "fulfilment_id": if_id, "status": "ItemShip:C" }))),
        "shipped in NetSuite",
    )
    .await;
    assert_eq!((said["changed"].as_u64(), said["closed"].as_str()), (Some(1), Some("shipped")));
    assert!(!listed(&open(&app).await), "shipped there is not open here");
    let again = common::ok_json(
        &app,
        call("POST", "/import/fulfilment/status", Some(json!({ "fulfilment_id": if_id, "status": "Shipped" }))),
        "shipped again",
    )
    .await;
    assert_eq!(again["changed"], 0, "saying it twice changes nothing");
    let r = test::call_service(&app, call("POST", "/import/fulfilment/status", Some(json!({ "fulfilment_id": if_id, "status": "Lost" })))).await;
    assert_eq!(r.status().as_u16(), 400);

    // Back to Picked there: work here again.
    let mut back = picked.clone();
    back["observed_at"] = json!("2026-10-02T11:00:00+10:00");
    common::ok_json(&app, call("POST", "/import/fulfilment?apply=true", Some(back)), "picked again").await;
    assert!(listed(&open(&app).await), "picked again is open again");
}

/// A kit is ordered, and its parts are packed (D223): the kit's own line
/// commits nothing and records no picks; its parts are the goods, and the
/// bench shows them under it. A kit sent before the sender said it was one
/// stays committed, and the send says so.
#[actix_web::test]
async fn a_kit_is_ordered_and_its_parts_are_packed() {
    let _file = common::file_gate(module_path!());
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
            .insert_header(("authorization", session.clone()))
            .set_json(json!({ "label": "the bridge, kits, from a test" }))
            .to_request(),
        "POST /tokens",
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());

    let run = &Uuid::new_v4().simple().to_string()[..8];
    let (kit, head, bottle) = (format!("KIT-{run}"), format!("HEAD-{run}"), format!("BOTTLE-{run}"));
    // The kit, then its parts, as the bridge sends them: by line key.
    let body = |order: &str, if_id: &str, said: bool| {
        let (kit_type, part_of) = if said { (json!("Kit"), json!("0")) } else { (Value::Null, Value::Null) };
        json!({
            "order": order, "customer": format!("Kit Test {run}"),
            "order_id": format!("so-{if_id}"), "fulfilment_id": if_id, "fulfilment_number": format!("IF-{if_id}"),
            "status": "Picked", "observed_at": "2026-10-06T09:00:00Z",
            "lines": [
                { "line": 1, "item": kit, "description": "Sprayer and bottle", "location": "Melbourne Warehouse",
                  "quantity": 10, "external_line": "0", "item_type": kit_type },
                { "line": 2, "item": head, "description": "Trigger head", "location": "Melbourne Warehouse",
                  "quantity": 10, "external_line": "1", "kit_line": part_of, "item_type": "InvtPart" },
                { "line": 3, "item": bottle, "description": "1L bottle", "location": "Melbourne Warehouse",
                  "quantity": 10, "external_line": "4", "kit_line": part_of, "item_type": "InvtPart" },
            ]
        })
    };
    let send = |b: Value| {
        test::TestRequest::post()
            .uri("/import/fulfilment?apply=true")
            .insert_header(("authorization", token.clone()))
            .set_payload(b.to_string())
            .to_request()
    };
    let (db, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.expect("connect");
    tokio::spawn(async move { let _ = connection.await; });
    // Each line of the order: its item, its kit's item, and what it commits.
    let lines = |order: String| {
        let db = &db;
        async move {
            db.query(
                "SELECT i.code, ki.code, fl.quantity
                   FROM order_line ol
                   JOIN \"order\" o ON o.id = ol.order_id
                   JOIN item i ON i.id = ol.item_id
                   LEFT JOIN order_line kl ON kl.id = ol.kit_line_id
                   LEFT JOIN item ki ON ki.id = kl.item_id
                   LEFT JOIN fulfilment_line fl ON fl.order_line_id = ol.id
                  WHERE o.confirmation_number = $1
                  ORDER BY ol.line_number",
                &[&order],
            )
            .await
            .expect("the lines")
            .iter()
            .map(|r| (r.get::<_, String>(0), r.get::<_, Option<String>>(1), r.get::<_, Option<i64>>(2)))
            .collect::<Vec<_>>()
        }
    };

    // ── sent saying which is the kit ─────────────────────────────────────
    let order = format!("S-kit-{run}");
    let sent = common::ok_json(&app, send(body(&order, &format!("if-kit-{run}"), true)), "kit").await;
    assert_eq!(
        (sent["loaded"]["units_to_pick"].as_i64(), sent["loaded"]["parts_of_kits"].as_i64()),
        (Some(20), Some(2)),
        "the parts are the goods: {sent}"
    );
    assert_eq!(sent["picks"]["recorded"], json!(2), "picks on the parts, none on the kit: {sent}");
    assert_eq!(sent["picks"]["unmatched"], json!([]), "{sent}");
    assert_eq!(
        lines(order.clone()).await,
        vec![
            (kit.clone(), None, None),
            (head.clone(), Some(kit.clone()), Some(10)),
            (bottle.clone(), Some(kit.clone()), Some(10)),
        ],
        "the kit is ordered and commits nothing; its parts say whose they are"
    );

    // ── the bench: the parts, under their kit ────────────────────────────
    let fulfilment: Uuid = db
        .query_one("SELECT id FROM fulfilment WHERE external_id = $1", &[&format!("if-kit-{run}")])
        .await
        .expect("the fulfilment")
        .get(0);
    let bench = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/fulfilments/{fulfilment}/bench"))
            .insert_header(("authorization", session.clone()))
            .to_request(),
        "the bench",
    )
    .await;
    let on_bench: Vec<(String, Value)> = bench["lines"]
        .as_array()
        .unwrap_or_else(|| panic!("no lines: {bench}"))
        .iter()
        .map(|l| (l["item_code"].as_str().unwrap().to_string(), l["kit"]["item_code"].clone()))
        .collect();
    assert_eq!(on_bench, vec![(bottle.clone(), json!(kit)), (head.clone(), json!(kit))], "{bench}");
    assert_eq!(bench["lines"][0]["kit"]["ordered"], 10);

    // ── sent before the sender said, then again saying ───────────────────
    let before = format!("S-old-{run}");
    let if_old = format!("if-old-{run}");
    common::ok_json(&app, send(body(&before, &if_old, false)), "unsaid").await;
    let again = common::ok_json(&app, send(body(&before, &if_old, true)), "said").await;
    assert_eq!(again["loaded"]["parts_of_kits"], 2, "the parts learn their kit: {again}");
    let differs = again["loaded"]["differs"].as_array().unwrap();
    assert!(
        differs.iter().any(|d| d["item"] == json!(kit) && d["field"] == "kit" && d["on_file"] == 10),
        "the kit's commitment stays, and is said: {again}"
    );
    assert_eq!(
        lines(before).await,
        vec![
            (kit.clone(), None, Some(10)),
            (head.clone(), Some(kit.clone()), Some(10)),
            (bottle.clone(), Some(kit.clone()), Some(10)),
        ]
    );
}
