//! A product's own carton, over HTTP (migration 98).
//!
//! The path the bench takes for goods picked in NetSuite: the fulfilment
//! arrives, the item's case pack says six to a carton, and whole cartons ship
//! as they are: made at the pack location, handed over into, sealed.
//!
//! Order numbers and the item are random, so this runs against a database it
//! has already written to.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

mod common;
use common::{pool, url};

/// PACK-1, where the fixture's Melbourne packs (migration 97).
const PACK: &str = "10c00000-0000-0000-0000-000000000004";
/// The fixture's small box preset.
const SMALL_BOX: &str = "9a7e0000-0000-0000-0000-0000000000b1";
/// The fixture's case pack for an item this test's fulfilment does not carry.
const OTHER_CONFIG: &str = "9ac40000-0000-0000-0000-000000000001";

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
async fn whole_cartons_ship_as_they_are() {
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
        test::TestRequest::post()
            .uri("/tokens")
            .insert_header(auth.clone())
            .set_json(json!({ "label": "own carton test" })),
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());

    // ── NetSuite says twelve are picked ──────────────────────────────────
    let run = &Uuid::new_v4().simple().to_string()[..8];
    let code = format!("OC-{run}");
    let if_id = format!("if-oc-{run}");
    let (status, imported) = call(
        &app,
        test::TestRequest::post()
            .uri("/import/fulfilment?apply=true")
            .insert_header(("authorization", token.clone()))
            .set_payload(
                json!({
                    "order": format!("S-oc-{run}"),
                    "customer": format!("Own Carton Test {run}"),
                    "fulfilment_id": if_id,
                    "fulfilment_number": "IF-OC",
                    "status": "Picked",
                    "observed_at": "2026-09-30T09:00:00Z",
                    "lines": [
                        { "line": 1, "item": code, "description": "Own carton test item",
                          "location": "Melbourne Warehouse", "quantity": 12, "external_line": "1" },
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
    let (fulfilment, line): (Uuid, Uuid) = (row.get(0), row.get(1));

    // Six to a carton, as a prepack list would say.
    let config: Uuid = db
        .query_one(
            "INSERT INTO item_packing_config (tenant_id, item_id, units_per_inner, inners_per_carton)
             SELECT tenant_id, id, 1, 6 FROM item WHERE code = $1 RETURNING id",
            &[&code],
        )
        .await
        .expect("a case pack")
        .get(0);

    let bench = || {
        test::TestRequest::get()
            .uri(&format!("/fulfilments/{fulfilment}/bench"))
            .insert_header(auth.clone())
    };

    // ── the bench offers the carton ──────────────────────────────────────
    let (status, before) = call(&app, bench()).await;
    assert_eq!(status, 200, "{before}");
    let l = &before["lines"][0];
    assert_eq!(l["own_carton"]["units"], 6, "{l}");
    assert_eq!(l["own_carton"]["item_packing_config_id"], config.to_string());
    assert!(l["own_carton"]["size"].is_null(), "nothing has measured a carton of it: {l}");
    assert!(l["own_carton"]["listed_weight_g"].is_null());
    assert!(before["unready"].is_null(), "the fixture's site packs at PACK-1 and has an owner");

    // ── one carton: made, handed over into, sealed ───────────────────────
    let carton = Uuid::now_v7();
    let (status, made) = call(
        &app,
        test::TestRequest::post().uri("/packages").insert_header(auth.clone()).set_json(json!({
            "id": carton,
            "fulfilment_id": fulfilment,
            "item_packing_config_id": config,
            "location_id": PACK,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-09-30T09:10:00Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "{made}");
    let (status, handed) = call(
        &app,
        test::TestRequest::post().uri("/handovers").insert_header(auth.clone()).set_json(json!({
            "fulfilment_line_id": line,
            "quantity": 6,
            "to_package_id": carton,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-09-30T09:10:01Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "{handed}");
    let (status, sealed) = call(
        &app,
        test::TestRequest::post()
            .uri(&format!("/packages/{carton}/seal"))
            .insert_header(auth.clone())
            .set_json(json!({ "client_event_id": Uuid::now_v7(), "occurred_at": "2026-09-30T09:10:02Z" })),
    )
    .await;
    assert_eq!(status, 200, "{sealed}");

    let (_, after) = call(&app, bench()).await;
    assert_eq!(after["lines"][0]["remaining"], 6, "one carton of six is packed: {after}");
    let c = after["cartons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == carton.to_string())
        .expect("the carton is on the bench");
    assert_eq!(c["own_carton_of"], code.as_str(), "{c}");
    assert!(c["package_type"].is_null(), "not a box type");
    assert_eq!(c["sealed"], true);
    assert_eq!(c["contents"][0]["quantity"], 6);

    // ── what a caller can get wrong ─────────────────────────────────────
    let refused = |body: Value| test::TestRequest::post().uri("/packages").insert_header(auth.clone()).set_json(body);
    let (status, both) = call(
        &app,
        refused(json!({
            "fulfilment_id": fulfilment,
            "item_packing_config_id": config,
            "package_type_id": SMALL_BOX,
            "location_id": PACK,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-09-30T09:11:00Z"
        })),
    )
    .await;
    assert_eq!(status, 400, "a box type and a product's carton at once: {both}");
    let (status, elsewhere) = call(
        &app,
        refused(json!({
            "fulfilment_id": fulfilment,
            "item_packing_config_id": OTHER_CONFIG,
            "location_id": PACK,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-09-30T09:11:00Z"
        })),
    )
    .await;
    assert_eq!(status, 400, "a carton of something not on the order: {elsewhere}");
    assert!(elsewhere.to_string().contains("does not carry"), "{elsewhere}");
}
