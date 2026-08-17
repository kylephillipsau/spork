//! D138 and D139 over HTTP: an arrangement recorded, an absence declared, and a
//! part that is a subject.
//!
//! The unit tests own the pure halves — `observing::check_absent_reason`,
//! `observing::presentation_required`, `capture::classify` over its nine
//! combinations. What they cannot reach is the four things this file is for.
//!
//! **That the writer refuses.** `presentation_required` returning true is not
//! the same as a request being rejected, and the gap between them is where the
//! rule was nearly written after the event row went in — a rejection arriving
//! after four observations have been inserted is a rejection that has already
//! half-happened.
//!
//! **That an absence survives the projection.** `observation_current` had no
//! `absent_reason` column until migration 81, and the whole of D138's second
//! half is that a declared *not applicable* stops looking like silence. That
//! only shows up after a rebuild, which is a round trip through SQL no unit
//! test performs.
//!
//! **That a part is a subject the write path accepts and the read enumerates.**
//! Two enumerations that disagree is the failure D108 named and `capture.rs`
//! structurally prevents; a part reachable by the writer and invisible to the
//! worklist would be that failure in its other direction.
//!
//! **That the two together settle a subject.** The pan set is the case: a
//! weight off a scale, dimensions declared absent, and it must stop being
//! asked for rather than sit on `unconfirmed` for ever because the newest
//! method resolved to the keyed act that declared the absence.
//!
//! Written against rows this file creates and cleans up after, because the
//! suite runs against a database other tests have already written to.

use actix_web::{test, web, App};
use nylonite_server::{routes, AppState};
use serde_json::{json, Value};

mod common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

/// A direct connection, for the rows this test owns and for the rebuild.
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
            "SELECT set_config('nylonite.tenant_id', $1, false)",
            &[&TENANT],
        )
        .await
        .expect("the tenant scope");
    client
}

/// Everything this test owns, removed in the one order that works.
///
/// **`observation_current` first.** It carries foreign keys to both
/// `observation` and `observable`, so deleting the facts underneath it fails
/// and the first draft's `.ok()` swallowed that — which left the item behind,
/// and the next run of the suite failed on its unique code instead of on the
/// thing that actually went wrong. Run at both ends: a test that cleans up
/// only on success leaves the mess exactly when there is a mess.
async fn remove(db: &tokio_postgres::Client, code: &str) {
    for sql in [
        "DELETE FROM observation_current oc USING observable ob, item i
          WHERE ob.id = oc.observable_id AND i.code = $1
            AND (ob.item_id = i.id
                 OR ob.item_part_id IN (SELECT id FROM item_part WHERE item_id = i.id))",
        "DELETE FROM observation o USING observable ob, item i
          WHERE ob.id = o.observable_id AND i.code = $1
            AND (ob.item_id = i.id
                 OR ob.item_part_id IN (SELECT id FROM item_part WHERE item_id = i.id))",
        "DELETE FROM observation_event e USING observable ob, item i
          WHERE ob.id = e.observable_id AND i.code = $1
            AND (ob.item_id = i.id
                 OR ob.item_part_id IN (SELECT id FROM item_part WHERE item_id = i.id))",
        "DELETE FROM observable ob USING item i
          WHERE i.code = $1
            AND (ob.item_id = i.id
                 OR ob.item_part_id IN (SELECT id FROM item_part WHERE item_id = i.id))",
        "DELETE FROM item_part p USING item i WHERE i.code = $1 AND p.item_id = i.id",
        "DELETE FROM item WHERE code = $1",
    ] {
        db.execute(sql, &[&code])
            .await
            .unwrap_or_else(|e| panic!("cleaning up {code}: {sql}\n{e}"));
    }
}

#[actix_web::test]
async fn an_arrangement_is_required_an_absence_is_an_answer_and_a_part_is_a_subject() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let db = admin(&u).await;

    // ── the subject this test owns ──────────────────────────────────────
    //
    // Its own item rather than the fixture's, because the assertions below are
    // about what is recorded against *this* code and the suite shares one
    // database. The code carries the test's name for the same reason.
    let code = "TEST-D138-PANSET";
    remove(&db, code).await;
    let item: uuid::Uuid = db
        .query_one(
            "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
             SELECT $1::uuid, $2, 'Lobby pan set, for D138', u.id, 'none'
               FROM unit u WHERE u.code = 'ea'
             RETURNING id",
            &[&uuid::Uuid::parse_str(TENANT).unwrap(), &code],
        )
        .await
        .expect("the test's own item")
        .get(0);

    let handle: uuid::Uuid = db
        .query_one(
            "INSERT INTO item_part (tenant_id, item_id, code, label, ordinal)
             VALUES ($1, $2, 'HANDLE', 'Handle', 1) RETURNING id",
            &[&uuid::Uuid::parse_str(TENANT).unwrap(), &item],
        )
        .await
        .expect("a part")
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
                    .uri("/observations")
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

    // ── 1. a length at `each` with no arrangement is refused ─────────────
    let (status, said) = post(json!({
        "item_id": item,
        "packaging_level": "each",
        "measurements": [{ "metric": "length", "entered_value": "330", "unit": "mm" }],
        "client_event_id": uuid::Uuid::new_v4(),
        "occurred_at": "2026-08-17T02:00:00Z",
    }))
    .await;
    assert_eq!(status, 400, "a length of a single thing needs the word: {said}");
    assert!(
        said.contains("presentation"),
        "the refusal says what is missing rather than naming a column: {said}"
    );

    // **And it refused before writing anything.** The rule is checked ahead of
    // the event row precisely so that a rejection is not half an act.
    let events: i64 = db
        .query_one(
            "SELECT count(*) FROM observation_event e
               JOIN observable o ON o.id = e.observable_id
              WHERE o.item_id = $1",
            &[&item],
        )
        .await
        .expect("count")
        .get(0);
    assert_eq!(events, 0, "the refused act left an event behind");

    // ── 2. a weight needs no arrangement ────────────────────────────────
    //
    // Arranging a thing does not change what it weighs, which is why the rule
    // names three metrics rather than all of them.
    let (status, said) = post(json!({
        "item_id": item,
        "packaging_level": "each",
        "measurements": [{ "metric": "gross_weight", "entered_value": "2.1", "unit": "kg" }],
        "method": "instrument",
        "client_event_id": uuid::Uuid::new_v4(),
        "occurred_at": "2026-08-17T02:01:00Z",
    }))
    .await;
    assert!(status.is_success(), "a weight needs no arrangement: {said}");

    // ── 3. the absence, declared ─────────────────────────────────────────
    let (status, said) = post(json!({
        "item_id": item,
        "packaging_level": "each",
        "measurements": [
            { "metric": "length", "absent_reason": "not_applicable" },
            { "metric": "width",  "absent_reason": "not_applicable" },
            { "metric": "height", "absent_reason": "not_applicable" },
        ],
        "client_event_id": uuid::Uuid::new_v4(),
        "occurred_at": "2026-08-17T02:02:00Z",
    }))
    .await;
    assert!(
        status.is_success(),
        "an absence needs no arrangement — there is no number to reproduce: {said}"
    );

    // ── 4. the two words a capture may not write ─────────────────────────
    for (word, why) in [
        ("not_measured", "silence is not a fact somebody recorded"),
        ("retracted", "a retraction names the row it retracts"),
    ] {
        let (status, said) = post(json!({
            "item_id": item,
            "packaging_level": "each",
            "measurements": [{ "metric": "length", "absent_reason": word }],
            "client_event_id": uuid::Uuid::new_v4(),
            "occurred_at": "2026-08-17T02:03:00Z",
        }))
        .await;
        assert_eq!(status, 400, "{word} was accepted, and {why}: {said}");
    }

    // A value and an absence in one measurement is two answers to one question.
    let (status, said) = post(json!({
        "item_id": item,
        "packaging_level": "each",
        "measurements": [{
            "metric": "length", "entered_value": "330", "unit": "mm",
            "absent_reason": "not_applicable"
        }],
        "presentation": "knocked_down",
        "client_event_id": uuid::Uuid::new_v4(),
        "occurred_at": "2026-08-17T02:04:00Z",
    }))
    .await;
    assert_eq!(status, 400, "both a value and an absence was accepted: {said}");

    // ── 5. the part, measured, arrangement optional ──────────────────────
    let (status, said) = post(json!({
        "item_part_id": handle,
        "measurements": [
            { "metric": "gross_weight", "entered_value": "0.6", "unit": "kg" },
            { "metric": "length", "entered_value": "1200", "unit": "mm" },
            { "metric": "width",  "entered_value": "60",   "unit": "mm" },
            { "metric": "height", "entered_value": "40",   "unit": "mm" },
        ],
        "presentation": "as_supplied",
        "method": "instrument",
        "client_event_id": uuid::Uuid::new_v4(),
        "occurred_at": "2026-08-17T02:05:00Z",
    }))
    .await;
    assert!(status.is_success(), "a part is a subject: {said}");
    let recorded: Value = serde_json::from_str(&said).expect("JSON");
    assert_eq!(
        recorded["observation_ids"].as_array().map(Vec::len),
        Some(4),
        "four figures from one act, which is what D133 says a session is"
    );

    // A part takes no level, and saying one is a different subject entirely.
    let (status, said) = post(json!({
        "item_part_id": handle,
        "packaging_level": "each",
        "measurements": [{ "metric": "gross_weight", "entered_value": "0.6", "unit": "kg" }],
        "client_event_id": uuid::Uuid::new_v4(),
        "occurred_at": "2026-08-17T02:06:00Z",
    }))
    .await;
    assert_eq!(status, 400, "a part with a packaging level was accepted: {said}");

    // The arrangement landed on the act rather than on any one figure.
    let arranged: i64 = db
        .query_one(
            "SELECT count(*) FROM observation_event e
               JOIN observable o ON o.id = e.observable_id
               JOIN presentation p ON p.id = e.presentation_id
              WHERE o.item_part_id = $1 AND p.code = 'as_supplied'",
            &[&handle],
        )
        .await
        .expect("count")
        .get(0);
    assert_eq!(arranged, 1, "one act, one arrangement");

    // ── 6. what the worklist says once the fold has run ──────────────────
    db.execute(
        "SELECT projection_observation_current_rebuild($1)",
        &[&uuid::Uuid::parse_str(TENANT).unwrap()],
    )
    .await
    .expect("the fold");

    // **Through the scan rather than the walk, and the reason is the walk's.**
    // `GET /capture` is a lap of the building now: it lists what stock says is
    // on a shelf at this site, in bin order, because a row nobody can walk to
    // is not work. This test's set is a bare catalogue row with no stock, so it
    // is correctly absent from that list — and `subjects_for_item`, which is
    // what a scan lands on, answers for a subject whether or not it is on a
    // shelf. Its own comment says why: *answering "nothing" to a box in
    // somebody's hand is not an answer.* Same rows, same read, no shelf.
    let resolution: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/resolve?scan={code}"))
                .insert_header(("authorization", bearer.clone()))
                .to_request(),
        )
        .await;
        assert!(r.status().is_success(), "the scan resolves");
        test::read_body_json(r).await
    };

    let rows: Vec<&Value> = resolution["subjects"]
        .as_array()
        .expect("the scan found the item")
        .iter()
        .flat_map(|s| s["capture"].as_array().cloned().unwrap_or_default())
        .collect::<Vec<Value>>()
        .leak()
        .iter()
        .collect();

    let set = rows
        .iter()
        .find(|s| s["item_id"].as_str() == Some(&item.to_string()))
        .expect("the set's each is a subject");
    assert_eq!(
        set["dimensions_absent"], true,
        "the declared absence reached the read: {set}"
    );
    assert_eq!(
        set["parts"], 1,
        "the read says the box to measure is not this one: {set}"
    );
    let wants: Vec<&str> = set["wants"]
        .as_array()
        .expect("wants")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        !wants.contains(&"dimensions"),
        "the worklist is still asking for dimensions it was told do not exist: {wants:?}"
    );
    assert!(
        !wants.contains(&"weight"),
        "the weight was recorded and is still being asked for: {wants:?}"
    );

    // **The one that would have been silent.** With absences left in the method
    // aggregation the newest method here is the `keyed` act that declared them,
    // `ever_measured("keyed")` is false, and the subject sits on `unconfirmed`
    // for ever with nothing to re-measure. The weight came off a scale.
    assert_eq!(
        set["method"], "instrument",
        "an absence decided how the figures were come by: {set}"
    );

    let part = rows
        .iter()
        .find(|s| s["item_part_id"].as_str() == Some(&handle.to_string()));
    if let Some(part) = part {
        // It is complete and measured, so it is only here if it has aged — and
        // either way it must carry no level and name its part.
        assert!(part["packaging_level"].is_null(), "a part has no level: {part}");
        assert_eq!(part["part_label"], "Handle", "the part names itself: {part}");
    }

    // ── 7. J72 holds over everything this wrote ─────────────────────────
    let unqualified: i64 = db
        .query_one(
            "SELECT count(*) FROM observation o
               JOIN observable ob ON ob.id = o.observable_id
               JOIN observation_event e ON e.id = o.observation_event_id
               JOIN metric m ON m.id = o.metric_id
              WHERE ob.packaging_level = 'each'
                AND m.code IN ('length','width','height')
                AND o.absent_reason IS NULL
                AND e.asserted_by_party_id IS NULL
                AND e.presentation_id IS NULL",
            &[],
        )
        .await
        .expect("count")
        .get(0);
    assert_eq!(
        unqualified, 0,
        "J72: a single thing's size with no arrangement got through the writer"
    );

    // ── clean up after itself ───────────────────────────────────────────
    remove(&db, code).await;
    db.execute(
        "SELECT projection_observation_current_rebuild($1)",
        &[&uuid::Uuid::parse_str(TENANT).unwrap()],
    )
    .await
    .ok();
}
