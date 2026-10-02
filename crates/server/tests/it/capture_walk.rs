//! A capture session, over HTTP, exactly as the handheld performs it.
//!
//! **D133 is a claim about a sequence of requests, and only a sequence of
//! requests can check it.** The unit tests prove the classifier is total and
//! `capture_worklist.rs` proves the read runs; neither touches the thing the
//! decision is actually about — that four figures and a photograph produced by
//! one look at one box end up on **one** `observation_event`, so D132's join
//! answers the question it was built for.
//!
//! That property has a specific way of failing and it is not a crash. A client
//! written against the obvious reading of a per-call endpoint sends the weight,
//! then the dimensions, then the pictures, and gets two events with the images
//! hanging off whichever was last. Every row involved looks correct. Nothing
//! errors. The join simply starts answering about a different subset of the
//! session than the person reading it believes.
//!
//! This is also the wiring test. `crates/server/tests/pack_walk_http.rs` exists
//! because thirty-one endpoints had been written and never executed, and three
//! defects shipped that way — a query against a table that does not exist, a
//! cast to a type that does not exist, and an INSERT on a column the
//! application holds no grant on. The image upload was in exactly that position
//! before this: tested in isolation, never reached from the act that precedes
//! it.
//!
//! # What it writes
//!
//! Handlers own their transactions, so this commits. It captures against the
//! fixture's own glove at `each`, then removes every row it made, because a
//! test that leaves observations behind changes what the next run of
//! `capture_worklist.rs` classifies.

use actix_web::{test, web, App};
use spork_server::{routes, AppState};
use serde_json::{json, Value};
use uuid::Uuid;

use super::common;
use common::{pool, url};

/// Nitrile glove, medium. Fixture-owned, and `each` needs no case pack.
const GLOVE: &str = "17e10000-0000-0000-0000-000000000001";

/// The smallest thing `images::sniff` will accept as a PNG.
///
/// A real encoder is not needed and would be worse: the endpoint reads the type
/// from the bytes rather than believing the header, so what is under test is
/// the sniff and the store, and eight magic bytes exercise both. The trailing
/// text keeps two "photographs" distinguishable, since a content address
/// deduplicates identical bytes by construction (D132) — two faces uploading
/// the same picture would otherwise share one file and prove less than it
/// looks.
fn png(tag: &str) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    bytes.extend_from_slice(tag.as_bytes());
    bytes
}

/// A PNG with a real IHDR, padded to `total` bytes.
///
/// `images::dimensions` reads the width and height as big-endian u32 at offsets
/// 16 and 20, which is where they sit after the 8-byte signature, the 4-byte
/// chunk length and `IHDR`. [`png`] stops before all that, so anything long
/// enough for the parser to look reads its size as 0 x 0 — and
/// `observation_image` requires both to be positive when either is present. A
/// fixture that is merely *long* therefore fails a constraint rather than
/// testing what it meant to.
fn png_sized(width: u32, height: u32, total: usize) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    bytes.extend_from_slice(&13u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.resize(total.max(bytes.len()), 0);
    bytes
}

#[actix_web::test]
async fn a_capture_session_is_one_event_with_figures_and_photographs_on_it() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };

    // Somewhere to put the bytes that is not an image's directory.
    let images = std::env::temp_dir().join(format!("spork-capture-{}", Uuid::new_v4()));
    std::env::set_var("SPORK_IMAGE_DIR", &images);

    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            // The binary's own registration, so "is it wired up" is a question
            // this suite answers rather than production.
            .configure(routes::configure),
    )
    .await;

    let bearer = common::bearer(&app).await;

    // ── the one act ─────────────────────────────────────────────────────
    //
    // Four metrics, one request, one `client_event_id`. This is the whole of
    // D133 and the only place it can be asserted.
    let client_event = Uuid::new_v4();
    let recorded: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/observations")
                .insert_header(("authorization", bearer.clone()))
                .set_json(json!({
                    "item_id": GLOVE,
                    "packaging_level": "each",
                    "measurements": [
                        // Entered as the operator typed them, in the units they
                        // read. 0.4 kg is not a double and must survive as 400 g
                        // exactly, which is the argument Principle 5 makes.
                        { "metric": "gross_weight", "entered_value": "0.4", "unit": "kg" },
                        { "metric": "length", "entered_value": "240", "unit": "mm" },
                        { "metric": "width",  "entered_value": "120", "unit": "mm" },
                        { "metric": "height", "entered_value": "35",  "unit": "mm" }
                    ],
                    // D138. A single glove is a loose thing, so the act says
                    // how it was arranged — a glove measured flat and a glove
                    // measured folded are two different boxes, and the writer
                    // refuses a length at `each` without the word.
                    "presentation": "flat",
                    "method": "instrument",
                    "ingestion_channel": "keyed",
                    "client_event_id": client_event,
                    "occurred_at": chrono::Utc::now(),
                }))
                .to_request(),
        )
        .await;
        let status = r.status();
        let body = test::read_body(r).await;
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(status.is_success(), "POST /observations returned {status}: {text}");
        serde_json::from_str(&text).expect("the act answers with JSON")
    };

    let event: String = recorded["observation_event_id"]
        .as_str()
        .expect("the act names the event the photographs hang off")
        .to_string();
    assert_eq!(
        recorded["observation_ids"].as_array().unwrap().len(),
        4,
        "four figures went in one request and four observations came out"
    );

    // ── the photographs, against that event ─────────────────────────────
    for face in ["front", "label"] {
        let r = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/observations/{event}/images/{face}"))
                .insert_header(("authorization", bearer.clone()))
                .insert_header(("content-type", "image/png"))
                .set_payload(png(face))
                .to_request(),
        )
        .await;
        let status = r.status();
        let body = test::read_body(r).await;
        assert!(
            status.is_success(),
            "photographing the {face} returned {status}: {}",
            String::from_utf8_lossy(&body)
        );
    }

    // ── the property D132 exists for ────────────────────────────────────
    let client = {
        let (c, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls)
            .await
            .expect("connect");
        tokio::spawn(async move {
            let _ = connection.await;
        });
        c
    };
    let event_id = Uuid::parse_str(&event).unwrap();

    // **One event for the whole session.** Not "at least one" — the failure
    // being guarded is a second event, so the count is the assertion.
    let events: i64 = client
        .query_one(
            "SELECT count(*) FROM observation_event WHERE client_event_id = $1",
            &[&client_event],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(events, 1, "one act, one look, one event");

    // And the join the whole photograph model was built to make possible:
    // same event, so same who, same when, same method, same box.
    let joined = client
        .query(
            "SELECT oi.face, o.metric_id IS NOT NULL
               FROM observation_image oi
               JOIN observation o ON o.observation_event_id = oi.observation_event_id
              WHERE oi.observation_event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap();
    assert_eq!(
        joined.len(),
        8,
        "two photographs joined to four figures without a correlated subquery: \
         that is what sharing the event buys"
    );

    let images_rows: i64 = client
        .query_one(
            "SELECT count(*) FROM observation_image WHERE observation_event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(images_rows, 2, "both faces landed on the session's own event");

    // The bytes are addressed rather than stored, so the row's digest has to
    // resolve to a file that is actually there.
    let digests: Vec<String> = client
        .query(
            "SELECT digest FROM observation_image WHERE observation_event_id = $1",
            &[&event_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    for digest in &digests {
        assert_eq!(digest.len(), 64, "a content address is a SHA-256 in hex");
        let served = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/images/{digest}"))
                .insert_header(("authorization", bearer.clone()))
                .to_request(),
        )
        .await;
        assert!(
            served.status().is_success(),
            "the row addresses bytes the store can find: {digest}"
        );
    }

    // ── and one the size a camera actually produces ─────────────────────
    //
    // **`images::MAX_BYTES` says twelve megabytes and reasons about a handheld
    // at full resolution, and for a long time nothing above 256kB could arrive
    // at all.** `web::Bytes` takes its ceiling from `PayloadConfig`, whose
    // default is 262,144 bytes, and nothing configured one — so actix refused
    // the body before the handler ran and the explicit size check in
    // `record_observation_image` was unreachable code. Every photograph in
    // every test was a few bytes long, so the suite never noticed.
    //
    // Half a megabyte is small for a photograph and twice the old ceiling,
    // which is the point: this fails on the default and passes on the real one.
    {
        let big = png_sized(3024, 4032, 512 * 1024);
        let r = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/observations/{event}/images/detail"))
                .insert_header(("authorization", bearer.clone()))
                .insert_header(("content-type", "image/png"))
                .set_payload(big)
                .to_request(),
        )
        .await;
        let status = r.status();
        let body = test::read_body(r).await;
        assert!(
            status.is_success(),
            "a 512kB photograph returned {status}: {}",
            String::from_utf8_lossy(&body)
        );
    }

    // ── and the exact figure survived the unit conversion ───────────────
    //
    // 0.4 kg is 400 g and a float would have made it 400.00000000000006.
    let grams: i64 = client
        .query_one(
            "SELECT o.value_numeric::bigint
               FROM observation o
               JOIN metric m ON m.id = o.metric_id
              WHERE o.observation_event_id = $1 AND m.code = 'gross_weight'",
            &[&event_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(grams, 400, "0.4 kg is exactly 400 g");

    // ── a look taken for its photographs, with nothing measured ─────────
    //
    // An item already weighed and measured that still wants a picture is
    // photographed on its own: one look, no figures, the photographs on it.
    // An empty look nobody asked for is still refused.
    let look = |client_event: Uuid, photographs: bool| {
        let bearer = bearer.clone();
        let app = &app;
        async move {
            let r = test::call_service(
                app,
                test::TestRequest::post()
                    .uri("/observations")
                    .insert_header(("authorization", bearer))
                    .set_json(json!({
                        "item_id": GLOVE,
                        "packaging_level": "each",
                        "measurements": [],
                        "photographs": photographs,
                        "client_event_id": client_event,
                        "occurred_at": chrono::Utc::now(),
                    }))
                    .to_request(),
            )
            .await;
            let status = r.status().as_u16();
            let body: Value = serde_json::from_slice(&test::read_body(r).await).unwrap_or(Value::Null);
            (status, body)
        }
    };
    let (status, refused) = look(Uuid::new_v4(), false).await;
    assert_eq!(status, 400, "an empty look nobody meant is refused: {refused}");
    let photo_event = Uuid::new_v4();
    let (status, looked) = look(photo_event, true).await;
    assert_eq!(status, 200, "{looked}");
    assert_eq!(looked["observation_ids"], json!([]), "nothing measured: {looked}");
    let look_id = looked["observation_event_id"]
        .as_str()
        .expect("an event to hang the photographs off")
        .to_string();
    let (status, again) = look(photo_event, true).await;
    assert_eq!(status, 200, "a repeated press is the same act: {again}");
    assert_eq!(again["observation_event_id"].as_str(), Some(look_id.as_str()), "and the same look");
    let r = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/observations/{look_id}/images/front"))
            .insert_header(("authorization", bearer.clone()))
            .insert_header(("content-type", "image/png"))
            .set_payload(png("a look for its photographs"))
            .to_request(),
    )
    .await;
    assert!(r.status().is_success(), "a photograph on a look with no figures");

    // The item's page shows it against its subject, with what gets measured.
    let page: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/items/{GLOVE}"))
                .insert_header(("authorization", bearer.clone()))
                .to_request(),
        )
        .await;
        assert!(r.status().is_success());
        serde_json::from_slice(&test::read_body(r).await).unwrap()
    };
    let subjects = page["subjects"].as_array().expect("what gets measured for it");
    assert!(
        subjects.iter().any(|s| s["item_id"] == GLOVE && s["packaging_level"] == "each"),
        "the glove's each is one: {page}"
    );
    let front = page["photos"]
        .as_array()
        .expect("its photographs")
        .iter()
        .find(|p| p["item_id"] == GLOVE && p["packaging_level"] == "each" && p["face"] == "front")
        .expect("the newest front of its each");
    assert_eq!(front["digest"].as_str().map(str::len), Some(64), "{front}");
    assert_eq!(front["cut"], Value::Null, "nobody has cut it to its face yet: {front}");
    let photo_id = front["image_id"].as_str().expect("the photograph a cut points at").to_string();

    // ── waiting to be cut, at a computer (D181) ─────────────────────────
    let queue = || {
        let bearer = bearer.clone();
        let app = &app;
        async move {
            let r = test::call_service(
                app,
                test::TestRequest::get().uri("/photos/uncut").insert_header(("authorization", bearer)).to_request(),
            )
            .await;
            assert!(r.status().is_success(), "the queue answers");
            serde_json::from_slice::<Value>(&test::read_body(r).await).unwrap()
        }
    };
    let waiting = queue().await;
    let mine = waiting
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["image_id"] == photo_id.as_str())
        .unwrap_or_else(|| panic!("the uncut front waits to be cut: {waiting}"));
    assert_eq!(mine["item_id"], GLOVE, "and opens on the item whose page shows it: {mine}");
    assert_eq!(mine["face"], "front");
    assert_eq!(mine["code"], "GLOVE-M");
    let work: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::get().uri("/work").insert_header(("authorization", bearer.clone())).to_request(),
        )
        .await;
        serde_json::from_slice(&test::read_body(r).await).unwrap()
    };
    assert!(work["crop"].as_i64().unwrap_or(0) >= 1, "the sidebar counts it: {work}");

    // ── cut to its face (D176) ──────────────────────────────────────────
    //
    // Two requests: the straightened face's bytes, then the act naming the
    // photograph, the corners and where the bytes went.
    let stored: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/images")
                .insert_header(("authorization", bearer.clone()))
                .insert_header(("content-type", "image/png"))
                .set_payload(png_sized(600, 400, 64))
                .to_request(),
        )
        .await;
        assert!(r.status().is_success(), "the cut's bytes are kept");
        serde_json::from_slice(&test::read_body(r).await).unwrap()
    };
    let cut_digest = stored["digest"].as_str().expect("their address").to_string();
    assert_eq!((stored["width_px"].as_i64(), stored["height_px"].as_i64()), (Some(600), Some(400)), "{stored}");

    // Kept is not readable: nothing names the bytes until the act does.
    let unnamed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/images/{cut_digest}"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(unnamed.status().as_u16(), 404, "bytes no row names are not served");

    let cut_event = Uuid::new_v4();
    let corners = json!([0.2, 0.15, 0.8, 0.15, 0.9, 0.85, 0.1, 0.85]);
    let cut = |image: String, body: Value| {
        let bearer = bearer.clone();
        let app = &app;
        async move {
            let r = test::call_service(
                app,
                test::TestRequest::post()
                    .uri(&format!("/observation-images/{image}/cuts"))
                    .insert_header(("authorization", bearer))
                    .set_json(body)
                    .to_request(),
            )
            .await;
            let status = r.status().as_u16();
            let body: Value = serde_json::from_slice(&test::read_body(r).await).unwrap_or(Value::Null);
            (status, body)
        }
    };
    let act = |id: Uuid, digest: &str, corners: &Value| {
        json!({ "digest": digest, "corners": corners, "client_event_id": id, "occurred_at": chrono::Utc::now() })
    };

    let (status, made) = cut(photo_id.clone(), act(cut_event, &cut_digest, &corners)).await;
    assert_eq!(status, 200, "{made}");
    let after = queue().await;
    assert!(
        after.as_array().unwrap().iter().all(|q| q["image_id"] != photo_id.as_str()),
        "a cut photograph leaves the queue: {after}"
    );
    let (status, again) = cut(photo_id.clone(), act(cut_event, &cut_digest, &corners)).await;
    assert_eq!(status, 200, "a repeated press is the same act: {again}");
    assert_eq!(again["cut_id"], made["cut_id"], "and the same cut");

    // Refused, each in words: corners that are not a face, bytes never kept,
    // a photograph that is not there, and one act reused for another cut.
    let crossed = json!([0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0]);
    let (status, _) = cut(photo_id.clone(), act(Uuid::new_v4(), &cut_digest, &crossed)).await;
    assert_eq!(status, 400, "crossed corners");
    let (status, _) = cut(photo_id.clone(), act(Uuid::new_v4(), &"0".repeat(64), &corners)).await;
    assert_eq!(status, 400, "nothing kept at that address");
    let (status, _) = cut(Uuid::new_v4().to_string(), act(Uuid::new_v4(), &cut_digest, &corners)).await;
    assert_eq!(status, 404, "no such photograph here");
    let other = json!([0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]);
    let (status, _) = cut(photo_id.clone(), act(cut_event, &digests[0], &other)).await;
    assert_eq!(status, 400, "one act, one cut");

    // The item shows the photograph with its cut, and serves the cut's bytes.
    let page: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/items/{GLOVE}"))
                .insert_header(("authorization", bearer.clone()))
                .to_request(),
        )
        .await;
        serde_json::from_slice(&test::read_body(r).await).unwrap()
    };
    let front = page["photos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["image_id"] == photo_id.as_str())
        .expect("the photograph, still");
    assert_eq!(front["cut"]["digest"], cut_digest.as_str(), "{front}");
    assert_eq!(front["cut"]["corners"], corners, "the corners as marked");
    let served = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/images/{cut_digest}"))
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
    )
    .await;
    assert!(served.status().is_success(), "a cut's row authorises its bytes");

    // And it is the person signed in who cut it, not the photographer by default.
    let cutter: bool = client
        .query_one(
            "SELECT c.recorded_by_id = ce.recorded_by_id
               FROM observation_image_cut c
               JOIN client_event ce ON ce.tenant_id = c.tenant_id AND ce.client_event_id = c.client_event_id
              WHERE c.client_event_id = $1",
            &[&cut_event],
        )
        .await
        .unwrap()
        .get(0);
    assert!(cutter, "the cut names the session's person, as its act does");

    // ── clean up after itself ───────────────────────────────────────────
    //
    // In dependency order, because the foreign keys are real. The client_event
    // goes last: every fact points at it.
    client
        .batch_execute(&format!(
            "DELETE FROM observation_image_cut WHERE client_event_id = '{cut_event}';
             DELETE FROM observation_image WHERE observation_event_id IN ('{event}', '{look_id}');
             DELETE FROM observation WHERE observation_event_id = '{event}';
             DELETE FROM observation_event WHERE id IN ('{event}', '{look_id}');
             DELETE FROM client_event WHERE client_event_id IN ('{client_event}', '{photo_event}', '{cut_event}');"
        ))
        .await
        .expect("the walk removes its own rows");

    let _ = std::fs::remove_dir_all(&images);
}
