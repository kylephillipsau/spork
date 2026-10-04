//! A site's layout over HTTP: drafted from the bin list, and read back. D173.
//!
//! What this is for is the path a warehouse actually takes: bins on file and no
//! layout, a draft somebody previews and then applies, and a scanned bin that
//! lands on the face of the rack that holds it. The draft's own judgement is
//! tested without a database in `layout`; this is the rows.

use actix_web::{test, web, App};
use serde_json::Value;
use spork_server::{routes, AppState};

use super::common;
use common::{pool, url, SITE};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

/// Take the fixture's site back to having no layout, and none of this file's
/// bins. A layout left behind would put every other suite's bins under J77.
async fn clear(c: &tokio_postgres::Client) {
    c.batch_execute(&format!(
        "UPDATE location SET place_id = NULL, slot_bay = NULL, slot_level = NULL,
                             slot_row = NULL, slot_position = NULL, slot_side = NULL
          WHERE site_id = '{SITE}';
         DELETE FROM reported_stock WHERE item_id IN (SELECT id FROM item WHERE code LIKE 'LT-ITEM-%');
         DELETE FROM item WHERE code LIKE 'LT-ITEM-%';
         DELETE FROM location WHERE code LIKE 'LT-%' OR code LIKE 'LX-%' OR code LIKE 'LW-%';
         DELETE FROM place WHERE site_id = '{SITE}';
         DELETE FROM place_change WHERE site_id = '{SITE}';"
    ))
    .await
    .expect("clear the layout");
}

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

#[actix_web::test]
async fn a_draft_lays_out_the_bin_list_and_a_bin_lands_on_its_rack() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let db = common::connect(&u, false).await;
    clear(&db).await;

    // A rack's worth of bins on odd bays, as one side of an aisle is numbered,
    // one bin whose code follows no pattern, a lone code that reads as a rack
    // of its own and is no rack at all, and a rack with a face on each side,
    // numbered round it: LW-01 and LW-02 along the front, LW-03 and LW-04
    // back along the other side.
    db.batch_execute(&format!(
        "INSERT INTO location (tenant_id, site_id, code, kind, active)
         SELECT '{TENANT}', '{SITE}', c, 'pick_face', true
           FROM unnest(ARRAY['LT-01-1', 'LT-01-2', 'LT-03-1', 'LT-03-2', 'LT-FLOOR', 'LX-9-01',
                             'LW-01-1', 'LW-02-1', 'LW-03-1', 'LW-04-1']) c;"
    ))
    .await
    .expect("the bins");
    // What NetSuite last said is on one of them: two items, the fewer first.
    db.batch_execute(&format!(
        "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
         SELECT '{TENANT}', c, 'For the bins list', u.id, 'none'
           FROM unit u, unnest(ARRAY['LT-ITEM-1', 'LT-ITEM-2']) c WHERE u.code = 'ea';
         INSERT INTO reported_stock (tenant_id, site_id, item_id, location_id, on_hand, as_at, source)
         SELECT '{TENANT}', '{SITE}', i.id, l.id, v.q, now(), 'test'
           FROM (VALUES ('LT-ITEM-1', 4), ('LT-ITEM-2', 30)) v(code, q)
           JOIN item i ON i.tenant_id = '{TENANT}' AND i.code = v.code
           JOIN location l ON l.site_id = '{SITE}' AND l.code = 'LT-03-2';"
    ))
    .await
    .expect("the report");

    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);

    // ── nothing drawn yet ────────────────────────────────────────────────
    let (status, before) = call(&app, test::TestRequest::get().uri("/layout").insert_header(auth.clone())).await;
    assert_eq!(status, 200, "{before}");
    assert_eq!(before["places"], Value::Array(vec![]), "{before}");
    assert_eq!(before["unplaced"], before["bins"], "every bin is waiting: {before}");

    // ── a dry run says what it would draw and keeps nothing ─────────────
    let (status, dry) =
        call(&app, test::TestRequest::post().uri("/layout/draft").insert_header(auth.clone())).await;
    assert_eq!(status, 200, "{dry}");
    assert_eq!(dry["applied"], false);
    assert_eq!(dry["inside"], "Building");
    assert_eq!(dry["inside_created"], true);
    let lt = dry["places"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Rack LT")
        .unwrap_or_else(|| panic!("no Rack LT in {dry}"));
    assert_eq!(lt["pattern"], "LT-{bay:02}-{level}");
    assert_eq!((lt["bays"].as_i64(), lt["levels"].as_i64(), lt["bins"].as_i64()), (Some(2), Some(2), Some(4)));
    assert!(
        dry["unplaced_sample"].as_array().unwrap().iter().any(|c| c == "LT-FLOOR"),
        "a code that follows no pattern waits to be placed by hand: {dry}"
    );
    let kept: i64 = db
        .query_one(&format!("SELECT count(*) FROM place WHERE site_id = '{SITE}'"), &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(kept, 0, "a dry run drew something");
    assert!(
        dry["places"].as_array().unwrap().iter().any(|p| p["name"] == "Rack LX" && p["bins"] == 1),
        "the lone code is proposed as a rack: {dry}"
    );
    let lw = dry["places"].as_array().unwrap().iter().find(|p| p["name"] == "Rack LW").expect("Rack LW");
    assert_eq!(lw["split"], serde_json::json!(["01–02", "03–04"]), "what two sides would hold: {lw}");
    assert!(
        dry["places"].as_array().unwrap().iter().find(|p| p["name"] == "Rack LX").unwrap()["split"].is_null(),
        "one bay has one side"
    );

    // ── a place the person knows is no rack is left out, and a rack with
    //    two sides is made as two ─────────────────────────────────────────
    let leave = serde_json::json!({ "leave_out": ["Rack LX"], "two_sided": ["Rack LW"] });
    let (status, stale) = call(
        &app,
        test::TestRequest::post()
            .uri("/layout/draft?apply=true")
            .insert_header(auth.clone())
            .set_json(serde_json::json!({ "leave_out": ["Rack Nowhere"] })),
    )
    .await;
    assert_eq!(status, 400, "a name the draft does not propose is refused: {stale}");
    assert!(stale.to_string().contains("preview it again"), "{stale}");
    let kept: i64 = db
        .query_one(&format!("SELECT count(*) FROM place WHERE site_id = '{SITE}'"), &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(kept, 0, "a refused draft drew something");

    let (_, dry) = call(
        &app,
        test::TestRequest::post().uri("/layout/draft").insert_header(auth.clone()).set_json(leave.clone()),
    )
    .await;
    assert_eq!(dry["left_out"], serde_json::json!(["Rack LX"]), "{dry}");
    assert!(!dry["places"].as_array().unwrap().iter().any(|p| p["name"] == "Rack LX"), "{dry}");
    assert!(
        dry["unplaced_sample"].as_array().unwrap().iter().any(|c| c == "LX-9-01"),
        "its bin waits in the tray: {dry}"
    );

    // ── applied ──────────────────────────────────────────────────────────
    let (status, applied) = call(
        &app,
        test::TestRequest::post()
            .uri("/layout/draft?apply=true")
            .insert_header(auth.clone())
            .set_json(leave.clone()),
    )
    .await;
    assert_eq!(status, 200, "{applied}");
    assert_eq!(applied["bins_placed"], dry["bins_placed"], "the dry run is the apply, undone");
    assert_eq!(applied["unplaced"], dry["unplaced"]);
    let lx: i64 = db
        .query_one(&format!("SELECT count(*) FROM place WHERE site_id = '{SITE}' AND name = 'Rack LX'"), &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(lx, 0, "a place left out was made");

    // Rack LW is one rack two columns long with a face on each side: LW-04 on
    // the back of the first column, behind LW-01, and the back reading 03, 04
    // as you face it.
    let made = applied["places"].as_array().unwrap();
    let lw = made.iter().find(|p| p["name"] == "Rack LW").expect("Rack LW, once");
    assert_eq!((lw["bays"].as_i64(), lw["sides"].as_i64(), lw["bins"].as_i64()), (Some(2), Some(2), Some(4)), "{lw}");
    let back: String = db
        .query_one("SELECT id::text FROM location WHERE code = 'LW-04-1'", &[])
        .await
        .unwrap()
        .get(0);
    let (_, lw4) =
        call(&app, test::TestRequest::get().uri(&format!("/bins/{back}")).insert_header(auth.clone())).await;
    assert_eq!(lw4["place"]["name"], "Rack LW", "{lw4}");
    assert_eq!(lw4["place"]["sides"], 2, "{lw4}");
    assert_eq!((lw4["cell"]["bay"].as_i64(), lw4["cell"]["side"].as_i64()), (Some(1), Some(2)), "behind LW-01: {lw4}");
    assert_eq!(lw4["place"]["bay_labels"], serde_json::json!(["01", "02"]), "{lw4}");
    assert_eq!(lw4["place"]["back_labels"], serde_json::json!(["03", "04"]), "{lw4}");
    let front: String = db
        .query_one("SELECT id::text FROM location WHERE code = 'LW-01-1'", &[])
        .await
        .unwrap()
        .get(0);
    let (_, lw1) =
        call(&app, test::TestRequest::get().uri(&format!("/bins/{front}")).insert_header(auth.clone())).await;
    assert_eq!((lw1["cell"]["bay"].as_i64(), lw1["cell"]["side"].as_i64()), (Some(1), Some(1)), "{lw1}");
    let rack_lw = lw4["place"]["place_id"].as_str().unwrap().to_string();
    let (_, listed) =
        call(&app, test::TestRequest::get().uri(&format!("/bins?place={rack_lw}")).insert_header(auth.clone())).await;
    let four = listed["bins"].as_array().unwrap().iter().find(|b| b["code"] == "LW-04-1").unwrap();
    assert_eq!(four["whereabouts"], "back, bay 04", "in the words the rack's labels use: {four}");

    // ── a bin that comes off its cell drops back in by its name ─────────
    db.batch_execute(
        "UPDATE location SET place_id = NULL, slot_bay = NULL, slot_level = NULL, slot_row = NULL,
                             slot_position = NULL, slot_side = NULL
          WHERE code = 'LT-03-1';",
    )
    .await
    .unwrap();
    // Rack LW is made, so the next draft proposes only what is still waiting.
    let (_, again) = call(
        &app,
        test::TestRequest::post()
            .uri("/layout/draft?apply=true")
            .insert_header(auth.clone())
            .set_json(serde_json::json!({ "leave_out": ["Rack LX"] })),
    )
    .await;
    assert_eq!(again["bins_filled"], 1, "{again}");
    assert_eq!(again["places"], Value::Array(vec![]), "nothing new to draw: {again}");

    let (_, after) = call(&app, test::TestRequest::get().uri("/layout").insert_header(auth.clone())).await;
    assert!(after["places"].as_array().unwrap().len() >= 2, "{after}");
    assert!(after["unplaced_sample"].as_array().unwrap().iter().any(|c| c == "LT-FLOOR"), "{after}");

    // ── the whole site as a plan: what the warehouse view draws ─────────
    let site_plan = after["plan"].as_array().expect("the site's plan");
    assert!(site_plan.iter().any(|s| s["name"] == "Building" && s["nesting"] == 0), "{after}");
    let rack = site_plan.iter().find(|s| s["name"] == "Rack LT").expect("the rack on the site's plan");
    assert_eq!(rack["nesting"], 1, "inside the building");
    assert_eq!(rack["corners"].as_array().unwrap().len(), 4);
    assert!(rack["height"].as_f64().unwrap() > 0.0, "tall enough to draw in 3D");

    // ── the bins list: one place's, the tray's, and a search ────────────
    let rack_id = rack["place_id"].as_str().unwrap();
    let (status, in_rack) =
        call(&app, test::TestRequest::get().uri(&format!("/bins?place={rack_id}")).insert_header(auth.clone())).await;
    assert_eq!(status, 200, "{in_rack}");
    let bins = in_rack["bins"].as_array().unwrap();
    assert_eq!(bins.len(), 4, "{in_rack}");
    let b = bins.iter().find(|b| b["code"] == "LT-03-2").unwrap();
    assert_eq!(b["whereabouts"], "bay 03, level 2", "in the words the rack's labels use");
    assert_eq!(b["place_name"], "Rack LT");
    assert_eq!(b["reported_items"], 2, "{b}");
    assert_eq!(b["reported"][0]["item_code"], "LT-ITEM-2", "the most first: {b}");
    assert_eq!(b["reported"][0]["on_hand"], "30");
    assert_eq!(b["held"], 0);
    let bare = bins.iter().find(|b| b["code"] == "LT-01-1").unwrap();
    assert_eq!(bare["reported_items"], 0, "nothing reported is none, not an error: {bare}");
    assert_eq!(bare["reported"], Value::Array(vec![]));
    let (_, tray) = call(&app, test::TestRequest::get().uri("/bins?unplaced=true").insert_header(auth.clone())).await;
    let tray = tray["bins"].as_array().unwrap();
    assert!(tray.iter().any(|b| b["code"] == "LT-FLOOR"), "the bins the draft left");
    assert!(tray.iter().any(|b| b["code"] == "LX-9-01"), "and the bin whose place was left out");
    assert!(tray.iter().all(|b| b["place_id"].is_null() && b["whereabouts"].is_null()));
    let (_, searched) = call(&app, test::TestRequest::get().uri("/bins?q=LT-01").insert_header(auth.clone())).await;
    let searched = searched["bins"].as_array().unwrap();
    assert!(!searched.is_empty() && searched.iter().all(|b| b["code"].as_str().unwrap().contains("LT-01")));

    // ── reach: the floor bin before the bigger pile up high (D180) ──────
    // LT-ITEM-2: 30 on level 2, and 5 more on level 1.
    db.batch_execute(&format!(
        "INSERT INTO reported_stock (tenant_id, site_id, item_id, location_id, on_hand, as_at, source)
         SELECT '{TENANT}', '{SITE}', i.id, l.id, 5, now(), 'test'
           FROM item i JOIN location l ON l.site_id = '{SITE}' AND l.code = 'LT-03-1'
          WHERE i.tenant_id = '{TENANT}' AND i.code = 'LT-ITEM-2';"
    ))
    .await
    .expect("a few on the floor level");
    let pick_from = |app| {
        let auth = auth.clone();
        async move {
            let (status, page) = call(app, test::TestRequest::get().uri("/items?q=LT-ITEM-2").insert_header(auth)).await;
            assert_eq!(status, 200, "{page}");
            let row = page["items"][0].clone();
            (row["bin_code"].as_str().unwrap_or_default().to_string(), row["bin_within_reach"].clone())
        }
    };
    assert_eq!(b["within_reach"], false, "level 2 is above the default reach: {b}");
    assert_eq!(bins.iter().find(|b| b["code"] == "LT-03-1").unwrap()["within_reach"], true);
    assert!(
        tray.iter().find(|b| b["code"] == "LT-FLOOR").unwrap()["within_reach"] == true,
        "a bin off the layout is in reach when the other system calls it a pick bin"
    );
    assert_eq!(pick_from(&app).await, ("LT-03-1".into(), Value::Bool(true)), "the floor bin, though it holds less");

    let reach = |levels: i64| {
        let auth = auth.clone();
        let app = &app;
        async move {
            call(
                app,
                test::TestRequest::post()
                    .uri(&format!("/places/{rack_id}/reach"))
                    .insert_header(auth)
                    .set_json(serde_json::json!({ "levels": levels })),
            )
            .await
        }
    };
    let (status, said) = reach(2).await;
    assert_eq!(status, 200, "{said}");
    assert_eq!((said["reach_levels"].as_i64(), said["levels"].as_i64()), (Some(2), Some(2)));
    let (_, page) = call(&app, test::TestRequest::get().uri(&format!("/places/{rack_id}")).insert_header(auth.clone())).await;
    assert_eq!(page["reach_levels"], 2, "{page}");
    assert_eq!(pick_from(&app).await, ("LT-03-2".into(), Value::Bool(true)), "both in reach: the bigger pile");
    let (status, refused) = reach(3).await;
    assert_eq!(status, 400, "a rack of two levels has no third in reach: {refused}");
    let (status, _) = reach(0).await;
    assert_eq!(status, 200, "none of it in reach");
    assert_eq!(pick_from(&app).await, ("LT-03-2".into(), Value::Bool(false)), "none in reach: the biggest pile, said so");

    // ── the bin map: every placed bin at once, with its cell and its counts (D208)
    let (status, map) = call(&app, test::TestRequest::get().uri("/layout/bins").insert_header(auth.clone())).await;
    assert_eq!(status, 200, "{map}");
    let drawn = map["bins"].as_array().unwrap();
    let lw4 = drawn.iter().find(|b| b["code"] == "LW-04-1").expect("a bin on the back of a rack");
    assert_eq!(
        (lw4["side"].as_i64(), lw4["bay"].as_i64(), lw4["level"].as_i64()),
        (Some(2), Some(1), Some(1)),
        "behind LW-01, in column 1: {lw4}"
    );
    let floor = drawn.iter().find(|b| b["code"] == "LT-03-1").unwrap();
    assert_eq!(floor["reported_items"], 1, "{floor}");
    assert_eq!(floor["reported_on_hand"], 5.0, "{floor}");
    assert_eq!(floor["within_reach"], false, "none of the rack is in reach now: {floor}");
    assert!(drawn.iter().all(|b| b["code"] != "LT-FLOOR"), "a bin in no cell is not drawn");
    assert!(map["unplaced"].as_i64().unwrap() >= 1, "and is counted instead: {}", map["unplaced"]);
    let (_, laid) = call(&app, test::TestRequest::get().uri("/layout").insert_header(auth.clone())).await;
    let lw_place = laid["places"].as_array().unwrap().iter().find(|p| p["name"] == "Rack LW").unwrap();
    assert_eq!(lw_place["positions"], serde_json::json!([1]), "one bin to a bay on its one level: {lw_place}");

    // ── what is not there says so ────────────────────────────────────────
    let (status, _) = call(
        &app,
        test::TestRequest::get().uri(&format!("/bins/{}", uuid::Uuid::new_v4())).insert_header(auth.clone()),
    )
    .await;
    assert_eq!(status, 404);
    let nowhere = ("authorization", common::bearer_without_site(&app).await);
    let (status, refused) = call(&app, test::TestRequest::get().uri("/layout").insert_header(nowhere)).await;
    assert_eq!(status, 400, "{refused}");
    assert!(refused.to_string().contains("warehouse"), "{refused}");

    clear(&db).await;
}
