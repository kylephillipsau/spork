//! Figures and photos recorded on the wrong card, filed on the right one
//! (D219), over HTTP.
//!
//! A box of ten respirators weighed, measured, photographed and cut on its
//! item's carton card. Once the item says it is sold by the box, the carton's
//! card is moved to the box: the box has the figures and the photo with its
//! cut, the carton has nothing, nothing was rewritten, and moving again, or
//! onto a card with records of its own, is refused.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

#[actix_web::test]
async fn a_box_measured_on_the_carton_card_is_moved_to_the_box() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let db = state.pool.get().await.expect("a connection");
    db.execute(
        "SELECT set_config('spork.tenant_id', $1, false)",
        &[&TENANT],
    )
    .await
    .expect("the tenant scope");
    let n = Uuid::new_v4().simple().to_string()[..8].to_uppercase();
    let item: Uuid = db
        .query_one(
            "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
             SELECT current_tenant(), $1, 'P2 respirator with valve 10/box', u.id, 'none' FROM unit u WHERE u.code = 'ea'
             RETURNING id",
            &[&format!("P2R-{n}")],
        )
        .await
        .expect("the item")
        .get(0);

    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(routes::configure),
    )
    .await;
    let auth = ("authorization", common::bearer(&app).await);
    let call = |req: test::TestRequest| {
        let auth = auth.clone();
        let app = &app;
        async move {
            let r = test::call_service(app, req.insert_header(auth).to_request()).await;
            let status = r.status().as_u16();
            let body = test::read_body(r).await;
            (
                status,
                serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null),
            )
        }
    };
    let post = |uri: String, body: Value| call(test::TestRequest::post().uri(&uri).set_json(body));
    let now = "2026-10-05T04:00:00Z";
    let fold = || async {
        db.execute(
            "SELECT projection_observation_current_rebuild(current_tenant())",
            &[],
        )
        .await
        .expect("the fold");
    };
    let card = |page: &Value, level: &str| -> Value {
        page["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| {
                s["item_id"] == item.to_string()
                    && s["packaging_level"] == level
                    && s["lot_id"].is_null()
            })
            .unwrap_or_else(|| panic!("its {level} card: {page}"))
            .clone()
    };
    let page = || async {
        let (status, page) = call(test::TestRequest::get().uri(&format!("/items/{item}"))).await;
        assert_eq!(status, 200, "{page}");
        page
    };

    // ── the box of ten, recorded on the carton card ─────────────────────
    let (status, said) = post(
        format!("/items/{item}/carton"),
        json!({ "holds": 10, "client_event_id": Uuid::new_v4(), "occurred_at": now }),
    )
    .await;
    assert_eq!(status, 200, "{said}");
    let (status, look) = post(
        "/observations".into(),
        json!({
            "item_id": item, "packaging_level": "carton",
            "measurements": [
                { "metric": "gross_weight", "entered_value": "0.23", "unit": "kg" },
                { "metric": "length", "entered_value": "14", "unit": "cm" },
                { "metric": "width",  "entered_value": "13", "unit": "cm" },
                { "metric": "height", "entered_value": "16.5", "unit": "cm" },
            ],
            "photographs": true, "method": "instrument",
            "client_event_id": Uuid::new_v4(), "occurred_at": now,
        }),
    )
    .await;
    assert_eq!(status, 200, "{look}");
    let event = look["observation_event_id"].as_str().unwrap().to_string();
    let (status, photo) = call(
        test::TestRequest::post()
            .uri(&format!("/observations/{event}/images/front"))
            .insert_header(("content-type", "image/png"))
            .set_payload(common::png(1200, 900)),
    )
    .await;
    assert_eq!(status, 200, "{photo}");
    let photo_id = photo["image_id"].as_str().unwrap().to_string();
    let (status, cut) = post(
        format!("/observation-images/{photo_id}/cuts"),
        json!({ "digest": photo["digest"], "corners": [0.1, 0.1, 0.9, 0.1, 0.9, 0.9, 0.1, 0.9],
                "client_event_id": Uuid::new_v4(), "occurred_at": now }),
    )
    .await;
    assert_eq!(status, 200, "{cut}");
    fold().await;
    assert_eq!(card(&page().await, "carton")["gross_weight_g"], 230);

    // ── it is sold by the box: ten boxes of ten to a carton ─────────────
    let (status, _) = post(
        format!("/items/{item}/unit"),
        json!({ "level": "inner", "client_event_id": Uuid::new_v4(), "occurred_at": now }),
    )
    .await;
    assert_eq!(status, 204);
    let (status, said) = post(
        format!("/items/{item}/carton"),
        json!({ "holds": 10, "per": 10, "client_event_id": Uuid::new_v4(), "occurred_at": now }),
    )
    .await;
    assert_eq!(status, 200, "{said}");

    // ── the carton's card, moved to the box ─────────────────────────────
    let act = Uuid::new_v4();
    let refile = |from: &str, to: &str, act: Uuid| {
        post(
            format!("/items/{item}/refile"),
            json!({ "from": from, "to": to, "client_event_id": act, "occurred_at": now }),
        )
    };
    let (status, moved) = refile("carton", "inner", act).await;
    assert_eq!(status, 200, "{moved}");
    assert_eq!(
        (moved["figures"].as_i64(), moved["photos"].as_i64()),
        (Some(4), Some(1)),
        "{moved}"
    );
    let (status, again) = refile("carton", "inner", act).await;
    assert_eq!(status, 200);
    assert_eq!(
        again["replay"], true,
        "a retry is the same act, and does nothing twice"
    );
    fold().await;

    let p = page().await;
    let the_box = card(&p, "inner");
    assert_eq!(
        (
            the_box["gross_weight_g"].as_i64(),
            the_box["length_mm"].as_i64(),
            the_box["width_mm"].as_i64(),
            the_box["height_mm"].as_i64()
        ),
        (Some(230), Some(140), Some(130), Some(165)),
        "the box has its figures: {the_box}"
    );
    assert_eq!(
        the_box["method"], "instrument",
        "still how they were come by"
    );
    assert_eq!(the_box["is_unit"], true);
    let carton = card(&p, "carton");
    assert!(
        carton["gross_weight_g"].is_null() && carton["length_mm"].is_null(),
        "the carton has none: {carton}"
    );
    assert!(
        carton["faces"].as_array().unwrap().is_empty(),
        "nor a photo: {carton}"
    );
    let photos: Vec<&Value> = p["photos"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["face"] == "front")
        .collect();
    assert_eq!(photos.len(), 1, "one front, on the box: {p}");
    assert_eq!(photos[0]["packaging_level"], "inner");
    assert!(photos[0]["cut"].is_object(), "with its cut: {}", photos[0]);
    let (_, queue) = call(test::TestRequest::get().uri("/photos/uncut")).await;
    assert!(
        queue
            .as_array()
            .unwrap()
            .iter()
            .all(|q| q["item_id"] != item.to_string()),
        "a cut photo moved is not waiting to be cut again: {queue}"
    );

    // ── nothing rewritten: retracted where it was, derived where it is ──
    let r = db
        .query_one(
            "SELECT
               (SELECT count(*) FROM observation o JOIN observable s ON s.id = o.observable_id
                 WHERE s.item_id = $1 AND s.packaging_level = 'carton' AND o.retracts_observation_id IS NOT NULL),
               (SELECT count(*) FROM observation_event e JOIN observable s ON s.id = e.observable_id
                 WHERE s.item_id = $1 AND s.packaging_level = 'inner' AND e.derived_from_event_id IS NOT NULL),
               (SELECT count(*) FROM observation_image_move m JOIN observation_image i ON i.id = m.observation_image_id
                 WHERE i.id = $2::text::uuid)",
            &[&item, &photo_id],
        )
        .await
        .expect("the history");
    assert_eq!(
        (r.get::<_, i64>(0), r.get::<_, i64>(1), r.get::<_, i64>(2)),
        (4, 1, 1),
        "four figures retracted, one event derived, the photo marked moved"
    );

    // ── refused, in words ───────────────────────────────────────────────
    let (status, said) = refile("carton", "inner", Uuid::new_v4()).await;
    assert_eq!(status, 400, "nothing left on the carton card: {said}");
    let (status, said) = post(
        "/observations".into(),
        json!({
            "item_id": item, "packaging_level": "each",
            "measurements": [{ "metric": "gross_weight", "entered_value": "0.02", "unit": "kg" }],
            "method": "instrument", "client_event_id": Uuid::new_v4(), "occurred_at": now,
        }),
    )
    .await;
    assert_eq!(status, 200, "{said}");
    let (status, said) = refile("inner", "each", Uuid::new_v4()).await;
    assert_eq!(
        status, 400,
        "a card with its own figures is not filled from another: {said}"
    );
    let (status, _) = refile("inner", "inner", Uuid::new_v4()).await;
    assert_eq!(status, 400);
    let (status, _) = refile("inner", "pallet", Uuid::new_v4()).await;
    assert_eq!(status, 400);

    // ── to another item's card (D222): a kit's part measured on the kit ─
    let part: Uuid = db
        .query_one(
            "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
             SELECT current_tenant(), $1, 'Valve for a P2 respirator', u.id, 'none' FROM unit u WHERE u.code = 'ea'
             RETURNING id",
            &[&format!("P2V-{n}")],
        )
        .await
        .expect("the other item")
        .get(0);
    let elsewhere = |to_item: Uuid, act: Uuid| {
        post(
            format!("/items/{item}/refile"),
            json!({ "from": "inner", "to": "each", "to_item": to_item, "client_event_id": act, "occurred_at": now }),
        )
    };
    let (status, moved) = elsewhere(part, Uuid::new_v4()).await;
    assert_eq!(status, 200, "{moved}");
    assert_eq!((moved["figures"].as_i64(), moved["photos"].as_i64()), (Some(4), Some(1)), "{moved}");
    fold().await;
    let (_, there) = call(test::TestRequest::get().uri(&format!("/items/{part}"))).await;
    let each = there["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["item_id"] == part.to_string() && s["packaging_level"] == "each" && s["lot_id"].is_null())
        .unwrap_or_else(|| panic!("its each card: {there}"))
        .clone();
    assert_eq!(each["gross_weight_g"], 230, "the figures are the other item's now: {each}");
    let unstated: i64 = db
        .query_one(
            "SELECT count(*) FROM observation_event e JOIN observable s ON s.id = e.observable_id
              WHERE s.item_id = $1 AND s.packaging_level = 'each' AND e.presentation_id IS NULL",
            &[&part],
        )
        .await
        .expect("its events")
        .get(0);
    assert_eq!(unstated, 0, "a box's size on a single thing is as supplied (J72)");
    assert!(card(&page().await, "inner")["gross_weight_g"].is_null(), "and gone from the box");
    let (status, said) = elsewhere(part, Uuid::new_v4()).await;
    assert_eq!(status, 400, "nothing left to move: {said}");
    let (status, said) = elsewhere(Uuid::new_v4(), Uuid::new_v4()).await;
    assert_eq!(status, 400, "an item that isn't in the catalogue: {said}");
}
