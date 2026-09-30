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

use chrono::NaiveDate;
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
    /// What is done at the bench, per line the larger of what this system
    /// picked and what has gone into a carton (D172). For work picked here the
    /// two are one movement; for work picked elsewhere it is what is boxed.
    pub picked: i64,
    /// What another system reports picked (D172), zero when it reports nothing.
    pub reported: i64,
    /// Where the picking happened when it was not here, server-phrased (D114):
    /// "Picked in NetSuite · IF270947 · by Casual Melbourne".
    pub provenance: Option<String>,
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
                                coalesce(sum(greatest(fl.picked_quantity, bx.q)), 0)::bigint,
                                (o.promised_to AT TIME ZONE coalesce(st.timezone, 'UTC'))::date,
                                (SELECT count(*) FROM package pk
                                  WHERE pk.fulfilment_id = f.id),
                                coalesce(sum(fl.external_picked_quantity), 0)::bigint,
                                (SELECT sc.name FROM source_channel sc
                                  WHERE sc.id = f.source_channel_id),
                                (SELECT ep.picked_by FROM external_pick ep
                                   JOIN fulfilment_line el ON el.id = ep.fulfilment_line_id
                                  WHERE el.fulfilment_id = f.id
                                  ORDER BY ep.observed_at DESC, ep.recorded_at DESC LIMIT 1),
                                EXISTS (SELECT 1 FROM external_pick ep
                                          JOIN fulfilment_line el ON el.id = ep.fulfilment_line_id
                                         WHERE el.fulfilment_id = f.id),
                                (now() AT TIME ZONE coalesce(st.timezone, 'UTC'))::date
                           FROM fulfilment f
                           JOIN \"order\" o ON o.id = f.order_id
                           LEFT JOIN site st ON st.id = f.site_id
                           LEFT JOIN party p ON p.id = o.customer_party_id
                           LEFT JOIN fulfilment_line fl ON fl.fulfilment_id = f.id
                           LEFT JOIN LATERAL (
                               SELECT coalesce(sum(v.effective_quantity), 0)::bigint AS q
                                 FROM stock_movement m
                                 JOIN stock_movement_effective v
                                   ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
                                WHERE m.fulfilment_line_id = fl.id
                                  AND m.to_package_id IS NOT NULL) bx ON true
                          WHERE f.state <> 'cancelled'
                            AND ($1::uuid IS NULL OR f.site_id = $1)
                            AND ($2::text IS NULL
                                 OR f.reference ILIKE $2
                                 OR o.confirmation_number ILIKE $2
                                 OR o.external_ref ILIKE $2
                                 OR p.name ILIKE $2)
                          GROUP BY f.id, f.reference, o.confirmation_number,
                                   o.external_ref, p.name, o.promised_to, f.source_channel_id,
                                   st.timezone
                          ORDER BY o.promised_to NULLS LAST, f.id",
                        &[&site, &like],
                    )
                    .await?;

                // **Today and the promise, both on the site's clock.** Read in UTC,
                // an Australian morning is still yesterday until ten or eleven, so
                // a promise for tomorrow read "due today" and one for today read
                // "overdue" for the first part of every shift. A fulfilment with no
                // site has no clock of its own, and reads in UTC as all did before.
                Ok(rows
                    .iter()
                    .map(|r| {
                        let committed: i64 = r.get(5);
                        let picked: i64 = r.get(6);
                        let promised: Option<NaiveDate> = r.get(7);
                        let today: NaiveDate = r.get(13);
                        let reference: Option<String> = r.get(1);
                        let provenance = r.get::<_, bool>(12).then(|| {
                            picked_elsewhere(r.get(10), reference.as_deref(), r.get(11))
                        });
                        PackJob {
                            fulfilment_id: r.get(0),
                            reference,
                            order_reference: r.get(2),
                            customer: r.get(3),
                            lines: r.get(4),
                            committed,
                            picked,
                            cartons: r.get(8),
                            reported: r.get(9),
                            provenance,
                            due: promised.map(|p| due(p, today)),
                            stage: stage(picked, committed),
                        }
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await
}

/// Where the picking happened, in a sentence (D114): the channel, the
/// document a person quotes, and who picked, when each is known.
pub fn picked_elsewhere(
    channel: Option<&str>,
    document: Option<&str>,
    picked_by: Option<&str>,
) -> String {
    let mut out = format!("Picked in {}", channel.filter(|c| !c.is_empty()).unwrap_or("another system"));
    if let Some(d) = document.filter(|d| !d.is_empty()) {
        out.push_str(" · ");
        out.push_str(d);
    }
    if let Some(p) = picked_by.filter(|p| !p.is_empty()) {
        out.push_str(" · by ");
        out.push_str(p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn where_the_picking_happened_reads_as_a_sentence() {
        assert_eq!(
            picked_elsewhere(Some("NetSuite"), Some("IF270947"), Some("Casual Melbourne")),
            "Picked in NetSuite · IF270947 · by Casual Melbourne"
        );
        assert_eq!(picked_elsewhere(Some("NetSuite"), None, None), "Picked in NetSuite");
        assert_eq!(picked_elsewhere(None, Some("IF1"), Some("")), "Picked in another system · IF1");
    }

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
