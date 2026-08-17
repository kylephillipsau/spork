//! What has been received and has nowhere to live yet.
//!
//! # The step the floor actually takes
//!
//! `POST /receipts` takes `to_location_id`, documented as "dock **or** bin", so
//! receiving straight to the shelf has always been possible and put-away only
//! exists as a separate step on a floor that does not work that way. This one
//! does: goods are checked in at the dock and somebody puts them away
//! afterwards, often later and often a different person. That answer is what
//! makes this read worth writing.
//!
//! # What counts as waiting, and why it is a kind rather than a reason
//!
//! **A cell held directly by a `dock` location.** Not *"whose last movement was
//! a receipt"*, which was the other candidate and is narrower than the truth: an
//! adjustment that lands at the dock, or a return, also needs a home, and the
//! ledger-based rule would miss both while the kind-based one catches them.
//!
//! **Package-held cells are excluded**, and that is the same line
//! [`crate::moving`] already draws from the other side: it refuses
//! `FromPackageHeld` because a container is relocated by a `package_event`, not
//! by a stock movement. The fixture makes the case concrete — `DOCK-1` holds
//! three cells and one of them is inside `CARTON-D`, an outbound carton that has
//! already been despatched. It is not put-away work and this rule does not
//! offer it.
//!
//! An inbound *pallet* would be a package too, and D6 promised `package` would
//! serve putaway LPNs. It is not a gap being declined here: a receipt lands
//! loose stock at a location and has no package arm at all, so a received pallet
//! is unreachable today. When receiving grows one, this read grows a second
//! shape and the placement goes through `POST /packages/{id}/place`.
//!
//! # Where it should go is not answered here
//!
//! `docs/inbound-analysis.md` sketches the directed version: `putaway_policy`,
//! a `location_occupancy` projection, six more columns on `location`. The
//! competitor analysis warns about exactly that accretion in its own words —
//! *"declining the engine while accepting five small rule tables is how you get
//! a rules engine you never designed"* — and the location survey those scores
//! would read has not happened. So the operator names the bin, by scanning it.
//!
//! What this *does* carry is `homes`: the storage bins already holding this
//! item. That is information rather than instruction, the same species as the
//! stock-on-hand figure on the capture walk, and it turns a blind choice into an
//! informed one with one query and no policy table. A scored suggestion can be
//! added later without changing a byte of what the ledger records, which is what
//! makes declining it now cost nothing.

use serde::Serialize;
use uuid::Uuid;

use actix_web::web;

use crate::auth::Caller;
use crate::error::ApiError;
use crate::pictures::{self, Picture};
use crate::tenancy::TenantScope;
use crate::AppState;

/// A storage bin already holding this item.
#[derive(Serialize)]
pub struct Home {
    pub location_id: Uuid,
    pub location_code: String,
    /// How much of this item is already there.
    pub quantity: i64,
    /// Where it sits on the walk, so the nearest home is offered first.
    pub pick_sequence: Option<i32>,
}

/// One thing on the dock with nowhere to live.
#[derive(Serialize)]
pub struct PutawayCell {
    /// The cell to move from, which is what `POST /moves` needs.
    pub stock_id: Uuid,
    pub item_id: Uuid,
    pub item_code: String,
    pub description: Option<String>,
    /// Where it is now. A dock, by definition of this list.
    pub location_code: String,
    pub lot_code: Option<String>,
    /// What is in the cell.
    pub quantity: i64,
    /// What is free to move. Below `quantity` means some of it is claimed —
    /// which does not stop a put-away and is worth saying before it happens.
    pub available: i64,
    /// Bins that already hold this item, nearest first. Never a direction.
    pub homes: Vec<Home>,
    /// What it looks like. D141, and the same argument as the pick walk: the
    /// person who does not know the catalogue is the person hired this week.
    pub picture: Option<Picture>,
}

#[derive(Serialize)]
pub struct PutawayScreen {
    pub site: String,
    pub cells: Vec<PutawayCell>,
}

pub async fn screen(
    state: &web::Data<AppState>,
    who: &Caller,
    site_id: Uuid,
    limit: i64,
) -> Result<PutawayScreen, ApiError> {
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let site: String = tx
                    .query_opt("SELECT code FROM site WHERE id = $1", &[&site_id])
                    .await?
                    .map(|r| r.get(0))
                    .ok_or(ApiError::NotFound)?;

                let rows = tx.query(CELLS.as_str(), &[&site_id, &limit]).await?;
                let mut cells: Vec<PutawayCell> = Vec::with_capacity(rows.len());
                for r in &rows {
                    let stock_id: Uuid = r.get(0);
                    let item_id: Uuid = r.get(1);
                    let homes = tx
                        .query(HOMES, &[&item_id, &site_id])
                        .await?
                        .iter()
                        .map(|h| Home {
                            location_id: h.get(0),
                            location_code: h.get(1),
                            quantity: h.get(2),
                            pick_sequence: h.get(3),
                        })
                        .collect();
                    cells.push(PutawayCell {
                        stock_id,
                        item_id,
                        item_code: r.get(2),
                        description: r.get(3),
                        location_code: r.get(4),
                        lot_code: r.get(5),
                        quantity: r.get(6),
                        available: r.get(7),
                        homes,
                        picture: pictures::from_row(r.get(8), r.get(9)),
                    });
                }

                Ok(PutawayScreen { site, cells })
            })
        })
        .await
}

/// **A query per cell, and the list is short by construction.**
///
/// `homes` could be one lateral join, and at the sizes this read sees — what is
/// on a dock, not what is in a warehouse — the join would be harder to read for
/// no measurable gain. If a dock ever holds enough for this to matter, the fix
/// is one `LEFT JOIN LATERAL` and this comment is the note saying so.
const HOMES: &str = "SELECT l.id, l.code, s.quantity::bigint, l.pick_sequence
                       FROM stock s
                       JOIN location l ON l.id = s.resolved_location_id
                      WHERE s.item_id = $1
                        AND s.site_id = $2
                        AND s.quantity > 0
                        AND s.holder_package_id IS NULL
                        AND l.kind IN ('pick_face', 'bulk', 'overflow')
                      ORDER BY l.pick_sequence NULLS LAST, l.code
                      LIMIT 4";

static CELLS: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!(
        "WITH {picture}
     SELECT s.id,
            s.item_id,
            i.code,
            i.description,
            l.code,
            lt.code,
            s.quantity::bigint,
            s.available_quantity::bigint,
            p.digest,
            p.source
       FROM stock s
       JOIN location l ON l.id = s.resolved_location_id
       JOIN item i ON i.id = s.item_id
       LEFT JOIN lot lt ON lt.id = s.lot_id
       LEFT JOIN picture p ON p.item_id = s.item_id
      WHERE s.site_id = $1
        AND s.quantity > 0
        -- The rule, twice stated: a container moves as a container.
        AND s.holder_package_id IS NULL
        AND l.kind = 'dock'
      ORDER BY l.code, i.code, s.id
      LIMIT $2",
        picture = pictures::PICTURE_CTE
    )
});
