//! What is waiting for you, at this site, now.
//!
//! D112 states the rule this module exists to enforce: *"a badge counts work
//! waiting for you, at this site, now. Never a total."* A badge reading 1,247
//! is a decoration; one reading 4 is an instruction.
//!
//! # One read, because two would disagree
//!
//! The landing screen and the navigation rail show the same numbers. Fetched
//! separately they would be two computations of one fact, free to drift — the
//! failure this repository has recorded more times than any other. So there is
//! one endpoint, and both read it.
//!
//! # Scoped here rather than trusted to the client
//!
//! Every count below is narrowed to the caller's site and to work that can
//! actually be acted on. Doing that server-side means **the client cannot
//! render a total it was never sent**, which is a stronger guarantee than a
//! rule in a document about what the client ought to filter.
//!
//! # Zero is absent rather than nought
//!
//! The counts come back whatever they are; hiding is the screen's job (a
//! `Badge` drawing nothing at zero). The server says what is true and the
//! interface decides what is worth drawing, which is D114's split.

use serde::Serialize;

use crate::error::ApiError;
use crate::tenancy::TenantScope;
use crate::AppState;
use actix_web::web;
use uuid::Uuid;

/// The counts, grouped the way D110 groups the rail.
#[derive(Serialize, Debug, Default)]
pub struct WorkWaiting {
    /// Commitments at this site with something still to pick.
    pub pack: i64,
    /// Lines waiting to be walked.
    pub pick: i64,
    /// Cartons sealed and not yet on a consignment.
    pub despatch: i64,
    /// Discrepancies open or under investigation — the findings screen's own
    /// default view, so the badge and the screen agree by construction.
    pub findings: i64,
    /// True when the session names no site. Everything above is then zero, and
    /// the screen must say *choose where you are working* rather than *nothing
    /// to do* — opposite instructions that must not look the same.
    pub no_site: bool,
}

/// **No `weigh` count, deliberately, and this is the honest omission.**
///
/// What is due for weighing is not a SQL predicate: `revalidation` reads every
/// current gross weight and then decides in Rust, from the method that produced
/// it and how long ago, using `staleness` and `trust_of`. A `count(*)` here
/// would be a *second* definition of "stale", free to disagree with the screen
/// it labels — which is the exact failure this module exists to avoid.
///
/// The fix is to extract that worklist so the handler and this both call it,
/// and it belongs with the port of the weigh screen rather than bolted on here.
/// Until then the rail carries no weigh badge, which is a gap somebody can see
/// rather than a number nobody can trust.
pub async fn waiting(
    state: &web::Data<AppState>,
    tenant_id: Uuid,
    site_id: Option<Uuid>,
) -> Result<WorkWaiting, ApiError> {
    let Some(site) = site_id else {
        return Ok(WorkWaiting {
            no_site: true,
            ..Default::default()
        });
    };

    let mut scope = TenantScope::begin(&state.pool, tenant_id).await?;
    let row = scope
        .run(move |tx| {
            Box::pin(async move {
                // One round trip. Four correlated counts is one plan and one
                // set of locks, where four statements would be four of each on
                // a screen somebody opens all day.
                Ok(tx
                    .query_one(
                        "SELECT
                           (SELECT count(*) FROM fulfilment f
                             WHERE f.site_id = $1
                               AND f.state <> 'cancelled'
                               AND EXISTS (SELECT 1 FROM fulfilment_line fl
                                            WHERE fl.fulfilment_id = f.id
                                              AND fl.picked_quantity < fl.quantity)),
                           (SELECT count(*) FROM fulfilment_line fl
                              JOIN fulfilment f ON f.id = fl.fulfilment_id
                             WHERE f.site_id = $1
                               AND f.state <> 'cancelled'
                               AND fl.picked_quantity < fl.quantity),
                           -- Sealed and on no consignment: the despatch bench's
                           -- own definition of a carton waiting.
                           (SELECT count(*) FROM package p
                              JOIN fulfilment f ON f.id = p.fulfilment_id
                              LEFT JOIN consignment_package cp ON cp.package_id = p.id
                             WHERE f.site_id = $1
                               AND p.sealed_at IS NOT NULL
                               AND cp.consignment_id IS NULL),
                           (SELECT count(*) FROM discrepancy d
                             WHERE d.state IN ('open', 'investigating'))",
                        &[&site],
                    )
                    .await?)
            })
        })
        .await?;

    Ok(WorkWaiting {
        pack: row.get(0),
        pick: row.get(1),
        despatch: row.get(2),
        findings: row.get(3),
        no_site: false,
    })
}
