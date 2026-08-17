//! The client bundle, served from the same origin as the API.
//!
//! **Same origin is not a convenience here, it is the auth story.** The session
//! is a cookie and `domain/api.ts` sends `credentials: "same-origin"`; hosting
//! the bundle anywhere else turns a working sign-on into a CORS and SameSite
//! problem in exchange for nothing. D11's non-repudiable floor depends on the
//! session reaching the server, so the bundle rides with it.
//!
//! # Why the root, and what used to make that unsafe
//!
//! This file argued for `/ui` for two years of commits, and the argument was
//! sound rather than timid. A single-page application needs a fallback: any
//! path it owns must return `index.html` so the client router can read it.
//! Mounted at the root, that fallback swallows every unmatched path — including
//! the API's, which would answer a request for a route that does not exist with
//! a page instead of a 404.
//!
//! That was not hypothetical. A route registered in `configure` but missing
//! from a stale build answered 404 with an empty body, and **the emptiness is
//! what identified it**. A root-mounted bundle would have returned 200 and an
//! HTML document, and the diagnosis would have been considerably longer.
//!
//! D144 removed the hazard rather than tolerating it. The API lives under
//! `/api` in a scope with its own `default_service`, so an unmatched endpoint
//! is answered — and answered with an empty 404 — before anything here is
//! reached. `crates/server/tests/api_mount.rs` asserts exactly that, and it was
//! written a commit early, while it still passed trivially, because it is the
//! guard for this change rather than for that one.
//!
//! So the fallback is the root's now, and the screens have names a person can
//! read down a phone. What is left outside it is `/api`, and `/app` and
//! `/print` for the documents D113 keeps server-rendered.
//!
//! # Registration order is load-bearing
//!
//! `main.rs` registers the API and the documents first and this last, because
//! the fallback below matches everything. That was true before — the `/ui`
//! scope's own default handler had the same property inside its scope — and it
//! is now true of the whole application.

use std::path::{Path, PathBuf};

use actix_files::{Files, NamedFile};
use actix_web::dev::{fn_service, ServiceRequest, ServiceResponse};
use actix_web::http::header;
use actix_web::middleware::DefaultHeaders;
use actix_web::web;

/// Where the application answers. The screens are the root.
pub const MOUNT: &str = "/";

/// Where the bundle is, in the image. Overridable so `cargo run` beside a
/// `vite build` works without an image.
pub fn directory() -> PathBuf {
    std::env::var("NYLONITE_CLIENT_DIR")
        .unwrap_or_else(|_| "/usr/local/share/nylonite/client".to_string())
        .into()
}

/// Whether there is anything to serve.
///
/// **Checked rather than assumed.** A missing bundle is a normal state — the
/// binary runs in tests, in `cargo run`, and in an image built before the
/// client existed — and the failure mode worth avoiding is a mount that exists
/// and serves nothing. `main` logs which of the two it got.
pub fn present(dir: &Path) -> bool {
    dir.join("index.html").is_file()
}

/// # Caching, which is the whole reason a deploy can be invisible
///
/// The build gives every asset a content hash in its filename, so an asset is
/// immutable by construction: a change produces a different *name* rather than
/// different bytes at the same one. `index.html` is the opposite — one fixed
/// name whose entire job is naming today's hashes.
///
/// **Served with no `Cache-Control` at all, that is exactly backwards.** A
/// response carrying only `Last-Modified` invites *heuristic* caching, where
/// the browser invents a freshness lifetime — commonly a tenth of the age of
/// the document — and serves its old copy without asking. The old copy names
/// the old hashes, so a deploy that built correctly and is sitting on the
/// server is invisible to anyone who has visited before, and redeploying
/// changes nothing because the document was never the thing being refetched.
///
/// That happened twice here before anyone read the headers.
///
/// So the contract is stated rather than left to a heuristic:
///
/// - `assets/*` — a year, `immutable`. The name changes when the bytes do.
/// - everything else under the mount — `no-cache`: revalidate every time. It
///   is one small document and it decides which application you are running.
///
/// `no-cache` is not `no-store`. The browser keeps the copy and asks whether
/// it is still good, which is a 304 and a few bytes on an unchanged deploy.
pub fn configure(cfg: &mut web::ServiceConfig, dir: &Path) {
    let index = dir.join("index.html");

    // **Before the catch-all, and its own scope, because the cache contracts
    // differ.** An asset is named by the hash of its bytes and can be kept for
    // a year; `index.html` is one fixed name whose whole job is naming today's
    // hashes, and a browser that keeps it is a browser running last week's
    // application. That is not theoretical either — it happened twice.
    cfg.service(
        web::scope("/assets")
            .wrap(DefaultHeaders::new().add((
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable",
            )))
            .service(Files::new("", dir.join("assets"))),
    );

    // Everything else. `main.rs` registers this last, so `/api`, `/app` and
    // `/print` have already had their say.
    cfg.service(
        Files::new(MOUNT, dir)
            .index_file("index.html")
            .prefer_utf8(true)
            // Client-side routing: a deep link is a path this server has never
            // heard of, and the answer is the document that knows how to read
            // it. A path the *client* does not have either is a 404 the client
            // draws — the server cannot know the route table, and teaching it
            // one would be a second copy to keep in agreement.
            .default_handler(fn_service(move |req: ServiceRequest| {
                let index = index.clone();
                async move {
                    let (http_req, _) = req.into_parts();
                    let file = NamedFile::open_async(&index).await?;
                    let mut response = file.into_response(&http_req);
                    response.headers_mut().insert(
                        header::CACHE_CONTROL,
                        header::HeaderValue::from_static("no-cache"),
                    );
                    Ok(ServiceResponse::new(http_req, response))
                }
            })),
    );
}
