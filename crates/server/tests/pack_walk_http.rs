//! The pack walkthrough, over HTTP, through the handlers that serve it.
//!
//! **Thirty-one endpoints existed and none had ever been executed.** Every other
//! test in this crate writes SQL directly or calls the pure modules a handler
//! calls, which checks the reasoning and not the wiring. Three defects in the
//! endpoints shipped that way and were found only because a test happened to run
//! *similar* SQL: a query against `projection_run`, a table that does not exist;
//! a cast to `package_event_kind`, a type that does not exist; and an INSERT of
//! `consignment.status`, a column the application holds no grant on. A handler
//! compiles whatever its SQL says. It is a string until something runs it.
//!
//! So this walks the recorded process — stages 1 to 6 — through
//! `test::init_service` and the same `routes::configure` the binary registers
//! from, which is what makes "is this endpoint wired up" a question the suite can
//! answer rather than one production answers.
//!
//! # What it writes
//!
//! Handlers own their transactions, so unlike the rest of the suite this commits.
//! Every identifier is minted, so a leak is an orphan nobody collides with, and
//! the walk removes its own rows at the end. It also **allocates before it
//! picks**: a pick with no allocation drives `picked > covered`, which J56 raises
//! as a finding — D100 says so in as many words — and a harness that quietly
//! seeded findings for everyone else would be worse than no harness.

use actix_web::{test, web, App};
use chrono::Utc;
use spork_server::{routes, AppState};
use serde_json::{json, Value};
use uuid::Uuid;

mod common;
use common::{pool, url};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const PERSON: &str = "77770000-0000-0000-0000-000000000001";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const DOCK: &str = "10c00000-0000-0000-0000-000000000003";
const SMALL_BOX: &str = "9a7e0000-0000-0000-0000-0000000000b1";
/// IF400187: gumboots, and the only fixture job whose item is not lot-tracked.
const GUMBOOT_FULFILMENT: &str = "f01f0000-0000-0000-0000-000000000004";
const GUMBOOT: &str = "17e10000-0000-0000-0000-000000000002";
const SWIFT_NBD: &str = "ca450000-0000-0000-0000-000000000001";

#[actix_web::test]
async fn the_pack_walkthrough_runs_end_to_end_over_http() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            // The binary's own registration. A route added to routes.rs and not
            // wired here would fail below rather than in production.
            .configure(routes::configure)
            .configure(spork_server::web::configure),
    )
    .await;

    // **Sign on first.** `x-tenant-id` is gone: the tenant comes from the
    // session now, and so does `recorded_by_id` — D11's non-repudiable floor,
    // which was a number the request typed until migration 70.
    let bearer = common::bearer(&app).await;

    let carton = Uuid::now_v7();
    let consignment = Uuid::now_v7();
    let now = Utc::now();
    // **Named up front so the cleanup can reach them.** The first version minted
    // these inline and removed the facts without the envelopes, and four
    // `client_event` rows accumulated on every run — a slower version of the
    // leak this harness exists to have caught.
    let act_package = Uuid::now_v7();
    let act_pick = Uuid::now_v7();
    let act_measure = Uuid::now_v7();
    let act_seal = Uuid::now_v7();
    let acts = vec![act_package, act_pick, act_measure, act_seal];

    // -----------------------------------------------------------------
    // Stage 0: the server is up and the presets are readable
    // -----------------------------------------------------------------
    let health = common::ok_json(
        &app,
        test::TestRequest::get().uri("/health").to_request(),
        "GET /health",
    )
    .await;
    assert_eq!(health["status"], "ok");

    let presets = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri("/package-types")
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /package-types",
    )
    .await;
    let names: Vec<&str> = presets
        .as_array()
        .expect("a list of presets")
        .iter()
        .filter_map(|p| p["name"].as_str())
        .collect();
    assert!(names.contains(&"PALLET"), "the shipped standard is offered");
    assert!(names.contains(&"small box"), "and this warehouse's boxes");

    // -----------------------------------------------------------------
    // Stage 0: the latest, for somebody who was not given a number
    // -----------------------------------------------------------------
    //
    // A search box is only usable by a person who already knows the answer. The
    // screen opens on this instead, so `reference` is optional and its absence
    // means "the recent ones here" rather than "everything", which is what the
    // handler used to reject an empty one for.
    let latest = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri("/orders")
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /orders with no reference",
    )
    .await;
    let rows = latest.as_array().expect("a list of orders");
    assert!(
        !rows.is_empty(),
        "the fixture has orders, so a way in that shows none is the bug this closes"
    );
    assert!(
        rows.iter()
            .any(|o| o["confirmation_number"] == "S260041"),
        "the order the rest of this walk uses is reachable without typing its number"
    );
    // Newest first: a list you have to read backwards is not a way in either.
    let placed: Vec<&str> = rows
        .iter()
        .filter_map(|o| o["placed_at"].as_str())
        .collect();
    assert!(
        placed.windows(2).all(|w| w[0] >= w[1]),
        "the latest orders come back latest first, got {placed:?}"
    );
    // The same shape as a search result, so one screen draws both.
    assert!(
        rows.iter().any(|o| o["fulfilments"].is_array()),
        "a listed order carries its fulfilments, or the list cannot be walked from"
    );

    // -----------------------------------------------------------------
    // Stages 1 and 2: find the order, and read the gate
    // -----------------------------------------------------------------
    let found = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri("/orders?reference=S260041")
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /orders",
    )
    .await;
    let order = &found.as_array().expect("matches")[0];
    assert_eq!(order["confirmation_number"], "S260041");
    assert!(
        order["customer_name"].is_string(),
        "stage 1 is 'confirm it matches the contact', so the contact comes back"
    );
    let fulfilment = &order["fulfilments"].as_array().expect("fulfilments")[0];
    let fulfilment_id = fulfilment["fulfilment_id"].as_str().unwrap().to_string();
    assert!(
        fulfilment["committed_quantity"].as_i64().unwrap() > 0,
        "the gate reports how far, not merely yes or no"
    );

    // The line to pack. **From the open-work index**, which is what an operator
    // works from and what stops this walk grabbing a line the fixture has
    // already covered — `/allocations` refused that with J56's own sentence,
    // which is the check doing its job and the harness learning the difference
    // between an endpoint that works and a sequence that composes.
    let open = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/open-lines"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /sites/{id}/open-lines",
    )
    .await;
    let line = open
        .as_array()
        .expect("open work")
        .iter()
        .find(|l| {
            l["quantity"].as_i64().unwrap_or(0) - l["covered_quantity"].as_i64().unwrap_or(0) > 0
        })
        .expect("a line with room left to claim");
    let line_id = line["fulfilment_line_id"].as_str().unwrap().to_string();
    // **The cell has to hold what the line commits.** This walk used to take the
    // first location-held cell with two units in it, which was right only while
    // the fixture stocked one item: the moment a second arrived it claimed boots
    // against a line that commits gloves, and `/allocations` said so. The check
    // was doing its job; the walk was asserting less than it looked like it was.
    let item_code = line["item_code"].as_str().expect("the line names its item").to_string();
    let _ = &fulfilment_id;

    let stock = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri("/stock")
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /stock",
    )
    .await;
    let cell = stock
        .as_array()
        .expect("stock rows")
        .iter()
        .find(|r| {
            r["item_code"].as_str() == Some(item_code.as_str())
                && r["location_code"].is_string()
                && r["quantity"].as_i64().unwrap_or(0) >= 2
        })
        .expect("a location-held cell holding what the line commits");
    let stock_id = cell["stock_id"]
        .as_str()
        .or_else(|| cell["id"].as_str())
        .expect("a stock id")
        .to_string();

    // -----------------------------------------------------------------
    // Stage 3: the pack. Allocate, then pick into a carton.
    // -----------------------------------------------------------------
    // **Allocated first on purpose.** A pick with no allocation makes
    // `picked > covered`, which J56 raises; D100 names it exactly.
    let allocation_id = Uuid::now_v7();
    let allocation = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/allocations")
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({
                "id": allocation_id,
                "fulfilment_line_id": line_id,
                "stock_id": stock_id,
                "quantity": 1
            }))
            .to_request(),
        "POST /allocations",
    )
    .await;
    assert_eq!(allocation["replayed"], false);

    let created = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/packages")
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({
                "id": carton,
                "fulfilment_id": fulfilment_id,
                "package_type_id": SMALL_BOX,
                "location_id": DOCK,
                "client_event_id": act_package,
                "occurred_at": now
            }))
            .to_request(),
        "POST /packages",
    )
    .await;
    assert_eq!(created["package_id"], carton.to_string());

    let pick = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/picks")
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({
                "from_stock_id": stock_id,
                "to_package_id": carton,
                "fulfilment_line_id": line_id,
                "quantity": 1,
                "client_event_id": act_pick,
                "occurred_at": now
            }))
            .to_request(),
        "POST /picks",
    )
    .await;
    assert!(pick["movement_id"].is_string(), "the pick is a ledger row");

    // **D11's floor, asserted.** The request said nothing about who was doing
    // this — it cannot, the field is gone — and the ledger names the person who
    // signed on. That is the whole of *"we always know which person recorded
    // this"*, and until migration 70 it was whatever the body claimed.
    {
        let (c, conn) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.unwrap();
        tokio::spawn(async move {
            let _ = conn.await;
        });
        let movement = Uuid::parse_str(pick["movement_id"].as_str().unwrap()).unwrap();
        let row = c
            .query_one(
                "SELECT m.recorded_by_id::text, e.recorded_by_id::text, e.site_id::text
                   FROM stock_movement m
                   JOIN client_event e ON e.client_event_id = m.client_event_id
                  WHERE m.id = $1",
                &[&movement],
            )
            .await
            .expect("the movement and its act");
        assert_eq!(
            row.get::<_, String>(0),
            PERSON,
            "the movement names the signed-in person"
        );
        assert_eq!(
            row.get::<_, String>(1),
            PERSON,
            "and so does the act envelope — S19 requires them to agree"
        );
        assert_eq!(
            row.get::<_, String>(2),
            SITE,
            "at the site the session signed on to"
        );
    }

    // -----------------------------------------------------------------
    // Stage 4: the only genuinely variable input — weigh and measure
    // -----------------------------------------------------------------
    let measured = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/observations")
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({
                "package_id": carton,
                "measurements": [
                    { "metric": "gross_weight", "entered_value": "4.2", "unit": "kg" },
                    { "metric": "height", "entered_value": "185", "unit": "mm" }
                ],
                "method": "instrument",
                "ingestion_channel": "scale",
                "client_event_id": act_measure,
                "occurred_at": now
            }))
            .to_request(),
        "POST /observations",
    )
    .await;
    assert_eq!(
        measured["observation_ids"].as_array().unwrap().len(),
        2,
        "one act, one subject, two metrics"
    );
    let written = measured["package_columns_written"].as_array().unwrap();
    assert!(
        written.iter().any(|c| c == "gross_weight_g"),
        "the cache J12 compares against is part of the act: {written:?}"
    );

    // 4.2 kg is 4200 g, and a float would not have held it exactly.
    let pkg = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/packages/{carton}/contents"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /packages/{id}/contents",
    )
    .await;
    assert_eq!(pkg["gross_weight_g"], 4200);
    assert_eq!(pkg["height_mm"], 185);

    // -----------------------------------------------------------------
    // Stage 5: the packing list, which is the thing NetSuite cannot print
    // -----------------------------------------------------------------
    let packed = pkg["lines"].as_array().expect("packing list lines");
    assert_eq!(packed.len(), 1, "one line in this carton");
    assert_eq!(packed[0]["quantity"], 1);
    assert_eq!(
        packed[0]["order_reference"], "S260041",
        "the carton names the order it serves"
    );

    // Seal it. A consignment takes sealed cartons only.
    common::ok_json(
        &app,
        test::TestRequest::post()
            .uri(&format!("/packages/{carton}/seal"))
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({
                "client_event_id": act_seal,
                "occurred_at": now
            }))
            .to_request(),
        "POST /packages/{id}/seal",
    )
    .await;

    // -----------------------------------------------------------------
    // Stage 6: hand it to a carrier, with the count that existed nowhere
    // -----------------------------------------------------------------
    let consigned = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/consignments")
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({
                "id": consignment,
                "package_ids": [carton],
                "carrier_service_id": SWIFT_NBD,
                "despatch_at": now
            }))
            .to_request(),
        "POST /consignments",
    )
    .await;
    assert_eq!(consigned["package_count"], 1);
    assert_eq!(consigned["carrier_name"], "Swift Transport Services");
    assert!(
        consigned["status"].is_null(),
        "no carrier has been asked, so there is no status to report"
    );
    let carrier_line = &consigned["carrier_lines"].as_array().unwrap()[0];
    assert_eq!(carrier_line["package_type"], "small box");
    assert_eq!(carrier_line["gross_weight_g"], 4200);

    // The retry a handheld would send. Q174's shape, over HTTP.
    let again = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/consignments")
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({
                "id": consignment,
                "package_ids": [carton],
                "carrier_service_id": SWIFT_NBD
            }))
            .to_request(),
        "POST /consignments (retry)",
    )
    .await;
    assert_eq!(again["replayed"], true, "a retry is a replay");
    assert_eq!(again["package_count"], 1, "and writes no second link");

    cleanup(&u, carton, consignment, &acts, allocation_id).await;
}

/// The tenancy boundary is the session now, not a header anyone can type.
#[actix_web::test]
async fn an_unauthenticated_request_is_refused_and_a_header_no_longer_helps() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)
            .configure(spork_server::web::configure)).await;

    // No session at all.
    let resp = test::call_service(&app, test::TestRequest::get().uri("/stock").to_request()).await;
    assert_eq!(resp.status(), 401, "a read needs a session");

    // **The old key, offered again.** `x-tenant-id` was the whole tenancy
    // boundary and nothing verified it; asserting it now buys nothing, which is
    // the property worth pinning so nobody reintroduces the header quietly.
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/stock")
            .insert_header(("x-tenant-id", ALPHA))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), 401, "naming a tenant is not being one");

    // A session for Beta cannot read Alpha's orders, and does not need to be
    // told which tenant it is: the session says.
    //
    // **Dana, not the fixture's default operator**, and this is the one sign-in
    // in the suite that `common::bearer` must not stand in for: the assertion
    // below is that a *different tenant's* session sees nothing, so signing in
    // as Alpha makes it pass for the wrong reason. A mechanical lift replaced
    // it once and the count went from zero to one, which is the whole test.
    let bearer = common::sign_in_as(&app, "dana@example.test", None).await;
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/orders?reference=S260041")
            .insert_header(("authorization", bearer))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success());
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body.as_array().map(|a| a.len()),
        Some(0),
        "a confirmation number is not a global key"
    );
}

/// Remove what the walk committed. Handlers own their transactions, so this is
/// the only way; minted ids mean a leak from a failed run collides with nothing.
async fn cleanup(u: &str, carton: Uuid, consignment: Uuid, acts: &[Uuid], allocation: Uuid) {
    let (client, connection) = tokio_postgres::connect(u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
        .batch_execute(&format!(
            "DELETE FROM consignment_package WHERE consignment_id = '{consignment}';
             DELETE FROM consignment WHERE id = '{consignment}';
             DELETE FROM observation WHERE observable_id IN
                 (SELECT id FROM observable WHERE package_id = '{carton}');
             DELETE FROM observation_event WHERE observable_id IN
                 (SELECT id FROM observable WHERE package_id = '{carton}');
             DELETE FROM observable WHERE package_id = '{carton}';
             DELETE FROM stock_movement WHERE to_package_id = '{carton}';
             DELETE FROM stock_allocation WHERE id = '{allocation}';
             UPDATE package SET placement_event_id = NULL, placement_occurred_at = NULL
              WHERE id = '{carton}';
             DELETE FROM package_containment WHERE package_id = '{carton}'
                                                 OR parent_package_id = '{carton}';
             DELETE FROM stock WHERE holder_package_id = '{carton}';
             DELETE FROM package_event WHERE package_id = '{carton}';
             DELETE FROM package WHERE id = '{carton}';"
        ))
        .await
        .expect("the walk removes what it committed");

    // Checked, not hoped for. The projections above are folded from the events
    // this walk wrote, so a maintainer running mid-walk leaves rows that make the
    // package delete fail — which is exactly how a carton leaked before this was
    // ordered properly.
    // The envelopes last: every fact above carries a foreign key to one.
    client
        .execute(
            "DELETE FROM client_event WHERE client_event_id = ANY($1)",
            &[&acts],
        )
        .await
        .expect("the acts go too");

    // Checked, not hoped for. The projections above are folded from the events
    // this walk wrote, so a maintainer running mid-walk leaves rows that make the
    // package delete fail — which is exactly how a carton leaked before this was
    // ordered properly.
    let left: i64 = client
        .query_one(
            "SELECT (SELECT count(*) FROM package WHERE id = $1)
                  + (SELECT count(*) FROM client_event WHERE client_event_id = ANY($2))",
            &[&carton, &acts],
        )
        .await
        .expect("count")
        .get(0);
    assert_eq!(left, 0, "the walk left rows behind");
}
/// The one page left renders, and a signed-out browser is shown a form rather
/// than a status code.
///
/// **Nine screens became one.** `/app` is retired: the packing list is what
/// D113 always said would stay, because it is an A4 sheet folded onto a pallet
/// rather than a port in progress. What is still worth asserting is that it is
/// mounted where it says, that its stylesheet carries the print rules, and that
/// a person who walks up signed-out is redirected instead of refused.
#[actix_web::test]
async fn the_printable_packing_list_renders() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)
            .configure(spork_server::web::configure)).await;

    // The stylesheet is served from the binary; there is no asset pipeline.
    let css = test::call_service(
        &app,
        test::TestRequest::get().uri("/print/style.css").to_request(),
    )
    .await;
    assert!(css.status().is_success());
    let css_body = String::from_utf8(test::read_body(css).await.to_vec()).unwrap();
    assert!(css_body.contains("@page"), "the A4 print rules, which are the point of it");

    // Signed out: a person gets sent to the form, not a status code. It is the
    // React one now, which is the only sign-in there is.
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/print/packing-list/9ac00000-0000-0000-0000-00000000000a")
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), 303, "a browser is redirected to sign in");
    assert_eq!(
        resp.headers().get("location").unwrap().to_str().unwrap(),
        "/sign-in"
    );

    let bearer = common::bearer(&app).await;

    // The packing list renders for a fixture carton.
    let page = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/print/packing-list/9ac00000-0000-0000-0000-00000000000a")
            .insert_header(("authorization", bearer))
            .to_request(),
    )
    .await;
    assert!(page.status().is_success());
    let html = String::from_utf8(test::read_body(page).await.to_vec()).unwrap();
    assert!(html.contains("Packing list"));
    assert!(html.contains("312.50 kg"), "the pallet the fixture weighed");
    // The chrome it used to share with eight other screens is gone with them.
    assert!(!html.contains("/app/"), "and it links to nothing that no longer exists");
}
/// The findings queue (D8), which is the screen the rest of the system fills.
///
/// **It used to read the maud page and now reads the endpoint the screen reads.**
/// `GET /discrepancies` had no test of its own — the page was standing in for
/// one — so retiring the page turned an assertion about HTML into the first
/// assertion about the JSON every finding now travels in.
#[actix_web::test]
async fn the_findings_queue_shows_the_evidence() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let bearer = common::bearer(&app).await;

    let open: Value = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri("/discrepancies?state=open")
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /discrepancies",
    )
    .await;

    // The fixture's short pick, with the evidence on the row rather than a
    // sentence about it: expected, counted, and the difference.
    let short = open
        .as_array()
        .expect("a list")
        .iter()
        .find(|f| f["item_code"] == "GLOVE-M")
        .expect("the fixture's short pick is open");
    assert_eq!(short["kind"], "short_pick", "the kind, in words");
    assert_eq!(short["state"], "open", "an open finding can be picked up");
    // **Decimal strings, not numbers.** The quantity is `numeric` and a JSON
    // number is an IEEE double, so the server sends the digits.
    assert_eq!(short["expected_quantity"], "40", "what was expected");
    assert_eq!(short["variance"], "-4", "and the difference, which is the finding");

    // Closed is a different tab, and the fixture has nothing in it.
    let closed: Value = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri("/discrepancies?state=resolved,accepted")
            .insert_header(("authorization", bearer))
            .to_request(),
        "GET /discrepancies closed",
    )
    .await;
    assert_eq!(closed.as_array().expect("a list").len(), 0, "nothing to chase");
}
/// The queue, which is the way in rather than a search box.
///
/// **A packer does not know the name of anything yet.** They know there is a
/// day's work. So the landing screen is what there is to pack, each job called
/// by its own item fulfilment number — migration 71's column, which until then
/// every screen borrowed from the order and therefore could not tell three
/// commitments against `S260041` apart.
///
/// **It used to read the maud queue and now reads `GET /packing`**, which the
/// React screen reads and which had no test of its own. The grouping S44
/// forbids storing is computed by `packing::stage` and comes back on each job,
/// so this asserts the arithmetic rather than four headings in some HTML.
#[actix_web::test]
async fn the_queue_is_the_way_in() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let bearer = common::bearer(&app).await;

    let queue: Value = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri("/packing")
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /packing",
    )
    .await;
    let jobs = queue.as_array().expect("a list of jobs");
    let named: Vec<&str> = jobs.iter().filter_map(|j| j["reference"].as_str()).collect();
    assert!(named.contains(&"IF400187"), "the job is called by its own number");
    assert!(named.contains(&"IF265591"), "and so is every other one");

    // The groups S44 forbids storing, computed. A row is in the one its own
    // quantities put it in, and the server says which so one rule serves every
    // screen.
    for j in jobs {
        let (picked, committed) = (
            j["picked"].as_i64().unwrap(),
            j["committed"].as_i64().unwrap(),
        );
        // **Picked against committed, and cartons have nothing to do with it.**
        // The first version of this assertion put a job with a carton open on
        // the bench, which reads plausibly and is not the rule: a carton can be
        // started before anything is picked. `nothing_committed` is tested
        // first because zero committed makes `picked >= committed` true of an
        // empty commitment and would file it under packed.
        let expected = if committed <= 0 {
            "nothing_committed"
        } else if picked <= 0 {
            "ready"
        } else if picked < committed {
            "on_the_bench"
        } else {
            "packed"
        };
        assert_eq!(
            j["stage"], expected,
            "{} is in the group its own quantities put it in",
            j["reference"]
        );
    }

    // **The promise is the one date in the fixture that moves.** Everything
    // else is history and is pinned; a window promised last August would read
    // `overdue` on every row no matter when the database was built.
    let due: Vec<&str> = jobs.iter().filter_map(|j| j["due"].as_str()).collect();
    assert!(due.contains(&"due today"), "the fixture promises against today");
    assert!(due.contains(&"due tomorrow"), "and against tomorrow");
    assert!(!due.contains(&"overdue"), "and nothing in it is stale by construction");

    // Searching narrows the queue rather than replacing it with a different
    // answer.
    let narrowed: Value = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri("/packing?q=IF400187")
            .insert_header(("authorization", bearer))
            .to_request(),
        "GET /packing?q=",
    )
    .await;
    let found: Vec<&str> = narrowed
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|j| j["reference"].as_str())
        .collect();
    assert!(found.contains(&"IF400187"));
    assert!(!found.contains(&"IF265591"), "and excludes what does not match");
}

/// Working out how to pack: put units in a carton, take them back out, discard
/// the carton.
///
/// **The ledger is append-only, so none of this deletes anything.** Taking units
/// out is a correction with reason `wrong_location` — while goods sit on the
/// bench a pick into a particular carton asserts which box holds them, not that
/// stock moved — and `stock_movement_effective` nets it out so the line needs
/// those units again. Discarding is a `voided` event, which the package fold has
/// understood since migration 4 and nothing had ever written.
#[actix_web::test]
async fn a_packer_can_change_their_mind() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)
            .configure(spork_server::web::configure)).await;

    let bearer = common::bearer(&app).await;
    let get = |uri: String, b: String| {
        test::TestRequest::get()
            .uri(&uri)
            .insert_header(("authorization", b))
            .to_request()
    };

    // A line with room, and a cell to take from.
    let open: Value = test::read_body_json(
        test::call_service(&app, get(format!("/sites/{SITE}/open-lines"), bearer.clone())).await,
    )
    .await;
    let line = open
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["quantity"].as_i64().unwrap_or(0) - l["covered_quantity"].as_i64().unwrap_or(0) > 0)
        .expect("a line with room");
    let line_id = line["fulfilment_line_id"].as_str().unwrap().to_string();
    let fulfilment_id = line["fulfilment_id"].as_str().unwrap().to_string();
    // Matching the item, for the reason the walk above records.
    let item_code = line["item_code"].as_str().expect("the line names its item").to_string();

    let stock: Value =
        test::read_body_json(test::call_service(&app, get("/stock".into(), bearer.clone())).await)
            .await;
    let stock_id = stock
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r["item_code"].as_str() == Some(item_code.as_str())
                && r["location_code"].is_string()
                && r["quantity"].as_i64().unwrap_or(0) >= 2
        })
        .expect("a location-held cell holding what the line commits")["stock_id"]
        .as_str()
        .unwrap()
        .to_string();

    let carton = Uuid::now_v7();
    let acts: Vec<Uuid> = (0..5).map(|_| Uuid::now_v7()).collect();
    let now = Utc::now();
    let post = |uri: &str, body: Value, b: String| {
        test::TestRequest::post()
            .uri(uri)
            .insert_header(("authorization", b))
            .set_json(body)
            .to_request()
    };

    // Start a carton and put one unit in it.
    common::ok_json(
        &app,
        post(
            "/packages",
            json!({ "id": carton, "fulfilment_id": fulfilment_id,
                    "location_id": DOCK, "client_event_id": acts[0],
                    "occurred_at": now }),
            bearer.clone(),
        ),
        "POST /packages",
    )
    .await;
    let allocation_id = Uuid::now_v7();
    common::ok_json(
        &app,
        post(
            "/allocations",
            json!({ "id": allocation_id, "fulfilment_line_id": line_id,
                    "stock_id": stock_id, "quantity": 1 }),
            bearer.clone(),
        ),
        "POST /allocations",
    )
    .await;
    let pick = common::ok_json(
        &app,
        post(
            "/picks",
            json!({ "from_stock_id": stock_id, "to_package_id": carton,
                    "fulfilment_line_id": line_id, "quantity": 1,
                    "client_event_id": acts[1], "occurred_at": now }),
            bearer.clone(),
        ),
        "POST /picks",
    )
    .await;
    let movement = pick["movement_id"].as_str().unwrap().to_string();

    // The carton holds it, and voiding is refused while it does.
    let contents: Value = test::read_body_json(
        test::call_service(&app, get(format!("/packages/{carton}/contents"), bearer.clone())).await,
    )
    .await;
    assert_eq!(contents["lines"][0]["quantity"], 1);

    let refused = test::call_service(
        &app,
        post(
            &format!("/packages/{carton}/void"),
            json!({ "client_event_id": acts[2], "occurred_at": now }),
            bearer.clone(),
        ),
    )
    .await;
    assert_eq!(
        refused.status(),
        400,
        "a carton holding stock cannot be voided; the units would belong to \
         something no screen lists"
    );

    // Change of mind: take it back out. The reason is a record error about which
    // box, which is what `/corrections` accepts.
    let reason: Value = {
        let (c, conn) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.unwrap();
        tokio::spawn(async move {
            let _ = conn.await;
        });
        let id: Uuid = c
            .query_one(
                "SELECT id FROM adjustment_reason WHERE code = 'wrong_location'",
                &[],
            )
            .await
            .expect("the fixture ships a wrong_location reason")
            .get(0);
        json!(id)
    };
    common::ok_json(
        &app,
        post(
            "/corrections",
            json!({ "reverses_movement_id": movement, "quantity": 1,
                    "adjustment_reason_id": reason, "client_event_id": acts[3] }),
            bearer.clone(),
        ),
        "POST /corrections",
    )
    .await;

    let contents: Value = test::read_body_json(
        test::call_service(&app, get(format!("/packages/{carton}/contents"), bearer.clone())).await,
    )
    .await;
    assert_eq!(
        contents["lines"].as_array().map(|a| a.len()),
        Some(0),
        "netted out of the fold, so the carton reads empty"
    );

    // And now it can be discarded.
    let voided = common::ok_json(
        &app,
        post(
            &format!("/packages/{carton}/void"),
            json!({ "client_event_id": acts[4], "occurred_at": now }),
            bearer.clone(),
        ),
        "POST /packages/{id}/void",
    )
    .await;
    assert_eq!(voided["status"], "voided");

    // It leaves the bench — asked of the endpoint the bench reads, now that the
    // page which used to answer this is gone.
    let bench: Value = common::ok_json(
        &app,
        get(format!("/fulfilments/{fulfilment_id}/bench"), bearer.clone()),
        "GET /fulfilments/{id}/bench",
    )
    .await;
    let still_there = bench["cartons"]
        .as_array()
        .expect("the bench lists its cartons")
        .iter()
        .any(|c| c["id"] == carton.to_string());
    assert!(!still_there, "a voided carton is off the bench");

    cleanup_change_of_mind(&u, carton, &acts, allocation_id).await;
}

async fn cleanup_change_of_mind(u: &str, carton: Uuid, acts: &[Uuid], allocation: Uuid) {
    let (c, conn) = tokio_postgres::connect(u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = conn.await;
    });
    c.batch_execute(&format!(
        "UPDATE package SET placement_event_id = NULL, placement_occurred_at = NULL
          WHERE id = '{carton}';
         DELETE FROM package_containment WHERE package_id = '{carton}';
         DELETE FROM stock WHERE holder_package_id = '{carton}';
         DELETE FROM stock_movement WHERE reverses_movement_id IN
             (SELECT id FROM stock_movement WHERE to_package_id = '{carton}');
         DELETE FROM stock_movement WHERE to_package_id = '{carton}';
         DELETE FROM package_event WHERE package_id = '{carton}';
         DELETE FROM package WHERE id = '{carton}';"
    ))
    .await
    .expect("the test removes what it committed");
    // **By id, never by a time window.** These tests run in parallel against one
    // database, and a `bound_at > now() - interval` delete reaches into whatever
    // the other one is doing — which is how the walk started failing
    // intermittently the moment a second test claimed stock.
    c.execute("DELETE FROM stock_allocation WHERE id = $1", &[&allocation])
        .await
        .expect("the test removes its own claim");
    c.execute(
        "DELETE FROM client_event WHERE client_event_id = ANY($1)",
        &[&acts.to_vec()],
    )
    .await
    .expect("the acts go too");
}

/// A prepack list is thousands of item dimensions, and the item arm had never
/// been written.
///
/// **`observable` has had one since migration 7** — keyed by (item, packaging
/// level, packing config), with `applies_to` on length, width, height and the
/// weights already listing `item` — and `/observations` said in its own doc that
/// item subjects were absent because no screen needed them. This is the path a
/// prepack list takes, so it needs to work before the list arrives rather than
/// after.
#[actix_web::test]
async fn what_a_carton_of_something_measures_is_a_fact_about_the_kind() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)
            .configure(spork_server::web::configure)).await;

    let bearer = common::bearer(&app).await;

    // The fixture's transcribed prepack line, read back.
    let measured = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/items/{GUMBOOT}/measurements"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /items/{id}/measurements",
    )
    .await;
    let carton = measured
        .as_array()
        .expect("levels")
        .iter()
        .find(|r| r["packaging_level"] == "carton")
        .expect("a carton of these has been measured");
    // **Inherited, not its own.** The prepack list measures `STY-7720` and the
    // catalogue sells thirteen sizes of it; nobody put a carton of size 8 on a
    // scale. Migration 73 keeps that distinction rather than copying the style's
    // numbers onto every variant and calling each one measured.
    assert_eq!(carton["source"], "style", "the measurement came from the style");
    assert_eq!(carton["style_code"], "STY-7720");
    // Canonical throughout: the sheet said 45 cm and the database holds 450 mm.
    assert_eq!(carton["length_mm"], 450);
    assert_eq!(carton["width_mm"], 340);
    assert_eq!(carton["height_mm"], 410);
    assert_eq!(carton["gross_weight_g"], 11400);
    assert_eq!(
        carton["method"], "transcribed",
        "a prepack sheet is transcribed, not measured — and a cubing scanner's \
         answer for the same carton would be a different fact about it"
    );
    assert!(
        carton["item_packing_config_id"].is_string(),
        "a carton is only a definite object relative to a case pack, so the \
         subject names the one in force"
    );

    // And the write path the loader will use.
    let act = Uuid::now_v7();
    let recorded = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/observations")
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({
                "item_id": GUMBOOT,
                "packaging_level": "each",
                "measurements": [
                    { "metric": "gross_weight", "entered_value": "1.9", "unit": "kg" },
                    { "metric": "height", "entered_value": "38", "unit": "cm" }
                ],
                // D138. A pair of boots is a single loose thing, and the
                // writer refuses a length at `each` with no arrangement. A
                // prepack sheet describes the pair as it comes.
                "presentation": "as_supplied",
                "method": "transcribed",
                "ingestion_channel": "csv",
                "client_event_id": act,
                "occurred_at": chrono::Utc::now().to_rfc3339()
            }))
            .to_request(),
        "POST /observations against an item",
    )
    .await;
    assert_eq!(recorded["observation_ids"].as_array().unwrap().len(), 2);

    // **The same subject twice finds the same registry row.** Before migration 72
    // the item arm's unique index was NULLS DISTINCT, so `each` — whose packing
    // config is legitimately null — conflicted with nothing and minted a rival
    // subject on every line of the list.
    let again = Uuid::now_v7();
    let second = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/observations")
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({
                "item_id": GUMBOOT,
                "packaging_level": "each",
                "measurements": [
                    { "metric": "gross_weight", "entered_value": "1.95", "unit": "kg" }
                ],
                "client_event_id": again,
                "occurred_at": chrono::Utc::now().to_rfc3339()
            }))
            .to_request(),
        "POST /observations a second time",
    )
    .await;
    assert_eq!(
        recorded["observable_id"], second["observable_id"],
        "the second observer of an each finds the first one's subject"
    );

    // **A measurement of this code beats one of its style**, which is D22's
    // most-specific-wins one level down. Nothing about the style row changes;
    // the resolution simply stops reaching for it.
    let own = Uuid::now_v7();
    common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/observations")
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({
                "item_id": GUMBOOT,
                "packaging_level": "carton",
                "measurements": [
                    { "metric": "gross_weight", "entered_value": "12.1", "unit": "kg" }
                ],
                "method": "instrument", "ingestion_channel": "scale",
                "client_event_id": own,
                "occurred_at": chrono::Utc::now().to_rfc3339()
            }))
            .to_request(),
        "POST /observations against the variant itself",
    )
    .await;
    // **No tenant-wide refresh here.** `/projections/refresh` folds every
    // projection for the tenant, including `fulfilment_line.covered_quantity`.
    // Other tests in this binary commit an allocation, then remove it with raw
    // SQL and never re-fold — which was fine while nothing folded in between.
    // Calling refresh mid-flight wrote their allocations into the projection and
    // left it asserting coverage with nothing behind it, and J31 said so. It was
    // right. A test must not move shared state that outlives it.
    //
    // So specificity is proven against what the fixture already folded: the size
    // was weighed, the carton was not.
    let after = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/items/{GUMBOOT}/measurements"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /items/{id}/measurements after measuring the variant",
    )
    .await;
    let levels = after.as_array().unwrap();
    let at = |lvl: &str| {
        levels
            .iter()
            .find(|r| r["packaging_level"] == lvl)
            .unwrap_or_else(|| panic!("a {lvl} level"))
    };

    // **Per fact, not per subject, and the two levels resolve differently.**
    // Somebody weighed one pair of size 8; nobody weighed a carton of them. So
    // `each` is this code's own measurement and `carton` is still the style's.
    // Resolving by subject rather than by fact would have let the each-level
    // weight decide the carton row too.
    assert_eq!(at("each")["source"], "own", "the size was weighed here");
    assert_eq!(at("each")["gross_weight_g"], 1900);
    assert!(at("each")["style_code"].is_null());

    assert_eq!(at("carton")["source"], "style", "the carton was not");
    assert_eq!(at("carton")["style_code"], "STY-7720");
    assert_eq!(at("carton")["length_mm"], 450);
    assert_eq!(at("carton")["gross_weight_g"], 11400);

    // An item subject with no level is refused rather than guessed at.
    let bad = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/observations")
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({
                "item_id": GUMBOOT,
                "measurements": [{ "metric": "height", "entered_value": "38", "unit": "cm" }],
                "client_event_id": Uuid::now_v7(),
                "occurred_at": chrono::Utc::now().to_rfc3339()
            }))
            .to_request(),
    )
    .await;
    assert_eq!(bad.status(), 400, "a pair of boots and a carton of them differ");

    let (c, conn) = tokio_postgres::connect(&u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = conn.await;
    });
    // Facts first, then the subject they hang off, then the acts. The `each`
    // subject is this test's own; the fixture's carton subject stays.
    c.batch_execute(&format!(
        // **The facts by act, then only the subjects left with no facts on
        // them.** Deleting every observable for this item took the fixture's
        // each-level subject with it, and the next run had nothing to resolve —
        // the snapshot caught it, going 5 observables to 4 and staying there.
        //
        // A test may remove what it created and must leave the fixture exactly
        // as it found it, which here means the subject survives if anything else
        // still points at it.
        "DELETE FROM observation WHERE client_event_id IN ('{act}', '{again}', '{own}');
         DELETE FROM observation_event
           WHERE client_event_id IN ('{act}', '{again}', '{own}');
         DELETE FROM observation_current WHERE observable_id IN
             (SELECT o.id FROM observable o
               WHERE o.item_id = '{GUMBOOT}'
                 AND NOT EXISTS (SELECT 1 FROM observation_event e
                                  WHERE e.observable_id = o.id));
         DELETE FROM observable o
           WHERE o.item_id = '{GUMBOOT}'
             AND NOT EXISTS (SELECT 1 FROM observation_event e
                              WHERE e.observable_id = o.id);
         DELETE FROM client_event WHERE client_event_id IN ('{act}', '{again}', '{own}');"
    ))
    .await
    .expect("the test removes what it recorded");
}

/// The bench, as JSON, agreeing with the page that draws it.
///
/// **There is one query set, and now only one renderer.** `crate::bench` was
/// extracted from `web/` when D113 gave the screen to React, and the risk that
/// move carried was two renderings of "what is left to pack", free to disagree.
/// This asserted the page and the endpoint agreed; the page is gone, so the
/// risk is closed by construction and what is left to assert is the shape of
/// the answer the one remaining reader depends on.
#[actix_web::test]
async fn the_bench_reads_the_same_figures_the_page_draws() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)
            .configure(spork_server::web::configure)).await;

    let bearer = common::bearer(&app).await;

    // Unauthenticated is refused rather than served an empty bench, which is
    // what an RLS-only answer would look like.
    let anon = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/fulfilments/{GUMBOOT_FULFILMENT}/bench"))
            .to_request(),
    )
    .await;
    assert_eq!(anon.status(), 401, "a machine is told plainly");

    let bench: Value = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/fulfilments/{GUMBOOT_FULFILMENT}/bench"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
        "GET /fulfilments/{id}/bench",
    )
    .await;

    // The four things the screen opens with, in one round trip.
    let reference = bench["reference"].as_str().expect("a reference");
    assert!(bench["customer"].is_string());
    assert!(bench["dock_id"].is_string(), "somewhere to mint a carton, per D97");
    assert!(!bench["lines"].as_array().unwrap().is_empty(), "a job with work in it");
    assert!(
        !bench["presets"].as_array().unwrap().is_empty(),
        "and the preset catalogue, which is stage 4's whole input"
    );

    // A line offers cells to pick from, because question 26 is deferred and the
    // operator still has to choose one.
    let line = &bench["lines"][0];
    assert!(line["line_id"].is_string());
    assert!(line["item_code"].is_string());
    assert!(line["remaining"].is_i64());
    for cell in line["cells"].as_array().unwrap() {
        assert!(cell["available"].as_i64().unwrap() > 0, "only what is free to claim");
        assert!(cell["location"].is_string(), "and where it actually is");
    }

    assert!(!reference.is_empty(), "a job is called something");
}

/// The despatch bench, and the number the walkthrough says exists nowhere.
///
/// Stages 6 to 9 had no screen in any form, so this is the first thing to run
/// them. The assertion that matters is not that the endpoint answers — it is
/// that **a carton is on exactly one of the three lists**: waiting to be
/// consigned, booked and not gone, or gone today. A carton on two of them is a
/// screen telling an operator to send something twice.
#[actix_web::test]
async fn the_despatch_bench_puts_each_carton_on_one_list() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)
            .configure(spork_server::web::configure)).await;

    let bearer = common::bearer(&app).await;

    let anon = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/despatch"))
            .to_request(),
    )
    .await;
    assert_eq!(anon.status(), 401, "a machine is told plainly");

    let screen: Value = common::ok_json(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/despatch"))
            .insert_header(("authorization", bearer))
            .to_request(),
        "GET /sites/{id}/despatch",
    )
    .await;

    assert!(screen["site"].is_string(), "the bench says where it is");

    // **The route is data, not recall.** Stage 7 has the operator apply a
    // carrier's rules from memory; the fixture's two carriers are here with
    // their services attached, which is the whole of that removal.
    let carriers = screen["carriers"].as_array().expect("carriers");
    assert!(!carriers.is_empty(), "the fixture's carriers are offered");
    let named: Vec<&str> = carriers.iter().filter_map(|c| c["name"].as_str()).collect();
    assert!(named.contains(&"Swift Transport Services"));
    assert!(named.contains(&"Direct Transport"));
    assert!(
        carriers
            .iter()
            .any(|c| !c["services"].as_array().unwrap().is_empty()),
        "and a carrier carries its services, because D1 keeps them separate"
    );

    // The one invariant a despatch screen owes: no carton on two lists.
    let mut seen: Vec<String> = vec![];
    for job in screen["waiting"].as_array().unwrap() {
        for c in job["cartons"].as_array().unwrap() {
            seen.push(c["id"].as_str().unwrap().to_string());
        }
    }
    for con in screen["booked"].as_array().unwrap() {
        for p in con["packages"].as_array().unwrap() {
            seen.push(p["id"].as_str().unwrap().to_string());
        }
    }
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        seen.len(),
        "a carton is waiting or booked, never both: {seen:?}"
    );

    // A waiting job's total is unknown rather than light when a carton in it
    // was never weighed — the figure a carrier's invoice later disagrees with.
    for job in screen["waiting"].as_array().unwrap() {
        let any_unweighed = job["cartons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| !c["weighed"].as_bool().unwrap());
        if any_unweighed {
            assert!(
                job["gross_weight_g"].is_null(),
                "an unweighed carton makes the job's total unknown, not lighter"
            );
        }
    }
}

/// A photograph, attached to the look that produced it.
///
/// **The assertions that matter are the ones about bytes rather than rows.** An
/// endpoint that stores what a client says it stored, and serves it back with
/// the type the client claimed, is how an image upload becomes an XSS — so the
/// type is read from the bytes, a payload that is not an image is refused, and
/// a content address that could traverse a path never reaches the filesystem.
#[actix_web::test]
async fn a_photograph_hangs_off_the_look_that_produced_it() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };

    let store = std::env::temp_dir().join(format!("spork-images-{}", Uuid::now_v7()));
    std::env::set_var("SPORK_IMAGE_DIR", &store);

    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)
            .configure(spork_server::web::configure)).await;

    let bearer = common::bearer(&app).await;

    // One look at one carton: the figures first, then the pictures on the same
    // event. That join is the whole reason images do not carry their own event.
    let observation: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/observations")
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({
                "item_id": GUMBOOT,
                "packaging_level": "carton",
                "measurements": [
                    { "metric": "length", "entered_value": "600", "unit": "mm" },
                    { "metric": "width",  "entered_value": "400", "unit": "mm" },
                    { "metric": "height", "entered_value": "300", "unit": "mm" }
                ],
                "method": "instrument",
                "ingestion_channel": "keyed",
                "client_event_id": Uuid::now_v7(),
                "occurred_at": Utc::now()
            }))
            .to_request(),
        "POST /observations",
    )
    .await;
    let event = observation["observation_event_id"]
        .as_str()
        .expect("the event the figures went on")
        .to_string();

    // A one-pixel PNG, which is a real PNG and therefore a real photograph as
    // far as anything here is concerned.
    let png: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    let stored: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri(&format!("/observations/{event}/images/front"))
            .insert_header(("authorization", bearer.clone()))
            .insert_header(("content-type", "image/png"))
            .set_payload(png.clone())
            .to_request(),
        "POST /observations/{id}/images/front",
    )
    .await;

    let digest = stored["digest"].as_str().expect("a content address");
    assert_eq!(digest.len(), 64, "sha-256, lower-case hex");
    assert_eq!(stored["mime"], "image/png", "read from the bytes");
    assert_eq!(stored["width_px"], 1, "and so are the pixels");
    assert_eq!(stored["height_px"], 1);

    // **The address is the content.** Sending the same bytes again is a retake,
    // not a second file and not a second row.
    let again: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri(&format!("/observations/{event}/images/front"))
            .insert_header(("authorization", bearer.clone()))
            .insert_header(("content-type", "image/png"))
            .set_payload(png.clone())
            .to_request(),
        "POST the same face twice",
    )
    .await;
    // Same bytes, same address, one file — and a *new row*, because a retake
    // is a thing that happened and this is a fact table.
    assert_eq!(again["digest"], stored["digest"], "same bytes, same address");
    assert_ne!(
        again["image_id"], stored["image_id"],
        "a retake appends rather than editing what was already recorded"
    );

    // It comes back as what it is, with the headers that keep it inert.
    let served = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/images/{digest}"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
    )
    .await;
    assert!(served.status().is_success());
    let headers = served.headers().clone();
    assert_eq!(headers.get("content-type").unwrap(), "image/png");
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert!(headers.get("content-security-policy").is_some());
    assert_eq!(test::read_body(served).await.to_vec(), png, "byte for byte");

    // Not an image, however it is labelled.
    let refused = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/observations/{event}/images/back"))
            .insert_header(("authorization", bearer.clone()))
            .insert_header(("content-type", "image/png"))
            .set_payload(b"<svg onload=alert(1)>".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(refused.status(), 400, "the bytes decide, not the header");

    // Not a face.
    let nonsense = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/observations/{event}/images/sideways"))
            .insert_header(("authorization", bearer.clone()))
            .insert_header(("content-type", "image/png"))
            .set_payload(png.clone())
            .to_request(),
    )
    .await;
    assert_eq!(nonsense.status(), 400, "seven faces, and that is not one");

    // And a path that is not an address never reaches the filesystem.
    for bad in ["../../etc/passwd", "not-a-digest", &"A".repeat(64)] {
        let r = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/images/{bad}"))
                .insert_header(("authorization", bearer.clone()))
                .to_request(),
        )
        .await;
        assert!(
            r.status() == 400 || r.status() == 404,
            "{bad} must not be read: {}",
            r.status()
        );
    }

    let anon = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/images/{digest}"))
            .to_request(),
    )
    .await;
    assert_eq!(anon.status(), 401, "a content address is not a secret");

    let _ = std::fs::remove_dir_all(&store);
}
