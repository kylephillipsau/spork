//! An item matched from another of its family (D228), over HTTP.
//!
//! A brush in blue and in red: the same carton, the same size and weight, the
//! same sides but for the label. The blue's carton is weighed, measured and
//! photographed; the red's page lists the blue as family, with what is on its
//! carton. The red is matched from the blue: its carton has the blue's figures
//! and photos, with the blue's case pack, and the blue keeps its own. Its
//! label, photographed again, is the red's own. Matching again brings nothing
//! new, and an item outside the family, or a carton that holds another count,
//! is refused.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

#[actix_web::test]
async fn a_colour_is_matched_from_another_of_its_family() {
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
    let n = Uuid::new_v4().simple().to_string()[..8].to_uppercase();
    let style: Uuid = db
        .query_one(
            "INSERT INTO item_style (tenant_id, code, description) VALUES (current_tenant(), $1, 'Floor brush')
             RETURNING id",
            &[&format!("FB-{n}")],
        )
        .await
        .expect("the family")
        .get(0);
    let item = |code: String, family: Option<Uuid>| {
        let db = &db;
        async move {
            db.query_one(
                "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking, style_id)
                 SELECT current_tenant(), $1, 'Floor brush, 450 mm', u.id, 'none', $2 FROM unit u WHERE u.code = 'ea'
                 RETURNING id",
                &[&code, &family],
            )
            .await
            .expect("an item")
            .get::<_, Uuid>(0)
        }
    };
    let blue = item(format!("FB-{n}-B"), Some(style)).await;
    let red = item(format!("FB-{n}-R"), Some(style)).await;
    let green = item(format!("FB-{n}-G"), Some(style)).await;
    let stranger = item(format!("MOP-{n}"), None).await;

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
    let post = |uri: String, body: Value| call(test::TestRequest::post().uri(&uri).set_json(body));
    let then = "2026-10-05T04:00:00Z";
    let later = "2026-10-05T05:00:00Z";
    let fold = || async {
        db.execute("SELECT projection_observation_current_rebuild(current_tenant())", &[])
            .await
            .expect("the fold");
    };
    let page = |of: Uuid| async move {
        let (status, page) = call(test::TestRequest::get().uri(&format!("/items/{of}"))).await;
        assert_eq!(status, 200, "{page}");
        page
    };
    let carton = |page: &Value, of: Uuid| -> Value {
        page["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["item_id"] == of.to_string() && s["packaging_level"] == "carton" && s["lot_id"].is_null())
            .unwrap_or_else(|| panic!("its carton card: {page}"))
            .clone()
    };
    let photograph = |of: Uuid, at: &'static str, faces: &'static [&'static str]| {
        let post = &post;
        let call = &call;
        async move {
            let (status, look) = post(
                "/observations".into(),
                json!({
                    "item_id": of, "packaging_level": "carton", "measurements": [],
                    "photographs": true, "method": "instrument",
                    "client_event_id": Uuid::new_v4(), "occurred_at": at,
                }),
            )
            .await;
            assert_eq!(status, 200, "{look}");
            let event = look["observation_event_id"].as_str().unwrap().to_string();
            let mut ids = vec![];
            for face in faces {
                let (status, photo) = call(
                    test::TestRequest::post()
                        .uri(&format!("/observations/{event}/images/{face}"))
                        .insert_header(("content-type", "image/png"))
                        .set_payload(common::png(1200, 900)),
                )
                .await;
                assert_eq!(status, 200, "{photo}");
                ids.push(photo);
            }
            ids
        }
    };

    // ── the blue's carton: twelve to it, weighed, measured, photographed ─
    for (of, holds) in [(blue, 12), (green, 24)] {
        let (status, said) = post(
            format!("/items/{of}/carton"),
            json!({ "holds": holds, "client_event_id": Uuid::new_v4(), "occurred_at": then }),
        )
        .await;
        assert_eq!(status, 200, "{said}");
    }
    let (status, look) = post(
        "/observations".into(),
        json!({
            "item_id": blue, "packaging_level": "carton",
            "measurements": [
                { "metric": "gross_weight", "entered_value": "4.6", "unit": "kg" },
                { "metric": "length", "entered_value": "60", "unit": "cm" },
                { "metric": "width",  "entered_value": "30", "unit": "cm" },
                { "metric": "height", "entered_value": "25", "unit": "cm" },
            ],
            "method": "instrument", "client_event_id": Uuid::new_v4(), "occurred_at": then,
        }),
    )
    .await;
    assert_eq!(status, 200, "{look}");
    let shots = photograph(blue, then, &["front", "label"]).await;
    let (status, cut) = post(
        format!("/observation-images/{}/cuts", shots[0]["image_id"].as_str().unwrap()),
        json!({ "digest": shots[0]["digest"], "corners": [0.1, 0.1, 0.9, 0.1, 0.9, 0.9, 0.1, 0.9],
                "client_event_id": Uuid::new_v4(), "occurred_at": then }),
    )
    .await;
    assert_eq!(status, 200, "{cut}");
    fold().await;

    // ── the red's page lists its family, and what is on their cards ─────
    let p = page(red).await;
    let family = p["family"].as_array().unwrap();
    assert_eq!(family.len(), 2, "the blue and the green, not itself: {p}");
    let b = family.iter().find(|m| m["item_id"] == blue.to_string()).expect("the blue");
    assert_eq!(
        b["cards"],
        json!([{ "level": "carton", "weighed": true, "measured": true, "faces": 2 }]),
        "{b}"
    );
    assert!(p["packing"].is_null(), "the red has no case pack yet: {p}");

    // ── matched ─────────────────────────────────────────────────────────
    let act = Uuid::new_v4();
    let matched = |from: Uuid, levels: Value, act: Uuid| {
        post(
            format!("/items/{red}/match"),
            json!({ "from_item": from, "levels": levels, "client_event_id": act, "occurred_at": then }),
        )
    };
    let (status, done) = matched(blue, json!(["carton"]), act).await;
    assert_eq!(status, 200, "{done}");
    assert_eq!((done["figures"].as_i64(), done["photos"].as_i64()), (Some(4), Some(2)), "{done}");
    let (status, again) = matched(blue, json!(["carton"]), act).await;
    assert_eq!(status, 200);
    assert_eq!(again["replay"], true, "a retry is the same act");
    fold().await;

    let p = page(red).await;
    let theirs = page(blue).await;
    assert_eq!(p["packing"]["inners_per_carton"], theirs["packing"]["inners_per_carton"], "the blue's case pack: {p}");
    assert_eq!(p["packing"]["units_per_inner"], theirs["packing"]["units_per_inner"]);
    let mine = carton(&p, red);
    assert_eq!(
        (mine["gross_weight_g"].as_i64(), mine["length_mm"].as_i64(), mine["height_mm"].as_i64()),
        (Some(4600), Some(600), Some(250)),
        "the blue's figures: {mine}"
    );
    assert_eq!(mine["method"], "instrument", "still how they were come by");
    let front = |p: &Value, of: Uuid| -> Value {
        p["photos"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["item_id"] == of.to_string() && f["packaging_level"] == "carton" && f["face"] == "front")
            .cloned()
            .unwrap_or(Value::Null)
    };
    assert!(front(&p, red)["cut"].is_object(), "the front, with its cut: {p}");
    assert_eq!(carton(&theirs, blue)["gross_weight_g"], 4600, "the blue keeps its own");
    assert!(front(&theirs, blue)["cut"].is_object(), "and its photos");

    // ── matching again brings only what is new: nothing ─────────────────
    let (status, none) = matched(blue, json!(["carton"]), Uuid::new_v4()).await;
    assert_eq!(status, 200, "{none}");
    assert_eq!((none["figures"].as_i64(), none["photos"].as_i64()), (Some(0), Some(0)), "{none}");

    // ── the red's label, photographed again, is its own ─────────────────
    let retaken = photograph(red, later, &["label"]).await;
    let p = page(red).await;
    let label = p["photos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["item_id"] == red.to_string() && f["packaging_level"] == "carton" && f["face"] == "label")
        .cloned()
        .expect("a label");
    assert_eq!(label["image_id"], retaken[0]["image_id"], "the newest stands: {label}");

    // ── refused, in words ───────────────────────────────────────────────
    let (status, said) = matched(stranger, json!(["carton"]), Uuid::new_v4()).await;
    assert_eq!(status, 400, "not of its family: {said}");
    let (status, said) = matched(red, json!(["carton"]), Uuid::new_v4()).await;
    assert_eq!(status, 400, "itself: {said}");
    let (status, said) = matched(blue, json!([]), Uuid::new_v4()).await;
    assert_eq!(status, 400, "no card: {said}");
    let (status, said) = matched(blue, json!(["pallet"]), Uuid::new_v4()).await;
    assert_eq!(status, 400, "no such card: {said}");
    let (status, said) = matched(blue, json!(["each"]), Uuid::new_v4()).await;
    assert_eq!(status, 400, "nothing on the blue's each: {said}");
    let (status, said) = post(
        format!("/items/{green}/match"),
        json!({ "from_item": blue, "levels": ["carton"], "client_event_id": Uuid::new_v4(), "occurred_at": then }),
    )
    .await;
    assert_eq!(status, 400, "a carton of 24 is not a carton of 12: {said}");
}
