//! What a box carries, and a carrier's rules, over HTTP (D249, D250,
//! migrations 145 and 146).
//!
//! A set of build rules is started, saved as a new version and refused when a
//! rule is missing. A carrier points at sets and is made the usual one; a
//! pallet made with no carrier said is built for the usual one, to the rules
//! its kind of pallet takes as they stand, and keeps them when the set
//! changes. A box is refused a carrier. A carton is sealed hollow. A box's
//! board and limits are said, and the warehouse's conditions move its
//! estimate.
//!
//! The usual carrier and the conditions are the workspace's, so both are put
//! back as they were at the end. Names are random, so this runs against a
//! database it has already written to.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

/// PACK-1, where the fixture's Melbourne packs (migration 97).
const PACK: &str = "10c00000-0000-0000-0000-000000000004";
/// The fixture's small box preset, for the workspace it belongs to.
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

/// A rule of a set, by its kind.
fn rule<'a>(set: &'a Value, kind: &str) -> &'a Value {
    set["rules"].as_array().unwrap().iter().find(|r| r["kind"] == kind).unwrap_or_else(|| panic!("{kind} in {set}"))
}

#[actix_web::test]
async fn a_pallet_is_built_to_its_carriers_rules_as_they_stood() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let session = common::bearer(&app).await;
    let auth = ("authorization", session.clone());
    let post = |uri: String, body: Value| test::TestRequest::post().uri(&uri).insert_header(auth.clone()).set_json(body);
    let get = |uri: String| test::TestRequest::get().uri(&uri).insert_header(auth.clone());
    let run = &Uuid::new_v4().simple().to_string()[..8];

    // ── a set started from the rules to start, and saved ─────────────────
    let (status, made) = call(&app, post("/build-rules".into(), json!({ "name": format!("Order pallet {run}") }))).await;
    assert_eq!(status, 200, "{made}");
    let as_one = made["id"].as_str().unwrap().to_string();
    let (status, made) = call(&app, post("/build-rules".into(), json!({ "name": format!("Breakdown {run}"), "from": as_one }))).await;
    assert_eq!(status, 200, "{made}");
    let breakdown = made["id"].as_str().unwrap().to_string();
    let (status, twice) = call(&app, post("/build-rules".into(), json!({ "name": format!("Breakdown {run}") }))).await;
    assert_eq!(status, 400, "one set a name: {twice}");

    let sets = || get("/build-rules".into());
    let set_of = |all: &Value, id: &str| all.as_array().unwrap().iter().find(|s| s["id"] == id).cloned().expect("the set");
    let (status, all) = call(&app, sets()).await;
    assert_eq!(status, 200, "{all}");
    let first = set_of(&all, &breakdown);
    assert_eq!(first["rules"].as_array().unwrap().len(), 12, "{first}");
    assert_eq!(rule(&first, "heavy")["amount"], 20_000);
    let first_version = first["version_id"].clone();

    // The strict carrier's: no overhang either side of the forks, labels out.
    let mut rules: Vec<Value> = first["rules"].as_array().unwrap().clone();
    for r in rules.iter_mut() {
        if r["kind"] == "overhang_sides" || r["kind"] == "labels" {
            r["strength"] = json!("must");
            r["weight"] = Value::Null;
        }
    }
    let save = |id: &str, rules: &[Value]| {
        test::TestRequest::put()
            .uri(&format!("/build-rules/{id}"))
            .insert_header(auth.clone())
            .set_json(json!({ "name": format!("Breakdown {run}"), "rules": rules }))
    };
    let (status, body) = call(&app, save(&breakdown, &rules)).await;
    assert_eq!(status, 204, "{body}");
    let (_, all) = call(&app, sets()).await;
    let strict = set_of(&all, &breakdown);
    assert_ne!(strict["version_id"], first_version, "saved as a new version");
    assert_eq!(rule(&strict, "overhang_sides")["strength"], "must");
    let (status, _) = call(&app, save(&breakdown, &rules)).await;
    assert_eq!(status, 204);
    let (_, all) = call(&app, sets()).await;
    assert_eq!(set_of(&all, &breakdown)["version_id"], strict["version_id"], "the same rules again are no new version");
    let (status, short) = call(&app, save(&breakdown, &rules[1..])).await;
    assert_eq!(status, 400, "a set has every rule: {short}");

    // ── a carrier pointing at them, the usual one ────────────────────────
    let (status, carrier) = call(&app, post("/carriers".into(), json!({ "name": format!("Road {run}") }))).await;
    assert_eq!(status, 200, "{carrier}");
    let carrier = carrier["id"].as_str().unwrap().to_string();
    let (status, again) = call(&app, post("/carriers".into(), json!({ "name": format!("road{run}") }))).await;
    assert_eq!(status, 400, "a code once: {again}");
    let (status, body) = call(
        &app,
        post(format!("/carriers/{carrier}"), json!({
            "name": format!("Road {run}"), "breakdown_rules_id": breakdown, "as_one_rules_id": as_one,
        })),
    )
    .await;
    assert_eq!(status, 204, "{body}");
    let (status, service) = call(&app, post(format!("/carriers/{carrier}/services"), json!({ "name": "General" }))).await;
    assert_eq!(status, 200, "{service}");
    let (_, before) = call(&app, get("/carriers".into())).await;
    let usual_before = before.as_array().unwrap().iter().find(|c| c["usual"] == true).map(|c| c["id"].clone());
    let (status, body) = call(&app, post("/workspace/usual-carrier".into(), json!({ "carrier_id": carrier }))).await;
    assert_eq!(status, 204, "{body}");
    let (_, listed) = call(&app, get("/carriers".into())).await;
    let ours = listed.as_array().unwrap().iter().find(|c| c["id"] == carrier.as_str()).cloned().expect("the carrier");
    assert_eq!(ours["usual"], true, "{ours}");
    assert_eq!(ours["services"][0]["code"], "GENERAL", "{ours}");
    let (_, all) = call(&app, sets()).await;
    assert_eq!(set_of(&all, &breakdown)["used_by"][0], format!("Road {run}'s breakdown pallets"));

    // ── pallets built for it, to the rules of their kind ─────────────────
    let make = |preset: &str, breaks_down: bool, carrier: Option<&str>| {
        let id = Uuid::now_v7();
        (id, post("/packages".into(), json!({
            "id": id, "package_type_id": preset, "location_id": PACK, "breaks_down": breaks_down,
            "carrier_id": carrier, "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-16T10:00:00Z"
        })))
    };
    let (pallet, req) = make(PALLET, true, None);
    let (status, body) = call(&app, req).await;
    assert_eq!(status, 200, "{body}");
    let (_, req) = make(SMALL_BOX, false, Some(&carrier));
    let (status, refused) = call(&app, req).await;
    assert_eq!(status, 400, "a box is built for no carrier: {refused}");

    let (db, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.expect("connect");
    tokio::spawn(async move { let _ = connection.await; });
    let built = db
        .query_one("SELECT carrier_id::text, build_rules_version_id::text FROM package WHERE id = $1", &[&pallet])
        .await
        .expect("the pallet");
    assert_eq!(built.get::<_, Option<String>>(0).as_deref(), Some(carrier.as_str()), "the usual carrier");
    assert_eq!(built.get::<_, Option<String>>(1).as_deref(), strict["version_id"].as_str(), "its breakdown rules, as they stand");

    // ── the set changed: the pallet keeps what it was built to ───────────
    for r in rules.iter_mut() {
        if r["kind"] == "heavy" {
            r["amount"] = json!(25_000);
        }
    }
    assert_eq!(call(&app, save(&breakdown, &rules)).await.0, 204);
    let kept: Option<String> = db
        .query_one("SELECT build_rules_version_id::text FROM package WHERE id = $1", &[&pallet])
        .await
        .unwrap()
        .get(0);
    assert_eq!(kept.as_deref(), strict["version_id"].as_str(), "as it stood when it was made");

    // ── an order's pallet and a hollow carton, on the bench ──────────────
    let (_, minted) = call(&app, post("/tokens".into(), json!({ "label": "carriers test" }))).await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());
    let if_id = format!("if-cr-{run}");
    let (status, imported) = call(
        &app,
        test::TestRequest::post()
            .uri("/import/fulfilment?apply=true")
            .insert_header(("authorization", token))
            .set_payload(
                json!({
                    "order": format!("S-cr-{run}"), "customer": format!("Carrier Test {run}"),
                    "fulfilment_id": if_id, "fulfilment_number": "IF-CR", "status": "Picked",
                    "observed_at": "2026-10-16T09:00:00Z",
                    "lines": [{ "line": 1, "item": format!("CR-{run}"), "description": "Carrier test item",
                                "location": "Melbourne Warehouse", "quantity": 2, "external_line": "1" }]
                })
                .to_string(),
            ),
    )
    .await;
    assert_eq!(status, 200, "{imported}");
    let row = db
        .query_one(
            "SELECT f.id, fl.id FROM fulfilment_line fl JOIN fulfilment f ON f.id = fl.fulfilment_id WHERE f.external_id = $1",
            &[&if_id],
        )
        .await
        .expect("the fulfilment");
    let (fulfilment, line): (Uuid, Uuid) = (row.get(0), row.get(1));
    let order_pallet = Uuid::now_v7();
    let carton = Uuid::now_v7();
    for (id, preset) in [(order_pallet, PALLET), (carton, SMALL_BOX)] {
        let (status, body) = call(&app, post("/packages".into(), json!({
            "id": id, "fulfilment_id": fulfilment, "package_type_id": preset, "location_id": PACK,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-16T10:05:00Z"
        })))
        .await;
        assert_eq!(status, 200, "{body}");
    }
    let (status, handed) = call(&app, post("/handovers".into(), json!({
        "fulfilment_line_id": line, "quantity": 2, "to_package_id": carton,
        "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-16T10:06:00Z"
    })))
    .await;
    assert_eq!(status, 200, "{handed}");
    let (status, sealed) = call(&app, post(format!("/packages/{carton}/seal"), json!({
        "hollow": true, "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-16T10:07:00Z"
    })))
    .await;
    assert_eq!(status, 200, "{sealed}");

    let (status, bench) = call(&app, get(format!("/fulfilments/{fulfilment}/bench"))).await;
    assert_eq!(status, 200, "{bench}");
    let parcel = |id: Uuid| bench["cartons"].as_array().unwrap().iter().find(|c| c["id"] == id.to_string()).cloned().expect("on the bench");
    assert_eq!(parcel(carton)["hollow"], true, "{bench}");
    let theirs = parcel(order_pallet);
    assert_eq!(theirs["carrier_id"], carrier.as_str());
    assert_eq!(theirs["rules"]["id"], as_one.as_str(), "a pallet that goes as one, to its set: {theirs}");
    let offered = bench["carriers"].as_array().unwrap().iter().find(|c| c["id"] == carrier.as_str()).cloned().expect("offered");
    assert_eq!(offered["usual"], true);
    assert_eq!(rule(&offered["breakdown"], "heavy")["amount"], 25_000, "the rules now, for a pallet not yet made: {offered}");

    // ── a pallet planned for another carrier is made for it ──────────────
    let (_, other) = call(&app, post("/carriers".into(), json!({ "name": format!("Other {run}") }))).await;
    let other = other["id"].as_str().unwrap().to_string();
    let planned = Uuid::now_v7();
    let plan = |preset: &str| {
        test::TestRequest::put().uri(&format!("/fulfilments/{fulfilment}/pack-plan")).insert_header(auth.clone()).set_json(json!({
            "parcels": [{ "id": planned, "package_type_id": preset, "on_parcel_id": null, "on_package_id": null,
                          "at": null, "placed_by_hand": true, "carrier_id": other, "lines": [] }]
        }))
    };
    let (status, boxed) = call(&app, plan(SMALL_BOX)).await;
    assert_eq!(status, 400, "a box is planned for no carrier: {boxed}");
    let (status, body) = call(&app, plan(PALLET)).await;
    assert_eq!(status, 204, "{body}");
    let (status, body) = call(&app, post("/packages".into(), json!({
        "id": planned, "fulfilment_id": fulfilment, "package_type_id": PALLET, "location_id": PACK,
        "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-16T10:08:00Z"
    })))
    .await;
    assert_eq!(status, 200, "{body}");
    let made_for: Option<String> = db
        .query_one("SELECT carrier_id::text FROM package WHERE id = $1", &[&planned])
        .await
        .unwrap()
        .get(0);
    assert_eq!(made_for.as_deref(), Some(other.as_str()), "the plan's carrier, not the usual one");

    // ── put back as it was ───────────────────────────────────────────────
    let (status, _) = call(&app, post("/workspace/usual-carrier".into(), json!({ "carrier_id": usual_before }))).await;
    assert_eq!(status, 204);
}

#[actix_web::test]
async fn a_box_carries_on_its_walls_and_its_lid() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let session = common::bearer(&app).await;
    let auth = ("authorization", session.clone());
    let post = |uri: String, body: Value| test::TestRequest::post().uri(&uri).insert_header(auth.clone()).set_json(body);
    let get = |uri: &str| test::TestRequest::get().uri(uri).insert_header(auth.clone());

    let (db, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.expect("connect");
    tokio::spawn(async move { let _ = connection.await; });
    let run = &Uuid::new_v4().simple().to_string()[..8];
    let full: Uuid = db
        .query_one(
            "INSERT INTO package_type (tenant_id, name, carrier_package_code, length_mm, width_mm, height_mm)
             SELECT tenant_id, $1, 'CTN', 450, 340, 410 FROM package_type WHERE id = $2::text::uuid
             RETURNING id",
            &[&format!("Carry Box {run}"), &SMALL_BOX],
        )
        .await
        .expect("a box of the test's own")
        .get(0);
    let (status, cut) = call(&app, post(format!("/package-types/{full}/heights"), json!({ "height_mm": 280 }))).await;
    assert_eq!(status, 200, "{cut}");
    let cut = cut["id"].as_str().unwrap().to_string();

    let boxes = || get("/package-types");
    let carries = |all: &Value, id: &str| all.as_array().unwrap().iter().find(|b| b["id"] == id).unwrap()["carries"].clone();
    let (_, all) = call(&app, boxes()).await;
    let estimated = carries(&all, &full.to_string());
    assert_eq!(estimated["walls_estimated"], true, "{estimated}");
    assert_eq!(estimated["board"], json!({ "walls": 1, "ect": 32 }), "single-wall 32 ECT, assumed");
    assert_eq!(estimated["lid_g"], 10_000, "10 kg to start");
    let walls = estimated["walls_g"].as_i64().unwrap();
    let pallet = all.as_array().unwrap().iter().find(|b| b["id"] == PALLET).unwrap();
    assert!(pallet["carries"].is_null(), "a pallet carries by its carrier's rules: {pallet}");

    // ── said: a double-wall board, and a lid that takes nothing ──────────
    let (status, body) = call(&app, post(format!("/package-types/{full}/carries"), json!({
        "board": { "walls": 2, "ect": 48 }, "wall_load_g": null, "lid_load_g": 0
    })))
    .await;
    assert_eq!(status, 204, "{body}");
    let (_, all) = call(&app, boxes()).await;
    let said = carries(&all, &full.to_string());
    assert!(said["walls_g"].as_i64().unwrap() > walls, "a stiffer board carries more: {said}");
    assert_eq!((said["lid_g"].clone(), said["lid_said"].clone()), (json!(0), json!(true)));
    assert_eq!(carries(&all, &cut), said, "a cut carries as its box, unless it says");

    // ── a test on the floor sets the walls ───────────────────────────────
    let (status, _) = call(&app, post(format!("/package-types/{full}/carries"), json!({
        "board": null, "wall_load_g": 40_000, "lid_load_g": null
    })))
    .await;
    assert_eq!(status, 204);
    let (_, all) = call(&app, boxes()).await;
    let tested = carries(&all, &full.to_string());
    assert_eq!((tested["walls_g"].clone(), tested["walls_estimated"].clone()), (json!(40_000), json!(false)));
    let (status, refused) = call(&app, post(format!("/package-types/{full}/carries"), json!({
        "board": { "walls": 3, "ect": 32 }, "wall_load_g": null, "lid_load_g": null
    })))
    .await;
    assert_eq!(status, 400, "single or double wall: {refused}");
    let (status, _) = call(&app, post(format!("/package-types/{PALLET}/carries"), json!({
        "board": null, "wall_load_g": null, "lid_load_g": null
    })))
    .await;
    assert_eq!(status, 404, "the platform's pallet is not the workspace's");

    // ── the conditions move every estimate ───────────────────────────────
    let (_, ws) = call(&app, get("/workspace")).await;
    let before = ws["conditions"].clone();
    let (status, body) = call(&app, post("/workspace/conditions".into(), json!({
        "humidity_pct": 50, "days_under_load": 1, "stacking": "columns", "deck_gaps": false,
        "handling": "careful", "road_factor": 3
    })))
    .await;
    assert_eq!(status, 204, "{body}");
    let (_, ws) = call(&app, get("/workspace")).await;
    assert!((ws["conditions_share"].as_f64().unwrap() - 1.0 / 3.0).abs() < 1e-9, "{ws}");
    let (_, all) = call(&app, boxes()).await;
    assert_eq!(carries(&all, &cut)["walls_g"], 40_000, "a test is a test, whatever the weather");
    let small = carries(&all, SMALL_BOX);
    let crush = small["crush_g"].as_f64().unwrap();
    assert_eq!(small["walls_g"].as_f64().unwrap(), (crush / 3.0 / 100.0).round() * 100.0, "dry, in columns, a third: {small}");
    let (status, odd) = call(&app, post("/workspace/conditions".into(), json!({
        "humidity_pct": 65, "days_under_load": 1, "stacking": "columns", "deck_gaps": false,
        "handling": "careful", "road_factor": 3
    })))
    .await;
    assert_eq!(status, 400, "a humidity the table has: {odd}");
    let (status, _) = call(&app, post("/workspace/conditions".into(), before)).await;
    assert_eq!(status, 204, "put back as they were");
}
