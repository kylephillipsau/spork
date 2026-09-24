//! What is on the dock with nowhere to live, over HTTP.
//!
//! The read exists because the floor works in two steps: goods are checked in at
//! the dock and put away afterwards. `POST /receipts` has always accepted a bin
//! directly, so this list is empty on a floor that receives straight to the
//! shelf — and that is the correct answer there, not a bug.
//!
//! # The assertion this file exists for
//!
//! **A container moves as a container.** `crate::moving` refuses
//! `FromPackageHeld` because a package is relocated by a `package_event`; this
//! read has to draw the same line from the other side or it offers work that the
//! write path will not accept. The fixture makes it concrete without any setup:
//! `DOCK-1` holds three cells and one of them sits inside `CARTON-D`, an
//! outbound carton that has already been despatched.
//!
//! Nothing here writes.

use actix_web::{test, web, App};
use spork_server::{routes, AppState};
use uuid::Uuid;

mod common;
use common::{pool, url};

const SITE: &str = "a5170000-0000-0000-0000-000000000001";

macro_rules! app {
    ($u:expr) => {{
        let state = web::Data::new(AppState { pool: pool($u) });
        test::init_service(App::new().app_data(state).configure(routes::configure)).await
    }};
}

#[actix_web::test]
async fn the_dock_list_offers_loose_stock_and_never_a_container() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);

    let anon = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/putaway"))
            .to_request(),
    )
    .await;
    assert_eq!(anon.status(), 401, "a machine is told plainly");

    let bearer = common::bearer(&app).await;
    let screen = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/putaway"))
            .insert_header(("authorization", bearer))
            .to_request(),
        "GET the dock list",
    )
    .await;

    assert!(screen["site"].is_string(), "the list says where it is");
    let cells = screen["cells"].as_array().expect("cells");
    // **The loop below proves nothing over an empty list.** The fixture puts
    // loose stock on `DOCK-1` precisely so this read has something to be wrong
    // about; an empty answer here means the fixture changed, not that the code
    // is fine.
    assert!(!cells.is_empty(), "the fixture leaves stock on the dock: {screen}");

    let (client, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });

    for cell in cells {
        let stock_id = Uuid::parse_str(cell["stock_id"].as_str().expect("a stock id")).unwrap();
        let row = client
            .query_one(
                "SELECT s.holder_package_id IS NULL, l.kind, s.quantity::bigint
                   FROM stock s
                   JOIN location l ON l.id = s.resolved_location_id
                  WHERE s.id = $1",
                &[&stock_id],
            )
            .await
            .expect("the cell");
        let loose: bool = row.get(0);
        let kind: String = row.get(1);
        let quantity: i64 = row.get(2);

        assert!(loose, "a package-held cell is not put-away work: {cell}");
        assert_eq!(kind, "dock", "everything offered is on a dock: {cell}");
        assert_eq!(cell["quantity"].as_i64(), Some(quantity), "{cell}");

        // `homes` is information, never a direction — so every bin it names is
        // a storage bin, and never the dock the goods are standing on.
        for home in cell["homes"].as_array().expect("homes") {
            let home_kind: String = client
                .query_one(
                    "SELECT kind FROM location WHERE id = $1",
                    &[&Uuid::parse_str(home["location_id"].as_str().unwrap()).unwrap()],
                )
                .await
                .expect("the home")
                .get(0);
            assert!(
                ["pick_face", "bulk", "overflow"].contains(&home_kind.as_str()),
                "a home is somewhere stock lives, not a dock: {home}"
            );
        }
    }

    // The despatched carton on the dock is the case the rule has to exclude, and
    // asserting it by name is what stops the rule being relaxed by accident.
    let hidden: i64 = client
        .query_one(
            "SELECT count(*) FROM stock s
               JOIN package p ON p.id = s.holder_package_id
               JOIN location l ON l.id = s.resolved_location_id
              WHERE l.kind = 'dock' AND s.quantity > 0",
            &[],
        )
        .await
        .expect("count")
        .get(0);
    assert!(
        hidden > 0,
        "the fixture still stands a package on the dock, or this test proves nothing"
    );
    let offered: Vec<String> = cells
        .iter()
        .map(|c| c["stock_id"].as_str().unwrap().to_string())
        .collect();
    let package_held: Vec<String> = client
        .query(
            "SELECT s.id::text FROM stock s
               JOIN location l ON l.id = s.resolved_location_id
              WHERE l.kind = 'dock' AND s.holder_package_id IS NOT NULL",
            &[],
        )
        .await
        .expect("package-held")
        .iter()
        .map(|r| r.get(0))
        .collect();
    for id in &package_held {
        assert!(!offered.contains(id), "a container was offered as put-away work: {id}");
    }
}
