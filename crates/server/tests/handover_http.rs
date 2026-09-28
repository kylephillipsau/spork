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

mod common;
use common::{pool, url};

/// PACK-1, the staging spot at Melbourne in the fixture.
const STAGING: &str = "10c00000-0000-0000-0000-000000000004";

#[actix_web::test]
async fn goods_picked_elsewhere_arrive_where_they_are_put() {
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
            .insert_header(("authorization", token))
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
    db.execute("SELECT projection_fulfilment_rebuild($1)", &[&tenant]).await.expect("fold");
    let p = db
        .query_one(
            "SELECT picked_quantity, external_picked_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap();
    assert_eq!(p.get::<_, i64>(0), 0, "nothing left storage here, so nothing was picked here");
    assert_eq!(p.get::<_, i64>(1), 4, "the report still says four");
}
