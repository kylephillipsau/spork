//! The API server.
//!
//! Sixty-seven endpoints under `/api`, thirty of them reads and thirty-seven
//! writes, with the client bundle mounted at the root beneath them.
//!
//! **This file's job is still the precondition rather than the endpoints.**
//! Everything `main` does before `HttpServer` exists to refuse to start rather
//! than serve wrong answers: tenancy enforced by the database per transaction
//! and a superuser connection rejected outright, a relying party whose origin
//! agrees with its id, a client bundle that says which of present or absent it
//! found, and setup's token reconciled against the database before anything
//! listens.
//!
//! The reads expose the folds the schema already maintains — stock, fulfilment
//! progress, package status. The writes append to `stock_movement` and let the
//! fold follow; nothing here UPDATEs a projected column, and the database holds
//! that rule rather than this comment.

use spork_server::{routes, tenancy, AppState};

use actix_web::{web, App, HttpServer};
use deadpool_postgres::{Config, Runtime};
use std::str::FromStr;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        // The default names spork_app rather than postgres on purpose. A
        // developer who runs this without thinking gets the role the design
        // assumes, not the one that silently disables row level security.
        "postgres://spork_app@localhost:55432/spork".to_string()
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let url = database_url();
    let pg_config = tokio_postgres::Config::from_str(&url)
        .expect("DATABASE_URL is not a valid Postgres connection string");

    let mut cfg = Config::new();
    cfg.host = pg_config.get_hosts().first().map(|h| match h {
        tokio_postgres::config::Host::Tcp(s) => s.clone(),
        #[cfg(unix)]
        tokio_postgres::config::Host::Unix(p) => p.to_string_lossy().into_owned(),
    });
    cfg.port = pg_config.get_ports().first().copied();
    cfg.user = pg_config.get_user().map(str::to_string);
    cfg.password = pg_config
        .get_password()
        .map(|p| String::from_utf8_lossy(p).into_owned());
    cfg.dbname = pg_config.get_dbname().map(str::to_string);

    let pool = cfg
        .create_pool(Some(Runtime::Tokio1), tokio_postgres::NoTls)
        .expect("could not create the connection pool");

    // Refuse to start rather than serve wrong answers. A server connected as a
    // superuser has no tenancy boundary while every policy still reads as
    // though it does, and that is not a condition to discover in production.
    if let Err(e) = tenancy::assert_not_superuser(&pool).await {
        tracing::error!("{e}");
        std::process::exit(1);
    }

    // **Fail here rather than at the first sign-in.** A relying party whose
    // origin disagrees with its id yields credentials no browser will offer
    // back, and the symptom is a button that silently does nothing.
    match spork_server::passkeys::shared() {
        Ok(r) => tracing::info!(rp_id = %r.rp_id, origin = %r.origin, "passkeys enabled"),
        Err(e) => {
            tracing::error!("{e}");
            std::process::exit(2);
        }
    }

    // **Say which of the two it got.** A missing bundle is a normal state —
    // tests, `cargo run`, an image built before the client existed — and the
    // failure worth avoiding is a mount that exists and serves nothing. An
    // absence that announces itself is the difference between "not built" and
    // "built and broken", and only one of those is a five-minute problem.
    let client_dir = spork_server::assets::directory();
    let serve_client = spork_server::assets::present(&client_dir);
    if serve_client {
        tracing::info!(
            dir = %client_dir.display(),
            mount = spork_server::assets::MOUNT,
            "client bundle found"
        );
    } else {
        tracing::info!(
            dir = %client_dir.display(),
            "no client bundle; {} will 404",
            spork_server::assets::MOUNT
        );
    }

    let state = web::Data::new(AppState { pool });

    // One connection LISTENs for changes and tells the devices (D206).
    let live = web::Data::new(spork_server::live::Live::listening(url.clone()));
    let stopping = live.clone();

    // **Setup's token, reconciled against the database before anything listens**
    // (D142). An empty deployment gets one minted and logged; a deployment that
    // has somebody in it gets any leftover token deleted, which is what stops a
    // restored backup leaving a live setup credential on disk.
    match state.pool.get().await {
        Ok(conn) => spork_server::setup::reconcile(&conn).await,
        Err(e) => tracing::warn!(error = %e, "could not reach the database to reconcile the setup token"),
    }

    let bind = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    tracing::info!(%bind, "spork server listening");

    // **This host's identity** (D255), when `SPORK_IDENTITY` asks for it: its
    // own authority made on first start, its certificate for the name it
    // announces renewed by itself, and a page to pair a device with. The
    // HTTPS in front (Caddy, `scripts/local.ps1`) serves the certificate
    // this writes.
    let identity = match std::env::var("SPORK_IDENTITY").as_deref() {
        Ok("1") => Some(spork_server::identity::ensure(&spork_server::identity::host_name())?),
        _ => None,
    };
    if let Some(id) = &identity {
        tracing::info!(name = %id.name, fingerprint = %id.fingerprint, "this host's identity; /pair to pair a device");
    }
    let pairing = identity.as_ref().map(|id| std::sync::Arc::new(spork_server::identity::Pairing::from(id)));

    let server = HttpServer::new(move || {
        let app = App::new()
            // Compressed for whoever asks (gzip, brotli): the client and the
            // reads are text, and a handheld on poor wifi pays for every byte.
            .wrap(actix_web::middleware::Compress::default())
            .app_data(state.clone())
            .app_data(live.clone())
            .configure(routes::mount);
        let app = match pairing.clone() {
            Some(p) => app.configure(move |cfg| spork_server::identity::configure(cfg, p)),
            None => app,
        };
        // Registered here rather than in `routes::mount` so the suite is
        // untouched: `test::init_service` builds its `App` from `configure`,
        // and a static mount pointed at a directory no test has would be a
        // service the harness has to know about to ignore.
        if serve_client {
            let dir = client_dir.clone();
            app.configure(move |cfg| spork_server::assets::configure(cfg, &dir))
        } else {
            app
        }
    })
    .bind(&bind)?
    // **Our signals, so readiness can go first** (D163). Actix installs its own
    // SIGTERM handler and begins stopping immediately, which leaves no moment
    // in which the process is running and reporting not-ready — and that moment
    // is the whole of a drain.
    .disable_signals()
    .run();

    let handle = server.handle();
    tokio::spawn(async move {
        // A process that cannot install a handler should still be killable.
        // Falling back to actix's own behaviour is worse than nothing only if it
        // is silent, so it says so.
        if let Err(e) = stop_requested().await {
            tracing::error!(error = %e, "no shutdown handler; shutdown will not drain");
            return;
        }
        spork_server::health::begin_shutdown();
        // Live streams last minutes; they end now rather than hold the drain.
        stopping.send(spork_server::live::Change::Closing);
        tracing::info!("draining: readiness now reports 503");

        // **Shorter than Docker's grace period, deliberately.** Compose sends
        // SIGKILL ten seconds after SIGTERM unless `stop_grace_period` says
        // otherwise, so a drain that waits longer than that is a drain on
        // paper: the process is killed mid-wait and nothing it was waiting for
        // happens. Two seconds to be noticed, then a graceful stop with the
        // rest of the budget for requests already in flight.
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        handle.stop(true).await;
    });

    server.await
}

/// SIGTERM is what Docker sends on `stop`; SIGINT is a person with a terminal.
/// Both mean the same thing to us.
#[cfg(unix)]
async fn stop_requested() -> std::io::Result<()> {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = term.recv() => {}
        _ = tokio::signal::ctrl_c() => {}
    }
    Ok(())
}

/// Windows has no SIGTERM; a native run is stopped from its console.
#[cfg(not(unix))]
async fn stop_requested() -> std::io::Result<()> {
    tokio::signal::ctrl_c().await
}
