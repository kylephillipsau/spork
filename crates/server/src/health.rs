//! Liveness, readiness, and what the deployment can be asked about itself.
//!
//! Ported in shape from Nosdesk's `handlers::health`, which draws the line this
//! module draws: a cheap check that the process is alive, and a separate one
//! that says whether this instance can serve correct answers. What is *not*
//! ported is the reasoning, because the reasoning there is about Kubernetes and
//! this runs under Compose.
//!
//! # What Docker actually does with an unhealthy container, tested
//!
//! Nosdesk's comment says liveness does no I/O so *"a transient DB or Redis
//! blip never restarts the container"*. Under Kubernetes that is exactly right:
//! a failing liveness probe kills the pod.
//!
//! **Docker does not restart a container for being unhealthy.** Run one with
//! `--restart unless-stopped` and a healthcheck that exits 1, and it sits there
//! `unhealthy` with `RestartCount=0` indefinitely — checked here rather than
//! recalled, because "the orchestrator restarts unhealthy containers" is the
//! kind of true-somewhere claim that reads as true everywhere.
//!
//! So the inversion costs something different here, and it is worth naming
//! because it decides which endpoint the compose healthcheck points at:
//!
//!   1. `depends_on: condition: service_healthy` gates on it, and D137's whole
//!      argument is that a dependent must not start until the thing it depends
//!      on can actually serve.
//!   2. The deploy platform reads it, and a green badge over a server whose database is
//!      unreachable is worse than no badge.
//!
//! **Which is why the compose healthcheck stays on the deep check**, and why
//! copying Kubernetes convention wholesale — where liveness is the cheap one
//! and the orchestrator has a second probe for the rest — would have quietly
//! weakened the one gate this deployment has.
//!
//! # What gates, and what is merely reported
//!
//! Only the database checks decide ready or not ready. Projection staleness is
//! reported and never gates, and that is the register's own line rather than a
//! preference: a structural failure blocks a deploy, a job-asserted failure
//! raises a finding and never stops the floor. J66 already reports a projection
//! past its `freshness_bound` as a finding. A readiness probe that turned the
//! same fact into a 503 would be a J-class failure stopping the floor, which is
//! precisely what D8 refuses.
//!
//! # The detail needs a caller
//!
//! This is reachable from the internet through the tunnel. The verdict is
//! public — anything probing it can already tell a 200 from a 503 — but the
//! migration head, the projection names and the build are schema and deployment
//! internals, and an unauthenticated reader gets none of them. Same rule
//! `GET /images/{digest}` follows: knowing a thing exists is not authority to
//! read it.

use std::sync::atomic::{AtomicBool, Ordering};

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde_json::json;

use crate::routes::caller;
use crate::AppState;

/// The commit this binary was built from, or `None` in a development build.
///
/// Read at compile time from an environment variable the image build sets. A
/// binary that cannot say which build it is turns "is the deploy live?" into a
/// question only the person with SSH can answer — which is the state this was
/// added from, having watched a deploy queue and been unable to tell from
/// outside whether it had landed.
pub const BUILD: Option<&str> = option_env!("SPORK_GIT_SHA");

/// Set once the process is shutting down. While set, readiness reports 503 so
/// anything routing conditionally drains this instance *before* it stops
/// accepting connections. Liveness stays 200: the process is alive and being
/// killed for being alive mid-drain is the failure this separation prevents.
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// Flip readiness to draining. Called from `main`'s signal handler.
pub fn begin_shutdown() {
    SHUTTING_DOWN.store(true, Ordering::Relaxed);
}

pub fn is_draining() -> bool {
    SHUTTING_DOWN.load(Ordering::Relaxed)
}

/// **The process is up. No I/O, no database, no opinion about anything else.**
///
/// Nothing in this deployment reads it — Compose has one healthcheck and it
/// points at the deep one. It exists for the orchestrator that routes
/// conditionally, and the trigger for it mattering is a second replica or a
/// load balancer in front. Until then it is the cheapest possible answer to
/// *did the binary get as far as serving*, which is a different question from
/// *can it serve*, and worth being able to ask separately.
#[get("/live")]
pub async fn liveness() -> HttpResponse {
    HttpResponse::Ok().json(json!({ "status": "live" }))
}

/// Can this instance serve correct answers.
///
/// Registered at both `/health` and `/readiness`: the first is what the compose
/// healthcheck and the runbook already name, and moving that in the same commit
/// that introduces the second would point a deployed healthcheck at a path the
/// running image does not have yet. Same reason the tunnel cutover used a
/// second hostname rather than moving the first.
#[get("/readiness")]
pub async fn readiness(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    respond(&req, &state).await
}

#[get("/health")]
pub async fn health(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    respond(&req, &state).await
}

async fn respond(req: &HttpRequest, state: &web::Data<AppState>) -> HttpResponse {
    // Checked before the database, because once we are terminating the answer
    // is the same whatever the database says and a probe should not open a
    // connection to find that out.
    if is_draining() {
        return HttpResponse::ServiceUnavailable()
            .insert_header(("retry-after", "5"))
            .json(json!({ "status": "draining" }));
    }

    let database = probe(state).await;
    let detail = match caller(state, req).await {
        Ok(_) => Some(detail(state).await),
        // Not an error: an unauthenticated probe is the ordinary case and gets
        // the verdict without the internals.
        Err(_) => None,
    };

    let body = |status: &str| {
        let mut b = json!({ "status": status, "checks": { "database": database.as_deref().map_or("ok", |_| "fail") } });
        if let Some(d) = &detail {
            b["detail"] = d.clone();
        }
        if let Some(why) = &database {
            b["checks"]["database_error"] = json!(why);
        }
        b
    };

    match database {
        // **`ok`, not `ready`, and the asymmetry with the other two words is
        // deliberate.** `{"status":"ok"}` is the contract `/health` has always
        // answered with, it is what the compose healthcheck greps for, and it
        // is what `pack_walk_http` asserts. Renaming it to match the vocabulary
        // of the states it never had would have broken a deployed healthcheck
        // to make three strings rhyme — and would have left the grep passing by
        // accident, on the `"database":"ok"` underneath it.
        None => HttpResponse::Ok().json(body("ok")),
        Some(_) => HttpResponse::ServiceUnavailable()
            .insert_header(("retry-after", "5"))
            .json(body("not_ready")),
    }
}

/// The one check that gates: a usable connection that is not a superuser.
///
/// `ensure_app_role` is the half worth keeping from the original handler. A
/// connection that bypasses row-level security serves *wrong* answers rather
/// than none, and this is the only place that would notice before a request
/// does.
async fn probe(state: &web::Data<AppState>) -> Option<String> {
    let conn = match state.pool.get().await {
        Ok(c) => c,
        Err(e) => return Some(format!("pool: {e}")),
    };
    if let Err(e) = crate::tenancy::ensure_app_role(&conn).await {
        return Some(format!("role: {e}"));
    }
    match conn.query_one("SELECT 1", &[]).await {
        Ok(_) => None,
        Err(e) => Some(format!("query: {e}")),
    }
}

/// What the deployment can be asked about itself, for a caller who is signed in.
///
/// Every number here is read from something that already exists: the migration
/// ledger D137 added, and the freshness table D95 added and J66 already reads.
/// Nothing is computed for the sake of being reported.
async fn detail(state: &web::Data<AppState>) -> serde_json::Value {
    let mut out = json!({ "build": BUILD });

    let Ok(conn) = state.pool.get().await else {
        return out;
    };

    if let Ok(row) = conn
        .query_one(
            "SELECT count(*)::bigint, coalesce(max(name), '')
               FROM schema_migration",
            &[],
        )
        .await
    {
        out["schema"] = json!({
            "applied": row.get::<_, i64>(0),
            "head": row.get::<_, String>(1),
        });
    }

    // J66's own comparison, counted rather than listed: a projection is overdue
    // when it has never run or its last run is older than the bound the step
    // declares. `last_error` is separate because a step that failed and a step
    // that is merely late are different things to be told about.
    //
    // **The grain is (tenant, step), and `steps` is counted distinctly.**
    // `projection_step` is global and `projection_freshness` is per tenant, so
    // a plain `count(*)` over the join reports one step per tenant as several
    // steps — which on a two-tenant deployment would have said twenty-six
    // where there are thirteen. Overdue really is a per-tenant fact, so it
    // keeps the pair grain and the field says so.
    if let Ok(row) = conn
        .query_one(
            "SELECT count(DISTINCT s.function_name)::bigint,
                    count(*) FILTER (WHERE f.last_run_at IS NULL
                                        OR now() - f.last_run_at > s.freshness_bound)::bigint,
                    count(*) FILTER (WHERE f.last_error IS NOT NULL)::bigint,
                    max(f.last_run_at)
               FROM projection_step s
               LEFT JOIN projection_freshness f ON f.function_name = s.function_name",
            &[],
        )
        .await
    {
        out["projections"] = json!({
            "steps": row.get::<_, i64>(0),
            "overdue_tenant_steps": row.get::<_, i64>(1),
            "failing_tenant_steps": row.get::<_, i64>(2),
            "last_run_at": row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(3)
                .map(|t| t.to_rfc3339()),
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draining_is_one_way_and_starts_off() {
        // Asserted because the flag is process-global and a test that flipped
        // it would poison every other test in the binary. Nothing here flips
        // it; `main` does, once, on the way out.
        assert!(!is_draining());
    }
}
