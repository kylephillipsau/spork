//! Live numbers from the ledger — the truth, not the cache.
//!
//! Projected columns on `fulfilment_line` and `package` lag until maintainers
//! run (D25, D95). After a pick or despatch the floor needs the consequence of
//! *that* write immediately. These queries restate D99/D103 over one line or
//! package only (D106's scoped form), so they are O(that entity), not O(tenant
//! history).
//!
//! They do not UPDATE anything. They are a second *view* of the same facts.

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::error::ApiError;

/// Progress quantities — same four numbers whether from the projection or the ledger.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub covered_quantity: i64,
    pub picked_quantity: i64,
    pub packed_quantity: i64,
    pub despatched_quantity: i64,
    pub uncovered_quantity: i64,
}

/// Projection cache, with how old it is (D95).
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct ProjectionProgress {
    #[serde(flatten)]
    pub progress: Progress,
    /// When the fulfilment rebuild last ran for this tenant. NULL = never.
    pub as_at: Option<DateTime<Utc>>,
}

/// Live fold of one fulfilment line from the ledger (D99 shapes, D103 netting).
pub async fn line_progress_ledger(
    tx: &Transaction<'_>,
    line_id: Uuid,
    commitment_quantity: i64,
) -> Result<Progress, ApiError> {
    let row = tx
        .query_one(
            "WITH RECURSIVE roots AS (
                 SELECT m.id, m.quantity, m.fulfilment_line_id,
                        m.from_location_id, fl.kind AS from_kind,
                        m.to_location_id, m.to_package_id
                   FROM stock_movement m
                   LEFT JOIN location fl ON fl.id = m.from_location_id
                  WHERE m.fulfilment_line_id = $1
                    AND m.reverses_movement_id IS NULL
             ),
             -- **The root's kind, carried down the chain.** A correction
             -- reverses a movement and takes its shape from it, so what matters
             -- is where the *root* came out of — a reversal of a pick is not a
             -- pick out of wherever the reversal happens to name.
             chain AS (
                 SELECT id AS root_id, id AS movement_id, quantity, 0 AS depth,
                        fulfilment_line_id, from_location_id, from_kind,
                        to_location_id, to_package_id
                   FROM roots
                 UNION ALL
                 SELECT c.root_id, r.id, r.quantity, c.depth + 1,
                        c.fulfilment_line_id, c.from_location_id, c.from_kind,
                        c.to_location_id, c.to_package_id
                   FROM chain c
                   JOIN stock_movement r ON r.reverses_movement_id = c.movement_id
             ),
             effective AS (
                 SELECT fulfilment_line_id, from_location_id, from_kind,
                        to_location_id, to_package_id,
                        sum(CASE WHEN depth % 2 = 0 THEN quantity ELSE -quantity END)::bigint
                            AS eq
                   FROM chain
                  GROUP BY 1, 2, 3, 4, 5
             ),
             ledger AS (
                 -- D166: out of *storage*, not merely out of a location. A
                 -- trolley pick is two legs — shelf to the packing station,
                 -- then station to the carton — and both leave a location, so
                 -- the older reading counted one order's units twice and J56
                 -- raised `picked > covered` about a warehouse that had done
                 -- nothing wrong. `staging` and `dock` are where things are
                 -- put down on the way somewhere; the other three are storage.
                 SELECT coalesce(sum(e.eq) FILTER (
                            WHERE e.from_kind IN ('pick_face', 'bulk', 'overflow')),
                        0)::bigint AS picked,
                        coalesce(sum(e.eq) FILTER (
                            WHERE p.status IN ('sealed', 'despatched')), 0)::bigint AS packed,
                        coalesce(sum(e.eq) FILTER (
                            WHERE e.to_location_id IS NULL
                              AND e.to_package_id IS NULL), 0)::bigint AS despatched
                   FROM effective e
                   LEFT JOIN package p ON p.id = e.to_package_id
             ),
             intention AS (
                 SELECT coalesce(sum(quantity), 0)::bigint AS covered
                   FROM stock_allocation
                  WHERE fulfilment_line_id = $1
                    AND state IN ('allocated','picking','picked','packed','fulfilled')
             )
             SELECT i.covered, l.picked, l.packed, l.despatched
               FROM ledger l CROSS JOIN intention i",
            &[&line_id],
        )
        .await?;

    let covered: i64 = row.get(0);
    let picked: i64 = row.get(1);
    let packed: i64 = row.get(2);
    let despatched: i64 = row.get(3);
    Ok(Progress {
        covered_quantity: covered,
        picked_quantity: picked,
        packed_quantity: packed,
        despatched_quantity: despatched,
        uncovered_quantity: commitment_quantity - covered,
    })
}

/// Projection columns already on the row, plus when fulfilment rebuild last ran.
pub async fn line_progress_projection(
    tx: &Transaction<'_>,
    covered: i64,
    picked: i64,
    packed: i64,
    despatched: i64,
    uncovered: i64,
) -> Result<ProjectionProgress, ApiError> {
    let as_at: Option<DateTime<Utc>> = tx
        .query_opt(
            "SELECT f.last_run_at
               FROM projection_freshness f
              WHERE f.function_name = 'projection_fulfilment_rebuild'
                AND f.tenant_id = nullif(current_setting('nylonite.tenant_id', true), '')::uuid",
            &[],
        )
        .await?
        .and_then(|r| r.get(0));

    Ok(ProjectionProgress {
        progress: Progress {
            covered_quantity: covered,
            picked_quantity: picked,
            packed_quantity: packed,
            despatched_quantity: despatched,
            uncovered_quantity: uncovered,
        },
        as_at,
    })
}

/// Winning package status from the event log (J6 order), without waiting for the fold.
pub async fn package_status_ledger(
    tx: &Transaction<'_>,
    package_id: Uuid,
) -> Result<Option<String>, ApiError> {
    let row = tx
        .query_opt(
            "SELECT kind
               FROM package_event
              WHERE package_id = $1
                AND kind IN ('created','placed','contained','sealed','opened','despatched','voided')
              ORDER BY occurred_at DESC, recorded_at DESC, id DESC
              LIMIT 1",
            &[&package_id],
        )
        .await?;
    Ok(row.map(|r| r.get(0)))
}
