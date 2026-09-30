//! An item's page over HTTP: what it is, and where each of two records says it
//! is. The page's point is that the two stay apart: this system's own ledger,
//! and a report exported from NetSuite, which carries its age.

use actix_web::{test, web, App};
use serde_json::Value;
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url, SITE};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

#[actix_web::test]
async fn an_item_says_what_it_is_and_where_netsuite_reported_it() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let db = common::connect(&u, false).await;

    let item: Uuid = db
        .query_one(
            "SELECT id FROM item WHERE tenant_id = $1::text::uuid AND code = 'GLOVE-M'",
            &[&TENANT],
        )
        .await
        .expect("the fixture's glove")
        .get(0);

    // A report under a feed of this run's own, so a parallel suite's rows are
    // not read as this one's, on a shelf and at the warehouse with none.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let source = format!("test-item-page-{}", nonce % 1_000_000);
    db.execute(
        &format!(
            "INSERT INTO reported_stock
                 (tenant_id, site_id, item_id, location_id, on_hand, available, status, as_at, source)
             SELECT '{TENANT}'::uuid, '{SITE}'::uuid, $1::uuid, l.id, 7.5, 7.5, 'Good',
                    '2026-09-30T00:00:00Z'::timestamptz, $2::text
               FROM location l WHERE l.site_id = '{SITE}' AND l.code = 'A-01-1'
             UNION ALL
             SELECT '{TENANT}'::uuid, '{SITE}'::uuid, $1::uuid, NULL::uuid, 2, NULL::numeric, NULL::text,
                    '2026-09-30T00:00:00Z'::timestamptz, $2::text"
        ),
        &[&item, &source],
    )
    .await
    .expect("a report of the glove");

    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);

    let page: Value = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/items/{item}"))
            .insert_header(auth.clone())
            .to_request(),
        "the glove's page",
    )
    .await;
    assert_eq!(page["code"], "GLOVE-M");
    assert_eq!(page["description"], "Nitrile glove, medium");
    assert!(page["held"].is_array(), "this system's own record is its own list: {page}");
    assert!(page["measurements"].is_array());

    let ours: Vec<&Value> = page["reported"]
        .as_array()
        .expect("what NetSuite reported")
        .iter()
        .filter(|r| r["source"] == source.as_str())
        .collect();
    assert_eq!(ours.len(), 2, "one on a shelf and one at the warehouse: {page}");
    let shelf = ours.iter().find(|r| !r["bin_code"].is_null()).expect("the shelf row");
    assert_eq!(shelf["bin_code"], "A-01-1");
    assert_eq!(shelf["on_hand"], "7.5", "a quantity is carried as the text it was");
    assert!(shelf["location_id"].is_string(), "so the page can link to the bin");
    assert!(shelf["as_at"].is_string(), "a report says how old it is");
    let warehouse = ours.iter().find(|r| r["bin_code"].is_null()).expect("the warehouse row");
    assert_eq!(warehouse["on_hand"], "2");
    assert!(warehouse["location_id"].is_null());

    // ── an id nobody holds is a 404, not an empty page ──────────────────
    let r = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/items/{}", Uuid::nil()))
            .insert_header(auth.clone())
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 404);

    db.execute("DELETE FROM reported_stock WHERE source = $1", &[&source])
        .await
        .expect("tidy");
}
