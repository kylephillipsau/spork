//! Goods picked elsewhere, handed over (D172).
//!
//! NetSuite says a line is picked; the packer puts the goods down at the
//! staging spot. That is an arrival on this ledger naming the report, and it is
//! not a pick: `picked_quantity` stays at nought because nothing left storage
//! here, while the stock at the staging spot is real and can be packed.
//!
//! Order numbers are random, so this runs against a database it has already
//! written to.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

/// PACK-1, the staging spot at Melbourne in the fixture.
const STAGING: &str = "10c00000-0000-0000-0000-000000000004";
/// DOCK-1, where a carton comes into being.
const DOCK: &str = "10c00000-0000-0000-0000-000000000003";
/// The fixture's small box preset.
const SMALL_BOX: &str = "9a7e0000-0000-0000-0000-0000000000b1";

#[actix_web::test]
async fn goods_picked_elsewhere_arrive_where_they_are_put() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let session = common::bearer(&app).await;

    let minted: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/tokens")
            .insert_header(("authorization", session.clone()))
            .set_json(json!({ "label": "handover test" }))
            .to_request(),
        "POST /tokens",
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());

    // ── NetSuite says four are picked on line 1, and nothing on line 2 ───
    let run = &Uuid::new_v4().simple().to_string()[..8];
    let if_id = format!("if-ho-{run}");
    common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/import/fulfilment?apply=true")
            .insert_header(("authorization", token.clone()))
            .set_payload(
                json!({
                    "order": format!("S-ho-{run}"),
                    "customer": format!("Handover Test {run}"),
                    "fulfilment_id": if_id,
                    "fulfilment_number": "IF-HO",
                    "status": "Picked",
                    "observed_at": "2026-09-28T09:00:00Z",
                    "lines": [
                        // Its own item: the stock this puts at the staging spot
                        // must not become a second cell another suite finds there.
                        { "line": 1, "item": format!("HO-{run}"), "description": "Handover test item",
                          "location": "Melbourne Warehouse", "quantity": 4, "external_line": "1" },
                    ]
                })
                .to_string(),
            )
            .to_request(),
        "import",
    )
    .await;

    let (db, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.expect("connect");
    tokio::spawn(async move { let _ = connection.await; });
    let line: Uuid = db
        .query_one(
            "SELECT fl.id FROM fulfilment_line fl JOIN fulfilment f ON f.id = fl.fulfilment_id
              WHERE f.external_id = $1",
            &[&if_id],
        )
        .await
        .expect("the line")
        .get(0);
    // A line of a fixture fulfilment that nobody reported picked.
    let unreported: Uuid = "f11e0000-0000-0000-0000-000000000001".parse().unwrap();

    // ── what NetSuite picked is not on Spork's walk (migration 117) ─────
    let walk: Value = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri("/sites/a5170000-0000-0000-0000-000000000001/picking")
            .insert_header(("authorization", session.clone()))
            .to_request(),
        "GET /sites/{id}/picking",
    )
    .await;
    assert!(
        !walk["lines"].as_array().unwrap().iter().any(|l| l["fulfilment_line_id"] == line.to_string()),
        "a line picked in NetSuite was on the walk, for somebody to pick again: {walk}"
    );

    let hand = |line: Uuid, qty: i64, event: Uuid| {
        test::TestRequest::post()
            .uri("/handovers")
            .insert_header(("authorization", session.clone()))
            .set_json(json!({
                "fulfilment_line_id": line,
                "quantity": qty,
                "to_location_id": STAGING,
                "client_event_id": event,
                "occurred_at": "2026-09-28T09:30:00Z"
            }))
            .to_request()
    };

    // ── two put down at the staging spot ─────────────────────────────────
    let first_event = Uuid::now_v7();
    let first = common::ok_json(&app, hand(line, 2, first_event), "first handover").await;
    assert_eq!(first["reported"], json!(4), "{first}");
    assert_eq!(first["handed"], json!(2), "{first}");
    assert!(first["warnings"].as_array().unwrap().is_empty(), "{first}");

    // ── the same act again is the same act ───────────────────────────────
    let again = common::ok_json(&app, hand(line, 2, first_event), "replay").await;
    assert_eq!(again["movement_id"], first["movement_id"], "a retry wrote a second movement");
    assert_eq!(again["handed"], json!(2), "{again}");

    // ── three more: past the report, recorded, and said so ───────────────
    let past = common::ok_json(&app, hand(line, 3, Uuid::now_v7()), "past the report").await;
    assert_eq!(past["handed"], json!(5), "{past}");
    let warned = past["warnings"].as_array().unwrap();
    assert!(
        warned.iter().any(|w| w.as_str().unwrap().contains("5 handed over against 4 reported")),
        "{past}"
    );

    // ── nothing reported, nothing to hand over ───────────────────────────
    let r = test::call_service(&app, hand(unreported, 1, Uuid::now_v7())).await;
    assert_eq!(r.status(), 400, "a line nobody reported picked took a handover");

    // ── the ledger: an arrival, not a pick ───────────────────────────────
    let movement: Uuid = first["movement_id"].as_str().unwrap().parse().unwrap();
    let m = db
        .query_one(
            "SELECT reason, from_location_id IS NULL AND from_package_id IS NULL,
                    to_location_id, external_pick_id IS NOT NULL, to_owner_id
               FROM stock_movement WHERE id = $1",
            &[&movement],
        )
        .await
        .expect("the movement");
    assert_eq!(m.get::<_, String>(0), "handover");
    assert!(m.get::<_, bool>(1), "an arrival has no from side");
    assert_eq!(m.get::<_, Uuid>(2), STAGING.parse::<Uuid>().unwrap(), "put down where it was put");
    assert!(m.get::<_, bool>(3), "it names the report it is the goods of");
    assert_eq!(
        m.get::<_, Uuid>(4),
        "9a247000-0000-0000-0000-000000000001".parse::<Uuid>().unwrap(),
        "owned by whoever the site says owns what it holds"
    );

    let tenant: Uuid = db
        .query_one("SELECT tenant_id FROM fulfilment_line WHERE id = $1", &[&line])
        .await
        .unwrap()
        .get(0);
    db.execute("SELECT projection_run_all($1)", &[&tenant]).await.expect("fold, as the scheduler would");
    let p = db
        .query_one(
            "SELECT picked_quantity, external_picked_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap();
    assert_eq!(p.get::<_, i64>(0), 0, "nothing left storage here, so nothing was picked here");
    assert_eq!(p.get::<_, i64>(1), 4, "the report still says four");

    // ── the queue: where it was picked, and not yet packed ───────────────
    let get = |uri: String| {
        test::TestRequest::get()
            .uri(&uri)
            .insert_header(("authorization", session.clone()))
            .to_request()
    };
    let job = |queue: &Value| {
        queue
            .as_array()
            .expect("a list")
            .iter()
            .find(|j| j["reference"] == json!("IF-HO"))
            .cloned()
            .expect("the job is in the queue at its site")
    };
    let queue = common::ok_json(&app, get(format!("/packing?q=S-ho-{run}")), "GET /packing").await;
    let j = job(&queue);
    assert_eq!(j["provenance"], json!("Picked in NetSuite · IF-HO"), "{j}");
    assert_eq!(j["reported"], json!(4), "{j}");
    assert_eq!(j["picked"], json!(0), "waiting at the staging spot is not packed: {j}");
    assert_eq!(j["stage"], json!("ready"), "{j}");

    // ── the bench: what is reported, what is handed over, and where ──────
    let fulfilment = j["fulfilment_id"].as_str().unwrap().to_string();
    let bench = common::ok_json(&app, get(format!("/fulfilments/{fulfilment}/bench")), "bench").await;
    let bl = &bench["lines"][0];
    assert_eq!(bl["elsewhere"]["reported"], json!(4), "{bench}");
    assert_eq!(bl["elsewhere"]["handed"], json!(5), "{bench}");
    assert_eq!(bl["elsewhere"]["document"], json!("IF-HO"), "{bench}");
    assert!(
        bl["cells"].as_array().unwrap().iter().any(|c| c["location"] == json!("PACK-1")),
        "the goods handed over are a cell at the staging spot to box from: {bench}"
    );

    // ── straight into a carton: on the bench ─────────────────────────────
    let carton = Uuid::now_v7();
    common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/packages")
            .insert_header(("authorization", session.clone()))
            .set_json(json!({ "id": carton, "fulfilment_id": fulfilment,
                              "package_type_id": SMALL_BOX, "location_id": DOCK,
                              "client_event_id": Uuid::now_v7(), "occurred_at": "2026-09-28T09:40:00Z" }))
            .to_request(),
        "POST /packages",
    )
    .await;
    common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/handovers")
            .insert_header(("authorization", session.clone()))
            .set_json(json!({ "fulfilment_line_id": line, "quantity": 1, "to_package_id": carton,
                              "client_event_id": Uuid::now_v7(), "occurred_at": "2026-09-28T09:41:00Z" }))
            .to_request(),
        "POST /handovers (into the carton)",
    )
    .await;
    db.execute("SELECT projection_run_all($1)", &[&tenant]).await.expect("fold, as the scheduler would");
    let queue = common::ok_json(&app, get(format!("/packing?q=S-ho-{run}")), "GET /packing again").await;
    let j = job(&queue);
    assert_eq!(j["picked"], json!(1), "one in a carton is one done at the bench: {j}");
    assert_eq!(j["stage"], json!("on_the_bench"), "{j}");

    // ── NetSuite is corrected: six were picked after all ─────────────────
    // Six are here against four reported, which J75 names until the record
    // catches up. A later report raising the level is how it does, and it
    // leaves this suite's rows consistent for every check that runs after.
    let corrected = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/import/fulfilment?apply=true")
            .insert_header(("authorization", token))
            .set_payload(
                json!({
                    "order": format!("S-ho-{run}"),
                    "customer": format!("Handover Test {run}"),
                    "fulfilment_id": if_id,
                    "fulfilment_number": "IF-HO",
                    "status": "Picked",
                    "observed_at": "2026-09-28T10:00:00Z",
                    "lines": [
                        { "line": 1, "item": format!("HO-{run}"), "description": "Handover test item",
                          "location": "Melbourne Warehouse", "quantity": 4, "picked": 6,
                          "external_line": "1" },
                    ]
                })
                .to_string(),
            )
            .to_request(),
        "the corrected report",
    )
    .await;
    assert_eq!(corrected["picks"]["recorded"], json!(1), "{corrected}");
    db.execute("SELECT projection_run_all($1)", &[&tenant]).await.expect("fold");
    let level: i64 = db
        .query_one("SELECT external_picked_quantity FROM fulfilment_line WHERE id = $1", &[&line])
        .await
        .unwrap()
        .get(0);
    assert_eq!(level, 6, "the later report is the level, raised to what is here");
}
