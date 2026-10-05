//! A round thing, measured across (D213), over HTTP.
//!
//! A bucket says it is round from what it is packed in. It is measured across
//! its top and its base, its whole height and the straight part at its top,
//! and the box it fits in goes up beside them, so packing reads it as before.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

#[actix_web::test]
async fn a_bucket_is_measured_across_and_fits_a_box() {
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
    let n = Uuid::new_v4().simple().to_string()[..10].to_string();
    let item: Uuid = db
        .query_one(
            "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
             SELECT current_tenant(), $1, 'Floor sealer, 20 L', u.id, 'none'
               FROM unit u WHERE u.code = 'ea'
             RETURNING id",
            &[&format!("SEAL-{n}")],
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
    let each = || async {
        let (status, page) = call(test::TestRequest::get().uri(&format!("/items/{item}"))).await;
        assert_eq!(status, 200, "{page}");
        page["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["packaging_level"] == "each" && s["lot_id"].is_null())
            .unwrap_or_else(|| panic!("an each card: {page}"))
            .clone()
    };
    let now = "2026-10-05T00:00:00Z";

    // ── GS1's round types say so; a box and a bag don't ─────────────────
    let (status, types) = call(test::TestRequest::get().uri("/packaging-types")).await;
    assert_eq!(status, 200, "{types}");
    let round = |code: &str| {
        let t = types
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["code"] == code)
            .unwrap_or_else(|| panic!("{code} listed"));
        t["round"].as_bool().unwrap()
    };
    assert!(
        round("BJ") && round("CNG") && round("JR"),
        "a bucket, a can and a jar are round"
    );
    assert!(!round("BX") && !round("BG"), "a box and a bag are not");

    // ── nothing said: a box, not round ──────────────────────────────────
    let before = each().await;
    assert_eq!(
        (before["box_shaped"].as_bool(), before["round"].as_bool()),
        (Some(true), Some(false)),
        "{before}"
    );

    // ── packed in a bucket ──────────────────────────────────────────────
    let (status, said) = call(test::TestRequest::post().uri("/packaging").set_json(json!({
        "item_id": item, "packaging_level": "each", "packaging_type": "BJ",
        "client_event_id": Uuid::new_v4(), "occurred_at": now,
    })))
    .await;
    assert_eq!(status, 204, "{said}");
    let bucket = each().await;
    assert_eq!(
        (bucket["box_shaped"].as_bool(), bucket["round"].as_bool()),
        (Some(false), Some(true)),
        "{bucket}"
    );

    // ── measured across, with the box it fits in beside ─────────────────
    let (status, said) = call(
        test::TestRequest::post()
            .uri("/observations")
            .set_json(json!({
                "item_id": item, "packaging_level": "each",
                "measurements": [
                    { "metric": "gross_weight",  "entered_value": "21.4", "unit": "kg" },
                    { "metric": "diameter",      "entered_value": "30",   "unit": "cm" },
                    { "metric": "base_diameter", "entered_value": "26.5", "unit": "cm" },
                    { "metric": "height",        "entered_value": "38",   "unit": "cm" },
                    { "metric": "top_height",    "entered_value": "7.5",  "unit": "cm" },
                    { "metric": "length",        "entered_value": "30",   "unit": "cm" },
                    { "metric": "width",         "entered_value": "30",   "unit": "cm" },
                ],
                "presentation": "as_supplied",
                "method": "instrument",
                "client_event_id": Uuid::new_v4(), "occurred_at": now,
            })),
    )
    .await;
    assert_eq!(status, 200, "{said}");
    // The scheduler folds what was observed into the current figures (D107).
    db.execute(
        "SELECT projection_observation_current_rebuild(current_tenant())",
        &[],
    )
    .await
    .expect("the fold");
    let measured = each().await;
    let mm = |field: &str| measured[field].as_i64();
    assert_eq!(
        (
            mm("diameter_mm"),
            mm("base_diameter_mm"),
            mm("top_height_mm")
        ),
        (Some(300), Some(265), Some(75)),
        "{measured}"
    );
    assert_eq!(
        (mm("length_mm"), mm("width_mm"), mm("height_mm")),
        (Some(300), Some(300), Some(380)),
        "the box it fits in"
    );
    assert_eq!(mm("gross_weight_g"), Some(21400));
    assert!(
        !measured["wants"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "weight" || w == "dimensions"),
        "a bucket measured across is measured: {measured}"
    );

    // ── a box is never round, whatever is said of its size ──────────────
    let (status, _) = call(test::TestRequest::post().uri("/packaging").set_json(json!({
        "item_id": item, "packaging_level": "each", "packaging_type": "BX",
        "client_event_id": Uuid::new_v4(), "occurred_at": now,
    })))
    .await;
    assert_eq!(status, 204);
    let boxed = each().await;
    assert_eq!(
        (boxed["box_shaped"].as_bool(), boxed["round"].as_bool()),
        (Some(true), Some(false)),
        "{boxed}"
    );
}
