//! D140 and D141 over HTTP: a record points at a look, and a picker gets a
//! picture.
//!
//! Four things no unit test reaches.
//!
//! **That the act is one act.** The event and the link are written in one
//! transaction, so there is no window where a photographic look exists that
//! nothing points at. The only way to see that is to make the call and count
//! both.
//!
//! **That the replay branch answers.** `POST /evidence` claims a
//! `client_event_id` like every other write, and a retry after a timeout has to
//! return the same event id — otherwise the photographs that were about to
//! follow have nowhere to go, which is the failure the whole idempotency
//! registry exists to prevent.
//!
//! **That a picture inherits and says so.** D141's entire honesty is the
//! `own`/`style` label, and the resolution that produces it is SQL. A test of
//! the Rust would prove nothing about which observable won.
//!
//! **That the pick walk is in walking order.** `location.pick_sequence` had
//! never been read by anything, so "ordered by the walk" was an untested claim
//! about a column with no consumer.

use actix_web::{test, web, App};
use spork_server::{routes, AppState};
use serde_json::{json, Value};

mod common;
use common::{pool, url};

const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const TENANT: &str = "11111111-1111-1111-1111-111111111111";
/// The fixture's nitrile glove, which has a location and open lines.
const GLOVE: &str = "17e10000-0000-0000-0000-000000000001";

async fn admin(u: &str) -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    // **The tenant, or every delete below matches nothing.** These tables force
    // row-level security and their policies read `current_tenant()`, which is
    // unset on a fresh connection — so a cleanup runs, reports success and
    // removes not one row. The inserts still work, because a policy with only
    // a `USING` clause does not constrain INSERT: D55's hole, met from the
    // other side, in a test that would otherwise have looked like it tidied up
    // after itself.
    client
        .execute(
            "SELECT set_config('spork.tenant_id', $1, false)",
            &[&TENANT],
        )
        .await
        .expect("the tenant scope");
    client
}

#[actix_web::test]
async fn a_record_points_at_the_look_that_supports_it() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let db = admin(&u).await;
    let tenant = uuid::Uuid::parse_str(TENANT).unwrap();

    // A finding to attach something to. Its own rather than the fixture's,
    // because the assertions below count what is attached to *this* one.
    let finding: uuid::Uuid = db
        .query_one(
            "INSERT INTO discrepancy
                 (tenant_id, kind, item_id, detail, automation_key)
             VALUES ($1, 'identity_mismatch', $2, 'for D140', 'test.d140')
             RETURNING id",
            &[&tenant, &uuid::Uuid::parse_str(GLOVE).unwrap()],
        )
        .await
        .expect("a finding")
        .get(0);

    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let bearer = common::bearer(&app).await;

    let post = |body: Value| {
        let app = &app;
        let bearer = bearer.clone();
        async move {
            let r = test::call_service(
                app,
                test::TestRequest::post()
                    .uri("/evidence")
                    .insert_header(("authorization", bearer))
                    .set_json(body)
                    .to_request(),
            )
            .await;
            let status = r.status();
            let text = String::from_utf8_lossy(&test::read_body(r).await).to_string();
            (status, text)
        }
    };

    // ── the act ─────────────────────────────────────────────────────────
    let act = uuid::Uuid::new_v4();
    let (status, said) = post(json!({
        "discrepancy_id": finding,
        // The subject is what was held up to the camera, and is not inferred
        // from the finding — which names an item, a location and a package
        // without those being exclusive.
        "item_id": GLOVE,
        "packaging_level": "each",
        "note": "crushed on the left face",
        "client_event_id": act,
        "occurred_at": "2026-08-17T04:00:00Z",
    }))
    .await;
    assert!(status.is_success(), "a look in support of a finding: {said}");
    let recorded: Value = serde_json::from_str(&said).expect("JSON");
    let event = recorded["observation_event_id"]
        .as_str()
        .expect("the photographs need an event")
        .to_string();

    // **One act, both rows.** The link is the act's fact, so there is never a
    // look that nothing points at.
    let (links, method): (i64, String) = {
        let r = db
            .query_one(
                "SELECT (SELECT count(*) FROM evidence WHERE discrepancy_id = $1),
                        (SELECT method::text FROM observation_event WHERE id = $2)",
                &[&finding, &uuid::Uuid::parse_str(&event).unwrap()],
            )
            .await
            .expect("counts");
        (r.get(0), r.get(1))
    };
    assert_eq!(links, 1, "the link and the event are one act");
    assert_eq!(
        method, "photographed",
        "a camera is not a measuring instrument, and saying it is would let \
         revalidation read a photograph as a weighing"
    );

    // ── the replay ──────────────────────────────────────────────────────
    //
    // The photographs were about to follow. A retry that answered with a fresh
    // event, or with nothing, would leave them nowhere to go.
    let (status, said) = post(json!({
        "discrepancy_id": finding,
        "item_id": GLOVE,
        "packaging_level": "each",
        "client_event_id": act,
        "occurred_at": "2026-08-17T04:00:00Z",
    }))
    .await;
    assert!(status.is_success(), "a replay answers: {said}");
    let again: Value = serde_json::from_str(&said).expect("JSON");
    assert_eq!(
        again["observation_event_id"], recorded["observation_event_id"],
        "a retry got a different event, so the photographs it was about to send \
         would hang off nothing"
    );
    assert_eq!(again["evidence_id"], recorded["evidence_id"]);

    // ── and it refuses what it should ───────────────────────────────────
    for (what, body) in [
        (
            "two records",
            json!({
                "discrepancy_id": finding, "stock_count_id": finding,
                "item_id": GLOVE, "packaging_level": "each",
                "client_event_id": uuid::Uuid::new_v4(),
                "occurred_at": "2026-08-17T04:01:00Z" }),
        ),
        (
            "no record",
            json!({
                "item_id": GLOVE, "packaging_level": "each",
                "client_event_id": uuid::Uuid::new_v4(),
                "occurred_at": "2026-08-17T04:01:00Z" }),
        ),
        (
            "no subject",
            json!({
                "discrepancy_id": finding,
                "client_event_id": uuid::Uuid::new_v4(),
                "occurred_at": "2026-08-17T04:01:00Z" }),
        ),
    ] {
        let (status, said) = post(body).await;
        assert_eq!(status, 400, "{what} was accepted: {said}");
    }

    // **A photographic look writes no observation**, which is why it can never
    // make a subject look measured. Asserted rather than assumed, because
    // `MEASURED_METHODS` is what stands between a photograph and a subject
    // dropping off the revalidation list.
    let figures: i64 = db
        .query_one(
            "SELECT count(*) FROM observation WHERE observation_event_id = $1",
            &[&uuid::Uuid::parse_str(&event).unwrap()],
        )
        .await
        .expect("count")
        .get(0);
    assert_eq!(figures, 0, "a look that produced a picture recorded a figure");

    // ── clean up ────────────────────────────────────────────────────────
    for sql in [
        "DELETE FROM evidence WHERE discrepancy_id = $1",
        "DELETE FROM discrepancy WHERE id = $1",
    ] {
        db.execute(sql, &[&finding]).await.expect("cleanup");
    }
    db.execute(
        "DELETE FROM observation_event WHERE id = $1",
        &[&uuid::Uuid::parse_str(&event).unwrap()],
    )
    .await
    .expect("cleanup the look");
    db.execute("DELETE FROM client_event WHERE client_event_id = $1", &[&act])
        .await
        .ok();
}

#[actix_web::test]
async fn the_pick_walk_is_in_walking_order_and_says_whose_picture_it_is() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let bearer = common::bearer(&app).await;

    let anon = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/picking"))
            .to_request(),
    )
    .await;
    assert_eq!(anon.status(), 401, "a machine is told plainly");

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/sites/{SITE}/picking"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
    )
    .await;
    let status = resp.status();
    let text = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(
        status.is_success(),
        "GET /sites/{{id}}/picking returned {status}: {}",
        if text.is_empty() {
            "(empty body — is the route registered?)"
        } else {
            &text
        }
    );
    let screen: Value = serde_json::from_str(&text).expect("the walk is JSON");
    assert!(screen["site"].is_string(), "the walk says where it is");

    let lines = screen["lines"].as_array().expect("lines");
    // **The loop below proves nothing over an empty list**, which is the
    // vacuity the invariant register keeps a column for. The fixture has open
    // lines; if it stops having them this fails loudly rather than passing
    // quietly.
    assert!(!lines.is_empty(), "the fixture has open lines and the walk found none");

    // ── the order is a route ────────────────────────────────────────────
    //
    // `location.pick_sequence` had no consumer before this read, so "in walking
    // order" was a claim about a column nothing exercised. Nulls sort last: a
    // bin nobody has placed on the route is a bin you walk to when you can.
    let mut seen_null = false;
    let mut previous: Option<i64> = None;
    for line in lines {
        match line["pick_sequence"].as_i64() {
            Some(seq) => {
                assert!(
                    !seen_null,
                    "a placed bin came after an unplaced one, so the list is not a walk"
                );
                if let Some(p) = previous {
                    assert!(p <= seq, "the walk goes backwards: {p} then {seq}");
                }
                previous = Some(seq);
            }
            None => seen_null = true,
        }
    }

    // ── and every row says what it is ───────────────────────────────────
    for line in lines {
        let code = line["item_code"].as_str().expect("a line names its item");
        assert!(
            line["remaining"].as_i64().unwrap_or(0) > 0,
            "{code} is on the walk with nothing left to pick"
        );

        // **A cell and a bin travel together.** A `stock_id` with no location
        // is a row telling somebody to walk nowhere, and a location with no
        // cell is a row `POST /picks` could not be built from.
        assert_eq!(
            line["stock_id"].is_string(),
            line["location_code"].is_string(),
            "{code} names one half of a place to pick from: {line}"
        );

        // **D141's whole honesty.** A picture with no source is a picture a
        // screen cannot label, and an unlabelled inherited one claims to be a
        // photograph of this code.
        if !line["picture"].is_null() {
            let source = line["picture"]["source"]
                .as_str()
                .unwrap_or_else(|| panic!("{code}'s picture has no source: {line}"));
            assert!(
                source == "own" || source == "style",
                "{code}'s picture claims a source that is neither: {source}"
            );
            assert!(
                line["picture"]["digest"]
                    .as_str()
                    .is_some_and(|d| d.len() == 64),
                "{code}'s picture is not a content address: {line}"
            );
        }
    }
}

/// A picture inherits from the style, and the answer says so.
///
/// **The path the HTTP test cannot reach.** Whether a borrowed picture turns up
/// on the walk depends on which items happen to have open lines, so asserting
/// it there would be asserting something about the fixture. This asserts it
/// about the resolution: an item with a style that has been photographed and no
/// photograph of its own resolves to the style's, labelled `style`.
///
/// Without it, D141's whole claim — that the inheritance is honest because it
/// is labelled — rests on a branch nothing runs.
#[actix_web::test]
async fn a_picture_inherits_from_the_style_and_says_so() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let db = admin(&u).await;
    let tenant = uuid::Uuid::parse_str(TENANT).unwrap();
    let code = "TEST-D141-VARIANT";

    /// **One parameter, everywhere.** The first draft passed two to statements
    /// that named one, which `tokio_postgres` refuses — and every delete
    /// therefore failed, silently, behind `.ok()`. So the variant is reached
    /// through its style rather than by its own code, and nothing here takes a
    /// second argument to get out of step.
    async fn cleanup(db: &tokio_postgres::Client) {
        for sql in [
            "DELETE FROM observation_image oi USING observation_event e, observable ob, item_style s
              WHERE oi.observation_event_id = e.id AND ob.id = e.observable_id
                AND s.id = ob.item_style_id AND s.code = $1",
            "DELETE FROM observation_current oc USING observable ob, item_style s
              WHERE ob.id = oc.observable_id AND s.id = ob.item_style_id AND s.code = $1",
            "DELETE FROM observation o USING observable ob, item_style s
              WHERE ob.id = o.observable_id AND s.id = ob.item_style_id AND s.code = $1",
            "DELETE FROM observation_event e USING observable ob, item_style s
              WHERE ob.id = e.observable_id AND s.id = ob.item_style_id AND s.code = $1",
            "DELETE FROM observable ob USING item_style s
              WHERE s.id = ob.item_style_id AND s.code = $1",
            "DELETE FROM item WHERE style_id IN (SELECT id FROM item_style WHERE code = $1)",
            "DELETE FROM item_style WHERE code = $1",
        ] {
            // **Not `.ok()`.** A cleanup that swallows its own errors leaves the
            // rows behind and reports success, and the next run fails on a
            // unique code instead of on the thing that actually went wrong.
            db.execute(sql, &[&"TEST-D141-STYLE"])
                .await
                .unwrap_or_else(|e| panic!("cleaning up: {sql}\n{e}"));
        }
    }
    cleanup(&db).await;

    let style: uuid::Uuid = db
        .query_one(
            "INSERT INTO item_style (tenant_id, code, description)
             VALUES ($1, 'TEST-D141-STYLE', 'for D141') RETURNING id",
            &[&tenant],
        )
        .await
        .expect("a style")
        .get(0);
    let variant: uuid::Uuid = db
        .query_one(
            "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking, style_id)
             SELECT $1, $2, 'never photographed', u.id, 'none', $3
               FROM unit u WHERE u.code = 'ea'
             RETURNING id",
            &[&tenant, &code, &style],
        )
        .await
        .expect("a variant")
        .get(0);

    // A front photograph against the style's carton, and none against the
    // variant. Written directly: what is under test is the resolution, and
    // going through the writer would test the writer.
    let act = uuid::Uuid::new_v4();
    db.execute(
        "INSERT INTO client_event
             (tenant_id, client_event_id, recorded_by_id, submitted_at, received_at)
         VALUES ($1, $2, '77770000-0000-0000-0000-000000000001', now(), now())",
        &[&tenant, &act],
    )
    .await
    .expect("the act");
    let subject: uuid::Uuid = db
        .query_one(
            "INSERT INTO observable (tenant_id, item_style_id, packaging_level)
             VALUES ($1, $2, 'each') RETURNING id",
            &[&tenant, &style],
        )
        .await
        .expect("the style subject")
        .get(0);
    let event: uuid::Uuid = db
        .query_one(
            "INSERT INTO observation_event
                 (tenant_id, client_event_id, observable_id, observed_at, recorded_by_id,
                  method, ingestion_channel)
             VALUES ($1, $2, $3, now(), '77770000-0000-0000-0000-000000000001',
                     'photographed', 'api')
             RETURNING id",
            &[&tenant, &act, &subject],
        )
        .await
        .expect("the look")
        .get(0);
    db.execute(
        "INSERT INTO observation_image
             (tenant_id, observation_event_id, face, digest, mime, byte_count, captured_at)
         VALUES ($1, $2, 'front',
                 'aaaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffff00000000111111ff',
                 'image/jpeg', 1024, now())",
        &[&tenant, &event],
    )
    .await
    .expect("the picture");

    let resolved = db
        .query_opt(
            &format!(
                "WITH {} SELECT digest, source FROM picture WHERE item_id = $1",
                spork_server::pictures::PICTURE_CTE
            ),
            &[&variant],
        )
        .await
        .expect("the resolution runs");

    let row = resolved.expect("a variant with a photographed style resolves to a picture");
    assert_eq!(
        row.get::<_, String>(1),
        "style",
        "the borrowed picture did not say it was borrowed, which is the whole of \
         what makes D141 honest"
    );

    cleanup(&db).await;
    db.execute("DELETE FROM client_event WHERE client_event_id = $1", &[&act])
        .await
        .ok();
}
