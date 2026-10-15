//! The board's plan kept as a draft, over HTTP (D245, migration 142).
//!
//! A pallet and a carton on it are planned, read back with the bench, and
//! replaced; a plan naming another order's line is refused; and making the
//! pallet, then the carton, from the plan uses it up, the carton planned onto
//! the pallet going onto the pallet as made. A thing as it is is planned onto
//! a pallet and made from the plan (migration 144), and a closed order's plan
//! is dropped by the scheduler (migration 143).
//!
//! Order numbers and items are random, so this runs against a database it has
//! already written to.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

/// PACK-1, where the fixture's Melbourne packs (migration 97).
const PACK: &str = "10c00000-0000-0000-0000-000000000004";
/// The fixture's small box preset.
const SMALL_BOX: &str = "9a7e0000-0000-0000-0000-0000000000b1";
/// The shared PALLET preset (migration 67).
const PALLET: &str = "9a7e0000-0000-0000-0000-000000000001";

async fn call<S>(app: &S, req: test::TestRequest) -> (u16, Value)
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let r = test::call_service(app, req.to_request()).await;
    let status = r.status().as_u16();
    let bytes = test::read_body(r).await;
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

/// An order picked in the other system, of eight of one new item: its
/// fulfilment, its line, and the other system's id for it.
async fn picked_order<S>(app: &S, token: &str, u: &str, name: &str) -> (Uuid, Uuid, String)
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let if_id = format!("if-pp{name}");
    let (status, imported) = call(
        app,
        test::TestRequest::post()
            .uri("/import/fulfilment?apply=true")
            .insert_header(("authorization", token.to_string()))
            .set_payload(
                json!({
                    "order": format!("S-pp{name}"),
                    "customer": format!("Plan Test {name}"),
                    "fulfilment_id": if_id,
                    "fulfilment_number": "IF-PP",
                    "status": "Picked",
                    "observed_at": "2026-10-14T12:00:00Z",
                    "lines": [
                        { "line": 1, "item": format!("PP{name}"), "description": "Plan test item",
                          "location": "Melbourne Warehouse", "quantity": 8, "external_line": "1" },
                    ]
                })
                .to_string(),
            ),
    )
    .await;
    assert_eq!(status, 200, "{imported}");
    let (db, connection) = tokio_postgres::connect(u, tokio_postgres::NoTls).await.expect("connect");
    tokio::spawn(async move { let _ = connection.await; });
    let row = db
        .query_one(
            "SELECT f.id, fl.id FROM fulfilment_line fl JOIN fulfilment f ON f.id = fl.fulfilment_id
              WHERE f.external_id = $1",
            &[&if_id],
        )
        .await
        .expect("the fulfilment");
    (row.get(0), row.get(1), if_id)
}

#[actix_web::test]
async fn the_board_keeps_its_plan_until_the_parcels_are_made() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let session = common::bearer(&app).await;
    let auth = ("authorization", session.clone());
    let post = |uri: String, body: Value| test::TestRequest::post().uri(&uri).insert_header(auth.clone()).set_json(body);
    let put = |uri: String, body: Value| test::TestRequest::put().uri(&uri).insert_header(auth.clone()).set_json(body);

    let (_, minted) = call(&app, post("/tokens".into(), json!({ "label": "plan test" }))).await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());
    let run = &Uuid::new_v4().simple().to_string()[..8];
    let (fulfilment, line, _) = picked_order(&app, &token, &u, &format!("a-{run}")).await;
    let (_, theirs, _) = picked_order(&app, &token, &u, &format!("b-{run}")).await;
    let plan_uri = format!("/fulfilments/{fulfilment}/pack-plan");
    let bench = || test::TestRequest::get().uri(&format!("/fulfilments/{fulfilment}/bench")).insert_header(auth.clone());

    // ── a pallet, and a carton planned onto it ──────────────────────────
    let (pallet, carton) = (Uuid::now_v7(), Uuid::now_v7());
    let plan = |quantity: i64| {
        json!({ "parcels": [
            { "id": pallet, "package_type_id": PALLET, "on_parcel_id": null, "on_package_id": null,
              "at": null, "placed_by_hand": true, "lines": [] },
            { "id": carton, "package_type_id": SMALL_BOX, "on_parcel_id": pallet, "on_package_id": null,
              "at": { "x_mm": 0, "y_mm": 0, "z_mm": 0, "turned": false },
              "placed_by_hand": false, "lines": [{ "line_id": line, "quantity": quantity }] },
        ]})
    };
    let (status, body) = call(&app, put(plan_uri.clone(), plan(6))).await;
    assert_eq!(status, 204, "{body}");
    let (status, body) = call(&app, put(plan_uri.clone(), plan(8))).await;
    assert_eq!(status, 204, "replaced, not added to: {body}");
    let (_, read) = call(&app, bench()).await;
    let planned = read["plan"].as_array().expect("the plan");
    assert_eq!(planned.len(), 2, "{read}");
    assert_eq!(planned[0]["id"], pallet.to_string(), "in the order sent");
    assert_eq!(planned[1]["on_parcel_id"], pallet.to_string());
    assert_eq!(planned[1]["lines"][0]["quantity"], 8, "the newest plan: {read}");
    assert_eq!(planned[1]["at"]["x_mm"], 0);

    // ── another order's line is not this plan's ─────────────────────────
    let (status, refused) = call(
        &app,
        put(plan_uri.clone(), json!({ "parcels": [
            { "id": Uuid::now_v7(), "package_type_id": SMALL_BOX, "on_parcel_id": null, "on_package_id": null,
              "at": null, "placed_by_hand": true, "lines": [{ "line_id": theirs, "quantity": 1 }] }
        ]})),
    )
    .await;
    assert_eq!(status, 400, "{refused}");
    let (_, still) = call(&app, bench()).await;
    assert_eq!(still["plan"].as_array().unwrap().len(), 2, "a refused plan changes nothing: {still}");

    // ── made from the plan, the plan is used up ─────────────────────────
    let make = |id: Uuid, preset: &str| {
        post("/packages".into(), json!({
            "id": id, "fulfilment_id": fulfilment, "package_type_id": preset, "location_id": PACK,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T12:10:00Z"
        }))
    };
    let (status, body) = call(&app, make(pallet, PALLET)).await;
    assert_eq!(status, 200, "{body}");
    let (_, read) = call(&app, bench()).await;
    let planned = read["plan"].as_array().unwrap();
    assert_eq!(planned.len(), 1, "the pallet is made: {read}");
    assert_eq!(planned[0]["on_package_id"], pallet.to_string(), "the carton goes on it as made: {read}");
    let (status, made) = call(&app, put(plan_uri.clone(), plan(8))).await;
    assert_eq!(status, 400, "a plan that names a made parcel is out of date: {made}");
    let (status, body) = call(&app, make(carton, SMALL_BOX)).await;
    assert_eq!(status, 200, "{body}");
    let (_, read) = call(&app, bench()).await;
    assert_eq!(read["plan"], json!([]), "everything planned is made: {read}");
}

/// **Closed, its plan goes** (settled 4, migration 143). An order closed in the
/// other system loses its plan when the scheduler next runs, and an open
/// order's plan stays.
#[actix_web::test]
async fn a_closed_orders_plan_is_dropped() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let session = common::bearer(&app).await;
    let auth = ("authorization", session.clone());
    let (_, minted) = call(
        &app,
        test::TestRequest::post().uri("/tokens").insert_header(auth.clone()).set_json(json!({ "label": "plan drop test" })),
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());
    let run = &Uuid::new_v4().simple().to_string()[..8];
    let (closing, closing_line, closing_if) = picked_order(&app, &token, &u, &format!("c-{run}")).await;
    let (open, open_line, _) = picked_order(&app, &token, &u, &format!("d-{run}")).await;

    let plan = |line: Uuid| json!({ "parcels": [
        { "id": Uuid::now_v7(), "package_type_id": SMALL_BOX, "on_parcel_id": null, "on_package_id": null,
          "at": null, "placed_by_hand": true, "lines": [{ "line_id": line, "quantity": 8 }] }
    ]});
    for (f, line) in [(closing, closing_line), (open, open_line)] {
        let put = test::TestRequest::put().uri(&format!("/fulfilments/{f}/pack-plan")).insert_header(auth.clone());
        let (status, body) = call(&app, put.set_json(plan(line))).await;
        assert_eq!(status, 204, "{body}");
    }
    let (status, said) = call(
        &app,
        test::TestRequest::post()
            .uri("/import/fulfilment/status")
            .insert_header(("authorization", token.clone()))
            .set_json(json!({ "fulfilment_id": closing_if, "status": "Packed" })),
    )
    .await;
    assert_eq!(status, 200, "{said}");

    let (db, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.expect("connect");
    tokio::spawn(async move { let _ = connection.await; });
    db.batch_execute("SET ROLE spork_scheduler").await.expect("the scheduler's role");
    let dropped: i64 = db.query_one("SELECT pack_plan_drop_closed()", &[]).await.expect("the drop").get(0);
    assert!(dropped >= 1, "the closed order's parcel at least");

    let plan_of = |f: Uuid| test::TestRequest::get().uri(&format!("/fulfilments/{f}/bench")).insert_header(auth.clone());
    let (_, read) = call(&app, plan_of(closing)).await;
    assert_eq!(read["plan"], json!([]), "closed, its plan is gone: {read}");
    let (_, read) = call(&app, plan_of(open)).await;
    assert_eq!(read["plan"].as_array().map(Vec::len), Some(1), "open, its plan stays: {read}");
}

/// **A thing as it is, planned onto a pallet** (migration 144): kept with its
/// item and level, refused holding another item, and used up when Ship makes
/// it from the plan.
#[actix_web::test]
async fn a_thing_as_it_is_is_planned_onto_a_pallet() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let session = common::bearer(&app).await;
    let auth = ("authorization", session.clone());
    let (_, minted) = call(
        &app,
        test::TestRequest::post().uri("/tokens").insert_header(auth.clone()).set_json(json!({ "label": "plan as is test" })),
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());
    let run = &Uuid::new_v4().simple().to_string()[..8];
    let (fulfilment, line, _) = picked_order(&app, &token, &u, &format!("e-{run}")).await;
    let (db, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.expect("connect");
    tokio::spawn(async move { let _ = connection.await; });
    let item: Uuid = db
        .query_one("SELECT ol.item_id FROM fulfilment_line fl JOIN order_line ol ON ol.id = fl.order_line_id WHERE fl.id = $1", &[&line])
        .await
        .expect("the line's item")
        .get(0);

    let (pallet, own) = (Uuid::now_v7(), Uuid::now_v7());
    let plan = |of: Uuid| json!({ "parcels": [
        { "id": pallet, "package_type_id": PALLET, "as_is": null, "on_parcel_id": null, "on_package_id": null,
          "at": null, "placed_by_hand": true, "lines": [] },
        { "id": own, "package_type_id": null, "as_is": { "item_id": of, "level": "each", "config_id": null },
          "on_parcel_id": pallet, "on_package_id": null, "at": null, "placed_by_hand": true,
          "lines": [{ "line_id": line, "quantity": 1 }] },
    ]});
    let put = |body: Value| test::TestRequest::put().uri(&format!("/fulfilments/{fulfilment}/pack-plan")).insert_header(auth.clone()).set_json(body);
    let (status, refused) = call(&app, put(plan(Uuid::now_v7()))).await;
    assert_eq!(status, 400, "another item's units: {refused}");
    let (status, body) = call(&app, put(plan(item))).await;
    assert_eq!(status, 204, "{body}");

    let bench = || test::TestRequest::get().uri(&format!("/fulfilments/{fulfilment}/bench")).insert_header(auth.clone());
    let (_, read) = call(&app, bench()).await;
    let planned = &read["plan"][1];
    assert_eq!(planned["as_is"]["item_id"], item.to_string(), "{read}");
    assert_eq!(planned["package_type_id"], Value::Null);
    assert_eq!(planned["on_parcel_id"], pallet.to_string());

    let (status, body) = call(
        &app,
        test::TestRequest::post().uri("/packages").insert_header(auth.clone()).set_json(json!({
            "id": own, "fulfilment_id": fulfilment, "own_item_id": item, "own_level": "each", "location_id": PACK,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-15T12:10:00Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (_, read) = call(&app, bench()).await;
    assert_eq!(read["plan"].as_array().map(Vec::len), Some(1), "made, it is no longer planned: {read}");
}
