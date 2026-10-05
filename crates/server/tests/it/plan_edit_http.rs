//! The plan editor over HTTP (D209): a rack moved and turned, a wall drawn,
//! one act that a retry repeats without writing twice, a save against a layout
//! that has changed refused, a rack with bins in it kept, and a history of who
//! did what.
//!
//! Its own file because it draws on the fixture site's layout, as
//! `places_http` does, and tests in one file run side by side.

use actix_web::{test, web, App};
use serde_json::Value;
use spork_server::{routes, AppState};

use super::common;
use common::{pool, url, SITE};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

/// The fixture site with no layout, none of this file's bins, and no history.
async fn clear(c: &tokio_postgres::Client) {
    c.batch_execute(&format!(
        "UPDATE location SET place_id = NULL, slot_bay = NULL, slot_level = NULL,
                             slot_row = NULL, slot_position = NULL, slot_side = NULL
          WHERE site_id = '{SITE}';
         DELETE FROM location WHERE code LIKE 'PE-%';
         DELETE FROM place WHERE site_id = '{SITE}';
         DELETE FROM place_change WHERE site_id = '{SITE}';
         UPDATE site SET cell_mm = NULL WHERE id = '{SITE}';"
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

/// The plan editor's save (D209): a rack moved and turned, a wall drawn, one
/// act that a retry repeats without writing twice, a save against a layout
/// that has changed refused, and a rack with bins in it kept.
#[actix_web::test]
async fn the_plan_editor_moves_draws_and_keeps_its_history() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let db = common::connect(&u, false).await;
    clear(&db).await;
    db.batch_execute(&format!(
        "INSERT INTO place (id, tenant_id, site_id, name, x, y, length, depth, height)
         VALUES ('9e0e0000-0000-0000-0000-000000000001', '{TENANT}', '{SITE}', 'Building', 0, 0, 30, 20, 8);
         INSERT INTO place (id, tenant_id, site_id, parent_id, name, solid, x, y, length, depth, height,
                            bays, levels, sides, bin_pattern)
         VALUES ('9e0e0000-0000-0000-0000-000000000002', '{TENANT}', '{SITE}', '9e0e0000-0000-0000-0000-000000000001',
                 'Rack LE', true, 1, 1, 4, 2, 3, 4, 3, 2, 'LE-{{bay:02}}-{{level}}');
         INSERT INTO location (tenant_id, site_id, code, kind, active, place_id, slot_bay, slot_level, slot_row, slot_position, slot_side)
         VALUES ('{TENANT}', '{SITE}', 'PE-01-1', 'pick_face', true, '9e0e0000-0000-0000-0000-000000000002', 1, 1, 1, 1, 1);"
    ))
    .await
    .expect("a building with a rack in it");

    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);
    let read = |app| {
        let auth = auth.clone();
        async move { call(app, test::TestRequest::get().uri("/layout").insert_header(auth)).await.1 }
    };
    let save = |app, body: Value| {
        let auth = auth.clone();
        async move { call(app, test::TestRequest::post().uri("/layout/edit").insert_header(auth).set_json(body)).await }
    };

    let layout = read(&app).await;
    let version = layout["version"].as_str().expect("a version").to_string();
    let rack = layout["places"].as_array().unwrap().iter().find(|p| p["name"] == "Rack LE").unwrap().clone();
    assert_eq!((rack["x"].as_f64(), rack["length"].as_f64(), rack["outlined"].as_bool()), (Some(1.0), Some(4.0), Some(false)), "{rack}");

    // ── the rack moved and turned a quarter, and a wall drawn ───────────
    let event = uuid::Uuid::new_v4();
    let wall = uuid::Uuid::new_v4();
    let body = serde_json::json!({
        "client_event_id": event, "occurred_at": "2026-10-04T01:00:00Z", "version": version,
        "changed": [{ "place_id": rack["place_id"], "name": "Rack LE", "solid": true,
                      "x": 10, "y": 4, "length": 4, "depth": 2, "height": 3, "turn": 90 }],
        "added": [{ "place_id": wall, "parent_id": "9e0e0000-0000-0000-0000-000000000001", "name": "North wall",
                    "solid": true, "x": 0, "y": 19.5, "length": 30, "depth": 0.5, "height": 4 }],
    });
    let (status, saved) = save(&app, body.clone()).await;
    assert_eq!(status, 200, "{saved}");
    assert_eq!((saved["changed"].as_i64(), saved["added"].as_i64(), saved["replay"].as_bool()), (Some(1), Some(1), Some(false)), "{saved}");
    let after = read(&app).await;
    let moved = after["places"].as_array().unwrap().iter().find(|p| p["name"] == "Rack LE").unwrap();
    assert_eq!((moved["x"].as_f64(), moved["y"].as_f64(), moved["turn"].as_f64()), (Some(10.0), Some(4.0), Some(90.0)), "{moved}");
    let shape = after["plan"].as_array().unwrap().iter().find(|s| s["name"] == "Rack LE").unwrap();
    assert_eq!(shape["frame"]["turn"].as_f64(), Some(90.0), "{shape}");
    // Turned a quarter about its corner: its front now runs up the plan.
    let corners: Vec<(f64, f64)> = shape["corners"].as_array().unwrap().iter().map(|c| (c[0].as_f64().unwrap(), c[1].as_f64().unwrap())).collect();
    assert!((corners[1].0 - 10.0).abs() < 1e-9 && (corners[1].1 - 8.0).abs() < 1e-9, "{corners:?}");
    assert!(after["places"].as_array().unwrap().iter().any(|p| p["name"] == "North wall"));
    let bin: i64 = db
        .query_one("SELECT slot_bay FROM location WHERE code = 'PE-01-1' AND place_id = '9e0e0000-0000-0000-0000-000000000002'", &[])
        .await
        .unwrap()
        .get::<_, i32>(0) as i64;
    assert_eq!(bin, 1, "the bin went with its rack, in the same cell");

    // ── the same act again writes nothing twice ─────────────────────────
    let (status, again) = save(&app, body).await;
    assert_eq!(status, 200, "{again}");
    assert_eq!((again["added"].as_i64(), again["replay"].as_bool()), (Some(1), Some(true)), "{again}");
    let walls: i64 = db
        .query_one(&format!("SELECT count(*) FROM place WHERE site_id = '{SITE}' AND name = 'North wall'"), &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(walls, 1, "one wall");

    // ── against a layout that has changed since, refused ────────────────
    let (status, stale) = save(
        &app,
        serde_json::json!({ "client_event_id": uuid::Uuid::new_v4(), "occurred_at": "2026-10-04T01:01:00Z",
                            "version": version, "removed": [wall] }),
    )
    .await;
    assert_eq!(status, 400, "{stale}");
    assert!(stale.to_string().contains("changed since"), "{stale}");

    // ── a rack with bins stays; a two-sided rack stays solid; a wall goes ─
    let now = saved["version"].as_str().unwrap().to_string();
    let (status, kept) = save(
        &app,
        serde_json::json!({ "client_event_id": uuid::Uuid::new_v4(), "occurred_at": "2026-10-04T01:02:00Z",
                            "version": now, "removed": [rack["place_id"]] }),
    )
    .await;
    assert_eq!(status, 400, "{kept}");
    assert!(kept.to_string().contains("1 bin in it"), "{kept}");
    let (status, flat) = save(
        &app,
        serde_json::json!({ "client_event_id": uuid::Uuid::new_v4(), "occurred_at": "2026-10-04T01:03:00Z", "version": now,
                            "changed": [{ "place_id": rack["place_id"], "name": "Rack LE", "solid": false,
                                          "x": 10, "y": 4, "length": 4, "depth": 2, "height": 3, "turn": 90 }] }),
    )
    .await;
    assert_eq!(status, 400, "{flat}");
    let (status, gone) = save(
        &app,
        serde_json::json!({ "client_event_id": uuid::Uuid::new_v4(), "occurred_at": "2026-10-04T01:04:00Z",
                            "version": now, "removed": [wall] }),
    )
    .await;
    assert_eq!(status, 200, "{gone}");

    // ── a bin from the tray, put on the plan as a spot of its own (D211) ─
    db.batch_execute(&format!(
        "INSERT INTO location (tenant_id, site_id, code, kind, active)
         VALUES ('{TENANT}', '{SITE}', 'PE-SPOT', 'staging', true);"
    ))
    .await
    .expect("a bin on no layout");
    let spot: String = db
        .query_one("SELECT id::text FROM location WHERE code = 'PE-SPOT'", &[])
        .await
        .unwrap()
        .get(0);
    let now = read(&app).await["version"].as_str().unwrap().to_string();
    let place_it = serde_json::json!({
        "client_event_id": uuid::Uuid::new_v4(), "occurred_at": "2026-10-04T01:05:00Z", "version": now,
        "spots": [{ "place_id": uuid::Uuid::new_v4(), "location_id": spot, "parent_id": "9e0e0000-0000-0000-0000-000000000001",
                    "solid": false, "x": 2, "y": 2, "length": 2, "depth": 2, "height": 1 }],
    });
    let (status, placed) = save(&app, place_it.clone()).await;
    assert_eq!(status, 200, "{placed}");
    assert_eq!((placed["placed"].as_i64(), placed["added"].as_i64()), (Some(1), Some(0)), "{placed}");
    let cell = db
        .query_one("SELECT p.name, l.slot_bay, l.slot_level FROM location l JOIN place p ON p.id = l.place_id WHERE l.code = 'PE-SPOT'", &[])
        .await
        .expect("it is in a cell now");
    assert_eq!((cell.get::<_, String>(0), cell.get::<_, i32>(1), cell.get::<_, i32>(2)), ("PE-SPOT".into(), 1, 1));
    let (_, again) = save(&app, place_it).await;
    assert_eq!((again["placed"].as_i64(), again["replay"].as_bool()), (Some(1), Some(true)), "a retry places nothing twice: {again}");
    let now = read(&app).await["version"].as_str().unwrap().to_string();
    let (status, twice) = save(
        &app,
        serde_json::json!({
            "client_event_id": uuid::Uuid::new_v4(), "occurred_at": "2026-10-04T01:06:00Z", "version": now,
            "spots": [{ "place_id": uuid::Uuid::new_v4(), "location_id": spot, "parent_id": null,
                        "solid": false, "x": 5, "y": 5, "length": 2, "depth": 2, "height": 1 }],
        }),
    )
    .await;
    assert_eq!(status, 400, "a bin on the layout stays where it is: {twice}");
    db.batch_execute("DELETE FROM place_change WHERE after ? 'bin'").await.unwrap();

    // ── the scale: a metre to a cell, said once (D210) ──────────────────
    assert!(read(&app).await["cell_mm"].is_null(), "not to scale until it is said");
    let scale = |app, mm: i64| {
        let auth = auth.clone();
        async move {
            call(app, test::TestRequest::post().uri("/layout/scale").insert_header(auth).set_json(serde_json::json!({ "cell_mm": mm }))).await
        }
    };
    let (status, set) = scale(&app, 1000).await;
    assert_eq!(status, 200, "{set}");
    assert_eq!(read(&app).await["cell_mm"], 1000);
    assert_eq!(scale(&app, 1000).await.0, 200, "saying it again is no change");
    let (status, refused) = scale(&app, 900).await;
    assert_eq!(status, 400, "a scale places were measured in stays: {refused}");
    assert_eq!(scale(&app, 0).await.0, 400);

    // ── and the history says who did what ───────────────────────────────
    let history = db
        .query(
            &format!(
                "SELECT pc.change, ce.recorded_by_id IS NOT NULL, pc.before->>'x', pc.after->>'x'
                   FROM place_change pc
                   JOIN client_event ce ON ce.tenant_id = pc.tenant_id AND ce.client_event_id = pc.client_event_id
                  WHERE pc.site_id = '{SITE}' ORDER BY pc.id"
            ),
            &[],
        )
        .await
        .unwrap();
    let kinds: Vec<(String, bool, Option<String>, Option<String>)> = history.iter().map(|r| (r.get(0), r.get(1), r.get(2), r.get(3))).collect();
    assert_eq!(
        kinds,
        vec![
            ("changed".into(), true, Some("1.0".into()), Some("10.0".into())),
            ("added".into(), true, None, Some("0.0".into())),
            ("removed".into(), true, Some("0.0".into()), None),
        ],
        "moved from 1 to 10, the wall drawn, then taken away"
    );

    // ── a rack numbered from its other end (D220) ───────────────────────
    // Rack PF read from the left: PE-F01 and PE-F02 along the front, PE-F03
    // and PE-F04 back along the other side, PE-F04 behind PE-F01. Numbered
    // from the right, every bin keeps its name and goes to the mirror of its
    // column, which is where that name is now.
    db.batch_execute(&format!(
        "INSERT INTO place (id, tenant_id, site_id, parent_id, name, solid, x, y, length, depth, height,
                            bays, levels, sides, bin_pattern)
         VALUES ('9e0e0000-0000-0000-0000-000000000003', '{TENANT}', '{SITE}', '9e0e0000-0000-0000-0000-000000000001',
                 'Rack PF', true, 1, 12, 2, 2, 1, 2, 1, 2, 'PE-F{{bay:02}}');
         INSERT INTO location (tenant_id, site_id, code, kind, active, place_id, slot_bay, slot_level, slot_row, slot_position, slot_side)
         SELECT '{TENANT}', '{SITE}', v.code, 'pick_face', true, '9e0e0000-0000-0000-0000-000000000003', v.bay, 1, 1, 1, v.side
           FROM (VALUES ('PE-F01', 1, 1::int2), ('PE-F02', 2, 1::int2), ('PE-F03', 2, 2::int2), ('PE-F04', 1, 2::int2)) v(code, bay, side);"
    ))
    .await
    .expect("a rack with two sides, numbered from the left");
    let numbered = |view: &Value, name: &str, from_right: bool| {
        let p = view["places"].as_array().unwrap().iter().find(|p| p["name"] == name).unwrap().clone();
        serde_json::json!({
            "client_event_id": uuid::Uuid::new_v4(), "occurred_at": "2026-10-06T01:00:00Z", "version": view["version"],
            "changed": [{ "place_id": p["place_id"], "name": name, "solid": p["solid"], "x": p["x"], "y": p["y"], "z": p["z"],
                          "length": p["length"], "depth": p["depth"], "height": p["height"], "turn": p["turn"],
                          "from_right": from_right }],
        })
    };
    let layout = read(&app).await;
    let pf = layout["places"].as_array().unwrap().iter().find(|p| p["name"] == "Rack PF").unwrap().clone();
    assert_eq!(pf["from_right"], false, "{pf}");
    let (status, saved) = save(&app, numbered(&layout, "Rack PF", true)).await;
    assert_eq!(status, 200, "{saved}");
    assert_eq!(
        cells_of(&db, "PE-F%").await,
        vec![("PE-F01".into(), 2, 1), ("PE-F02".into(), 1, 1), ("PE-F03".into(), 1, 2), ("PE-F04".into(), 2, 2)],
        "PE-F01 at the front's right end, PE-F04 behind it"
    );
    let (_, page) = call(&app, test::TestRequest::get().uri(&format!("/places/{}", pf["place_id"].as_str().unwrap())).insert_header(auth.clone())).await;
    assert_eq!((page["bay_labels"].clone(), page["back_labels"].clone()), (serde_json::json!(["02", "01"]), serde_json::json!(["04", "03"])), "{page}");
    let after = read(&app).await;
    assert_eq!(after["places"].as_array().unwrap().iter().find(|p| p["name"] == "Rack PF").unwrap()["from_right"], true);
    let kept = db
        .query_one(
            "SELECT before->>'from_right', after->>'from_right' FROM place_change
              WHERE place_id = '9e0e0000-0000-0000-0000-000000000003'",
            &[],
        )
        .await
        .expect("kept in the history");
    assert_eq!((kept.get::<_, Option<String>>(0), kept.get::<_, Option<String>>(1)), (Some("false".into()), Some("true".into())));

    // Rack LE holds PE-01-1, which its pattern does not name: refused, and
    // nothing moves.
    let (status, refused) = save(&app, numbered(&after, "Rack LE", true)).await;
    assert_eq!(status, 400, "{refused}");
    assert!(refused.to_string().contains("PE-01-1"), "{refused}");
    assert_eq!(cells_of(&db, "PE-01-%").await, vec![("PE-01-1".into(), 1, 1)]);

    clear(&db).await;
}

/// Bins whose codes are like this, with the column and side each is in.
async fn cells_of(db: &tokio_postgres::Client, like: &str) -> Vec<(String, i32, i16)> {
    db.query("SELECT code, slot_bay, slot_side FROM location WHERE code LIKE $1 ORDER BY code", &[&like])
        .await
        .unwrap()
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get(2)))
        .collect()
}
