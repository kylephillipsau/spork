//! What there is to pack, at the site the caller signed on at.
//!
//! # The four groups are computed, never stored
//!
//! S44 is explicit:
//!
//! > No table in the fulfilment set carries a stored `progress`, completion or
//! > rollup status column … any label it could hold is a function of the four
//! > coverage quantities that can disagree with them.
//!
//! So [`stage`] is a pure function of two numbers, tested beside itself, and
//! **both the server-rendered page and the JSON endpoint call it**. Two copies
//! of that arithmetic is two answers to "is this ready", and the one the
//! operator sees would depend on which screen they opened.
//!
//! # Scoped to the site, which now means something
//!
//! You pack what is in the building you are standing in; a commitment against
//! another warehouse is somebody else's queue. `Caller::site_id` is where they
//! signed on — and until D145 that was null on every browser session, so this
//! filter silently did nothing.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::auth::Caller;
use crate::error::ApiError;
use crate::tenancy::TenantScope;
use crate::AppState;
use actix_web::web;

/// Where a commitment has got to, derived from what is picked against what is
/// committed.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Committed, and nobody has started.
    Ready,
    /// Part picked. Somebody is on it.
    OnTheBench,
    /// Every line covered.
    Packed,
    /// An order with a fulfilment and no lines on it yet.
    NothingCommitted,
}

impl Stage {
    /// What the group is called on a screen. Server-phrased per D114: this is a
    /// judgement about progress rather than a value to format.
    pub fn label(self) -> &'static str {
        match self {
            Stage::Ready => "Ready to pack",
            Stage::OnTheBench => "On the bench",
            Stage::Packed => "Packed",
            Stage::NothingCommitted => "Nothing committed",
        }
    }
}

/// The whole of the grouping rule, in one place.
///
/// Order matters: `NothingCommitted` is tested first because zero committed
/// makes every other comparison meaningless — `picked >= committed` is true of
/// an empty commitment and would file it under Packed.
pub fn stage(picked: i64, committed: i64) -> Stage {
    if committed <= 0 {
        Stage::NothingCommitted
    } else if picked <= 0 {
        Stage::Ready
    } else if picked < committed {
        Stage::OnTheBench
    } else {
        Stage::Packed
    }
}

/// How a promise reads, phrased here rather than formatted on the client.
///
/// D114 draws the line at judgement: "in 3 days" is arithmetic, "overdue" is an
/// opinion about it, and opinions are the server's.
pub fn due(promised: NaiveDate, today: NaiveDate) -> String {
    match (promised - today).num_days() {
        d if d < 0 => "overdue".into(),
        0 => "due today".into(),
        1 => "due tomorrow".into(),
        d => format!("due in {d} days"),
    }
}

#[derive(Serialize, Debug)]
pub struct PackJob {
    pub fulfilment_id: Uuid,
    pub reference: Option<String>,
    pub order_reference: Option<String>,
    pub customer: String,
    pub lines: i64,
    pub committed: i64,
    pub picked: i64,
    pub cartons: i64,
    /// Server-phrased (D114), and absent when nothing was promised.
    pub due: Option<String>,
    pub stage: Stage,
}

/// The queue, at the caller's site.
///
/// **Cancelled commitments are gone; finished ones are not.** The group a job
/// lands in says it is done, and a queue that hides completed work makes "did
/// that go out?" a question you have to search to answer.
pub async fn queue(
    state: &web::Data<AppState>,
    who: &Caller,
    term: &str,
) -> Result<Vec<PackJob>, ApiError> {
    let site = who.site_id;
    // Empty means no filter. A `%%` pattern would match every row including
    // those whose reference is null, which is not the same list.
    let like = if term.trim().is_empty() {
        None
    } else {
        Some(format!("%{}%", term.trim()))
    };

    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        "SELECT f.id, f.reference,
                                coalesce(o.confirmation_number, o.external_ref),
                                coalesce(p.name, 'no customer named'),
                                count(fl.id),
                                coalesce(sum(fl.quantity), 0)::bigint,
                                coalesce(sum(fl.picked_quantity), 0)::bigint,
                                o.promised_to,
                                (SELECT count(*) FROM package pk
                                  WHERE pk.fulfilment_id = f.id)
                           FROM fulfilment f
                           JOIN \"order\" o ON o.id = f.order_id
                           LEFT JOIN party p ON p.id = o.customer_party_id
                           LEFT JOIN fulfilment_line fl ON fl.fulfilment_id = f.id
                          WHERE f.state <> 'cancelled'
                            AND ($1::uuid IS NULL OR f.site_id = $1)
                            AND ($2::text IS NULL
                                 OR f.reference ILIKE $2
                                 OR o.confirmation_number ILIKE $2
                                 OR o.external_ref ILIKE $2
                                 OR p.name ILIKE $2)
                          GROUP BY f.id, f.reference, o.confirmation_number,
                                   o.external_ref, p.name, o.promised_to
                          ORDER BY o.promised_to NULLS LAST, f.id",
                        &[&site, &like],
                    )
                    .await?;

                let today = Utc::now().date_naive();
                Ok(rows
                    .iter()
                    .map(|r| {
                        let committed: i64 = r.get(5);
                        let picked: i64 = r.get(6);
                        let promised: Option<DateTime<Utc>> = r.get(7);
                        PackJob {
                            fulfilment_id: r.get(0),
                            reference: r.get(1),
                            order_reference: r.get(2),
                            customer: r.get(3),
                            lines: r.get(4),
                            committed,
                            picked,
                            cartons: r.get(8),
                            due: promised.map(|p| due(p.date_naive(), today)),
                            stage: stage(picked, committed),
                        }
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_commitment_with_no_lines_is_not_packed() {
        // The ordering bug this function's shape exists to prevent: with zero
        // committed, `picked >= committed` is true and would file an empty
        // commitment under Packed — which reads as "that went out".
        assert_eq!(stage(0, 0), Stage::NothingCommitted);
    }

    #[test]
    fn nothing_picked_against_something_committed_is_ready() {
        assert_eq!(stage(0, 24), Stage::Ready);
    }

    #[test]
    fn part_picked_is_on_the_bench() {
        assert_eq!(stage(1, 24), Stage::OnTheBench);
        assert_eq!(stage(23, 24), Stage::OnTheBench);
    }

    #[test]
    fn fully_covered_is_packed_and_so_is_over_covered() {
        assert_eq!(stage(24, 24), Stage::Packed);
        // Over-picked is a finding J56 raises elsewhere; it is not this
        // function's job to hide it by refusing to call the job done.
        assert_eq!(stage(30, 24), Stage::Packed);
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    /// **The fixture promises relative to today, so nothing in it is late.**
    /// That is the right thing for a demo and it leaves the arm a warehouse
    /// cares about most unexercised by any page test. This is a pure function,
    /// so it costs nothing to check here instead.
    ///
    /// Moved with `due` itself when the queue was extracted: the assertions
    /// lived beside the server-rendered page that used to own the function, and
    /// leaving them there would have been a test of a copy that no longer
    /// exists.
    #[test]
    fn a_promise_is_read_against_today_and_phrased_rather_than_formatted() {
        let today = d(2026, 8, 13);
        assert_eq!(due(d(2026, 8, 12), today), "overdue");
        assert_eq!(due(d(2026, 1, 1), today), "overdue", "however late");
        assert_eq!(due(today, today), "due today");
        assert_eq!(due(d(2026, 8, 14), today), "due tomorrow");
        assert_eq!(due(d(2026, 8, 20), today), "due in 7 days");
    }
}
