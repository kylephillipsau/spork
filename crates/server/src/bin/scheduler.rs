//! Projection drain loop.
//!
//! Calls `projection_run_dirty` on an interval so tenants marked dirty by floor
//! writes (D107) get a full rebuild without the app holding EXECUTE on
//! `projection_run_all`. Connects as a role that can become `nylonite_scheduler`
//! (or already is that role). Superuser connections SET ROLE once and refuse to
//! run maintainers as a role that bypasses RLS.

use deadpool_postgres::{Config, Runtime};
use std::str::FromStr;
use std::time::Duration;
use tokio_postgres::NoTls;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:nylonite@localhost:55432/nylonite".to_string()
    })
}

fn interval() -> Duration {
    let secs: u64 = std::env::var("SCHEDULER_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    Duration::from_secs(secs.max(1))
}

#[tokio::main]
async fn main() {
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
        .create_pool(Some(Runtime::Tokio1), NoTls)
        .expect("could not create the connection pool");

    // Become the scheduler role when the connection is a superuser login that
    // only exists so NOLOGIN design roles can be assumed.
    {
        let conn = pool.get().await.expect("pool");
        let row = conn
            .query_one(
                "SELECT current_user::text, rolsuper, rolbypassrls
                   FROM pg_roles WHERE rolname = current_user",
                &[],
            )
            .await
            .expect("role check");
        let user: String = row.get(0);
        let superuser: bool = row.get(1);
        let bypass: bool = row.get(2);
        if superuser || bypass {
            conn.batch_execute("SET ROLE nylonite_scheduler")
                .await
                .expect("SET ROLE nylonite_scheduler");
            tracing::info!(
                from = %user,
                "assumed nylonite_scheduler (connection bypassed RLS)"
            );
        } else if user != "nylonite_scheduler" {
            // A login role that is a member of the scheduler may still need SET ROLE.
            match conn.batch_execute("SET ROLE nylonite_scheduler").await {
                Ok(()) => tracing::info!(from = %user, "assumed nylonite_scheduler"),
                Err(e) => {
                    tracing::error!(
                        role = %user,
                        error = %e,
                        "connect as nylonite_scheduler or a superuser that can SET ROLE it"
                    );
                    std::process::exit(1);
                }
            }
        } else {
            tracing::info!(role = %user, "running as nylonite_scheduler");
        }
    }

    let period = interval();
    tracing::info!(?period, "nylonite scheduler draining projection_dirty");

    loop {
        match drain_once(&pool).await {
            Ok(n) => {
                if n > 0 {
                    tracing::info!(rows_touched = n, "drained dirty tenants");
                } else {
                    tracing::debug!("no dirty work");
                }
            }
            Err(e) => tracing::error!(error = %e, "drain failed"),
        }
        tokio::time::sleep(period).await;
    }
}

async fn drain_once(pool: &deadpool_postgres::Pool) -> Result<i64, Box<dyn std::error::Error>> {
    let conn = pool.get().await?;
    // Role is session-level on pooled connections; re-assert so a recycled
    // connection that somehow reset still runs as the scheduler.
    let _ = conn.batch_execute("SET ROLE nylonite_scheduler").await;
    let n: i64 = conn
        .query_one("SELECT projection_run_dirty()", &[])
        .await?
        .get(0);
    Ok(n)
}
