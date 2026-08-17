//! What to pick, where it is, and what it looks like.
//!
//! # A walk list, and deliberately not a write path
//!
//! `POST /picks` exists and takes `to_package_id`: a pick moves stock out of a
//! cell and into a carton. **Which carton is a workflow question this model does
//! not answer** — pick straight into the despatch carton, pick into a tote and
//! consolidate at the bench, or pick a whole wave into one cage — and each of
//! those is a different screen. Inventing one here would make a choice on the
//! floor's behalf and then be wrong on somebody's floor.
//!
//! So this reads. It says what is outstanding, which bin to walk to, in what
//! order to walk, and what the thing looks like when you get there. Recording
//! the pick stays where it is until the carton question has an owner, and that
//! is the trigger for the other half.
//!
//! # The order is the floor's, not the database's
//!
//! `location.pick_sequence` is the walking order somebody typed in, and J71
//! reports when two bins claim the same one. Ordering by it is what makes the
//! list a route rather than a set — and it is the reason that column, which
//! nothing had read until now, exists at all.
//!
//! # One cell per line, chosen rather than listed
//!
//! A line could be served from several cells and a picker wants one bin, not a
//! menu. The choice prefers a cell this line is already allocated from — D12
//! makes allocation advisory, so it is a preference and never a refusal — and
//! otherwise takes the earliest on the walk with something available. A line
//! with nowhere to go still appears, with no bin, because *nothing to pick this
//! from* is the finding a picker most needs to see and the one a filtered list
//! would hide.

use serde::Serialize;
use uuid::Uuid;

use actix_web::web;

use crate::auth::Caller;
use crate::error::ApiError;
use crate::pictures::{self, Picture};
use crate::tenancy::TenantScope;
use crate::AppState;

/// One thing to walk to.
#[derive(Serialize)]
pub struct PickLine {
    pub fulfilment_line_id: Uuid,
    pub item_id: Uuid,
    pub item_code: String,
    pub description: Option<String>,
    /// What the order calls itself, for the picker who is asked about it.
    pub reference: Option<String>,
    /// The cell to take from, which is what `POST /picks` will need.
    pub stock_id: Option<Uuid>,
    pub location_code: Option<String>,
    /// Where it sits on the walk. Null sorts last: a bin with no sequence is a
    /// bin nobody has placed on the route.
    pub pick_sequence: Option<i32>,
    pub lot_code: Option<String>,
    /// Still to pick on this line, in base units.
    pub remaining: i64,
    /// Already picked, folded. With `covered` this is what tells the screen how
    /// much of what it is about to pick still needs claiming.
    pub picked: i64,
    /// **How much of this line is spoken for**, over every covering state
    /// (`allocating::COVERING`, J31) and every cell — not just `cell`.
    ///
    /// `allocated` is a bool about *this bin*; it cannot answer "may I claim
    /// two more?", and `POST /allocations` refuses an over-claim with
    /// `OverCovers` rather than absorbing it. So a screen that recorded a pick
    /// by always claiming first would be refused on any line planning had
    /// already covered, and a screen that never claimed would drive
    /// `picked > covered` and raise J56 against a warehouse that did nothing
    /// wrong. Both were reachable with what this read returned.
    pub covered: i64,
    /// Free in the chosen cell. Less than `remaining` is a short pick coming.
    pub available: Option<i64>,
    /// **True when the cell was claimed for this line.** D12: an allocation is
    /// an intention and advisory, so a picker who finds the goods somewhere
    /// else is right and the screen must not imply otherwise.
    pub allocated: bool,
    /// What it looks like, and whose picture that is. D141.
    pub picture: Option<Picture>,
}

#[derive(Serialize)]
pub struct PickListScreen {
    pub site: String,
    pub lines: Vec<PickLine>,
}

pub async fn screen(
    state: &web::Data<AppState>,
    who: &Caller,
    site_id: Uuid,
    limit: i64,
) -> Result<PickListScreen, ApiError> {
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let site: String = tx
                    .query_opt("SELECT code FROM site WHERE id = $1", &[&site_id])
                    .await?
                    .map(|r| r.get(0))
                    .ok_or(ApiError::NotFound)?;

                let rows = tx.query(LINES.as_str(), &[&site_id, &limit]).await?;
                let lines = rows
                    .iter()
                    .map(|r| PickLine {
                        fulfilment_line_id: r.get(0),
                        item_id: r.get(1),
                        item_code: r.get(2),
                        description: r.get(3),
                        reference: r.get(4),
                        stock_id: r.get(5),
                        location_code: r.get(6),
                        pick_sequence: r.get(7),
                        lot_code: r.get(8),
                        remaining: r.get(9),
                        picked: r.get(10),
                        covered: r.get(11),
                        available: r.get(12),
                        allocated: r.get(13),
                        picture: pictures::from_row(r.get(14), r.get(15)),
                    })
                    .collect();

                Ok(PickListScreen { site, lines })
            })
        })
        .await
}

/// Outstanding lines, one cell each, in walking order.
///
/// The cell is chosen by `LATERAL … LIMIT 1` rather than by grouping, because
/// what a picker wants is one bin and the ordering that picks it is the
/// interesting part: claimed cells first, then the walk.
static LINES: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!(
        "WITH {picture}
     SELECT fl.id,
            ol.item_id,
            i.code,
            i.description,
            coalesce(f.reference, o.external_ref),
            cell.stock_id,
            cell.location_code,
            cell.pick_sequence,
            cell.lot_code,
            (fl.quantity - fl.picked_quantity)::bigint,
            fl.picked_quantity::bigint,
            coalesce((SELECT sum(sa.quantity)::bigint
                        FROM stock_allocation sa
                       WHERE sa.fulfilment_line_id = fl.id
                         AND sa.state IN ('allocated', 'picking', 'picked',
                                          'packed', 'fulfilled')), 0),
            cell.available,
            coalesce(cell.allocated, false),
            p.digest,
            p.source
       FROM fulfilment_line fl
       JOIN fulfilment f ON f.id = fl.fulfilment_id
       JOIN order_line ol ON ol.id = fl.order_line_id
       JOIN \"order\" o ON o.id = f.order_id
       JOIN item i ON i.id = ol.item_id
       LEFT JOIN picture p ON p.item_id = ol.item_id
       LEFT JOIN LATERAL (
            SELECT s.id AS stock_id,
                   l.code AS location_code,
                   l.pick_sequence,
                   lt.code AS lot_code,
                   s.available_quantity::bigint AS available,
                   EXISTS (SELECT 1 FROM stock_allocation sa
                            WHERE sa.stock_id = s.id
                              AND sa.fulfilment_line_id = fl.id
                              AND sa.state = 'allocated') AS allocated
              FROM stock s
              JOIN location l ON l.id = s.resolved_location_id
              LEFT JOIN lot lt ON lt.id = s.lot_id
             WHERE s.item_id = ol.item_id
               AND s.site_id = f.site_id
               AND s.available_quantity > 0
             ORDER BY EXISTS (SELECT 1 FROM stock_allocation sa
                               WHERE sa.stock_id = s.id
                                 AND sa.fulfilment_line_id = fl.id
                                 AND sa.state = 'allocated') DESC,
                      l.pick_sequence NULLS LAST,
                      s.id
             LIMIT 1
       ) cell ON true
      WHERE f.site_id = $1
        AND f.state <> 'cancelled'
        AND fl.picked_quantity < fl.quantity
      ORDER BY cell.pick_sequence NULLS LAST, i.code, fl.id
      LIMIT $2",
        picture = pictures::PICTURE_CTE
    )
});
