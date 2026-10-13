//! Units moved from one carton to another, over HTTP (D246, migration 138).
//!
//! Ten handed into carton A, four moved on to carton B, both sealed: the line
//! is packed ten, not fourteen, and each carton holds what it does. A move out
//! of and into sealed cartons is recorded and said (D5); one into the carton it
//! is already in is refused.
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
async fn units_moved_between_cartons_are_packed_once() {
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
            .set_json(json!({ "label": "repack test" })),
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());

    // ── NetSuite says ten are picked ─────────────────────────────────────
    let run = &Uuid::new_v4().simple().to_string()[..8];
    let code = format!("RP-{run}");
    let if_id = format!("if-rp-{run}");
    let (status, imported) = call(
        &app,
        test::TestRequest::post()
            .uri("/import/fulfilment?apply=true")
            .insert_header(("authorization", token.clone()))
            .set_payload(
                json!({
                    "order": format!("S-rp-{run}"),
                    "customer": format!("Repack Test {run}"),
                    "fulfilment_id": if_id,
                    "fulfilment_number": "IF-RP",
                    "status": "Picked",
                    "observed_at": "2026-10-14T09:00:00Z",
                    "lines": [
                        { "line": 1, "item": code, "description": "Repack test item",
                          "location": "Melbourne Warehouse", "quantity": 10, "external_line": "1" },
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
            "SELECT f.id, fl.id, fl.tenant_id FROM fulfilment_line fl JOIN fulfilment f ON f.id = fl.fulfilment_id
              WHERE f.external_id = $1",
            &[&if_id],
        )
        .await
        .expect("the fulfilment");
    let (fulfilment, line, tenant): (Uuid, Uuid, Uuid) = (row.get(0), row.get(1), row.get(2));

    let post = |uri: String, body: Value| test::TestRequest::post().uri(&uri).insert_header(auth.clone()).set_json(body);
    let carton = || {
        let id = Uuid::now_v7();
        (id, json!({
            "id": id, "fulfilment_id": fulfilment, "package_type_id": SMALL_BOX, "location_id": PACK,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T09:10:00Z"
        }))
    };
    let (a, body) = carton();
    assert_eq!(call(&app, post("/packages".into(), body)).await.0, 200);
    let (b, body) = carton();
    assert_eq!(call(&app, post("/packages".into(), body)).await.0, 200);
    let (status, handed) = call(
        &app,
        post("/handovers".into(), json!({
            "fulfilment_line_id": line, "quantity": 10, "to_package_id": a,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T09:10:01Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "{handed}");

    // ── four moved on to B, sent twice: one act ──────────────────────────
    let act = Uuid::now_v7();
    let four = || {
        post("/repacks".into(), json!({
            "fulfilment_line_id": line, "from_package_id": a, "to_package_id": b, "quantity": 4,
            "client_event_id": act, "occurred_at": "2026-10-14T09:11:00Z"
        }))
    };
    let (status, moved) = call(&app, four()).await;
    assert_eq!(status, 200, "{moved}");
    assert_eq!(moved["warnings"].as_array().unwrap().len(), 0, "two open cartons of the order: {moved}");
    let (status, again) = call(&app, four()).await;
    assert_eq!(status, 200, "{again}");
    assert_eq!(again["movement_id"], moved["movement_id"], "the same act");

    // ── both sealed: packed ten, not fourteen ────────────────────────────
    for (n, carton) in [a, b].into_iter().enumerate() {
        let (status, sealed) = call(
            &app,
            post(format!("/packages/{carton}/seal"), json!({
                "client_event_id": Uuid::now_v7(), "occurred_at": format!("2026-10-14T09:12:0{n}Z")
            })),
        )
        .await;
        assert_eq!(status, 200, "{sealed}");
    }
    db.execute("SELECT projection_run_all($1)", &[&tenant]).await.expect("fold");
    let packed = || {
        let db = &db;
        async move {
            db.query_one("SELECT packed_quantity FROM fulfilment_line WHERE id = $1", &[&line])
                .await
                .unwrap()
                .get::<_, i64>(0)
        }
    };
    assert_eq!(packed().await, 10, "four moved on are counted once, in B");

    let (_, bench) = call(&app, test::TestRequest::get().uri(&format!("/fulfilments/{fulfilment}/bench")).insert_header(auth.clone())).await;
    let holds = |id: Uuid| -> i64 {
        bench["cartons"].as_array().unwrap().iter().find(|c| c["id"] == id.to_string()).expect("the carton")["contents"][0]["quantity"]
            .as_i64()
            .unwrap()
    };
    assert_eq!((holds(a), holds(b)), (6, 4), "each holds what it does: {bench}");
    assert_eq!(bench["lines"][0]["remaining"], 0, "nothing left to pack: {bench}");

    // ── out of and into sealed cartons: recorded, and said ───────────────
    let (status, said) = call(
        &app,
        post("/repacks".into(), json!({
            "fulfilment_line_id": line, "from_package_id": a, "to_package_id": b, "quantity": 1,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T09:13:00Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "a floor act is recorded (D5): {said}");
    assert_eq!(said["warnings"].to_string().matches("open it first").count(), 2, "{said}");
    db.execute("SELECT projection_run_all($1)", &[&tenant]).await.expect("fold");
    assert_eq!(packed().await, 10, "out of one sealed carton and into another");

    // ── into the carton it is already in: refused ────────────────────────
    let (status, refused) = call(
        &app,
        post("/repacks".into(), json!({
            "fulfilment_line_id": line, "from_package_id": a, "to_package_id": a, "quantity": 1,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T09:14:00Z"
        })),
    )
    .await;
    assert_eq!(status, 400, "{refused}");

    // ── taken out onto the bench, and back into a carton ─────────────────
    // Straight after the move, with no fold run: the ledger says what is
    // where.
    let pack: Uuid = PACK.parse().unwrap();
    let (status, out) = call(
        &app,
        post("/repacks".into(), json!({
            "fulfilment_line_id": line, "from_package_id": b, "to_location_id": pack, "quantity": 2,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T09:15:00Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "out of B onto the bench: {out}");
    let (status, back) = call(
        &app,
        post("/repacks".into(), json!({
            "fulfilment_line_id": line, "from_location_id": pack, "to_package_id": a, "quantity": 2,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T09:16:00Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "from the bench into A: {back}");
    let (status, shelf) = call(
        &app,
        post("/repacks".into(), json!({
            "fulfilment_line_id": line, "from_package_id": a, "to_location_id": "10c00000-0000-0000-0000-000000000001",
            "quantity": 1, "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T09:17:00Z"
        })),
    )
    .await;
    assert_eq!(status, 400, "a shelf is not the bench: {shelf}");

    db.execute("SELECT projection_run_all($1)", &[&tenant]).await.expect("fold");
    assert_eq!(packed().await, 10, "out and back, still ten");
    let (_, bench) = call(&app, test::TestRequest::get().uri(&format!("/fulfilments/{fulfilment}/bench")).insert_header(auth.clone())).await;
    let holds = |id: Uuid| -> i64 {
        bench["cartons"].as_array().unwrap().iter().find(|c| c["id"] == id.to_string()).expect("the carton")["contents"][0]["quantity"]
            .as_i64()
            .unwrap()
    };
    assert_eq!((holds(a), holds(b)), (7, 3), "{bench}");
}
