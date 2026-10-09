//! A round thing wrapped in its photographs (D240), over HTTP.
//!
//! A bucket with two sides photographed waits to be wrapped; the wrapping a
//! computer made is kept against it, its card carries it, and it waits no
//! longer.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

#[actix_web::test]
async fn a_bucket_photographed_round_waits_to_be_wrapped_until_it_is() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let db = state.pool.get().await.expect("a connection");
    db.execute("SELECT set_config('spork.tenant_id', $1, false)", &[&TENANT])
        .await
        .expect("the tenant scope");
    let n = Uuid::new_v4().simple().to_string()[..10].to_string();
    let item: Uuid = db
        .query_one(
            "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
             SELECT current_tenant(), $1, 'Floor sealer, 15 L bucket', u.id, 'none'
               FROM unit u WHERE u.code = 'ea'
             RETURNING id",
            &[&format!("PAIL-{n}")],
        )
        .await
        .expect("the item")
        .get(0);

    let app = test::init_service(App::new().app_data(state.clone()).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);
    let call = |req: test::TestRequest| {
        let auth = auth.clone();
        let app = &app;
        async move {
            let r = test::call_service(app, req.insert_header(auth).to_request()).await;
            let status = r.status().as_u16();
            let body = test::read_body(r).await;
            (status, serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null))
        }
    };
    let now = "2026-10-09T00:00:00Z";
    let waiting = || async {
        let (status, queue) = call(test::TestRequest::get().uri("/photos/unwrapped")).await;
        assert_eq!(status, 200, "{queue}");
        queue.as_array().unwrap().iter().any(|q| q["item_id"] == json!(item))
    };

    // A bucket, its front and right photographed.
    let (status, said) = call(test::TestRequest::post().uri("/packaging").set_json(json!({
        "item_id": item, "packaging_level": "each", "packaging_type": "BJ",
        "client_event_id": Uuid::new_v4(), "occurred_at": now,
    })))
    .await;
    assert_eq!(status, 204, "{said}");
    let (status, look) = call(test::TestRequest::post().uri("/observations").set_json(json!({
        "item_id": item, "packaging_level": "each", "measurements": [], "photographs": true,
        "method": "photographed", "ingestion_channel": "keyed",
        "client_event_id": Uuid::new_v4(), "occurred_at": now,
    })))
    .await;
    assert_eq!(status, 200, "{look}");
    let look_id = look["observation_event_id"].as_str().unwrap().to_string();
    let mut sides = vec![];
    for face in ["front", "right"] {
        let (status, photo) = call(
            test::TestRequest::post()
                .uri(&format!("/observations/{look_id}/images/{face}"))
                .insert_header(("content-type", "image/png"))
                .set_payload(common::png(1200, 1600)),
        )
        .await;
        assert_eq!(status, 200, "{photo}");
        sides.push(photo["image_id"].clone());
        if face == "front" {
            assert!(!waiting().await, "one side is not enough to wrap");
        }
    }
    assert!(waiting().await, "two sides wait to be wrapped");

    // The computer's wrapping, kept.
    let (status, side) = call(
        test::TestRequest::post().uri("/images").insert_header(("content-type", "image/png")).set_payload(common::png(1024, 360)),
    )
    .await;
    assert_eq!(status, 200, "{side}");
    let wrap = json!({
        "item_id": item, "packaging_level": "each", "side": side["digest"], "made_from": sides,
        "client_event_id": Uuid::new_v4(), "occurred_at": now,
    });
    let (status, said) = call(test::TestRequest::post().uri("/round-wraps").set_json(wrap.clone())).await;
    assert_eq!(status, 204, "{said}");
    let (status, _) = call(test::TestRequest::post().uri("/round-wraps").set_json(wrap)).await;
    assert_eq!(status, 204, "saying it again is the same act");
    assert!(!waiting().await, "wrapped, it waits no longer");
    let served = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/images/{}", side["digest"].as_str().unwrap()))
            .insert_header(auth.clone())
            .to_request(),
    )
    .await;
    assert_eq!(served.status().as_u16(), 200, "its pictures are served, as its row says they are its");

    let (_, page) = call(test::TestRequest::get().uri(&format!("/items/{item}"))).await;
    let each = page["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["packaging_level"] == "each" && s["lot_id"].is_null())
        .unwrap_or_else(|| panic!("an each card: {page}"));
    assert_eq!(each["wrap"]["side"], side["digest"], "{each}");
    assert_eq!(each["packed_in_name"], "Bucket");

    // Refused, in words.
    let (status, _) = call(test::TestRequest::post().uri("/round-wraps").set_json(json!({
        "item_id": item, "packaging_level": "each", "side": "0".repeat(64), "made_from": sides,
        "client_event_id": Uuid::new_v4(), "occurred_at": now,
    })))
    .await;
    assert_eq!(status, 400, "nothing kept at that address");
}
