//! How many orders each person packed, as NetSuite's Packed By says (D254).
//!
//! An administrator's read (D192): a count per person is a measure of people,
//! not work waiting, so it is not on the dashboard everybody opens.
//!
//! **Three spans in one read**: today, this week (from Monday) and the last 30
//! days, each by the site's own clock. The screen switches between them
//! without asking again, and the three cannot disagree about who was counted.

use actix_web::{get, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::error::ApiError;
use crate::routes::{administrator, caller};
use crate::tenancy::TenantScope;
use crate::AppState;

/// One packer's counts. `packed_by` is absent for fulfilments NetSuite packed
/// naming nobody.
#[derive(Serialize, Debug, PartialEq)]
pub struct PackerCounts {
    pub packed_by: Option<String>,
    pub today: i64,
    pub this_week: i64,
    pub last_30_days: i64,
}

#[derive(Serialize, Debug, Default)]
pub struct PackedByPerson {
    /// Everybody who packed in the last 30 days, most first.
    pub packers: Vec<PackerCounts>,
    /// When NetSuite last said what it had packed; absent before the Bridge
    /// has sent it.
    pub as_at: Option<DateTime<Utc>>,
    /// True when the session names no site, so nothing was counted.
    pub no_site: bool,
}

#[get("/packed-by")]
pub async fn packed_by(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    administrator(&state, &who).await?;
    let Some(site) = who.site_id else {
        return Ok(HttpResponse::Ok().json(PackedByPerson { no_site: true, ..Default::default() }));
    };
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let answer = scope
        .run(move |tx| {
            Box::pin(async move {
                let packers = tx
                    .query(
                        "WITH since AS (
                             SELECT date_trunc('day', now() AT TIME ZONE timezone) AT TIME ZONE timezone AS today,
                                    date_trunc('week', now() AT TIME ZONE timezone) AT TIME ZONE timezone AS week,
                                    (date_trunc('day', now() AT TIME ZONE timezone) - interval '29 days')
                                        AT TIME ZONE timezone AS month
                               FROM site WHERE id = $1
                         )
                         SELECT r.packed_by,
                                count(*) FILTER (WHERE r.packed_at >= since.today),
                                count(*) FILTER (WHERE r.packed_at >= since.week),
                                count(*) FILTER (WHERE r.packed_at >= since.month)
                           FROM reported_packing r, since
                          WHERE r.site_id = $1 AND r.packed_at >= since.month
                          GROUP BY r.packed_by
                          ORDER BY 4 DESC, r.packed_by NULLS LAST",
                        &[&site],
                    )
                    .await?
                    .iter()
                    .map(|r| PackerCounts { packed_by: r.get(0), today: r.get(1), this_week: r.get(2), last_30_days: r.get(3) })
                    .collect();
                let as_at = tx
                    .query_one("SELECT max(as_at) FROM reported_packing WHERE site_id = $1", &[&site])
                    .await?
                    .get(0);
                Ok(PackedByPerson { packers, as_at, no_site: false })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(answer))
}
