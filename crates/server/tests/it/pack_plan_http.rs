//! The board's plan kept as a draft, over HTTP (D245, migration 142).
//!
//! A pallet and a carton on it are planned, read back with the bench, and
//! replaced; a plan naming another order's line is refused; and making the
//! pallet, then the carton, from the plan uses it up, the carton planned onto
//! the pallet going onto the pallet as made.
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
    let mut lines = vec![];
    let mut fulfilments = vec![];
    for which in ["a", "b"] {
        let if_id = format!("if-pp{which}-{run}");
        let (status, imported) = call(
            &app,
            test::TestRequest::post()
                .uri("/import/fulfilment?apply=true")
                .insert_header(("authorization", token.clone()))
                .set_payload(
                    json!({
                        "order": format!("S-pp{which}-{run}"),
                        "customer": format!("Plan Test {run}"),
                        "fulfilment_id": if_id,
                        "fulfilment_number": "IF-PP",
                        "status": "Picked",
                        "observed_at": "2026-10-14T12:00:00Z",
                        "lines": [
                            { "line": 1, "item": format!("PP{which}-{run}"), "description": "Plan test item",
                              "location": "Melbourne Warehouse", "quantity": 8, "external_line": "1" },
                        ]
                    })
                    .to_string(),
                ),
        )
        .await;
        assert_eq!(status, 200, "{imported}");
        let (db, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.expect("connect");
        tokio::spawn(async move { let _ = connection.await; });
        let row = db
            .query_one(
                "SELECT f.id, fl.id FROM fulfilment_line fl JOIN fulfilment f ON f.id = fl.fulfilment_id
                  WHERE f.external_id = $1",
                &[&if_id],
            )
            .await
            .expect("the fulfilment");
        fulfilments.push(row.get::<_, Uuid>(0));
        lines.push(row.get::<_, Uuid>(1));
    }
    let (fulfilment, line, theirs) = (fulfilments[0], lines[0], lines[1]);
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
