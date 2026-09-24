//! Importing over HTTPS, on a token that is not a person (D158).
//!
//! The point of this file is the pair of refusals rather than the load. That an
//! import token can load a bin list is the easy half; what the design is made
//! of is that a **session cannot**, and that a withdrawn token stops working
//! immediately. A capability that anything else can also satisfy is not a
//! capability, and neither claim is visible in the schema.

use actix_web::{test, web, App};
use spork_server::{routes, AppState};
use serde_json::{json, Value};
use uuid::Uuid;

mod common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

/// Two bins in a warehouse the fixture already has, so the site is matched
/// rather than created and the test says nothing about site creation.
fn csv(codes: &[&str]) -> String {
    let mut s =
        String::from("Bin Number,Location,WMS Bin Type,WMS Picking Order,WMS Bin Sequence\n");
    for (i, c) in codes.iter().enumerate() {
        s.push_str(&format!("{c},Melbourne Warehouse,Pick,{},{}\n", i + 900, i + 900));
    }
    s
}

#[actix_web::test]
async fn an_import_token_loads_bins_and_nothing_else_can() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    // ── a session, which is how a token comes to exist ──────────────────
    let signed: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/sessions")
                .set_json(json!({ "email": "kyle@example.test", "password": "dock-station-1" }))
                .to_request(),
        )
        .await;
        assert!(r.status().is_success(), "the fixture signs in");
        test::read_body_json(r).await
    };
    let session_bearer = format!("Bearer {}", signed["token"].as_str().unwrap());

    // ── no credential at all ────────────────────────────────────────────
    let r = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/import/bins")
            .set_payload(csv(&["ZZ-01-01"]))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 401, "an import with no token is refused");

    // ── a session is a credential, and it is the wrong kind ─────────────
    //
    // **This is the assertion the whole decision rests on.** D158 scopes the
    // import path by the kind of credential rather than by a permission, and a
    // kind a session can also satisfy would be a suggestion instead of a scope.
    // It also means a leaked session cannot load a warehouse.
    let r = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/import/bins")
            .insert_header(("authorization", session_bearer.clone()))
            .set_payload(csv(&["ZZ-01-01"]))
            .to_request(),
    )
    .await;
    assert_eq!(
        r.status(),
        401,
        "a session reached the import path, so the token's kind is not a scope"
    );

    // ── mint one ────────────────────────────────────────────────────────
    let minted: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/tokens")
                .insert_header(("authorization", session_bearer.clone()))
                .set_json(json!({ "label": "the bin list, from a test" }))
                .to_request(),
        )
        .await;
        assert_eq!(r.status(), 201, "a session mints a token");
        test::read_body_json(r).await
    };
    let secret = minted["token"].as_str().expect("the secret is in the answer").to_string();
    assert!(
        secret.starts_with("nyl_"),
        "an import token wears its kind on the front so one lookup resolves it \
         and a scanner can be taught it: {secret}"
    );
    assert_eq!(secret.len(), 68, "the prefix and 32 bytes of hex");

    // A label is what makes a list of digests auditable, so it is required.
    let r = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/tokens")
            .insert_header(("authorization", session_bearer.clone()))
            .set_json(json!({ "label": "   " }))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 400, "an unlabelled token is one nobody will dare revoke");

    let bearer = format!("Bearer {secret}");

    // ── a dry run reports and keeps nothing ─────────────────────────────
    let code = format!("ZT-{}-01", std::process::id() % 100);
    let dry: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/import/bins")
                .insert_header(("authorization", bearer.clone()))
                .set_payload(csv(&[&code]))
                .to_request(),
        )
        .await;
        assert!(r.status().is_success(), "the token loads");
        test::read_body_json(r).await
    };
    assert_eq!(dry["loaded"]["applied"], false, "no `apply` means a dry run");
    assert_eq!(dry["loaded"]["bins_created"], 1, "and it says what it would have done");
    assert_eq!(
        dry["loaded"]["sites_created"], 0,
        "Melbourne Warehouse is the MEL the fixture already has"
    );
    assert_eq!(dry["loaded"]["sites_matched"], 1);

    let present = |c: &str| {
        let pool = pool(&u);
        let c = c.to_string();
        async move {
            let conn = pool.get().await.unwrap();
            conn.query_one("SELECT count(*) FROM location WHERE code = $1", &[&c])
                .await
                .unwrap()
                .get::<_, i64>(0)
        }
    };
    assert_eq!(present(&code).await, 0, "a dry run wrote nothing, having rolled itself back");

    // ── and applying keeps it ───────────────────────────────────────────
    let wet: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/import/bins?apply=true")
                .insert_header(("authorization", bearer.clone()))
                .set_payload(csv(&[&code]))
                .to_request(),
        )
        .await;
        assert!(r.status().is_success());
        test::read_body_json(r).await
    };
    assert_eq!(wet["loaded"]["applied"], true);
    assert_eq!(
        wet["loaded"]["bins_created"], dry["loaded"]["bins_created"],
        "the dry run is the same writes rolled back, so the two reports agree"
    );
    assert_eq!(present(&code).await, 1, "applying kept the bin");

    // ── withdrawing it takes effect at once ─────────────────────────────
    let id = minted["id"].as_str().unwrap();
    let r = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("/tokens/{id}"))
            .insert_header(("authorization", session_bearer.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 204, "a session withdraws a token");

    let r = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/import/bins")
            .insert_header(("authorization", bearer.clone()))
            .set_payload(csv(&[&code]))
            .to_request(),
    )
    .await;
    assert_eq!(r.status(), 401, "a withdrawn token stops working on the next request");

    // ── and it is listed, without its secret ────────────────────────────
    let listed: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/tokens")
                .insert_header(("authorization", session_bearer.clone()))
                .to_request(),
        )
        .await;
        assert!(r.status().is_success());
        test::read_body_json(r).await
    };
    let mine = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id)
        .expect("the token is on the list");
    assert!(mine["revoked_at"].is_string(), "and it is shown as withdrawn");
    assert!(mine["last_used_at"].is_string(), "and when it was last used");
    assert!(
        listed.to_string().find("nyl_").is_none(),
        "no secret is recoverable from the listing"
    );

    // Leave the fixture as it was found.
    let conn = pool(&u).get().await.unwrap();
    conn.execute("SELECT set_config('spork.tenant_id', $1, false)", &[&TENANT])
        .await
        .unwrap();
    conn.execute("DELETE FROM location WHERE code = $1", &[&code]).await.unwrap();
}

/// The export is on file before anything reads it (D21).
///
/// `party_message` was built in migration 34 and had no writer for four weeks.
/// What it buys is in the third block below: the same file loaded twice is one
/// arrival, detected on the bytes rather than on somebody remembering.
#[actix_web::test]
async fn an_import_files_the_export_before_it_reads_it() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let session_bearer = common::bearer_without_site(&app).await;
    let minted: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/tokens")
            .insert_header(("authorization", session_bearer.clone()))
            .set_json(json!({ "label": "filing the export, from a test" }))
            .to_request(),
        "minting an import token",
    )
    .await;
    let bearer = format!("Bearer {}", minted["token"].as_str().unwrap());
    let token_id = minted["id"].as_str().unwrap().to_string();

    // Unique to this run, so the hash is this run's and a second pass over the
    // same database does not read the previous run's arrival as this one's
    // replay. The pid alone was not enough: it repeats.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let code = format!("ZF-{}-01", nonce % 1_000_000);
    let body = csv(&[&code]);
    let hash = spork_server::importing::received::digest(body.as_bytes());

    let arrivals = |h: Vec<u8>| {
        let pool = pool(&u);
        async move {
            let conn = pool.get().await.unwrap();
            conn.query(
                "SELECT m.id, m.byte_count, m.parse_status::text, m.channel::text,
                        m.direction::text, m.transport_ref, m.party_id,
                        e.automation_key, e.recorded_by_id
                   FROM party_message m
                   JOIN client_event e
                     ON e.tenant_id = m.tenant_id AND e.client_event_id = m.client_event_id
                  WHERE m.content_hash = $1",
                &[&h],
            )
            .await
            .unwrap()
        }
    };

    // ── a dry run keeps no arrival ──────────────────────────────────────
    //
    // The loader rolls its writes back, and a receipt that outlived that
    // rollback would assert a file arrived which was never kept.
    let dry: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/import/bins")
            .insert_header(("authorization", bearer.clone()))
            .set_payload(body.clone())
            .to_request(),
        "a dry run",
    )
    .await;
    assert!(dry["arrival"].is_null(), "nothing had arrived, so there is nothing to report");
    assert_eq!(arrivals(hash.clone()).await.len(), 0, "and a dry run files none");

    // ── applying files the bytes, and names who did it ──────────────────
    let wet: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/import/bins?apply=true&filename=bins.csv")
            .insert_header(("authorization", bearer.clone()))
            .set_payload(body.clone())
            .to_request(),
        "applying",
    )
    .await;
    assert_eq!(wet["arrival"]["replay"], false, "the first arrival is not a replay");

    let rows = arrivals(hash.clone()).await;
    assert_eq!(rows.len(), 1, "one file, one arrival");
    let r = &rows[0];
    assert_eq!(
        r.get::<_, Uuid>(0).to_string(),
        wet["arrival"]["party_message_id"].as_str().unwrap(),
        "the report names the row it wrote"
    );
    assert_eq!(r.get::<_, i32>(1) as usize, body.len(), "byte_count is the file's length");
    assert_eq!(r.get::<_, &str>(2), "parsed", "the loader returned, so it was read");
    assert_eq!(r.get::<_, &str>(3), "csv", "the transport is a column, which is the point");
    assert_eq!(r.get::<_, &str>(4), "inbound");
    assert_eq!(r.get::<_, Option<&str>>(5), Some("bins.csv"));
    assert_eq!(
        r.get::<_, Option<Uuid>>(6),
        None,
        "there is no `party` row meaning the system of record, which is question 153"
    );
    // `client_event_actor_ck` is an exclusive or, and an import token is the
    // automation arm of it. The value is a pointer at the token rather than a
    // word, because question 105 has not settled what an `automation_key` is.
    assert_eq!(
        r.get::<_, Option<String>>(7),
        Some(format!("api_token:{token_id}")),
        "the act names the token that performed it"
    );
    assert_eq!(
        r.get::<_, Option<Uuid>>(8),
        None,
        "and names no person, because no person pressed anything"
    );

    // ── the same bytes twice are one arrival, and still a correction ────
    //
    // A bin list is reference data: re-importing it is how migration 74's
    // `pick_sequence` was filled at all. So the load runs again and the arrival
    // does not.
    let again: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/import/bins?apply=true")
            .insert_header(("authorization", bearer.clone()))
            .set_payload(body.clone())
            .to_request(),
        "applying the same file again",
    )
    .await;
    assert_eq!(again["arrival"]["replay"], true, "these bytes had already arrived");
    assert_eq!(
        again["arrival"]["party_message_id"], wet["arrival"]["party_message_id"],
        "and the report points at the arrival on file rather than a new one"
    );
    assert_eq!(arrivals(hash.clone()).await.len(), 1, "still one arrival");
    assert_eq!(again["loaded"]["applied"], true, "the load still ran");

    // ── and the dry run says so too ─────────────────────────────────────
    //
    // `bins::load` is built so that the dry run cannot disagree with the apply,
    // because it *is* the apply undone. A dry run that could not see an arrival
    // already on file would have broken that: it would report a fresh load of a
    // file the apply is about to recognise.
    let dry_again: Value = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/import/bins")
            .insert_header(("authorization", bearer.clone()))
            .set_payload(body.clone())
            .to_request(),
        "dry-running a file already on file",
    )
    .await;
    assert_eq!(
        dry_again["arrival"]["replay"], true,
        "a dry run says what applying would find"
    );
    assert_eq!(
        dry_again["arrival"]["party_message_id"], wet["arrival"]["party_message_id"],
        "naming the same arrival"
    );
    assert_eq!(arrivals(hash).await.len(), 1, "and having written nothing itself");
}
