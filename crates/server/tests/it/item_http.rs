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
    assert!(page["subjects"].is_array(), "what gets measured for it: {page}");
    assert!(page["photos"].is_array(), "and its photographs: {page}");

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

/// The list: found by code, description or barcode; narrowed to what is here,
/// or to what still needs measuring or a photo; paged by code.
#[actix_web::test]
async fn the_item_list_finds_narrows_and_pages() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);

    let list = |query: &str| {
        let uri = format!("/items{query}");
        let auth = auth.clone();
        let app = &app;
        async move {
            let r = test::call_service(app, test::TestRequest::get().uri(&uri).insert_header(auth).to_request()).await;
            let status = r.status().as_u16();
            let body: Value = serde_json::from_slice(&test::read_body(r).await).unwrap_or(Value::Null);
            (status, body)
        }
    };
    let codes = |page: &Value| -> Vec<String> {
        page["items"].as_array().unwrap().iter().map(|i| i["code"].as_str().unwrap().to_string()).collect()
    };

    // ── by code, and by description ─────────────────────────────────────
    let (status, found) = list("?q=GLOVE-M").await;
    assert_eq!(status, 200, "{found}");
    assert!(codes(&found).contains(&"GLOVE-M".to_string()), "{found}");
    let glove = found["items"].as_array().unwrap().iter().find(|i| i["code"] == "GLOVE-M").unwrap();
    assert!(glove["item_id"].is_string());
    assert!(["measured", "listed", "none"].contains(&glove["weight"].as_str().unwrap()), "{glove}");
    assert!(["measured", "listed", "none"].contains(&glove["size"].as_str().unwrap()), "{glove}");
    assert!(glove["demand"].is_i64(), "{glove}");
    let (_, described) = list("?q=nitrile").await;
    assert!(codes(&described).contains(&"GLOVE-M".to_string()), "a description matches too: {described}");

    // ── here: the fixture holds gloves at Melbourne in its own ledger ───
    let (_, here) = list("?q=GLOVE-M&stock=here").await;
    let g = here["items"].as_array().unwrap().iter().find(|i| i["code"] == "GLOVE-M").expect("gloves are here");
    assert!(g["held"].as_i64().unwrap() > 0, "this system's own count, as its own column: {g}");
    assert!(g["bin_code"].is_string(), "and the bin holding the most of it, to walk to: {g}");

    // ── paging: one at a time, in code order, until there are no more ───
    let (_, first) = list("?limit=1").await;
    let (_, second) = list(&format!("?limit=1&after={}", first["next"].as_str().expect("more than one item"))).await;
    assert_eq!(codes(&first).len(), 1);
    assert!(codes(&second)[0] > codes(&first)[0], "code order: {first} then {second}");
    assert_eq!(first["total"], second["total"], "the total is every page's, not this one's");
    let (_, nothing) = list("?q=no-such-item-anywhere").await;
    assert_eq!(nothing["total"], 0);
    assert!(nothing["next"].is_null(), "no next page after nothing");

    // ── needs: a filter that is not one is refused, not ignored ─────────
    let (status, _) = list("?needs=everything").await;
    assert_eq!(status, 400);
    let (status, unmeasured) = list("?needs=measuring").await;
    assert_eq!(status, 200);
    assert!(
        unmeasured["items"].as_array().unwrap().iter().all(|i| i["size"] != "measured"),
        "needs measuring lists nothing whose size is measured: {unmeasured}"
    );
    let (status, unweighed) = list("?needs=weighing").await;
    assert_eq!(status, 200);
    assert!(
        unweighed["items"].as_array().unwrap().iter().all(|i| i["weight"] != "measured"),
        "needs weighing lists nothing already weighed: {unweighed}"
    );
    let (status, _) = list("?needs=photo").await;
    assert_eq!(status, 200);

    // ── has: what has been done, the other way round from needs ─────────
    let (status, _) = list("?has=everything").await;
    assert_eq!(status, 400, "a filter that is not one is refused, not ignored");
    let (status, measured) = list("?has=measured&limit=200").await;
    assert_eq!(status, 200, "{measured}");
    assert!(
        measured["items"].as_array().unwrap().iter().all(|i| i["weight"] == "measured" || i["size"] == "measured"),
        "measured lists only what has a weight or a size measured: {measured}"
    );
    let (_, everything) = list("?limit=1").await;
    let (_, unweighed_all) = list("?needs=weighing&limit=1").await;
    let (_, unsized_all) = list("?needs=measuring&limit=1").await;
    assert!(
        measured["total"].as_i64().unwrap() >= everything["total"].as_i64().unwrap() - unweighed_all["total"].as_i64().unwrap(),
        "everything weighed is in it"
    );
    assert!(
        measured["total"].as_i64().unwrap() >= everything["total"].as_i64().unwrap() - unsized_all["total"].as_i64().unwrap(),
        "and everything sized"
    );
    let (status, photographed) = list("?has=photographed&limit=200").await;
    assert_eq!(status, 200, "{photographed}");
    assert!(
        photographed["items"].as_array().unwrap().iter().all(|i| i["picture"].is_object()),
        "photographed lists only what has a picture: {photographed}"
    );
    let (_, unpictured) = list("?needs=photo&limit=1").await;
    assert_eq!(
        photographed["total"].as_i64().unwrap() + unpictured["total"].as_i64().unwrap(),
        everything["total"].as_i64().unwrap(),
        "has a picture and needs one split every item between them"
    );
    // ── needs a size for packing, most needed first (D197) ──────────────
    let (status, packing) = list("?needs=packing&order=packing&limit=200").await;
    assert_eq!(status, 200, "{packing}");
    let rows = packing["items"].as_array().unwrap();
    assert!(rows.iter().all(|i| i["to_pack"].as_i64().unwrap() > 0), "only what is still to pack: {packing}");
    let lines: Vec<i64> = rows.iter().map(|i| i["to_pack_lines"].as_i64().unwrap()).collect();
    assert!(lines.windows(2).all(|w| w[0] >= w[1]), "on the most open orders first: {lines:?}");

    let (status, both) = list("?has=both&limit=200").await;
    assert_eq!(status, 200, "{both}");
    assert!(
        both["items"].as_array().unwrap().iter().all(|i| i["picture"].is_object() && (i["weight"] == "measured" || i["size"] == "measured")),
        "both is both: {both}"
    );

    // ── ordered by how much it is ordered, or the way the bins are walked ─
    let (status, _) = list("?order=size").await;
    assert_eq!(status, 400, "an order that is not one is refused");
    let (status, busiest) = list("?order=demand&limit=1").await;
    assert_eq!(status, 200, "{busiest}");
    let demand: Vec<i64> =
        busiest["items"].as_array().unwrap().iter().map(|i| i["demand"].as_i64().unwrap()).collect();
    assert!(demand.windows(2).all(|w| w[0] >= w[1]), "most ordered first: {demand:?}");
    let after = busiest["next"].as_str().unwrap_or_else(|| panic!("a next page: {busiest}"));
    let (_, next) = list(&format!("?order=demand&limit=1&after={after}")).await;
    assert!(
        next["items"].as_array().unwrap().iter().all(|i| i["demand"].as_i64().unwrap() <= *demand.last().unwrap()),
        "and the next page carries on down: {next}"
    );
    let (status, walked) = list("?order=walk&stock=here").await;
    assert_eq!(status, 200, "{walked}");
    let (status, _) = list("?order=walk&after=nowhere").await;
    assert_eq!(status, 400, "a page in walking order starts from a position");
}
