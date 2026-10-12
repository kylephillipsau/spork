//! A line checked at the bench, over HTTP (D244, migration 136).
//!
//! Two lines picked in NetSuite: one is checked, and the bench says so; the
//! other is handed into a carton unchecked, and sealing it is recorded with a
//! warning naming the line rather than refused (D5).
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
async fn a_line_is_checked_and_a_carton_sealed_without_its_check_says_so() {
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
            .set_json(json!({ "label": "pack check test" })),
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());

    // ── NetSuite says two lines are picked ───────────────────────────────
    let run = &Uuid::new_v4().simple().to_string()[..8];
    let (checked_code, unchecked_code) = (format!("PCA-{run}"), format!("PCB-{run}"));
    let if_id = format!("if-pc-{run}");
    let (status, imported) = call(
        &app,
        test::TestRequest::post()
            .uri("/import/fulfilment?apply=true")
            .insert_header(("authorization", token.clone()))
            .set_payload(
                json!({
                    "order": format!("S-pc-{run}"),
                    "customer": format!("Pack Check Test {run}"),
                    "fulfilment_id": if_id,
                    "fulfilment_number": "IF-PC",
                    "status": "Picked",
                    "observed_at": "2026-10-12T09:00:00Z",
                    "lines": [
                        { "line": 1, "item": checked_code, "description": "Checked item",
                          "location": "Melbourne Warehouse", "quantity": 12, "external_line": "1" },
                        { "line": 2, "item": unchecked_code, "description": "Unchecked item",
                          "location": "Melbourne Warehouse", "quantity": 4, "external_line": "2" },
                    ]
                })
                .to_string(),
            ),
    )
    .await;
    assert_eq!(status, 200, "{imported}");

    let (db, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.expect("connect");
    tokio::spawn(async move { let _ = connection.await; });
    let rows = db
        .query(
            "SELECT f.id, fl.id, i.code FROM fulfilment_line fl
               JOIN fulfilment f ON f.id = fl.fulfilment_id
               JOIN order_line ol ON ol.id = fl.order_line_id
               JOIN item i ON i.id = ol.item_id
              WHERE f.external_id = $1",
            &[&if_id],
        )
        .await
        .expect("the fulfilment");
    let fulfilment: Uuid = rows[0].get(0);
    let line_of = |code: &str| -> Uuid {
        rows.iter().find(|r| r.get::<_, String>(2) == code).expect("the line").get(1)
    };
    let (checked, unchecked) = (line_of(&checked_code), line_of(&unchecked_code));

    let bench = || {
        test::TestRequest::get()
            .uri(&format!("/fulfilments/{fulfilment}/bench"))
            .insert_header(auth.clone())
    };
    let line = |screen: &Value, id: Uuid| -> Value {
        screen["lines"].as_array().unwrap().iter().find(|l| l["line_id"] == id.to_string()).cloned().expect("the line")
    };

    let (status, before) = call(&app, bench()).await;
    assert_eq!(status, 200, "{before}");
    assert!(line(&before, checked)["check"].is_null(), "nothing checked yet: {before}");
    assert_eq!(line(&before, checked)["unit"]["level"], "each", "the unit the line counts: {before}");

    // ── one line checked, twice by the same act ──────────────────────────
    let act = Uuid::now_v7();
    let check = || {
        test::TestRequest::post().uri("/pack-checks").insert_header(auth.clone()).set_json(json!({
            "fulfilment_line_id": checked,
            "expected_quantity": 12,
            "seen_quantity": 11,
            "client_event_id": act,
            "occurred_at": "2026-10-12T09:05:00Z"
        }))
    };
    let (status, body) = call(&app, check()).await;
    assert_eq!(status, 204, "{body}");
    let (status, body) = call(&app, check()).await;
    assert_eq!(status, 204, "sent again, the same act: {body}");
    let kept: i64 = db
        .query_one("SELECT count(*) FROM pack_check WHERE fulfilment_line_id = $1", &[&checked])
        .await
        .unwrap()
        .get(0);
    assert_eq!(kept, 1, "one check, however often it is sent");

    let (_, after) = call(&app, bench()).await;
    let said = &line(&after, checked)["check"];
    assert_eq!(said["expected"], 12, "{after}");
    assert_eq!(said["seen"], 11, "short one, as the packer saw it: {after}");
    assert!(said["by"].is_string(), "who checked it: {after}");
    assert!(line(&after, unchecked)["check"].is_null(), "{after}");

    let (status, refused) = call(
        &app,
        test::TestRequest::post().uri("/pack-checks").insert_header(auth.clone()).set_json(json!({
            "fulfilment_line_id": checked,
            "expected_quantity": 12,
            "seen_quantity": -1,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-10-12T09:06:00Z"
        })),
    )
    .await;
    assert_eq!(status, 400, "a count below nought: {refused}");

    // ── sealed with the other line unchecked: recorded, and said ─────────
    let carton = Uuid::now_v7();
    let (status, made) = call(
        &app,
        test::TestRequest::post().uri("/packages").insert_header(auth.clone()).set_json(json!({
            "id": carton,
            "fulfilment_id": fulfilment,
            "package_type_id": SMALL_BOX,
            "location_id": PACK,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-10-12T09:10:00Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "{made}");
    for (id, quantity) in [(checked, 11), (unchecked, 4)] {
        let (status, handed) = call(
            &app,
            test::TestRequest::post().uri("/handovers").insert_header(auth.clone()).set_json(json!({
                "fulfilment_line_id": id,
                "quantity": quantity,
                "to_package_id": carton,
                "client_event_id": Uuid::now_v7(),
                "occurred_at": "2026-10-12T09:10:01Z"
            })),
        )
        .await;
        assert_eq!(status, 200, "{handed}");
    }
    let (status, sealed) = call(
        &app,
        test::TestRequest::post()
            .uri(&format!("/packages/{carton}/seal"))
            .insert_header(auth.clone())
            .set_json(json!({ "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-12T09:10:02Z" })),
    )
    .await;
    assert_eq!(status, 200, "a seal is a seal (D5): {sealed}");
    let warnings = sealed["warnings"].to_string();
    assert!(warnings.contains("not checked at the bench"), "{sealed}");
    assert!(warnings.contains(&unchecked_code), "names the line: {sealed}");
    assert!(!warnings.contains(&checked_code), "not the line that was checked: {sealed}");

    // ── how far a weight may be off is the site's to say ─────────────────
    let (_, ws) = call(&app, test::TestRequest::get().uri("/workspace").insert_header(auth.clone())).await;
    let site = ws["sites"].as_array().unwrap().iter().find(|s| s["current"] == true).cloned().expect("the caller's site");
    let set = |per_mille: i64| {
        test::TestRequest::post()
            .uri(&format!("/workspace/sites/{}/weight-tolerance", site["id"].as_str().unwrap()))
            .insert_header(auth.clone())
            .set_json(json!({ "per_mille": per_mille }))
    };
    let was = site["weight_tolerance_per_mille"].as_i64().unwrap();
    let (status, body) = call(&app, set(50)).await;
    assert_eq!(status, 204, "{body}");
    let (_, ws) = call(&app, test::TestRequest::get().uri("/workspace").insert_header(auth.clone())).await;
    let now = ws["sites"].as_array().unwrap().iter().find(|s| s["current"] == true).unwrap()["weight_tolerance_per_mille"].clone();
    assert_eq!(now, 50, "five per cent: {ws}");
    let (status, refused) = call(&app, set(0)).await;
    assert_eq!(status, 400, "nought is no tolerance: {refused}");
    let (status, _) = call(&app, set(was)).await;
    assert_eq!(status, 204, "put back as it was");
}
