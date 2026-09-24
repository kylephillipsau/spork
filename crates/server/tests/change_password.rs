//! Changing your own password, over HTTP, against the properties that make it
//! worth having.
//!
//! D142 owed this and said so. The interesting half is not "the password
//! changes" — it is what the endpoint refuses, and what it takes down with it:
//! a wrong current password is counted, a stolen session's other siblings die,
//! and the one doing the changing does not.
//!
//! **Every test makes its own person, and nothing here touches a seeded one.**
//! The first draft signed in as `kyle@example.test` and finished with a global
//! `DELETE FROM session`; three of six tests failed, because `cargo test` runs
//! them at once and they were changing one account's password out from under
//! each other and revoking each other's sessions. A test whose subject is a
//! credential cannot borrow a credential the rest of the suite signs on with.

use actix_web::{test, web, App};
use spork_server::{auth, routes, AppState};
use serde_json::{json, Value};
use uuid::Uuid;

mod common;
use common::{pool, url};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const PASSWORD: &str = "a-starting-password";
const NEXT: &str = "a-longer-password-still";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";

macro_rules! app {
    ($u:expr) => {{
        let state = web::Data::new(AppState { pool: pool($u) });
        test::init_service(App::new().app_data(state).configure(routes::configure)).await
    }};
}

/// A raw connection, as the owner. Making and unmaking a person is not
/// something the application role may do, and these tests are the fixture
/// rather than the subject.
async fn raw(u: &str) -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

/// A person of this test's own, with a password and a membership.
///
/// The email carries the test's name so a row left behind by a panic says which
/// test dropped it.
async fn make_person(client: &tokio_postgres::Client, whose: &str) -> Uuid {
    let email = format!("{whose}@change-password.test");
    let phc = auth::hash_password(PASSWORD).expect("hash");
    let person: Uuid = client
        .query_one(
            "INSERT INTO person (display_name, email) VALUES ($1, $2) RETURNING id",
            &[&"Test Person", &email],
        )
        .await
        .expect("a person")
        .get(0);
    client
        .execute(
            "INSERT INTO person_credential (person_id, kind, phc) VALUES ($1, 'password', $2)",
            &[&person, &phc],
        )
        .await
        .expect("a credential");
    client
        .execute(
            "INSERT INTO person_tenant (person_id, tenant_id, role) VALUES ($1, $2, 'operator')",
            &[&person, &Uuid::parse_str(ALPHA).unwrap()],
        )
        .await
        .expect("a membership, without which no session opens");
    person
}

/// Unmake it, sessions first, because they reference the person.
async fn unmake(client: &tokio_postgres::Client, person: Uuid) {
    for sql in [
        "DELETE FROM session WHERE person_id = $1",
        "DELETE FROM person_credential WHERE person_id = $1",
        "DELETE FROM person_tenant WHERE person_id = $1",
        "DELETE FROM person WHERE id = $1",
    ] {
        client.execute(sql, &[&person]).await.expect(sql);
    }
}

fn email_of(whose: &str) -> String {
    format!("{whose}@change-password.test")
}

/// Sign on and return the bearer, or `None` when the password is refused.
async fn sign_on<S>(app: &S, email: &str, password: &str) -> Option<String>
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let resp = test::call_service(
        app,
        test::TestRequest::post()
            .uri("/sessions")
            .set_json(json!({ "email": email, "password": password, "site_id": SITE }))
            .to_request(),
    )
    .await;
    if !resp.status().is_success() {
        return None;
    }
    let body: Value = test::read_body_json(resp).await;
    Some(body["token"].as_str().expect("a bearer").to_string())
}

fn change(token: &str, current: &str, next: &str) -> actix_http::Request {
    test::TestRequest::post()
        .uri("/credentials/password")
        .insert_header(("authorization", format!("Bearer {token}")))
        .set_json(json!({ "current_password": current, "new_password": next }))
        .to_request()
}

/// The whole point, in one walk: the new password works, the old one does not,
/// and every other session this person had is dead while this one is not.
#[actix_web::test]
async fn a_changed_password_replaces_the_old_one_and_ends_the_other_sessions() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);
    let client = raw(&u).await;
    let whose = "walk";
    let person = make_person(&client, whose).await;
    let who = email_of(whose);

    // Two sessions, because the interesting property is about the other one.
    let doomed = sign_on(&app, &who, PASSWORD).await.expect("their password");
    let keeping = sign_on(&app, &who, PASSWORD).await.expect("and a second");

    let resp = test::call_service(&app, change(&keeping, PASSWORD, NEXT)).await;
    assert!(resp.status().is_success(), "changing it with the right current one");
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["other_sessions_ended"], 1,
        "the other session, and only the other one"
    );

    // The session that did the changing still works. Signing somebody out of the
    // screen they are looking at is how people learn not to use it.
    let mine = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/sessions/current")
            .insert_header(("authorization", format!("Bearer {keeping}")))
            .to_request(),
    )
    .await;
    assert!(mine.status().is_success(), "the caller keeps their own session");

    // And the other one is gone, which is what makes this worth doing for
    // somebody who thinks a session was stolen.
    let stale = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/sessions/current")
            .insert_header(("authorization", format!("Bearer {doomed}")))
            .to_request(),
    )
    .await;
    assert_eq!(stale.status().as_u16(), 401, "the other session died with the password");

    // The credential itself actually moved.
    assert!(sign_on(&app, &who, PASSWORD).await.is_none(), "the old password is dead");
    let back = sign_on(&app, &who, NEXT).await.expect("the new one signs on");

    // And back, through the endpoint, which is the second half of the round
    // trip: a change is not one-way and nothing else has said so.
    let resp = test::call_service(&app, change(&back, NEXT, PASSWORD)).await;
    assert!(resp.status().is_success(), "and back again");
    assert!(sign_on(&app, &who, PASSWORD).await.is_some());

    unmake(&client, person).await;
}

/// A wrong current password is refused **and counted**. The counting is the
/// point: guessing it from inside a stolen session is the attack this endpoint
/// creates, and it gets the same ten attempts everything else does.
#[actix_web::test]
async fn a_wrong_current_password_is_refused_and_advances_the_counter() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);
    let client = raw(&u).await;
    let whose = "counter";
    let person = make_person(&client, whose).await;
    let who = email_of(whose);
    let token = sign_on(&app, &who, PASSWORD).await.expect("their password");

    let resp = test::call_service(&app, change(&token, "not the password", NEXT)).await;
    assert_eq!(resp.status().as_u16(), 400, "refused");

    let attempts: i32 = client
        .query_one(
            "SELECT failed_attempts FROM person_credential WHERE person_id = $1",
            &[&person],
        )
        .await
        .expect("the counter")
        .get(0);
    assert_eq!(attempts, 1, "a wrong current password is a failed attempt");

    // And the password is untouched, which the refusal implies and nothing else
    // has checked. Signing on also clears the counter, which is the same
    // function doing the other half of its job.
    assert!(sign_on(&app, &who, PASSWORD).await.is_some());

    unmake(&client, person).await;
}

/// A locked account is refused **before the digest is compared**, so the lock
/// beats a correct password rather than being a consolation for a wrong one.
///
/// The ordering is the claim, and it is the part a tidy-up would silently lose:
/// checking `locked_until` after verifying would still refuse, but it would also
/// let somebody with a stolen session confirm a guess while locked out.
#[actix_web::test]
async fn a_locked_account_is_refused_even_with_the_right_password() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);
    let client = raw(&u).await;
    let whose = "locked";
    let person = make_person(&client, whose).await;
    let who = email_of(whose);

    // Sign on first, then lock: the session is the thing this endpoint runs
    // behind, and locking is about the credential rather than the session.
    let token = sign_on(&app, &who, PASSWORD).await.expect("their password");
    client
        .execute(
            "UPDATE person_credential
                SET locked_until = now() + interval '15 minutes', failed_attempts = 10
              WHERE person_id = $1",
            &[&person],
        )
        .await
        .expect("lock it");

    let resp = test::call_service(&app, change(&token, PASSWORD, NEXT)).await;
    assert_eq!(resp.status().as_u16(), 400, "refused, with the right password");
    let body: Value = test::read_body_json(resp).await;
    assert!(
        body.to_string().contains("locked"),
        "and says the account is locked rather than that the password is wrong: {body}"
    );

    // Unlocked, the same request works — so the refusal was the lock and not
    // something else about this account.
    client
        .execute(
            "UPDATE person_credential SET locked_until = NULL, failed_attempts = 0
              WHERE person_id = $1",
            &[&person],
        )
        .await
        .expect("unlock it");
    let resp = test::call_service(&app, change(&token, PASSWORD, NEXT)).await;
    assert!(resp.status().is_success(), "the lock was the only thing in the way");

    unmake(&client, person).await;
}

/// The twelve-character floor is the same rule setup uses. One policy, because
/// two is one of them being a surprise at the worst moment.
#[actix_web::test]
async fn a_short_new_password_is_refused() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);
    let client = raw(&u).await;
    let whose = "floor";
    let person = make_person(&client, whose).await;
    let who = email_of(whose);
    let token = sign_on(&app, &who, PASSWORD).await.expect("their password");

    let resp = test::call_service(&app, change(&token, PASSWORD, "elevenchars")).await;
    assert_eq!(resp.status().as_u16(), 400, "eleven characters is under the line");
    let body: Value = test::read_body_json(resp).await;
    assert!(
        body.to_string().contains("twelve"),
        "and says which line it is: {body}"
    );

    // Reusing the one already in force is a wasted trip rather than a change,
    // and saying so beats storing a new salt and reporting success.
    let resp = test::call_service(&app, change(&token, PASSWORD, PASSWORD)).await;
    assert_eq!(resp.status().as_u16(), 400, "unchanged is not a change");

    assert!(sign_on(&app, &who, PASSWORD).await.is_some(), "still the old one");

    unmake(&client, person).await;
}

/// No session, no change. The endpoint reads the person from the session and
/// takes no person in the body, so there is no shape of this request that
/// changes somebody else's.
#[actix_web::test]
async fn an_unauthenticated_caller_cannot_change_anything() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);
    let client = raw(&u).await;
    let whose = "anonymous";
    let person = make_person(&client, whose).await;
    let who = email_of(whose);

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/credentials/password")
            .set_json(json!({ "current_password": PASSWORD, "new_password": NEXT }))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 401, "not signed in");

    // A bearer that names no session is the same answer, which is the case a
    // stolen-then-revoked token arrives as.
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/credentials/password")
            .insert_header(("authorization", "Bearer 00000000000000000000"))
            .set_json(json!({ "current_password": PASSWORD, "new_password": NEXT }))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 401, "an unknown token is nobody");

    assert!(sign_on(&app, &who, PASSWORD).await.is_some(), "untouched");
    unmake(&client, person).await;
}

/// The application still cannot reach the table, and the three new functions are
/// the whole of what it gained.
///
/// **The check that would have caught the grant being forgotten.** A definer
/// Postgres left executable by PUBLIC is the failure migration 75's comment
/// records seven of; this asserts the other direction, that `spork_app` has
/// what it needs and the tables stay shut.
#[actix_web::test]
async fn the_app_gained_three_functions_and_no_table() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let (client, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls)
        .await
        .expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client.batch_execute("SET ROLE spork_app").await.unwrap();

    let err = client
        .query("UPDATE person_credential SET phc = 'x'", &[])
        .await
        .expect_err("the app has no business writing this");
    assert_eq!(
        err.code(),
        Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE),
        "the definers are the interface, not a convenience over a grant"
    );

    for f in [
        "credential_phc",
        "credential_change_password",
        "session_revoke_others",
    ] {
        let ok: bool = client
            .query_one(
                "SELECT has_function_privilege('spork_app', p.oid, 'EXECUTE')
                   FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
                  WHERE p.proname = $1 AND n.nspname = 'public'",
                &[&f],
            )
            .await
            .unwrap()
            .get(0);
        assert!(ok, "{f} is how the app gets in");
    }
}

/// Somebody who signs in with a passkey and has no password is told so, and the
/// compare-and-set would refuse it anyway.
///
/// Migration 75 made `phc` nullable so somebody can hold a key and no password.
/// `NULL = anything` is never true, so the guard is structural rather than a
/// branch somebody has to remember — but a structural refusal alone would read
/// to the caller as *"that is not the current password"*, which is a lie. Both
/// halves are checked: the specific message over HTTP, and the database
/// refusing regardless of what it is told the current digest is.
#[actix_web::test]
async fn a_passkey_only_person_is_told_there_is_no_password_to_change() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let app = app!(&u);
    let client = raw(&u).await;

    // A person of this test's own, so a failure here cannot leave a seeded
    // account passwordless.
    let person = make_person(&client, "passkey-only").await;
    client
        .execute(
            "UPDATE person_credential SET phc = NULL WHERE person_id = $1",
            &[&person],
        )
        .await
        .expect("a credential with no digest, which migration 75 allows");

    // A session without a password, because that is the whole situation: they
    // are signed in, by a key, with nothing to change. `session_open` is the one
    // way to become signed in, and this is the passkey path's half of it.
    let minted = "0".repeat(64);
    let digest = auth::token_digest(&minted);
    client
        .query_one(
            "SELECT session_open($1, $2, $3, $4, make_interval(hours => 1),
                                 make_interval(mins => 30), false, NULL, NULL)",
            &[
                &person,
                &Uuid::parse_str(ALPHA).unwrap(),
                &Uuid::parse_str(SITE).unwrap(),
                &digest,
            ],
        )
        .await
        .expect("a session for somebody who never typed a password");

    let resp = test::call_service(&app, change(&minted, "anything at all", NEXT)).await;
    assert_eq!(resp.status().as_u16(), 400, "refused");
    let body: Value = test::read_body_json(resp).await;
    assert!(
        body.to_string().contains("passkey"),
        "and says why, rather than implying the current password was wrong: {body}"
    );

    for expected in [None, Some("$argon2id$whatever")] {
        let changed: bool = client
            .query_one(
                "SELECT credential_change_password($1, $2, $3)",
                &[&person, &expected, &"$argon2id$new"],
            )
            .await
            .expect("the function runs")
            .get(0);
        assert!(
            !changed,
            "a NULL digest matches nothing, including NULL: expected {expected:?}"
        );
    }

    let phc: Option<String> = client
        .query_one("SELECT phc FROM person_credential WHERE person_id = $1", &[&person])
        .await
        .unwrap()
        .get(0);
    assert!(phc.is_none(), "and nothing was written");

    unmake(&client, person).await;
}
