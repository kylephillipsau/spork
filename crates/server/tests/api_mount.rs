//! Where the API answers, asserted once.
//!
//! The API moved under `/api` so the client could have the root: every screen
//! wants a name a person could read down a phone, and `/capture` and `/setup`
//! were already endpoints. A convention that new endpoints avoid the page names
//! is a convention somebody eventually forgets.
//!
//! # Why this is one file and not ninety edits
//!
//! Ninety `.uri(...)` calls across eleven files could each have been rewritten
//! to say `/api/...`, and every one of them would have been restating the same
//! single fact. Those tests mount `routes::configure` at the root and ask
//! whether a *handler* works; the prefix is not their subject. So the prefix is
//! this file's subject, and it is asserted here in the shape a running server
//! actually has — `routes::mount`, which is what `main.rs` calls.
//!
//! # The assertion that matters is the empty 404
//!
//! `assets.rs` records what a root-mounted catch-all costs. A route registered
//! in `configure` but missing from a stale build answered 404 with an empty
//! body, and *"the emptiness is what identified it"* — a page returned as 200
//! would have made the diagnosis considerably longer.
//!
//! **This test was written a commit before that hazard existed**, while the
//! client was still confined to `/ui` and the fourth assertion below passed
//! trivially. The client is at the root now, so it is load-bearing: it is what
//! fails if the `/api` scope ever stops terminating its own misses and the
//! bundle's catch-all starts answering for endpoints that do not exist. A guard
//! written under pressure, after a deploy has already gone quiet, is a guard
//! written too late.

use actix_web::{test, web, App};
use spork_server::{routes, AppState};

mod common;
use common::{pool, url};

/// The whole of the mount, in one test, against the real composition.
#[actix_web::test]
async fn the_api_answers_under_api_and_nowhere_else() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    // `routes::mount`, not `routes::configure` — the point of this file is the
    // composition the binary uses, and `configure` is deliberately prefix-free.
    let app = test::init_service(App::new().app_data(state).configure(routes::mount)).await;

    // 1. The API is under the prefix.
    let resp = test::call_service(
        &app,
        test::TestRequest::get().uri("/api/health").to_request(),
    )
    .await;
    assert!(resp.status().is_success(), "the API answers under /api");

    // 2. And nowhere else. This is what frees `/pack`, `/findings`, `/capture`
    //    and the rest of the root for screens.
    let resp = test::call_service(&app, test::TestRequest::get().uri("/health").to_request()).await;
    assert_eq!(
        resp.status().as_u16(),
        404,
        "the root belongs to the client now"
    );

    // 3. A root-level name that a screen wants, and that used to be an
    //    endpoint. `/capture` and `/setup` are the two that actually collided.
    for taken in ["/capture", "/setup"] {
        let resp = test::call_service(&app, test::TestRequest::get().uri(taken).to_request()).await;
        assert_eq!(
            resp.status().as_u16(),
            404,
            "{taken} is a screen's name now, not an endpoint's"
        );
    }

    // 4. **The one that guards the root mount.** An endpoint that does not
    //    exist must answer 404 with nothing in it, so a stale build is
    //    diagnosable by the emptiness. When the client moves to the root, this
    //    is what fails if the scope stops terminating its own misses.
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/no-such-endpoint")
            .to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 404);
    let body = test::read_body(resp).await;
    assert!(
        body.is_empty(),
        "an unmatched API path answers with nothing, not with a page: {}",
        String::from_utf8_lossy(&body)
    );

    // 5. The one server-rendered page left keeps its own place, outside the
    //    scope. Putting it under `/api` would have been the easy mistake, and
    //    `/app` is retired — the printable packing list is all D113 ever meant
    //    to keep, and it answers at `/print`.
    let resp = test::call_service(
        &app,
        test::TestRequest::get().uri("/print/style.css").to_request(),
    )
    .await;
    assert!(
        resp.status().is_success(),
        "the print stylesheet is not API and did not move under it"
    );
    let gone = test::call_service(
        &app,
        test::TestRequest::get().uri("/app/packing").to_request(),
    )
    .await;
    assert_eq!(gone.status(), 404, "`/app` is retired, not merely unlinked");
}
