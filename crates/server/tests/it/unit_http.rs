//! Which level of an item is one in NetSuite (D218), over HTTP.
//!
//! Gloves sold by the carton of 1,000, earplugs by the box of 100 in cartons
//! of ten boxes, and a catalogue NetSuite says nothing about. Each item's page
//! leads with what it is sold as; a carton nobody said and a single product
//! inside a box are offered, not there, until somebody records them; and the
//! unit can be said in Spork over NetSuite's word.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url, SITE};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

#[actix_web::test]
async fn an_item_leads_with_what_netsuite_counts_one_of() {
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
    let site = Uuid::parse_str(SITE).unwrap();
    let n = Uuid::new_v4().simple().to_string()[..8].to_uppercase();
    let made = |code: String,
                description: &'static str,
                unit: Option<&'static str>,
                case: Option<(i32, i32)>| {
        let db = &db;
        async move {
            let id: Uuid = db
                .query_one(
                    "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
                     SELECT current_tenant(), $1, $2, u.id, 'none' FROM unit u WHERE u.code = 'ea'
                     RETURNING id",
                    &[&code, &description],
                )
                .await
                .expect("the item")
                .get(0);
            if let Some(unit) = unit {
                db.execute(
                    "INSERT INTO reported_item (tenant_id, item_id, selling_unit, as_at, source)
                     VALUES (current_tenant(), $1, $2, now(), 'netsuite-item-details')",
                    &[&id, &unit],
                )
                .await
                .expect("NetSuite's unit");
            }
            if let Some((per, inners)) = case {
                db.execute(
                    "INSERT INTO item_packing_config (tenant_id, item_id, units_per_inner, inners_per_carton, effective_from)
                     VALUES (current_tenant(), $1, $2, $3, DATE '2026-10-01')",
                    &[&id, &per, &inners],
                )
                .await
                .expect("the case pack");
            }
            id
        }
    };
    let gloves = made(
        format!("GLV-{n}"),
        "Nitrile gloves, ctn 1000",
        Some("CTN"),
        Some((1, 1000)),
    )
    .await;
    let plugs = made(
        format!("PLG-{n}"),
        "Earplugs, box 100",
        Some("Box"),
        Some((100, 10)),
    )
    .await;
    let book = made(format!("CAT-{n}"), "Catalogue", None, None).await;
    let _ = site;

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
    let page = |id: Uuid| async move {
        let (status, page) = call(test::TestRequest::get().uri(&format!("/items/{id}"))).await;
        assert_eq!(status, 200, "{page}");
        page
    };
    // Each subject as (level, is the unit, only offered, what it wants).
    let cards = |page: &Value| -> Vec<(String, bool, bool, usize)> {
        page["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["lot_id"].is_null() && s["item_part_id"].is_null())
            .map(|s| {
                (
                    s["packaging_level"].as_str().unwrap().to_string(),
                    s["is_unit"].as_bool().unwrap(),
                    s["offered"].as_bool().unwrap(),
                    s["wants"].as_array().unwrap().len(),
                )
            })
            .collect()
    };
    let card =
        |l: &str, unit: bool, offered: bool, wants: usize| (l.to_string(), unit, offered, wants);

    // ── gloves, sold by the carton: the carton leads; a single glove is offered
    let p = page(gloves).await;
    assert_eq!(
        (p["unit"]["level"].as_str(), p["unit"]["said"].as_bool()),
        (Some("carton"), Some(false))
    );
    assert_eq!(p["unit"]["netsuite_unit"], "CTN");
    assert_eq!(
        cards(&p),
        vec![card("carton", true, false, 3), card("inner", false, true, 0), card("each", false, true, 0)],
        "a pack is offered as the carton is (D234): {p}"
    );

    // ── earplugs, sold by the box: the box, its carton of ten, a pair offered
    let p = page(plugs).await;
    assert_eq!(p["unit"]["level"], "inner");
    assert_eq!(
        cards(&p),
        vec![
            card("inner", true, false, 3),
            card("carton", false, false, 3),
            card("each", false, true, 0)
        ],
        "{p}"
    );

    // ── a catalogue NetSuite says nothing of: the each; a carton offered ───
    let p = page(book).await;
    assert_eq!(p["unit"]["level"], "each");
    assert_eq!(
        cards(&p),
        vec![card("each", true, false, 3), card("inner", false, true, 0), card("carton", false, true, 0)],
        "{p}"
    );

    // ── a single pair measured is there, not offered ───────────────────────
    let (status, said) = call(test::TestRequest::post().uri("/observations").set_json(json!({
        "item_id": plugs, "packaging_level": "each",
        "measurements": [{ "metric": "gross_weight", "entered_value": "0.004", "unit": "kg" }],
        "method": "instrument", "client_event_id": Uuid::new_v4(), "occurred_at": "2026-10-05T03:00:00Z",
    })))
    .await;
    assert_eq!(status, 200, "{said}");
    db.execute(
        "SELECT projection_observation_current_rebuild(current_tenant())",
        &[],
    )
    .await
    .expect("the fold");
    let p = page(plugs).await;
    assert_eq!(
        cards(&p).last(),
        Some(&card("each", false, false, 2)),
        "measured, it is a card of its own: {p}"
    );

    // ── said in Spork over NetSuite's word ─────────────────────────────────
    let act = Uuid::new_v4();
    let say =
        |level: &'static str, act: Uuid| {
            call(test::TestRequest::post().uri(&format!("/items/{book}/unit")).set_json(json!({
            "level": level, "client_event_id": act, "occurred_at": "2026-10-05T03:00:00Z",
        })))
        };
    let (status, _) = say("carton", act).await;
    assert_eq!(status, 204);
    let (status, _) = say("carton", act).await;
    assert_eq!(status, 204, "said again, the same act");
    let p = page(book).await;
    assert_eq!(
        (p["unit"]["level"].as_str(), p["unit"]["said"].as_bool()),
        (Some("carton"), Some(true))
    );
    assert_eq!(
        cards(&p)[0],
        card("carton", true, false, 3),
        "the carton is the catalogue now: {p}"
    );
    let (status, _) = say("pallet", Uuid::new_v4()).await;
    assert_eq!(
        status, 400,
        "an item is sold as its each, its pack or its carton"
    );

    // ── what still needs weighing is the unit (D218) ──────────────────────
    let (status, said) = call(test::TestRequest::post().uri("/observations").set_json(json!({
        "item_id": gloves, "packaging_level": "carton",
        "measurements": [{ "metric": "gross_weight", "entered_value": "6.6", "unit": "kg" }],
        "method": "instrument", "client_event_id": Uuid::new_v4(), "occurred_at": "2026-10-05T03:00:00Z",
    })))
    .await;
    assert_eq!(status, 200, "{said}");
    db.execute(
        "SELECT projection_observation_current_rebuild(current_tenant())",
        &[],
    )
    .await
    .expect("the fold");
    let (status, listed) =
        call(test::TestRequest::get().uri(&format!("/items?needs=weighing&q={n}"))).await;
    assert_eq!(status, 200, "{listed}");
    let codes: Vec<&str> = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["code"].as_str().unwrap())
        .collect();
    assert!(
        !codes.contains(&format!("GLV-{n}").as_str()),
        "its carton weighed, the glove is: {codes:?}"
    );
    assert!(
        codes.contains(&format!("PLG-{n}").as_str()),
        "a single pair weighed is not the box: {codes:?}"
    );
    let (_, listed) =
        call(test::TestRequest::get().uri(&format!("/items?has=measured&q={n}"))).await;
    let codes: Vec<&str> = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["code"].as_str().unwrap())
        .collect();
    assert_eq!(
        codes,
        vec![format!("GLV-{n}").as_str()],
        "measured is the unit measured"
    );

    // ── what the export calls one ──────────────────────────────────────────
    let (status, rows) =
        call(test::TestRequest::get().uri(&format!("/items/export?format=json&q={n}"))).await;
    assert_eq!(status, 200, "{rows}");
    let level = |code: &str| {
        rows.as_array()
            .unwrap()
            .iter()
            .find(|r| r["code"] == code)
            .map(|r| r["unit_level"].clone())
            .unwrap()
    };
    assert_eq!(level(&format!("GLV-{n}")), "carton");
    assert_eq!(level(&format!("PLG-{n}")), "inner");
    assert_eq!(level(&format!("CAT-{n}")), "carton");

    // ── a pair is two single ones (D233) ───────────────────────────────────
    let boots = made(format!("BTS-{n}"), "Gumboots", Some("Pair"), None).await;
    let bagged = made(format!("WKG-{n}"), "Work gloves, bags of 12 pairs", Some("Pair"), Some((24, 10))).await;
    let glasses = made(format!("GLS-{n}"), "Safety glasses", Some("Pair"), Some((1, 12))).await;
    let unit = |p: &Value| (p["unit"]["level"].as_str().map(str::to_string), p["unit"]["singles"].as_i64());
    let p = page(boots).await;
    assert_eq!(unit(&p), (Some("inner".into()), Some(2)), "a pair packed as one is its pack: {p}");
    assert_eq!(cards(&p)[0].0, "inner", "and leads: {p}");
    assert_eq!(unit(&page(bagged).await), (Some("each".into()), Some(2)), "in bags of twelve, two of the each");
    let (status, _) = call(test::TestRequest::post().uri(&format!("/items/{glasses}/unit")).set_json(json!({
        "level": "each", "client_event_id": Uuid::new_v4(), "occurred_at": "2026-10-08T03:00:00Z",
    })))
    .await;
    assert_eq!(status, 204);
    assert_eq!(unit(&page(glasses).await), (Some("each".into()), Some(1)), "a pair of glasses is one thing, said so");
    let (status, _) = call(test::TestRequest::post().uri(&format!("/items/{glasses}/unit")).set_json(json!({
        "level": "carton", "quantity": 2, "client_event_id": Uuid::new_v4(), "occurred_at": "2026-10-08T03:00:00Z",
    })))
    .await;
    assert_eq!(status, 400, "two of a carton is not a pair");
}
