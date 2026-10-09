//! What a subject is packed in (D191), over HTTP.
//!
//! A GS1 packaging type said of the each, the carton or a family's carton. A
//! type without six sides is not cut to faces and not drawn, unless it is
//! said to be box-shaped (D239); a carton with nothing said of its own is
//! packed as its family's.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

#[actix_web::test]
async fn a_thing_packed_without_six_sides_is_photographed_not_drawn() {
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
    let style: Uuid = db
        .query_one(
            "INSERT INTO item_style (tenant_id, code, description) VALUES (current_tenant(), $1, 'Scrub brush family')
             RETURNING id",
            &[&format!("FAM-{n}")],
        )
        .await
        .expect("the family")
        .get(0);
    let item: Uuid = db
        .query_one(
            "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking, style_id)
             SELECT current_tenant(), $1, 'Scrub brush, 200 mm', u.id, 'none', $2
               FROM unit u WHERE u.code = 'ea'
             RETURNING id",
            &[&format!("BRUSH-{n}"), &style],
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
    let say = |body: Value| call(test::TestRequest::post().uri("/packaging").set_json(body));
    let subject = |page: &Value, level: &str| -> Value {
        page["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["packaging_level"] == level && s["lot_id"].is_null())
            .unwrap_or_else(|| panic!("a {level} card: {page}"))
            .clone()
    };
    let page = || call(test::TestRequest::get().uri(&format!("/items/{item}")));

    // ── GS1's list, the common types first ──────────────────────────────
    let (status, types) = call(test::TestRequest::get().uri("/packaging-types")).await;
    assert_eq!(status, 200, "{types}");
    let types = types.as_array().unwrap();
    assert_eq!(types[0]["code"], "CS", "a case leads: {:?}", types[0]);
    let sw = types.iter().find(|t| t["code"] == "SW").expect("shrinkwrapped is on the list");
    assert_eq!(sw["six_sided"], false);

    // ── nothing said: a box, as before ──────────────────────────────────
    let (_, before) = page().await;
    let each = subject(&before, "each");
    assert!(each["packed_in"].is_null(), "{each}");
    assert_eq!(each["box_shaped"], true, "nothing said is a box");

    // ── the each is shrinkwrapped; the family's carton is a case ────────
    let now = "2026-10-02T00:00:00Z";
    let shrink = Uuid::new_v4();
    let body = json!({ "item_id": item, "packaging_level": "each", "packaging_type": "SW",
                       "client_event_id": shrink, "occurred_at": now });
    let (status, said) = say(body.clone()).await;
    assert_eq!(status, 204, "{said}");
    let (status, _) = say(body).await;
    assert_eq!(status, 204, "saying it again is the same act");
    let (status, _) = say(json!({ "item_style_id": style, "packaging_level": "carton", "packaging_type": "CS",
                                  "client_event_id": Uuid::new_v4(), "occurred_at": now }))
    .await;
    assert_eq!(status, 204);

    let (_, after) = page().await;
    let each = subject(&after, "each");
    assert_eq!((each["packed_in"].as_str(), each["packed_in_source"].as_str()), (Some("SW"), Some("own")), "{each}");
    assert_eq!(each["box_shaped"], false, "shrink-wrap has no six sides");
    let carton = subject(&after, "carton");
    assert_eq!(
        (carton["packed_in"].as_str(), carton["packed_in_source"].as_str()),
        (Some("CS"), Some("style")),
        "the item's carton is packed as its family's until it says: {carton}"
    );
    assert_eq!(carton["box_shaped"], true);

    // ── its photo is not cut to faces, and is not drawn ─────────────────
    let (status, look) = call(test::TestRequest::post().uri("/observations").set_json(json!({
        "item_id": item, "packaging_level": "each", "measurements": [], "photographs": true,
        "method": "photographed", "ingestion_channel": "keyed",
        "client_event_id": Uuid::new_v4(), "occurred_at": now,
    })))
    .await;
    assert_eq!(status, 200, "{look}");
    let look_id = look["observation_event_id"].as_str().unwrap().to_string();
    let (status, photo) = call(
        test::TestRequest::post()
            .uri(&format!("/observations/{look_id}/images/front"))
            .insert_header(("content-type", "image/png"))
            .set_payload(common::png(1200, 1600)),
    )
    .await;
    assert_eq!(status, 200, "{photo}");
    let image_id = photo["image_id"].as_str().unwrap().to_string();
    let (_, queue) = call(test::TestRequest::get().uri("/photos/uncut")).await;
    assert!(
        queue.as_array().unwrap().iter().all(|q| q["image_id"] != image_id.as_str()),
        "a shrinkwrapped thing has no faces to cut: {queue}"
    );

    // A cut said anyway (the whole photo), then a drawing made from it, is refused.
    let digest = photo["digest"].as_str().unwrap().to_string();
    let (status, cut) = call(test::TestRequest::post().uri(&format!("/observation-images/{image_id}/cuts")).set_json(json!({
        "digest": digest, "corners": [0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0],
        "client_event_id": Uuid::new_v4(), "occurred_at": now,
    })))
    .await;
    assert_eq!(status, 200, "{cut}");
    let (status, drawn) = call(
        test::TestRequest::post().uri("/images").insert_header(("content-type", "image/png")).set_payload(common::png(512, 512)),
    )
    .await;
    assert_eq!(status, 200, "{drawn}");
    let (status, refused) = call(test::TestRequest::post().uri(&format!("/items/{item}/box-picture")).set_json(json!({
        "digest": drawn["digest"], "made_from": [digest, digest, digest],
        "client_event_id": Uuid::new_v4(), "occurred_at": now,
    })))
    .await;
    assert_eq!(status, 400, "only a box is drawn: {refused}");

    // ── said to be box-shaped (D239): cut to its faces and drawn after all ──
    let (status, said) = call(test::TestRequest::post().uri("/shape").set_json(json!({
        "item_id": item, "packaging_level": "each", "box_shaped": true,
        "client_event_id": Uuid::new_v4(), "occurred_at": now,
    })))
    .await;
    assert_eq!(status, 204, "{said}");
    let (_, shaped) = page().await;
    let each = subject(&shaped, "each");
    assert_eq!((each["packed_in"].as_str(), each["box_shaped"].as_bool()), (Some("SW"), Some(true)), "{each}");
    let (status, right) = call(
        test::TestRequest::post()
            .uri(&format!("/observations/{look_id}/images/right"))
            .insert_header(("content-type", "image/png"))
            .set_payload(common::png(1200, 1600)),
    )
    .await;
    assert_eq!(status, 200, "{right}");
    let (_, queue) = call(test::TestRequest::get().uri("/photos/uncut")).await;
    assert!(
        queue.as_array().unwrap().iter().any(|q| q["image_id"] == right["image_id"]),
        "a box-shaped wrapping's sides are cut: {queue}"
    );
    let (status, drawing) = call(test::TestRequest::post().uri(&format!("/items/{item}/box-picture")).set_json(json!({
        "digest": drawn["digest"], "made_from": [digest, digest, digest],
        "client_event_id": Uuid::new_v4(), "occurred_at": now,
    })))
    .await;
    assert_eq!(status, 200, "and drawn: {drawing}");

    // ── refused, in words ───────────────────────────────────────────────
    let (status, _) = say(json!({ "item_id": item, "packaging_level": "each", "packaging_type": "ZZ",
                                  "client_event_id": Uuid::new_v4(), "occurred_at": now }))
    .await;
    assert_eq!(status, 400, "not a GS1 code");
    let (status, _) = say(json!({ "item_id": item, "packaging_type": "BX",
                                  "client_event_id": Uuid::new_v4(), "occurred_at": now }))
    .await;
    assert_eq!(status, 400, "an item needs its level");
    let (status, _) = say(json!({ "item_id": item, "item_style_id": style, "packaging_level": "each",
                                  "packaging_type": "BX", "client_event_id": Uuid::new_v4(), "occurred_at": now }))
    .await;
    assert_eq!(status, 400, "one thing at a time");
    let (status, _) = say(json!({ "item_id": Uuid::new_v4(), "packaging_level": "each", "packaging_type": "BX",
                                  "client_event_id": Uuid::new_v4(), "occurred_at": now }))
    .await;
    assert_eq!(status, 404, "no such item");
}
