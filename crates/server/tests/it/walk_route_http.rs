//! The picking walk, routed (D211): with the fixture site's bins laid out on a
//! floor, the walk comes back in route order from the packing bench, with the
//! route's length beside the typed order's; with the bench off the layout it
//! starts where the typed order starts.
//!
//! Its own file because it draws on the fixture site's layout, as
//! `places_http` and `plan_edit_http` do.

use actix_web::{test, web, App};
use serde_json::Value;
use spork_server::{routes, AppState};

use super::common;
use common::{pool, url, SITE};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const BUILDING: &str = "9a7b0000-0000-0000-0000-000000000001";

async fn clear(c: &tokio_postgres::Client) {
    c.batch_execute(&format!(
        "UPDATE location SET place_id = NULL, slot_bay = NULL, slot_level = NULL,
                             slot_row = NULL, slot_position = NULL, slot_side = NULL
          WHERE site_id = '{SITE}';
         DELETE FROM place WHERE site_id = '{SITE}' AND parent_id IS NOT NULL;
         DELETE FROM place WHERE site_id = '{SITE}';"
    ))
    .await
    .expect("clear the layout");
}

#[actix_web::test]
async fn the_walk_comes_in_route_order_from_the_bench() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let db = common::connect(&u, false).await;
    clear(&db).await;

    // A building 20 by 10, a rack along the middle holding A-01-1 on its
    // front, a dock at the far right with DOCK-1, and the packing bench at
    // the left with PACK-1, the site's packing location.
    db.batch_execute(&format!(
        "INSERT INTO place (id, tenant_id, site_id, name, x, y, length, depth, height)
         VALUES ('{BUILDING}', '{TENANT}', '{SITE}', 'Building', 0, 0, 20, 10, 6);
         INSERT INTO place (id, tenant_id, site_id, parent_id, name, solid, x, y, length, depth, height, bays, levels)
         VALUES ('9a7b0000-0000-0000-0000-000000000002', '{TENANT}', '{SITE}', '{BUILDING}', 'Rack RA', true, 4, 4, 10, 1, 3, 10, 3),
                ('9a7b0000-0000-0000-0000-000000000003', '{TENANT}', '{SITE}', '{BUILDING}', 'Dock RD', false, 17, 1, 2, 2, 1, 1, 1),
                ('9a7b0000-0000-0000-0000-000000000004', '{TENANT}', '{SITE}', '{BUILDING}', 'Bench RP', false, 1, 1, 2, 2, 1, 1, 1);
         UPDATE location SET place_id = '9a7b0000-0000-0000-0000-000000000002', slot_side = 1, slot_bay = 8, slot_level = 1, slot_row = 1, slot_position = 1
          WHERE site_id = '{SITE}' AND code = 'A-01-1';
         UPDATE location SET place_id = '9a7b0000-0000-0000-0000-000000000003', slot_side = 1, slot_bay = 1, slot_level = 1, slot_row = 1, slot_position = 1
          WHERE site_id = '{SITE}' AND code = 'DOCK-1';
         UPDATE location SET place_id = '9a7b0000-0000-0000-0000-000000000004', slot_side = 1, slot_bay = 1, slot_level = 1, slot_row = 1, slot_position = 1
          WHERE site_id = '{SITE}' AND code = 'PACK-1';"
    ))
    .await
    .expect("the floor");

    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);
    let walk = |app| {
        let auth = auth.clone();
        async move {
            common::ok_json(
                app,
                test::TestRequest::get().uri(&format!("/sites/{SITE}/picking")).insert_header(auth).to_request(),
                "the walk",
            )
            .await
        }
    };

    let screen: Value = walk(&app).await;
    let route = &screen["route"];
    assert_eq!(route["from"], "pack", "the bench is on the layout: {route}");
    assert!(route["stops"].as_i64().unwrap() >= 1, "{route}");
    let (walked, typed) = (route["walked"].as_f64().unwrap(), route["typed"].as_f64().unwrap());
    assert!(walked > 0.0 && walked <= typed + 1e-9, "the route walks no further than the typed order: {walked} against {typed}");
    let path = route["path"].as_array().unwrap();
    let first = &path[0];
    assert!(first[0].as_f64().unwrap() < 3.5 && first[1].as_f64().unwrap() < 3.5, "from the bench: {first}");
    assert_eq!(path[0], path[path.len() - 1], "and back to it");
    // A-01-1 is in bay 8 of ten along a rack from x 4: its face is at 11.5.
    assert!(
        path.iter().any(|p| (p[0].as_f64().unwrap() - 11.5).abs() <= 0.5 && p[1].as_f64().unwrap() < 4.0),
        "the walk reaches the aisle in front of A-01-1: {path:?}"
    );
    // Lines on the route come before those that aren't, and a bin's lines
    // come together.
    let codes: Vec<Option<&str>> = screen["lines"].as_array().unwrap().iter().map(|l| l["location_code"].as_str()).collect();
    let on = ["A-01-1", "DOCK-1", "PACK-1"];
    let last_on = codes.iter().rposition(|c| c.is_some_and(|c| on.contains(&c)));
    let first_off = codes.iter().position(|c| !c.is_some_and(|c| on.contains(&c)));
    if let (Some(a), Some(b)) = (last_on, first_off) {
        assert!(a < b, "off-route lines last: {codes:?}");
    }
    let off = codes.iter().filter(|c| !c.is_some_and(|c| on.contains(&c))).count() as i64;
    assert_eq!(route["off_route"].as_i64(), Some(off), "{route}");

    // ── the bench off the layout: from the typed order's first stop ──────
    db.batch_execute(&format!(
        "UPDATE location SET place_id = NULL, slot_bay = NULL, slot_level = NULL, slot_row = NULL, slot_position = NULL, slot_side = NULL
          WHERE site_id = '{SITE}' AND code = 'PACK-1';"
    ))
    .await
    .unwrap();
    let screen: Value = walk(&app).await;
    assert_eq!(screen["route"]["from"], "first", "{}", screen["route"]);

    // ── nothing on the layout: no route, the typed order as it was ───────
    clear(&db).await;
    let screen: Value = walk(&app).await;
    assert!(screen["route"].is_null(), "{}", screen["route"]);
}
