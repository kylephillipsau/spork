//! Who packed what, as NetSuite says (D254): the Bridge's report of packed
//! fulfilments, loaded twice, and an administrator's counts per person.
//!
//! The first load of a feed reports weeks gone, so its rows are dated by the
//! fulfilment's own date; a row new in a later load was packed since the last
//! one. A row is kept once seen, and its Packed By follows NetSuite.
//!
//! The packers' names are this run's own, so the counts are exactly this
//! file's whatever else the site holds.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};

use super::common;
use common::{pool, url, SITE};

#[actix_web::test]
async fn who_packed_what_is_counted_by_the_site_s_day_week_and_month() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let db = common::connect(&u, false).await;
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let session = common::bearer(&app).await;
    let minted = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/tokens")
            .insert_header(("authorization", common::bearer_without_site(&app).await))
            .set_json(json!({ "label": "packed, from a test" }))
            .to_request(),
        "minting an import token",
    )
    .await;
    let machine = format!("Bearer {}", minted["token"].as_str().unwrap());

    let run = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() % 1_000_000;
    let (a, b) = (format!("Packer A {run}"), format!("Packer B {run}"));
    // The site's own today, and days before it, as NetSuite writes a date.
    let day = |back: i32| {
        let db = &db;
        async move {
            db.query_one(
                "SELECT to_char((now() AT TIME ZONE timezone)::date - $2::int, 'FMDD/FMMM/YYYY') FROM site WHERE id = $1::text::uuid",
                &[&SITE, &back],
            )
            .await
            .unwrap()
            .get::<_, String>(0)
        }
    };
    let (today, ten, twenty, forty) = (day(0).await, day(10).await, day(20).await, day(40).await);

    let source = format!("test-packed-{run}");
    let load = |rows: String| {
        let as_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let req = test::TestRequest::post()
            .uri(&format!("/import/packed?as_at={as_at}&source={source}&apply=true"))
            .insert_header(("authorization", machine.clone()))
            .insert_header(("content-type", "text/csv"))
            .set_payload(format!("Fulfilment,Fulfilment ID,Date,Packed By,Location\n{rows}"))
            .to_request();
        common::ok_json(&app, req, "POST /import/packed")
    };

    // The first load: weeks of it, dated by each fulfilment's date. Forty
    // days ago is outside every span.
    let first = load(format!(
        "IF{run}1,9{run}1,{today},{a},Melbourne Warehouse\n\
         IF{run}2,9{run}2,{ten},{a},Melbourne Warehouse\n\
         IF{run}3,9{run}3,{forty},{b},Melbourne Warehouse\n"
    ))
    .await;
    assert_eq!(first["loaded"]["fulfilments_new"], 3, "{first}");

    // The next: one packed since, though dated weeks ago, and the second's
    // Packed By corrected in NetSuite.
    let next = load(format!(
        "IF{run}1,9{run}1,{today},{a},Melbourne Warehouse\n\
         IF{run}2,9{run}2,{ten},{b},Melbourne Warehouse\n\
         IF{run}3,9{run}3,{forty},{b},Melbourne Warehouse\n\
         IF{run}4,9{run}4,{twenty},{b},Melbourne Warehouse\n"
    ))
    .await;
    assert_eq!((&next["loaded"]["fulfilments_new"], &next["loaded"]["fulfilments_known"]), (&json!(1), &json!(3)), "{next}");

    let read = common::ok_json(
        &app,
        test::TestRequest::get().uri("/packed-by").insert_header(("authorization", session.clone())).to_request(),
        "GET /packed-by",
    )
    .await;
    let counts = |who: &str| -> Value {
        read["packers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["packed_by"] == who)
            .map(|p| json!([p["today"], p["this_week"], p["last_30_days"]]))
            .unwrap_or(Value::Null)
    };
    assert_eq!(counts(&a), json!([1, 1, 1]), "today's, and the one moved to B is B's: {read}");
    assert_eq!(counts(&b), json!([1, 1, 2]), "packed since the last load is today's, whatever its date: {read}");
    assert!(read["as_at"].is_string(), "{read}");
}
