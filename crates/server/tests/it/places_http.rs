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
                             slot_row = NULL, slot_position = NULL
          WHERE site_id = '{SITE}';
         DELETE FROM location WHERE code LIKE 'LT-%';
         DELETE FROM place WHERE site_id = '{SITE}';"
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
    // and one bin whose code follows no pattern.
    db.batch_execute(&format!(
        "INSERT INTO location (tenant_id, site_id, code, kind, active)
         SELECT '{TENANT}', '{SITE}', c, 'pick_face', true
           FROM unnest(ARRAY['LT-01-1', 'LT-01-2', 'LT-03-1', 'LT-03-2', 'LT-FLOOR']) c;"
    ))
    .await
    .expect("the bins");

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

    // ── applied ──────────────────────────────────────────────────────────
    let (status, applied) = call(
        &app,
        test::TestRequest::post().uri("/layout/draft?apply=true").insert_header(auth.clone()),
    )
    .await;
    assert_eq!(status, 200, "{applied}");
    assert_eq!(applied["bins_placed"], dry["bins_placed"], "the dry run is the apply, undone");

    // ── a scanned bin lands on the face of its rack ─────────────────────
    let bin: String = db
        .query_one("SELECT id::text FROM location WHERE code = 'LT-03-2'", &[])
        .await
        .unwrap()
        .get(0);
    let (status, view) =
        call(&app, test::TestRequest::get().uri(&format!("/bins/{bin}")).insert_header(auth.clone())).await;
    assert_eq!(status, 200, "{view}");
    assert_eq!(view["code"], "LT-03-2");
    assert_eq!(view["cell"]["bay"], 2, "bay 03 is the second bay of a rack numbered by twos");
    assert_eq!(view["cell"]["level"], 2);
    let place = &view["place"];
    assert_eq!(place["name"], "Rack LT");
    assert_eq!(place["bay_labels"], serde_json::json!(["01", "03"]), "as the rack prints them");
    assert_eq!(place["level_labels"], serde_json::json!(["1", "2"]));
    assert_eq!(place["trail"][0]["name"], "Building", "the way out");
    assert_eq!(place["bins"].as_array().unwrap().len(), 4);
    let plan = place["plan"].as_array().unwrap();
    assert!(plan.iter().any(|s| s["name"] == "Building" && s["nesting"] == 0), "{place}");
    let me = plan.iter().find(|s| s["name"] == "Rack LT").expect("itself on the plan");
    assert_eq!(me["corners"].as_array().unwrap().len(), 4);

    // The building knows what is in it.
    let building = place["trail"][0]["place_id"].as_str().unwrap().to_string();
    let (_, inside) =
        call(&app, test::TestRequest::get().uri(&format!("/places/{building}")).insert_header(auth.clone())).await;
    let child = inside["children"].as_array().unwrap().iter().find(|c| c["name"] == "Rack LT").unwrap();
    assert_eq!(child["bins"], 4);

    // ── a bin that comes off its cell drops back in by its name ─────────
    db.batch_execute(
        "UPDATE location SET place_id = NULL, slot_bay = NULL, slot_level = NULL, slot_row = NULL,
                             slot_position = NULL
          WHERE code = 'LT-03-1';",
    )
    .await
    .unwrap();
    let (_, again) = call(
        &app,
        test::TestRequest::post().uri("/layout/draft?apply=true").insert_header(auth.clone()),
    )
    .await;
    assert_eq!(again["bins_filled"], 1, "{again}");
    assert_eq!(again["places"], Value::Array(vec![]), "nothing new to draw: {again}");

    let (_, after) = call(&app, test::TestRequest::get().uri("/layout").insert_header(auth.clone())).await;
    assert!(after["places"].as_array().unwrap().len() >= 2, "{after}");
    assert!(after["unplaced_sample"].as_array().unwrap().iter().any(|c| c == "LT-FLOOR"), "{after}");

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
