//! A thing that ships as it is, and a box the bench does not suggest (D196).
//!
//! Three rolls of wipers, each in its own box, were suggested into a shovel
//! box. Here a roll is said to ship as it is, the bench reads it so, one roll
//! is made a package of its own and sealed, and a box is taken out of the
//! suggestion while staying a box anybody can choose.
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
/// The fixture's large box, the workspace's own.
const LARGE_BOX: &str = "9a7e0000-0000-0000-0000-0000000000b3";
/// The platform's pallet, which is not the workspace's to change.
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

/// The line's pack at a level, from a bench read.
fn pack<'a>(bench: &'a Value, level: &str) -> &'a Value {
    bench["lines"][0]["packs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["level"] == level)
        .unwrap_or_else(|| panic!("a {level} on the line: {bench}"))
}

#[actix_web::test]
async fn a_roll_in_its_own_box_ships_as_it_is() {
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
            .set_json(json!({ "label": "ships as it is test" })),
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());

    // ── NetSuite says three rolls are picked ─────────────────────────────
    let run = &Uuid::new_v4().simple().to_string()[..8];
    let code = format!("ROLL-{run}");
    let if_id = format!("if-roll-{run}");
    let (status, imported) = call(
        &app,
        test::TestRequest::post()
            .uri("/import/fulfilment?apply=true")
            .insert_header(("authorization", token.clone()))
            .set_payload(
                json!({
                    "order": format!("S-roll-{run}"),
                    "customer": format!("Roll Test {run}"),
                    "fulfilment_id": if_id,
                    "fulfilment_number": "IF-ROLL",
                    "status": "Picked",
                    "observed_at": "2026-10-03T09:00:00Z",
                    "lines": [
                        { "line": 1, "item": code, "description": "Wiper roll in its own box",
                          "location": "Melbourne Warehouse", "quantity": 3, "external_line": "1" },
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
            "SELECT f.id, fl.id, ol.item_id FROM fulfilment_line fl
               JOIN fulfilment f ON f.id = fl.fulfilment_id
               JOIN order_line ol ON ol.id = fl.order_line_id
              WHERE f.external_id = $1",
            &[&if_id],
        )
        .await
        .expect("the fulfilment");
    let (fulfilment, line, item): (Uuid, Uuid, Uuid) = (row.get(0), row.get(1), row.get(2));

    let bench = || {
        test::TestRequest::get()
            .uri(&format!("/fulfilments/{fulfilment}/bench"))
            .insert_header(auth.clone())
    };

    // ── unsaid, an each goes into a box ──────────────────────────────────
    let (status, before) = call(&app, bench()).await;
    assert_eq!(status, 200, "{before}");
    assert_eq!(pack(&before, "each")["ships_as_is"], false, "the default for an each");

    // ── and with no size it is on the packing worklist (D197) ───────────
    let worklist = || {
        test::TestRequest::get()
            .uri(&format!("/items?needs=packing&order=packing&q={code}"))
            .insert_header(auth.clone())
    };
    let (status, needed) = call(&app, worklist()).await;
    assert_eq!(status, 200, "{needed}");
    let row = needed["items"].as_array().unwrap().iter().find(|i| i["code"] == code.as_str()).cloned();
    let row = row.unwrap_or_else(|| panic!("a roll with no size, still to pack: {needed}"));
    assert_eq!(row["to_pack"], 3, "{row}");
    assert_eq!(row["to_pack_lines"], 1, "{row}");

    // ── said: it ships as it is, and saying it again is the same act ────
    let said = Uuid::now_v7();
    let say = |event: Uuid, as_it_is: bool| {
        test::TestRequest::post().uri("/shipping").insert_header(auth.clone()).set_json(json!({
            "item_id": item,
            "packaging_level": "each",
            "as_it_is": as_it_is,
            "client_event_id": event,
            "occurred_at": "2026-10-03T09:01:00Z"
        }))
    };
    let (status, body) = call(&app, say(said, true)).await;
    assert_eq!(status, 204, "{body}");
    let (status, _) = call(&app, say(said, true)).await;
    assert_eq!(status, 204, "the same act again");
    let (_, after) = call(&app, bench()).await;
    assert_eq!(pack(&after, "each")["ships_as_is"], true, "{after}");
    let (_, unneeded) = call(&app, worklist()).await;
    assert_eq!(unneeded["total"], 0, "what ships as it is needs no size to be packed round: {unneeded}");

    // ── this way up, said the same way (D200) ───────────────────────────
    assert_eq!(pack(&after, "each")["upright"], false, "any way up, unless said");
    let (status, body) = call(
        &app,
        test::TestRequest::post().uri("/upright").insert_header(auth.clone()).set_json(json!({
            "item_id": item,
            "packaging_level": "each",
            "upright": true,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-10-03T09:02:00Z"
        })),
    )
    .await;
    assert_eq!(status, 204, "{body}");
    let (_, stood) = call(&app, bench()).await;
    assert_eq!(pack(&stood, "each")["upright"], true, "{stood}");
    let (status, partless) = call(
        &app,
        test::TestRequest::post().uri("/upright").insert_header(auth.clone()).set_json(json!({
            "item_id": item,
            "lot_id": Uuid::now_v7(),
            "packaging_level": "each",
            "upright": true,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-10-03T09:02:00Z"
        })),
    )
    .await;
    assert_eq!(status, 400, "said of one thing: {partless}");
    let (status, levelless) = call(
        &app,
        test::TestRequest::post().uri("/shipping").insert_header(auth.clone()).set_json(json!({
            "item_id": item,
            "as_it_is": true,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-10-03T09:01:00Z"
        })),
    )
    .await;
    assert_eq!(status, 400, "an item needs its level: {levelless}");

    // ── one roll, a package of its own: made, handed over into, sealed ──
    let parcel = Uuid::now_v7();
    let (status, made) = call(
        &app,
        test::TestRequest::post().uri("/packages").insert_header(auth.clone()).set_json(json!({
            "id": parcel,
            "fulfilment_id": fulfilment,
            "own_item_id": item,
            "own_level": "each",
            "location_id": PACK,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-10-03T09:10:00Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "{made}");
    let (status, handed) = call(
        &app,
        test::TestRequest::post().uri("/handovers").insert_header(auth.clone()).set_json(json!({
            "fulfilment_line_id": line,
            "quantity": 1,
            "to_package_id": parcel,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-10-03T09:10:01Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "{handed}");
    let (status, sealed) = call(
        &app,
        test::TestRequest::post()
            .uri(&format!("/packages/{parcel}/seal"))
            .insert_header(auth.clone())
            .set_json(json!({ "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-03T09:10:02Z" })),
    )
    .await;
    assert_eq!(status, 200, "{sealed}");

    let (_, packed) = call(&app, bench()).await;
    assert_eq!(packed["lines"][0]["remaining"], 2, "one roll gone as it is: {packed}");
    let p = packed["cartons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == parcel.to_string())
        .expect("the roll is on the bench");
    assert_eq!(p["own_carton_of"], code.as_str(), "{p}");
    assert_eq!(p["own_level"], "each", "one of it as an each, not a carton");
    assert!(p["package_type"].is_null(), "not a box type");
    assert_eq!(p["sealed"], true);

    // ── a carton ships as it is unless said, once the case pack counts it ─
    let config: Uuid = db
        .query_one(
            "INSERT INTO item_packing_config (tenant_id, item_id, units_per_inner, inners_per_carton)
             SELECT tenant_id, id, 1, 4 FROM item WHERE id = $1 RETURNING id",
            &[&item],
        )
        .await
        .expect("a case pack")
        .get(0);
    let (_, cased) = call(&app, bench()).await;
    assert_eq!(pack(&cased, "carton")["ships_as_is"], true, "the default for a carton: {cased}");
    assert_eq!(pack(&cased, "carton")["units"], 4);

    // ── what a caller can get wrong ─────────────────────────────────────
    let refused = |body: Value| test::TestRequest::post().uri("/packages").insert_header(auth.clone()).set_json(body);
    let (status, each_cased) = call(
        &app,
        refused(json!({
            "fulfilment_id": fulfilment,
            "item_packing_config_id": config,
            "own_level": "each",
            "location_id": PACK,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-10-03T09:11:00Z"
        })),
    )
    .await;
    assert_eq!(status, 400, "an each has no case pack: {each_cased}");
    let other: Uuid = db
        .query_one("SELECT id FROM item WHERE id <> $1 AND tenant_id = (SELECT tenant_id FROM item WHERE id = $1) LIMIT 1", &[&item])
        .await
        .expect("another item")
        .get(0);
    let (status, elsewhere) = call(
        &app,
        refused(json!({
            "fulfilment_id": fulfilment,
            "own_item_id": other,
            "own_level": "each",
            "location_id": PACK,
            "client_event_id": Uuid::now_v7(),
            "occurred_at": "2026-10-03T09:11:00Z"
        })),
    )
    .await;
    assert_eq!(status, 400, "one of something not on the order: {elsewhere}");
    assert!(elsewhere.to_string().contains("does not carry"), "{elsewhere}");
}

#[actix_web::test]
async fn a_box_can_be_left_out_of_the_suggestion() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);
    let suggest = |id: &str, on: bool| {
        test::TestRequest::post()
            .uri(&format!("/package-types/{id}/suggested"))
            .insert_header(auth.clone())
            .set_json(json!({ "suggested": on }))
    };
    let large = |types: &Value| types.as_array().unwrap().iter().find(|t| t["id"] == LARGE_BOX).cloned().expect("the large box");
    let types = || test::TestRequest::get().uri("/package-types").insert_header(auth.clone());

    let (status, body) = call(&app, suggest(LARGE_BOX, false)).await;
    assert_eq!(status, 204, "{body}");
    let (_, listed) = call(&app, types()).await;
    assert_eq!(large(&listed)["suggested"], false, "left out of the suggestion");

    let (status, _) = call(&app, suggest(PALLET, false)).await;
    assert_eq!(status, 404, "the platform's pallet is not the workspace's to change");

    let (status, _) = call(&app, suggest(LARGE_BOX, true)).await;
    assert_eq!(status, 204);
    let (_, back) = call(&app, types()).await;
    assert_eq!(large(&back)["suggested"], true, "and back in");

    // ── a box's weight limit (D199) ──────────────────────────────────────
    let weigh = |id: &str, grams: Value| {
        test::TestRequest::post()
            .uri(&format!("/package-types/{id}/max-weight"))
            .insert_header(auth.clone())
            .set_json(json!({ "max_payload_g": grams }))
    };
    let (status, body) = call(&app, weigh(LARGE_BOX, json!(25000))).await;
    assert_eq!(status, 204, "{body}");
    let (_, limited) = call(&app, types()).await;
    assert_eq!(large(&limited)["max_payload_g"], 25000);
    let (status, _) = call(&app, weigh(LARGE_BOX, json!(0))).await;
    assert_eq!(status, 400, "a limit of nothing is not a limit");
    let (status, _) = call(&app, weigh(PALLET, json!(25000))).await;
    assert_eq!(status, 404, "nor is the platform's pallet the workspace's to limit");
    let (status, _) = call(&app, weigh(LARGE_BOX, Value::Null)).await;
    assert_eq!(status, 204);
    let (_, unlimited) = call(&app, types()).await;
    assert!(large(&unlimited)["max_payload_g"].is_null(), "and none again");
}
