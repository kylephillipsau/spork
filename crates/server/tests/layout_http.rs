//! A site's layout over HTTP: racks place bins, and the floor has areas. D173.
//!
//! What this file is for is the reconciliation rather than the parse, which
//! `layout`'s own tests cover: that a dry run keeps nothing and says what
//! applying would do, that a measured box survives its rack, that a rack
//! re-imported shorter takes back the boxes it gave, and that two racks naming
//! one bin are refused rather than resolved by whichever was written last.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};

mod common;
use common::{pool, url, SITE};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

const HEAD: &str = "site,rack,aisle,x_mm,y_mm,rotation,depth_mm,height_mm,upright_mm,\
                    first_bay,bay_step,bay_widths_mm,level_z_mm,template\n";

/// One rack along aisle LT: bays numbered 1, 3, 5, … on this face, two levels.
fn racks(site: &str, bays: usize) -> String {
    format!("{HEAD}{site},LT-L,LT,10000,20000,0,1000,4000,100,1,2,{bays}x2000,0 2000,{{aisle}}-{{bay:02}}-{{level}}\n")
}

async fn clear(c: &tokio_postgres::Client) {
    c.batch_execute(
        "DELETE FROM floor_area WHERE code LIKE 'LT-%';
         DELETE FROM location WHERE aisle = 'LT';
         DELETE FROM rack WHERE aisle = 'LT';",
    )
    .await
    .expect("clear what an earlier run of this file left");
}

async fn bin_box(c: &tokio_postgres::Client, code: &str) -> Value {
    let r = c
        .query_one(
            "SELECT geometry_source, rack_id IS NOT NULL,
                    x_mm, y_mm, z_mm, length_mm, width_mm, height_mm
               FROM location WHERE code = $1",
            &[&code],
        )
        .await
        .expect("the bin");
    json!({
        "source": r.get::<_, Option<String>>(0),
        "on_rack": r.get::<_, bool>(1),
        "box": [r.get::<_, Option<i32>>(2), r.get::<_, Option<i32>>(3), r.get::<_, Option<i32>>(4),
                r.get::<_, Option<i32>>(5), r.get::<_, Option<i32>>(6), r.get::<_, Option<i32>>(7)],
    })
}

async fn post<S>(app: &S, uri: &str, bearer: &str, body: String) -> (u16, Value)
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let r = test::call_service(
        app,
        test::TestRequest::post()
            .uri(uri)
            .insert_header(("authorization", bearer.to_string()))
            .set_payload(body)
            .to_request(),
    )
    .await;
    let status = r.status().as_u16();
    let bytes = test::read_body(r).await;
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

#[actix_web::test]
async fn racks_place_the_bins_they_name_and_take_back_what_they_no_longer_do() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let db = common::connect(&u, false).await;
    clear(&db).await;
    let site: String = db
        .query_one("SELECT code FROM site WHERE id = $1::text::uuid", &[&SITE])
        .await
        .expect("the fixture's site")
        .get(0);

    // Six bins in aisle LT. LT-03-2 was measured, and LT-99-1 is in the aisle
    // but on no bay the rack has.
    db.batch_execute(&format!(
        "INSERT INTO location (tenant_id, site_id, code, aisle, bay, level, kind, active)
         SELECT '{TENANT}', '{SITE}', c, 'LT', split_part(c, '-', 2), split_part(c, '-', 3),
                'pick_face', true
           FROM unnest(ARRAY['LT-01-1', 'LT-01-2', 'LT-03-1', 'LT-03-2', 'LT-99-1']) c;
         UPDATE location
            SET geometry_source = 'survey',
                x_mm = 13000, y_mm = 20050, z_mm = 2010, length_mm = 1950,
                width_mm = 950, height_mm = 1900
          WHERE code = 'LT-03-2';"
    ))
    .await
    .expect("the bins");

    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let session = common::bearer(&app).await;

    // ── the layout is on the import path, so a session cannot load it ────
    let (status, _) = post(&app, "/import/racks", &session, racks(&site, 3)).await;
    assert_eq!(status, 401, "a session reached the rack import");

    let minted = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/tokens")
            .insert_header(("authorization", session.clone()))
            .set_json(json!({ "label": "the racks, from a test" }))
            .to_request(),
        "POST /tokens",
    )
    .await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());

    // ── a dry run says everything and keeps nothing ──────────────────────
    let (status, dry) = post(&app, "/import/racks", &token, racks(&site, 3)).await;
    assert_eq!(status, 200, "{dry}");
    assert_eq!(dry["loaded"]["applied"], false);
    assert_eq!(dry["loaded"]["racks_created"], 1);
    let s = &dry["loaded"]["sites"][0];
    assert_eq!(s["slots"], 6, "three bays of two levels");
    assert_eq!(s["bins_placed"], 3, "LT-01-1, LT-01-2 and LT-03-1: {s}");
    assert_eq!(s["bins_kept_measured"], 1, "LT-03-2 was surveyed: {s}");
    assert_eq!(s["slots_without_bin"], 2, "bay 5 has no bins on file: {s}");
    assert_eq!(s["slots_without_bin_sample"], json!(["LT-05-1", "LT-05-2"]));
    assert_eq!(s["bins_uncovered_sample"], json!(["LT-99-1"]), "J77, before it is a finding");
    let kept: i64 = db
        .query_one("SELECT count(*) FROM rack WHERE aisle = 'LT'", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(kept, 0, "a dry run wrote a rack");
    assert_eq!(bin_box(&db, "LT-01-1").await["source"], Value::Null);

    // ── applied: the boxes are where the rack says ───────────────────────
    let (status, applied) =
        post(&app, "/import/racks?apply=true", &token, racks(&site, 3)).await;
    assert_eq!(status, 200, "{applied}");
    assert!(applied["arrival"]["party_message_id"].is_string(), "the file is kept as it arrived");
    assert_eq!(
        bin_box(&db, "LT-01-1").await,
        json!({ "source": "template", "on_rack": true,
                "box": [10100, 20000, 0, 2000, 1000, 2000] }),
        "bay 1, level 1: after the first upright, the full depth, floor to level 2"
    );
    assert_eq!(
        bin_box(&db, "LT-03-1").await["box"],
        json!([12200, 20000, 0, 2000, 1000, 2000]),
        "bay 3 is the rack's second bay, one bay and one upright further along"
    );
    assert_eq!(
        bin_box(&db, "LT-03-2").await,
        json!({ "source": "survey", "on_rack": true,
                "box": [13000, 20050, 2010, 1950, 950, 1900] }),
        "a measured box is on its rack and is not redrawn"
    );

    // ── the same file again changes nothing ──────────────────────────────
    let (_, again) = post(&app, "/import/racks?apply=true", &token, racks(&site, 3)).await;
    assert_eq!(again["loaded"]["racks_unchanged"], 1, "{again}");
    let s = &again["loaded"]["sites"][0];
    assert_eq!((s["bins_placed"].as_i64(), s["bins_moved"].as_i64()), (Some(0), Some(0)), "{s}");

    // ── one bay shorter: bay 3's template box goes, the survey stays ─────
    let (_, shorter) = post(&app, "/import/racks?apply=true", &token, racks(&site, 1)).await;
    assert_eq!(shorter["loaded"]["racks_changed"], 1, "{shorter}");
    assert_eq!(shorter["loaded"]["sites"][0]["bins_unplaced"], 1, "{shorter}");
    assert_eq!(
        bin_box(&db, "LT-03-1").await,
        json!({ "source": null, "on_rack": false, "box": [null, null, null, null, null, null] }),
        "a bin no rack names does not keep a drawing of where it used to be"
    );
    let survey = bin_box(&db, "LT-03-2").await;
    assert_eq!((survey["source"].as_str(), survey["on_rack"].as_bool()), (Some("survey"), Some(false)));

    // ── two racks naming one bin is refused, whichever file it is in ─────
    let rival = format!(
        "{HEAD}{site},LT-R,LT,10000,25000,180,1000,4000,100,1,2,1x2000,0 2000,{{aisle}}-{{bay:02}}-{{level}}\n"
    );
    let (status, refused) = post(&app, "/import/racks?apply=true", &token, rival).await;
    assert_eq!(status, 400, "{refused}");
    assert!(refused.to_string().contains("LT-L and LT-R"), "{refused}");
    let racks_now: i64 = db
        .query_one("SELECT count(*) FROM rack WHERE aisle = 'LT'", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(racks_now, 1, "the refused rack was kept");

    // ── the floor ────────────────────────────────────────────────────────
    let pack: String = db
        .query_one(
            "SELECT code FROM location WHERE site_id = $1::text::uuid AND kind = 'staging' LIMIT 1",
            &[&SITE],
        )
        .await
        .expect("the fixture has a staging location")
        .get(0);
    let floor = format!(
        "site,area,kind,outline,height_mm,location\n\
         {site},LT-STAGE,staging,\"0,0 4000,0 4000,2000 0,2000\",,{pack}\n\
         {site},LT-WALL,wall,\"0,30000 40000,30000 40000,30200 0,30200\",5000,\n"
    );
    let (status, loaded) = post(&app, "/import/floor?apply=true", &token, floor).await;
    assert_eq!(status, 200, "{loaded}");
    assert_eq!(loaded["loaded"]["areas_created"], 2, "{loaded}");
    let linked: bool = db
        .query_one(
            "SELECT location_id IS NOT NULL FROM floor_area WHERE code = 'LT-STAGE'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert!(linked, "the staging area is the staging location");

    let nowhere = format!(
        "site,area,kind,outline,location\n{site},LT-DOCK,dock,\"0,0 10,0 10,10\",NO-SUCH-BIN\n"
    );
    let (status, refused) = post(&app, "/import/floor?apply=true", &token, nowhere).await;
    assert_eq!(status, 400, "{refused}");
    assert!(refused.to_string().contains("NO-SUCH-BIN"), "{refused}");

    let (status, refused) = post(
        &app,
        "/import/floor?apply=true",
        &token,
        "site,area,kind,outline\nNOWHERE,LT-X,dock,\"0,0 10,0 10,10\"\n".into(),
    )
    .await;
    assert_eq!(status, 400, "{refused}");
    assert!(refused.to_string().contains("never creates a warehouse"), "{refused}");

    // Leave nothing for the invariant suite to find.
    clear(&db).await;
}
