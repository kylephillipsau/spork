//! Where a pick lands, over HTTP (D166).
//!
//! `POST /picks` took `to_package_id: Uuid` — required — while the ledger it
//! writes has had `to_location_id` and `to_package_id` as an exclusive pair,
//! with whole-key CHECKs behind them, since migration 2. The write path was
//! narrower than the row.
//!
//! The floor is why it matters. A forklift picks a large order straight onto a
//! pallet, which is a package and always worked. A picker with a trolley takes
//! goods off the shelf and puts them down at the packing station — and a
//! trolley is not a container anybody should record.
//!
//! # The assertion this file exists for
//!
//! Not that a location can be named — the type system says that much. It is
//! that a **two-legged pick counts once**. `picked` folded as *left a
//! location*, so shelf → station and station → carton both counted, and one
//! order's units were picked twice: J56 raising `picked > covered` about a
//! warehouse that had done nothing wrong. Migration 91 makes it *left storage*,
//! and this drives both legs through the handlers to prove the arithmetic.
//!
//! Nothing here mutates a cell any other file reads, and it removes what it
//! wrote.

use actix_web::{http::StatusCode, test, web, App};
use chrono::Utc;
use nylonite_server::{routes, AppState};
use serde_json::{json, Value};
use uuid::Uuid;

mod common;
use common::{pool, url};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
/// The packing station the fixture gained with D166.
const STATION: &str = "10c00000-0000-0000-0000-000000000004";
const SMALL_BOX: &str = "9a7e0000-0000-0000-0000-0000000000b1";
const DOCK: &str = "10c00000-0000-0000-0000-000000000003";

async fn raw(u: &str) -> tokio_postgres::Client {
    let (c, conn) = tokio_postgres::connect(u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = conn.await;
    });
    c
}

async fn fold(c: &tokio_postgres::Client) {
    c.execute("SELECT projection_run_all($1)", &[&Uuid::parse_str(ALPHA).unwrap()])
        .await
        .expect("fold");
}

/// What the projection says about a line now.
async fn progress(c: &tokio_postgres::Client, line: Uuid) -> (i64, i64, i64) {
    let r = c
        .query_one(
            "SELECT covered_quantity, picked_quantity, packed_quantity
               FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .expect("the line");
    (r.get(0), r.get(1), r.get(2))
}

#[actix_web::test]
async fn a_trolley_pick_lands_at_the_station_and_is_picked_exactly_once() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let bearer = common::bearer(&app).await;
    let db = raw(&u).await;
    let now = Utc::now();

    let open: Value = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/open-lines"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /sites/{id}/open-lines",
    )
    .await;
    let line = open
        .as_array()
        .expect("open work")
        .iter()
        .find(|l| {
            l["quantity"].as_i64().unwrap_or(0) - l["covered_quantity"].as_i64().unwrap_or(0) > 0
        })
        .expect("a line with room left to claim");
    let line_id: Uuid = Uuid::parse_str(line["fulfilment_line_id"].as_str().unwrap()).unwrap();
    let fulfilment = line["fulfilment_id"].as_str().unwrap().to_string();
    let item_code = line["item_code"].as_str().unwrap().to_string();

    let shelf: Uuid = db
        .query_one(
            "SELECT s.id FROM stock s
               JOIN item i ON i.id = s.item_id
               JOIN location l ON l.id = s.holder_location_id
              WHERE i.code = $1 AND l.kind IN ('pick_face', 'bulk')
                AND s.quantity - s.allocated_quantity > 0
              ORDER BY s.quantity - s.allocated_quantity DESC LIMIT 1",
            &[&item_code],
        )
        .await
        .expect("a storage cell with something spare in it")
        .get(0);

    let allocation = Uuid::new_v4();
    let act_walk = Uuid::new_v4();
    let act_carton = Uuid::new_v4();
    let act_bench = Uuid::new_v4();
    let carton = Uuid::new_v4();

    let post = |uri: String, body: Value, bearer: String| {
        test::TestRequest::post()
            .uri(&uri)
            .insert_header(("authorization", bearer))
            .set_json(body)
            .to_request()
    };

    fold(&db).await;
    let (covered_before, picked_before, _) = progress(&db, line_id).await;

    common::ok_json(
        &app,
        post(
            "/allocations".into(),
            json!({ "id": allocation, "fulfilment_line_id": line_id,
                    "stock_id": shelf, "quantity": 1 }),
            bearer.clone(),
        ),
        "POST /allocations",
    )
    .await;

    // ── leg one: off the shelf, onto the trolley, down at the station ───
    let walked: Value = common::ok_json(
        &app,
        post(
            "/picks".into(),
            json!({ "from_stock_id": shelf, "to_location_id": STATION,
                    "fulfilment_line_id": line_id, "quantity": 1,
                    "client_event_id": act_walk, "occurred_at": now }),
            bearer.clone(),
        ),
        "POST /picks (to a location)",
    )
    .await;
    assert!(walked["movement_id"].is_string(), "{walked}");

    // The ledger row says where it went, and says nothing about a package.
    let (to_loc, to_pkg): (Option<Uuid>, Option<Uuid>) = db
        .query_one(
            "SELECT to_location_id, to_package_id FROM stock_movement
              WHERE client_event_id = $1",
            &[&act_walk],
        )
        .await
        .map(|r| (r.get(0), r.get(1)))
        .expect("the movement");
    assert_eq!(to_loc, Some(Uuid::parse_str(STATION).unwrap()));
    assert_eq!(to_pkg, None, "a pick to a location named a package as well");

    fold(&db).await;
    let (_, picked_walk, packed_walk) = progress(&db, line_id).await;
    assert_eq!(picked_walk, picked_before + 1, "the walk did not count as picked");
    assert_eq!(packed_walk, 0, "goods at the packing station are not packed");

    // ── leg two: the packer puts it in the box ──────────────────────────
    let station_cell: Uuid = db
        .query_one(
            "SELECT s.id FROM stock s
               JOIN item i ON i.id = s.item_id
              WHERE i.code = $1 AND s.holder_location_id = $2 AND s.quantity > 0",
            &[&item_code, &Uuid::parse_str(STATION).unwrap()],
        )
        .await
        .expect("the goods are at the station")
        .get(0);

    common::ok_json(
        &app,
        post(
            "/packages".into(),
            json!({ "id": carton, "fulfilment_id": fulfilment,
                    "package_type_id": SMALL_BOX, "location_id": DOCK,
                    "client_event_id": act_carton, "occurred_at": now }),
            bearer.clone(),
        ),
        "POST /packages",
    )
    .await;
    common::ok_json(
        &app,
        post(
            "/picks".into(),
            json!({ "from_stock_id": station_cell, "to_package_id": carton,
                    "fulfilment_line_id": line_id, "quantity": 1,
                    "client_event_id": act_bench, "occurred_at": now }),
            bearer.clone(),
        ),
        "POST /picks (into the carton)",
    )
    .await;

    // **The assertion this file exists for.** Both legs left a location. Only
    // one left storage.
    fold(&db).await;
    let (covered_after, picked_after, _) = progress(&db, line_id).await;
    assert_eq!(
        picked_after, picked_before + 1,
        "one unit walked to the bench and into a box was picked twice"
    );
    assert!(
        picked_after <= covered_after,
        "J56: picked {picked_after} past covered {covered_after}"
    );
    assert_eq!(covered_after, covered_before + 1, "the claim is the one this made");

    // ── the walk says how much is spoken for, not just whether ─────────
    //
    // **The screen cannot ask without these.** `POST /allocations` refuses an
    // over-claim with `OverCovers`, and a pick with no claim behind it drives
    // `picked` past `covered` and raises J56 — so a pick screen has to know how
    // much of the line is already covered before it decides whether to claim.
    // `allocated` is a bool about one bin and cannot answer it.
    let walk = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/picking"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET the walk",
    )
    .await;
    // Found, not maybe-found: this line had room left before the pick and one
    // unit went, so it is still on the walk. An `if let` here would let the
    // assertions below never run and still report a pass.
    let row = walk["lines"]
        .as_array()
        .expect("lines")
        .iter()
        .find(|l| l["fulfilment_line_id"].as_str() == Some(&line_id.to_string()))
        .unwrap_or_else(|| panic!("the line this test picked from is still on the walk: {walk}"));

    let (covered_now, picked_now, _) = progress(&db, line_id).await;
    assert_eq!(
        row["picked"].as_i64(),
        Some(picked_now),
        "the walk's picked is the line's picked: {row}"
    );
    assert_eq!(
        row["covered"].as_i64(),
        Some(covered_now),
        "the walk's covered is the line's covered, over every cell: {row}"
    );
    // The claim a screen would make to pick one more, and the reason both
    // figures are on the wire: usually nothing, because planning covered it.
    assert!(
        picked_now + 1 - covered_now <= 1,
        "the shortfall for one more unit is computable from the row: {row}"
    );

    // ── and a pick that lands nowhere, or in two places, is refused ─────
    for body in [
        json!({ "from_stock_id": shelf, "fulfilment_line_id": line_id, "quantity": 1,
                "client_event_id": Uuid::new_v4(), "occurred_at": now }),
        json!({ "from_stock_id": shelf, "to_location_id": STATION, "to_package_id": carton,
                "fulfilment_line_id": line_id, "quantity": 1,
                "client_event_id": Uuid::new_v4(), "occurred_at": now }),
    ] {
        let refused =
            test::call_service(&app, post("/picks".into(), body, bearer.clone())).await;
        assert_eq!(
            refused.status(),
            StatusCode::BAD_REQUEST,
            "a pick landed in no place or two"
        );
    }

    db.batch_execute(&format!(
        "DELETE FROM stock_movement WHERE client_event_id IN ('{act_walk}', '{act_bench}');
         DELETE FROM stock_allocation WHERE id = '{allocation}';
         UPDATE package SET placement_event_id = NULL, placement_occurred_at = NULL
          WHERE id = '{carton}';
         DELETE FROM stock WHERE holder_package_id = '{carton}';
         -- Containment before the events it points at: `package_containment`
         -- carries `source_event_id`, so the rows have to go in that order.
         DELETE FROM package_containment WHERE package_id = '{carton}'
                                            OR parent_package_id = '{carton}';
         DELETE FROM package_event WHERE package_id = '{carton}';
         DELETE FROM package WHERE id = '{carton}';
         DELETE FROM client_event WHERE client_event_id
             IN ('{act_walk}', '{act_carton}', '{act_bench}');
         SELECT projection_run_all('{ALPHA}');"
    ))
    .await
    .expect("the test removes what it recorded");
}
