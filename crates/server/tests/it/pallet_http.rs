//! Cartons onto pallets, over HTTP (D246, D247, migration 139).
//!
//! A sealed carton goes onto its order's pallet at a spot, and still reads as
//! sealed on the bench. The pallet goes into a wrap: three deep, the most. A
//! fourth level, or a loop, is refused. A breakdown pallet names no order, and
//! an open carton put on one is said.
//!
//! Order numbers and the item are random, so this runs against a database it
//! has already written to.

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
async fn cartons_go_onto_pallets_three_deep_at_most() {
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
        test::TestRequest::post()
            .uri("/tokens")
            .insert_header(auth.clone())
            .set_json(json!({ "label": "pallet test" })),
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());

    let run = &Uuid::new_v4().simple().to_string()[..8];
    let if_id = format!("if-pl-{run}");
    let (status, imported) = call(
        &app,
        test::TestRequest::post()
            .uri("/import/fulfilment?apply=true")
            .insert_header(("authorization", token.clone()))
            .set_payload(
                json!({
                    "order": format!("S-pl-{run}"),
                    "customer": format!("Pallet Test {run}"),
                    "fulfilment_id": if_id,
                    "fulfilment_number": "IF-PL",
                    "status": "Picked",
                    "observed_at": "2026-10-14T10:00:00Z",
                    "lines": [
                        { "line": 1, "item": format!("PL-{run}"), "description": "Pallet test item",
                          "location": "Melbourne Warehouse", "quantity": 6, "external_line": "1" },
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

    let post = |uri: String, body: Value| test::TestRequest::post().uri(&uri).insert_header(auth.clone()).set_json(body);
    let make = |preset: &str, order: Option<Uuid>, breaks_down: bool| {
        let id = Uuid::now_v7();
        (id, json!({
            "id": id, "fulfilment_id": order, "package_type_id": preset, "location_id": PACK,
            "breaks_down": breaks_down,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T10:10:00Z"
        }))
    };
    let contain = |child: Uuid, parent: Uuid, at: Option<Value>| {
        post(format!("/packages/{child}/contain"), json!({
            "parent_package_id": parent, "at": at,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T10:20:00Z"
        }))
    };

    // ── a sealed carton onto its order's pallet, at a spot ───────────────
    let (carton, body) = make(SMALL_BOX, Some(fulfilment), false);
    assert_eq!(call(&app, post("/packages".into(), body)).await.0, 200);
    let (status, handed) = call(
        &app,
        post("/handovers".into(), json!({
            "fulfilment_line_id": line, "quantity": 6, "to_package_id": carton,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T10:11:00Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "{handed}");
    let (status, _) = call(
        &app,
        post(format!("/packages/{carton}/seal"), json!({
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T10:12:00Z"
        })),
    )
    .await;
    assert_eq!(status, 200);

    let (pallet, body) = make(PALLET, Some(fulfilment), false);
    assert_eq!(call(&app, post("/packages".into(), body)).await.0, 200);
    let (status, on) = call(&app, contain(carton, pallet, Some(json!({ "x_mm": 0, "y_mm": 0, "z_mm": 0, "turned": true })))).await;
    assert_eq!(status, 200, "{on}");
    assert_eq!(on["warnings"].as_array().unwrap().len(), 0, "{on}");
    let spot = db
        .query_one(
            "SELECT at_x_mm, at_z_mm, turned FROM package_event WHERE package_id = $1 AND kind = 'contained'",
            &[&carton],
        )
        .await
        .expect("the containment");
    assert_eq!((spot.get::<_, Option<i32>>(0), spot.get::<_, Option<bool>>(2)), (Some(0), Some(true)));

    let (_, bench) = call(&app, test::TestRequest::get().uri(&format!("/fulfilments/{fulfilment}/bench")).insert_header(auth.clone())).await;
    let on_bench = bench["cartons"].as_array().unwrap().iter().find(|c| c["id"] == carton.to_string()).cloned().expect("the carton");
    assert_eq!(on_bench["sealed"], true, "on a pallet, still sealed: {on_bench}");

    // ── the pallet into a wrap: three deep, the most ─────────────────────
    let (wrap, body) = make(PALLET, Some(fulfilment), false);
    assert_eq!(call(&app, post("/packages".into(), body)).await.0, 200);
    let (status, wrapped) = call(&app, contain(pallet, wrap, None)).await;
    assert_eq!(status, 200, "{wrapped}");

    let (another, body) = make(SMALL_BOX, Some(fulfilment), false);
    assert_eq!(call(&app, post("/packages".into(), body)).await.0, 200);
    let (status, deep) = call(&app, contain(another, carton, None)).await;
    assert_eq!(status, 400, "a fourth level: {deep}");
    assert!(deep.to_string().contains("4 deep"), "{deep}");
    let (status, looped) = call(&app, contain(wrap, carton, None)).await;
    assert_eq!(status, 400, "the wrap inside what it wraps: {looped}");
    assert!(looped.to_string().contains("inside itself"), "{looped}");

    // ── a breakdown pallet carries many orders, sealed and labelled ──────
    let (_, body) = make(PALLET, Some(fulfilment), true);
    let (status, named) = call(&app, post("/packages".into(), body)).await;
    assert_eq!(status, 400, "a breakdown pallet names no order: {named}");
    let (breakdown, body) = make(PALLET, None, true);
    assert_eq!(call(&app, post("/packages".into(), body)).await.0, 200);
    let (status, open) = call(&app, contain(another, breakdown, None)).await;
    assert_eq!(status, 200, "{open}");
    assert!(open["warnings"].to_string().contains("seal and label it first"), "{open}");

    // ── what is delivered is what is booked (D247) ───────────────────────
    for (n, p) in [pallet, wrap].into_iter().enumerate() {
        let (status, _) = call(
            &app,
            post(format!("/packages/{p}/seal"), json!({
                "client_event_id": Uuid::now_v7(), "occurred_at": format!("2026-10-14T10:3{n}:00Z")
            })),
        )
        .await;
        assert_eq!(status, 200);
    }
    let book = |packages: Vec<Uuid>| post("/consignments".into(), json!({ "id": Uuid::now_v7(), "package_ids": packages }));
    let (status, riding) = call(&app, book(vec![carton])).await;
    assert_eq!(status, 400, "a carton on a pallet that goes as one: {riding}");
    assert!(riding.to_string().contains("goes with the pallet"), "{riding}");
    let (status, broken) = call(&app, book(vec![breakdown])).await;
    assert_eq!(status, 400, "{broken}");
    assert!(broken.to_string().contains("broken down"), "{broken}");
    let (status, booked) = call(&app, book(vec![wrap])).await;
    assert_eq!(status, 200, "the outermost, which goes as one: {booked}");

    // ── despatched whole: the carton by its line, the rest by themselves ─
    db.execute("SELECT projection_run_all(tenant_id) FROM fulfilment WHERE id = $1", &[&fulfilment])
        .await
        .expect("fold");
    let (_, despatch) = call(
        &app,
        test::TestRequest::get().uri("/sites/a5170000-0000-0000-0000-000000000001/despatch").insert_header(auth.clone()),
    )
    .await;
    let wrap_row = despatch["booked"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["packages"].as_array().unwrap().iter())
        .find(|p| p["id"] == wrap.to_string())
        .cloned()
        .expect("the wrap is booked");
    let on_it = wrap_row["inside"].as_array().unwrap();
    assert_eq!(on_it.len(), 2, "the pallet and the carton on it: {wrap_row}");
    let carton_row = on_it.iter().find(|p| p["id"] == carton.to_string()).expect("the carton");
    assert_eq!(carton_row["lines"][0]["quantity"], 6, "{carton_row}");

    let (status, line_gone) = call(
        &app,
        post(format!("/packages/{carton}/despatch"), json!({
            "fulfilment_line_id": line, "quantity": 6,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T11:00:00Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "{line_gone}");
    for p in [pallet, wrap] {
        let (status, gone) = call(
            &app,
            post(format!("/packages/{p}/despatch"), json!({
                "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T11:00:01Z"
            })),
        )
        .await;
        assert_eq!(status, 200, "{gone}");
        assert_eq!(gone["package_status_ledger"], "despatched", "{gone}");
        assert_eq!(gone["warnings"].as_array().unwrap().len(), 0, "nothing of its own left behind: {gone}");
    }
}
