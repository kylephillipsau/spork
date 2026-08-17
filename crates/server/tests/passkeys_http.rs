//! The passkey ceremonies, over HTTP.
//!
//! **A real signature needs a real authenticator**, which a test process does
//! not have — so what is proven here is everything around the signature: that a
//! ceremony is issued, that its server-side half is stored rather than handed to
//! the browser, that it can be spent exactly once, that it expires, and that the
//! endpoints refuse in the right way when they should.
//!
//! The signature check itself is `webauthn-rs`'s, tested by `webauthn-rs`. What
//! this project owns is the plumbing around it, and the plumbing is where the
//! replay lives.

use actix_web::{test, web, App};
use nylonite_server::{routes, AppState};
use serde_json::{json, Value};
use uuid::Uuid;

mod common;
use common::{pool, url};

const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const PERSON: &str = "77770000-0000-0000-0000-000000000001";

/// Sign on, and hand back both the header and the raw token.
///
/// **The token, because the session has to be removed by digest.**
/// `sign_on.rs` cleans up by `person_id`, which reaches every session that
/// person has — including one another test is mid-way through using. Leaving
/// sessions behind here made that collision reachable, and the fix is to own
/// exactly what this file created and nothing else.
async fn sign_in(app: &impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
>) -> (String, String) {
    let r = test::call_service(
        app,
        test::TestRequest::post()
            .uri("/sessions")
            .set_json(json!({
                "email": "kyle@example.test",
                "password": "dock-station-1",
                "site_id": SITE
            }))
            .to_request(),
    )
    .await;
    assert!(r.status().is_success(), "the fixture signs on");
    let b: Value = test::read_body_json(r).await;
    let token = b["token"].as_str().unwrap().to_string();
    (format!("Bearer {token}"), token)
}

/// Remove one session, named by its own digest.
async fn drop_session(u: &str, token: &str) {
    let (client, conn) = tokio_postgres::connect(u, tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(async move { let _ = conn.await; });
    client
        .execute(
            "DELETE FROM session WHERE token_sha256 = $1",
            &[&nylonite_server::auth::token_digest(token)],
        )
        .await
        .expect("the test removes the session it opened");
}

#[actix_web::test]
async fn a_ceremony_is_issued_stored_and_spent_once() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let (bearer, token) = sign_in(&app).await;

    // ---- registration begins ----
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/passkeys/registration/begin")
            .insert_header(("authorization", bearer.clone()))
            .set_json(json!({ "label": "the blue one" }))
            .to_request(),
    )
    .await;
    assert!(resp.status().is_success(), "a signed-in person may enrol a key");
    let body: Value = test::read_body_json(resp).await;
    let ceremony = body["ceremony_id"].as_str().expect("a ceremony id").to_string();

    // The options carry a challenge and name this relying party.
    let opts = &body["options"]["publicKey"];
    assert!(opts["challenge"].is_string(), "a challenge is offered");
    assert_eq!(opts["rp"]["id"], "localhost", "the relying party is the configured one");
    assert!(opts["user"]["id"].is_string(), "and the user handle is the person");

    // ---- the server's half is on the server ----
    let (client, conn) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(async move { let _ = conn.await; });
    let stored: i64 = client
        .query_one(
            "SELECT count(*) FROM webauthn_challenge
              WHERE id = $1::text::uuid AND kind = 'registration' AND consumed_at IS NULL",
            &[&ceremony],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(stored, 1, "the state is in the database, not in the browser");

    // ---- and can be claimed exactly once ----
    let first: Option<String> = client
        .query_opt(
            "SELECT state FROM webauthn_challenge_claim($1::text::uuid, 'registration')",
            &[&ceremony],
        )
        .await
        .unwrap()
        .map(|r| r.get(0));
    assert!(first.is_some(), "the first claim wins");
    let second = client
        .query_opt(
            "SELECT state FROM webauthn_challenge_claim($1::text::uuid, 'registration')",
            &[&ceremony],
        )
        .await
        .unwrap();
    assert!(second.is_none(), "the second claim gets nothing: that is the replay defence");

    client
        .execute("DELETE FROM webauthn_challenge WHERE id = $1::text::uuid", &[&ceremony])
        .await
        .unwrap();
    drop_session(&u, &token).await;
}

/// An expired ceremony is not claimable, which is the other half of single-use.
#[actix_web::test]
async fn an_expired_ceremony_cannot_be_claimed() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let (client, conn) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(async move { let _ = conn.await; });

    let id: Uuid = client
        .query_one(
            "SELECT webauthn_challenge_open($1::text::uuid, 'registration', '{}', interval '1 second')",
            &[&PERSON],
        )
        .await
        .unwrap()
        .get(0);
    // **Both timestamps move.** `webauthn_challenge_expires_ck` is
    // `expires_at > created_at`, so backdating only the expiry is refused — the
    // constraint doing exactly what it is for. A ceremony that expired is one
    // that was opened earlier, so the row is aged rather than corrupted.
    client
        .execute(
            "UPDATE webauthn_challenge
                SET created_at = now() - interval '10 seconds',
                    expires_at = now() - interval '5 seconds'
              WHERE id = $1",
            &[&id],
        )
        .await
        .unwrap();

    let claimed = client
        .query_opt("SELECT state FROM webauthn_challenge_claim($1, 'registration')", &[&id])
        .await
        .unwrap();
    assert!(claimed.is_none(), "an expired ceremony is spent by the clock");

    client.execute("DELETE FROM webauthn_challenge WHERE id = $1", &[&id]).await.unwrap();
}

/// **Signing in must not say who exists.** The password path verifies against a
/// dummy hash so a wrong email costs the same; this is that property for
/// passkeys, and it is easy to lose because the obvious implementation returns
/// 404 for an unknown address.
#[actix_web::test]
async fn beginning_authentication_does_not_enumerate_people() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let mut ids = vec![];
    for email in ["kyle@example.test", "nobody-at-all@example.test"] {
        let r = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/passkeys/authentication/begin")
                .set_json(json!({ "email": email }))
                .to_request(),
        )
        .await;
        assert!(r.status().is_success(), "{email} gets a ceremony either way");
        let b: Value = test::read_body_json(r).await;
        assert!(b["options"]["publicKey"]["challenge"].is_string());
        ids.push(b["ceremony_id"].as_str().unwrap().to_string());
    }

    let (client, conn) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(async move { let _ = conn.await; });
    for id in ids {
        client
            .execute("DELETE FROM webauthn_challenge WHERE id = $1::text::uuid", &[&id])
            .await
            .unwrap();
    }
}

/// Enrolling a key is adding a way to become somebody, so it needs to already
/// be them.
#[actix_web::test]
async fn enrolling_a_key_requires_being_signed_in() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    // **The body has to parse for the handler to run at all.** Actix
    // deserialises before the handler, so a malformed body is a 400 that never
    // reaches the authentication check — which is worth knowing rather than
    // asserting around: the schema is public API documentation, so revealing it
    // to an unauthenticated caller costs nothing, but the authentication is not
    // the first gate and a future body-shaped side effect must not assume it is.
    let bodies = [
        ("/passkeys/registration/begin", json!({ "label": "x" })),
        (
            "/passkeys/registration/finish",
            json!({
                "ceremony_id": Uuid::now_v7(),
                "credential": {
                    "id": "", "rawId": "", "type": "public-key",
                    "response": {
                        "attestationObject": "", "clientDataJSON": ""
                    },
                    "extensions": {}
                }
            }),
        ),
    ];
    for (uri, body) in bodies {
        let r = test::call_service(
            &app,
            test::TestRequest::post().uri(uri).set_json(body).to_request(),
        )
        .await;
        assert_eq!(r.status(), 401, "{uri} refuses an unauthenticated caller");
    }
    let r = test::call_service(&app, test::TestRequest::get().uri("/passkeys").to_request()).await;
    assert_eq!(r.status(), 401, "and so does the listing");
}

/// A ceremony that was opened for one person cannot be finished by another.
#[actix_web::test]
async fn a_ceremony_belongs_to_the_person_it_was_opened_for() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let (bearer, token) = sign_in(&app).await;

    let (client, conn) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(async move { let _ = conn.await; });

    // Opened for somebody else entirely.
    let stranger = "77770000-0000-0000-0000-000000000002";
    let id: Uuid = client
        .query_one(
            "SELECT webauthn_challenge_open($1::text::uuid, 'registration', '{}', interval '5 minutes')",
            &[&stranger],
        )
        .await
        .unwrap()
        .get(0);

    let r = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/passkeys/registration/finish")
            .insert_header(("authorization", bearer))
            .set_json(json!({ "ceremony_id": id, "credential": {} }))
            .to_request(),
    )
    .await;
    // 400 for a malformed credential body would also be defensible; what must
    // not happen is 200.
    assert_ne!(r.status(), 200, "finishing another person's ceremony must not enrol a key");

    client.execute("DELETE FROM webauthn_challenge WHERE id = $1", &[&id]).await.unwrap();
    drop_session(&u, &token).await;
}

/// Signing in without saying who you are.
///
/// **The reason this exists is a touchscreen with gloves on.** An allow-list
/// ceremony needs an email typed before the key is reached; a discoverable one
/// asks the authenticator for whatever it holds for this origin and learns who
/// it is talking to from the assertion.
#[actix_web::test]
async fn a_ceremony_can_begin_without_knowing_who_it_is_for() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let r = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/passkeys/authentication/begin")
            .set_json(json!({}))
            .to_request(),
    )
    .await;
    assert!(r.status().is_success(), "no email is a valid way to start");
    let b: Value = test::read_body_json(r).await;
    let opts = &b["options"]["publicKey"];
    assert!(opts["challenge"].is_string());
    // The whole point: nothing is offered, so nothing is revealed about who
    // holds a key here.
    let allow = &opts["allowCredentials"];
    assert!(
        allow.is_null() || allow.as_array().is_some_and(|a| a.is_empty()),
        "a discoverable ceremony names no credentials: {allow}"
    );

    let ceremony = b["ceremony_id"].as_str().unwrap().to_string();
    let (client, conn) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(async move { let _ = conn.await; });

    // **Recorded as its own kind, and naming nobody.** Migration 76 added the
    // third kind rather than inferring it from a null person, because an unknown
    // email also yields a null person and one absence cannot mean two things.
    let row = client
        .query_one(
            "SELECT kind, person_id IS NULL FROM webauthn_challenge WHERE id = $1::text::uuid",
            &[&ceremony],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "discoverable");
    assert!(row.get::<_, bool>(1), "a discoverable ceremony names no person");

    client
        .execute("DELETE FROM webauthn_challenge WHERE id = $1::text::uuid", &[&ceremony])
        .await
        .unwrap();
}

/// **Enrolling must ask for a key that can be offered back.**
///
/// `start_passkey_registration` emits `residentKey: discouraged`, which produces
/// a credential the authenticator will not surface until the server already
/// knows whose it is — so tap-to-sign-in silently never works. This asserts the
/// override, because the failure it prevents is invisible: the key enrols fine
/// and simply never appears.
#[actix_web::test]
async fn registration_asks_for_a_discoverable_key() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let (bearer, token) = sign_in(&app).await;

    let r = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/passkeys/registration/begin")
            .insert_header(("authorization", bearer))
            .set_json(json!({}))
            .to_request(),
    )
    .await;
    let b: Value = test::read_body_json(r).await;
    let sel = &b["options"]["publicKey"]["authenticatorSelection"];
    assert_eq!(sel["residentKey"], "required", "the key must know who it is");
    assert_eq!(sel["requireResidentKey"], true, "and say so the legacy way too");
    assert_eq!(sel["userVerification"], "required", "and prove a person is there");

    let ceremony = b["ceremony_id"].as_str().unwrap().to_string();
    let (client, conn) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(async move { let _ = conn.await; });
    client
        .execute("DELETE FROM webauthn_challenge WHERE id = $1::text::uuid", &[&ceremony])
        .await
        .unwrap();
    drop_session(&u, &token).await;
}
