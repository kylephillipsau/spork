//! What every integration test needs before it can assert anything.
//!
//! Three functions, and between them they had **forty-nine copies** across
//! twenty-six files: `url` twenty-six times, `pool` fourteen, `connect` nine.
//! The copies were not variations on a theme — `pool` differed only in where
//! `rustfmt` had broken the lines, and `connect` only in the wording of an
//! `expect` nobody reads. One of them was wrong: `import_over_http` read
//! `DATABASE_URL` alone and would have skipped rather than run wherever
//! `DATABASE_URL_APP` is the one that is set.
//!
//! **`tests/common/mod.rs` rather than `tests/common.rs`**, because cargo
//! compiles every top-level file in `tests/` as its own test binary and the
//! second form would run as an empty suite reporting success — which is the
//! shape of failure this repository writes down most.
//!
//! Every helper is allowed to be unused: each test binary takes what it needs
//! and a file that uses two of the three would otherwise fail `-D warnings` for
//! the third.

#![allow(dead_code)]

use std::str::FromStr;

use tokio_postgres::NoTls;

/// The database, and whether a connection to it has to assume the app role.
///
/// **The boolean is the whole reason there are two of these.** D94: every role
/// in this schema is NOLOGIN, so `DATABASE_URL_APP` names a connection nobody
/// has made and the suites fall back to `DATABASE_URL` — which arrives as
/// `postgres`, bypasses row-level security, and would make a tenancy assertion
/// pass by not applying. `true` means "you are the superuser, `SET ROLE` before
/// you assert anything about what a tenant can see".
///
/// Skipping silently when neither is set is deliberate and documented: the
/// alternative is a suite that cannot run on a machine without a database.
pub fn url_and_role() -> Option<(String, bool)> {
    if let Ok(u) = std::env::var("DATABASE_URL_APP") {
        return Some((u, false));
    }
    std::env::var("DATABASE_URL").ok().map(|u| (u, true))
}

/// The same, for tests that go through the HTTP layer and never hold a
/// connection of their own — `TenantScope` assumes the role for them.
pub fn url() -> Option<String> {
    url_and_role().map(|(u, _)| u)
}

/// A pool shaped the way `main` shapes one.
///
/// Built field by field from a parsed connection string rather than handed the
/// string, because `deadpool` wants a `Config` and the two disagree about what
/// a URL is.
pub fn pool(u: &str) -> deadpool_postgres::Pool {
    let pg = tokio_postgres::Config::from_str(u).expect("a usable connection string");
    let mut cfg = deadpool_postgres::Config::new();
    cfg.host = pg.get_hosts().first().map(|h| match h {
        tokio_postgres::config::Host::Tcp(s) => s.clone(),
        #[cfg(unix)]
        tokio_postgres::config::Host::Unix(p) => p.to_string_lossy().into_owned(),
    });
    cfg.port = pg.get_ports().first().copied();
    cfg.user = pg.get_user().map(str::to_string);
    cfg.password = pg
        .get_password()
        .map(|p| String::from_utf8_lossy(p).into_owned());
    cfg.dbname = pg.get_dbname().map(str::to_string);
    cfg.create_pool(Some(deadpool_postgres::Runtime::Tokio1), tokio_postgres::NoTls)
        .expect("a pool")
}

/// One connection, with the role assumed when [`url_and_role`] says to.
///
/// The connection task is spawned and its error printed rather than dropped: a
/// connection that dies mid-test otherwise shows up as a query hanging, which
/// is a considerably longer afternoon than a line on stderr.
pub async fn connect(u: &str, assume_role: bool) -> tokio_postgres::Client {
    schema_is_current().await;
    let (client, connection) = tokio_postgres::connect(u, NoTls).await.expect("connect");
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {e}");
        }
    });
    if assume_role {
        client
            .batch_execute("SET ROLE nylonite_app")
            .await
            .expect("become the application role");
    }
    client
}

// ---------------------------------------------------------------------------
// What every test does before it can assert anything over HTTP
// ---------------------------------------------------------------------------

/// The fixture's operator, and the site they work at.
///
/// Named here rather than re-declared per file so that a fixture change is one
/// edit. `sign_on` and `change_password` still spell the credential out: they
/// are tests *about* signing in, and a test that authenticates through a helper
/// cannot assert what the helper hides.
pub const EMAIL: &str = "kyle@example.test";
pub const PASSWORD: &str = "dock-station-1";
pub const SITE: &str = "a5170000-0000-0000-0000-000000000001";

/// Sign in and return the `authorization` header value.
///
/// **Fourteen files opened with this block.** Twelve named the same site, two
/// deliberately did not, and the only other difference between them was the
/// sentence in the `assert!` — which nobody reads, because a failure here means
/// the fixture is missing rather than the test being wrong.
pub async fn bearer<S>(app: &S) -> String
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    sign_in(app, Some(SITE)).await
}

/// Sign in without naming a site.
///
/// The state `where_you_are_working` is about, and the one `import_over_http`
/// happens not to need. Separate rather than an `Option` at every call site,
/// because two callers should not cost twelve `Some(SITE)`s.
pub async fn bearer_without_site<S>(app: &S) -> String
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    sign_in(app, None).await
}

/// Sign in as somebody other than the fixture's default operator.
///
/// Exposed because tenancy tests need a session belonging to a different
/// tenant, and a helper that always signs in as Alpha would make those tests
/// pass by not testing anything.
pub async fn sign_in_as<S>(app: &S, email: &str, site: Option<&str>) -> String
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    sign_in_with(app, email, PASSWORD, site).await
}

async fn sign_in<S>(app: &S, site: Option<&str>) -> String
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    sign_in_with(app, EMAIL, PASSWORD, site).await
}

async fn sign_in_with<S>(app: &S, email: &str, password: &str, site: Option<&str>) -> String
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    // **Here and in `connect`, which between them is every test that touches
    // the database.** Signing in is the first thing an HTTP test does and
    // taking a connection is the first thing the others do, so a stale schema
    // is reported before the first assertion rather than by it.
    schema_is_current().await;
    let mut body = serde_json::json!({ "email": email, "password": password });
    if let Some(s) = site {
        body["site_id"] = serde_json::json!(s);
    }
    let resp = actix_web::test::call_service(
        app,
        actix_web::test::TestRequest::post()
            .uri("/sessions")
            .set_json(body)
            .to_request(),
    )
    .await;
    let status = resp.status();
    let text = String::from_utf8_lossy(&actix_web::test::read_body(resp).await).to_string();
    assert!(
        status.is_success(),
        "signing in returned {status}: {text} — is the fixture loaded?"
    );
    let signed: serde_json::Value = serde_json::from_str(&text).expect("the session is JSON");
    format!(
        "Bearer {}",
        signed["token"].as_str().expect("a session carries a token")
    )
}

/// The body of a response, with the status attached to the panic when it is not
/// a success.
///
/// **Bytes first, then JSON.** A 404 comes back with an empty body, and
/// decoding that as JSON panics inside actix with no mention of which call it
/// was, which is exactly the unhelpful failure a harness exists to prevent.
pub async fn ok_json<S>(app: &S, req: actix_http::Request, what: &str) -> serde_json::Value
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let resp = actix_web::test::call_service(app, req).await;
    let status = resp.status();
    let bytes = actix_web::test::read_body(resp).await;
    let text = String::from_utf8_lossy(&bytes).to_string();
    assert!(
        status.is_success(),
        "{what} returned {status}: {}",
        if text.is_empty() {
            "(empty body — is the route registered?)"
        } else {
            &text
        }
    );
    serde_json::from_str(&text).unwrap_or_else(|e| {
        panic!("{what} returned {status} with a body that is not JSON ({e}): {text}")
    })
}

// ---------------------------------------------------------------------------
// Is this database the one the tree describes?
// ---------------------------------------------------------------------------

/// **Fail with the reason, rather than with whatever the reason breaks first.**
///
/// A schema behind the code does not announce itself. On 2026-09-07 the dev
/// database was missing migrations 85 and 90, and what that looked like from
/// outside was `binding_over_http` asserting `200 == 500` against a body
/// reading `{"error":"internal error"}` — the handler was inserting into
/// `item_barcode.packaging_level`, a column that was not there. Nothing said
/// so: `error.rs` maps everything unrecognised to that sentence, and the
/// `tracing::error!` beside it goes nowhere because the test harness installs
/// no subscriber. Worse, `cargo test` stops at the first failing binary, so
/// twenty-four of the thirty-six targets never ran and the suite looked like it
/// was mostly passing.
///
/// The same failure, with the same cause, had already been diagnosed a week
/// earlier and written down. It cost an afternoon the second time anyway, which
/// is the argument for putting the check in the code rather than in a note.
///
/// # Why this does not read `schema_migration`
///
/// Because the ledger is empty in CI, and a check that reads it would fail
/// every build. `verify-migrations.sh` applies every file with `psql` and never
/// writes the ledger — the ledger belongs to `migrate.sh` — and CI runs
/// `verify-migrations.sh` immediately before this suite. So a correct database
/// routinely reports nought of ninety-one, and `migrate.sh --status` lists
/// every migration as pending while the schema is complete.
///
/// **The schema is the only thing that cannot be lied to**, so this asks the
/// schema.
///
/// # What it probes, and why that stays right on its own
///
/// The newest migration that changes a table's shape, found by reading the
/// tree. Nothing is listed here to be kept up to date: the probe is derived
/// from the migration files, so the migration you add tomorrow is the one this
/// checks. That is deliberate — a hand-kept list of things to check is the
/// allow-list shape this repository has already been bitten by twice, and it
/// fails by passing.
///
/// It is a spot check and says so. A database missing something in the middle
/// of the set while holding the newest column is not a state anything here
/// produces, and catching it would mean replaying the set rather than probing
/// it.
pub async fn schema_is_current() {
    // Once per test binary. Two tests racing here costs one extra query and
    // nothing else, which is cheaper than a lock on a path that runs before
    // every sign-in.
    static CHECKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if CHECKED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }

    let Some((u, _)) = url_and_role() else { return };
    let Some(probe) = newest_shape_change() else { return };

    let (client, connection) = tokio_postgres::connect(&u, NoTls).await.expect("connect");
    tokio::spawn(async move { let _ = connection.await; });

    let present: bool = match &probe.column {
        Some(column) => client
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM information_schema.columns
                                 WHERE table_name = $1 AND column_name = $2)",
                &[&probe.table, column],
            )
            .await,
        None => client
            .query_one("SELECT to_regclass($1) IS NOT NULL", &[&probe.table])
            .await,
    }
    .expect("ask the schema what it has")
    .get(0);

    assert!(
        present,
        "this database is behind the tree: migration {} adds {}, and it is not there.\n\
         \n\
         Every failure after this one is about that, not about the code. Build a \
         database the suite can be trusted against:\n\
         \n    psql \"$DATABASE_URL\" -c 'DROP SCHEMA public CASCADE; CREATE SCHEMA public;'\
         \n    scripts/verify-migrations.sh\
         \n    scripts/migrate.sh --baseline\n\
         \n\
         `schema_migration` is not the thing to check — it is empty on a database \
         built by verify-migrations, so both it and `migrate.sh --status` will \
         tell you everything is pending when nothing is.",
        probe.migration,
        match &probe.column {
            Some(c) => format!("{}.{c}", probe.table),
            None => format!("the table {}", probe.table),
        }
    );
}

/// A table or column some migration introduces, and which one.
#[derive(Debug)]
pub struct ShapeChange {
    pub migration: String,
    pub table: String,
    /// `None` for a whole new table.
    pub column: Option<String>,
}

/// The newest migration that adds a table or a column, read out of the tree.
///
/// Newest first, because that is the end a stale database is behind at, and it
/// stops at the first migration with anything to probe: the newest migration is
/// not always one — 91 is a `CREATE OR REPLACE FUNCTION`, which exists either
/// way and would prove nothing.
pub fn newest_shape_change() -> Option<ShapeChange> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();
    dirs.sort();

    for dir in dirs.iter().rev() {
        let Ok(sql) = std::fs::read_to_string(dir.join("up.sql")) else { continue };
        // Comments in this repository are longer than the statements and quote
        // the DDL they are about, so they have to go before anything is read.
        let stripped: String = sql
            .lines()
            .map(|l| l.split("--").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join(" ");
        let flat = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
        let name = dir.file_name()?.to_string_lossy().into_owned();

        if let Some(found) = after(&flat, "ALTER TABLE ", &["ADD COLUMN "]) {
            return Some(ShapeChange { migration: name, table: found.0, column: Some(found.1) });
        }
        if let Some((table, _)) = after(&flat, "CREATE TABLE ", &[]) {
            return Some(ShapeChange { migration: name, table, column: None });
        }
    }
    None
}

/// The identifier after `head`, and the one after the first of `then`.
///
/// Deliberately small rather than general: it reads the two DDL forms above and
/// nothing else, and anything it does not understand yields `None`, which makes
/// the check skip rather than guess.
fn after(flat: &str, head: &str, then: &[&str]) -> Option<(String, String)> {
    let mut from = 0usize;
    while let Some(at) = flat[from..].find(head) {
        let start = from + at + head.len();
        let rest = &flat[start..];
        let first = ident(rest)?;
        let Some(next) = then.first() else {
            return Some((first, String::new()));
        };
        // Same statement only: a later `ADD COLUMN` belongs to a later `ALTER`.
        if let Some(stop) = rest.find(';') {
            if let Some(k) = rest[..stop].find(next) {
                if let Some(second) = ident(&rest[k + next.len()..]) {
                    return Some((first, second));
                }
            }
        }
        from = start;
    }
    None
}

/// One identifier, unqualified and unquoted, or nothing if what is there is not
/// one — `IF NOT EXISTS` and `ONLY` are stepped over rather than returned.
fn ident(s: &str) -> Option<String> {
    let mut rest = s.trim_start();
    for skip in ["IF NOT EXISTS ", "ONLY "] {
        if let Some(t) = rest.strip_prefix(skip) {
            rest = t.trim_start();
        }
    }
    let word: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
        .collect();
    let bare = word.rsplit('.').next().unwrap_or("").to_string();
    (!bare.is_empty()).then_some(bare)
}
