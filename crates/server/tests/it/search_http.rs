//! The global search over HTTP (D189): an item, a bin and an order, each found
//! by part of what it is called, and nothing for nothing.

use actix_web::{test, web, App};
use serde_json::Value;
use spork_server::{routes, AppState};

use super::common;
use common::{pool, url};

#[actix_web::test]
async fn bins_items_and_orders_are_found_as_you_type() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);
    let find = |q: &str| {
        let uri = format!("/search?q={}", q.replace(' ', "+"));
        let auth = auth.clone();
        let app = &app;
        async move {
            let body: Value = common::ok_json(app, test::TestRequest::get().uri(&uri).insert_header(auth).to_request(), "search").await;
            body["results"].as_array().unwrap().clone()
        }
    };

    let item = find("glove-m").await;
    assert_eq!(item[0]["kind"], "item", "{item:?}");
    assert_eq!(item[0]["title"], "GLOVE-M");
    assert!(item[0]["path"].as_str().unwrap().starts_with("/items/"));

    let words = find("nitrile").await;
    assert!(words.iter().any(|r| r["title"] == "GLOVE-M"), "by a word of its description: {words:?}");

    let bin = find("a-01").await;
    assert!(bin.iter().any(|r| r["kind"] == "bin" && r["title"] == "A-01-1"), "{bin:?}");
    assert!(bin.iter().find(|r| r["kind"] == "bin").unwrap()["path"].as_str().unwrap().starts_with("/bins/"));

    let order = find("260041").await;
    assert!(order.iter().any(|r| r["kind"] == "order" && r["title"] == "S260041"), "by part of its number: {order:?}");

    assert!(find("  ").await.is_empty(), "nothing typed, nothing found");
    assert!(find("zzqqxx").await.is_empty());
}
