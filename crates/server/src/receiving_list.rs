//! What is expected here and has not all arrived.
//!
//! # The read the write path has been waiting for
//!
//! `POST /receipts` has existed since migration 21 and nothing has ever called
//! it from a screen, because nothing said what to call it about. It takes an
//! `expected_supply_id` and there was no way to find one.
//!
//! # Why `requires_lot` is on the wire
//!
//! [`crate::receiving::disposition`] **refuses** a line when the policy requires
//! a lot and none is given — not accepts-with-a-finding, refuses, because
//! accepting it puts stock on the floor that cannot be recalled by lot. So a
//! screen that did not know whether a lot was needed would discover it from a
//! refusal at the dock, with the pallet already broken down.
//!
//! It is resolved per line rather than per site, because the policy resolver
//! keys on item and supplier and both vary down a truck. That is one resolve per
//! outstanding line — a handful, on a list of what is expected today.
//!
//! # And why `levels` is
//!
//! D92 and Q173: the receiver counts what is in front of them, which is cartons.
//! The conversion needs an `item_packing_config`, `POST /receipts` refuses a
//! non-`each` level without one (J57), and the screen cannot offer "6 cartons"
//! unless it knows a carton is a thing this item comes in. So the read says
//! which levels are answerable, and says nothing at all where only `each` is.
//!
//! # What this list cannot show
//!
//! **A delivery nobody promised.** `expected_supply_id` is required by the write
//! path, so goods arriving against no purchase order have no path through this
//! screen or any other. That is a real gap on a real dock and it is named in
//! D169 rather than papered over: it needs an ad-hoc supply write, and that
//! circles question 26 — who the goods are for — which has not been answered.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use actix_web::web;

use std::collections::BTreeMap;

use crate::auth::Caller;
use crate::baseline;
use crate::error::ApiError;
use crate::pictures::{self, Picture};
use crate::receiving;
use crate::tenancy::TenantScope;
use crate::AppState;

/// A packaging level this item can be counted in, and what it converts by.
#[derive(Serialize)]
pub struct PackLevel {
    /// `each` | `inner` | `carton` | `layer` | `pallet`.
    pub level: String,
    /// Base units per one of these. What the screen multiplies to show a total
    /// before the press — the server does the real conversion.
    pub units: i64,
    /// The config the conversion came from. `POST /receipts` needs it by id for
    /// any level above `each` (J57).
    pub item_packing_config_id: Option<Uuid>,
    /// What one of these has weighed before, when anybody has weighed one.
    ///
    /// **Per level, because that is what the receiver is counting in.** A
    /// baseline on the line would have to pick a level and the screen would
    /// then divide a dock weight by the wrong one. Null is the ordinary case
    /// today: `revalidation` records that 115 of 116 weights on file are
    /// `transcribed`, and this reads only what came off an instrument.
    pub baseline: Option<crate::baseline::WeightBaseline>,
}

/// A party that already owns this item here.
#[derive(Serialize)]
pub struct Owner {
    pub owner_id: Uuid,
    pub name: String,
}

/// One promise with something still to come.
#[derive(Serialize)]
pub struct ExpectedLine {
    pub expected_supply_id: Uuid,
    pub item_id: Uuid,
    pub item_code: String,
    pub description: Option<String>,
    pub order_number: Option<String>,
    pub supplier: Option<String>,
    /// The near edge of the expected window. Null when nobody said.
    pub expected_from: Option<DateTime<Utc>>,
    pub expected: i64,
    pub received: i64,
    pub outstanding: i64,
    /// **The policy refuses a line without a lot when this is true**, so the
    /// screen asks for one rather than finding out at the dock.
    pub requires_lot: bool,
    /// Levels this item can be counted in, `each` always first.
    pub levels: Vec<PackLevel>,
    /// **Whose the goods are, when the promise says.** Null is common: the
    /// fixture's own promise names nobody, and `POST /receipts` then refuses the
    /// line until somebody does.
    pub owner_id: Option<Uuid>,
    /// Parties already owning this item at this site — offered when the promise
    /// names none, the same species as put-away's `homes`. A fact rather than a
    /// default: there is no marked convention for who owns received stock, and
    /// inventing one is how `bench.rs` ended up choosing a dock with *"any
    /// location at the site will do for the demo"*.
    pub owners: Vec<Owner>,
    pub picture: Option<Picture>,
}

#[derive(Serialize)]
pub struct ReceivingScreen {
    pub site: String,
    pub lines: Vec<ExpectedLine>,
}

pub async fn screen(
    state: &web::Data<AppState>,
    who: &Caller,
    site_id: Uuid,
    limit: i64,
) -> Result<ReceivingScreen, ApiError> {
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let tenant = who.tenant_id;
    let mut screen = scope
        .run(move |tx| {
            Box::pin(async move {
                let site: String = tx
                    .query_opt("SELECT code FROM site WHERE id = $1", &[&site_id])
                    .await?
                    .map(|r| r.get(0))
                    .ok_or(ApiError::NotFound)?;

                let rows = tx.query(LINES.as_str(), &[&site_id, &limit]).await?;
                let now = Utc::now();
                let mut lines = Vec::with_capacity(rows.len());
                for r in &rows {
                    let item_id: Uuid = r.get(1);
                    let supplier_id: Option<Uuid> = r.get(10);

                    // Per line, because the resolver keys on item and supplier
                    // and a truck carries several of both.
                    let resolved = receiving::resolve_receiving_policy(
                        tx, tenant, item_id, supplier_id, Some(site_id), now,
                    )
                    .await?;

                    let configs = tx.query(LEVELS, &[&item_id]).await?;
                    let owners = tx
                        .query(OWNERS, &[&item_id, &site_id])
                        .await?
                        .iter()
                        .map(|o| Owner { owner_id: o.get(0), name: o.get(1) })
                        .collect();
                    lines.push(ExpectedLine {
                        expected_supply_id: r.get(0),
                        item_id,
                        item_code: r.get(2),
                        description: r.get(3),
                        order_number: r.get(4),
                        supplier: r.get(5),
                        expected_from: r.get(6),
                        expected: r.get(7),
                        received: r.get(8),
                        outstanding: r.get(9),
                        requires_lot: resolved.policy.require_lot,
                        levels: levels_of(&configs),
                        owner_id: r.get(11),
                        owners,
                        picture: pictures::from_row(r.get(12), r.get(13)),
                    });
                }

                Ok(ReceivingScreen { site, lines })
            })
        })
        .await?;

    // **One lookup per level, not one per line.** The levels a screen offers
    // are drawn from a set of five, so this is at most five queries however
    // many lines the truck has — and in practice two, because a config that
    // stops at cartons offers `each` and `carton` and nothing else.
    //
    // After the read transaction rather than inside it, for the reason
    // `bench::cartons_on` gives at length: the eligibility rules live in
    // `baseline` and restating them here would be a second copy of what counts
    // as a weighing.
    let mut levels: BTreeMap<String, Vec<(Uuid, Option<Uuid>)>> = BTreeMap::new();
    for line in &screen.lines {
        for level in &line.levels {
            levels
                .entry(level.level.clone())
                .or_default()
                .push((line.item_id, level.item_packing_config_id));
        }
    }
    for (level, subjects) in levels {
        let found = baseline::for_items(state, who, &subjects, &level).await?;
        for line in &mut screen.lines {
            for l in &mut line.levels {
                if l.level == level {
                    l.baseline = found.get(&line.item_id).map(|b| (*b).into());
                }
            }
        }
    }
    Ok(screen)
}

/// Turn one config's rungs into the levels a receiver can count in.
///
/// **A rung with no number is not offered.** `packing_factor` is documented
/// against inventing one — *"a made-up 1 there would be a wrong answer rather
/// than a missing one"* — so a config that says nothing about layers means the
/// screen does not offer layers, and the receiver counts in something they can
/// actually convert.
fn levels_of(rows: &[tokio_postgres::Row]) -> Vec<PackLevel> {
    // Always answerable, and needs no config: an each is an each.
    let mut out = vec![PackLevel {
        level: "each".into(),
        units: 1,
        item_packing_config_id: None,
        baseline: None,
    }];
    let Some(row) = rows.first() else { return out };
    let id: Uuid = row.get(0);
    let units_per_inner: Option<i32> = row.get(1);
    let inners_per_carton: Option<i32> = row.get(2);
    let cartons_per_layer: Option<i32> = row.get(3);
    let layers_per_pallet: Option<i32> = row.get(4);

    let mut running: Option<i64> = None;
    for (level, rung) in [
        ("inner", units_per_inner),
        ("carton", inners_per_carton),
        ("layer", cartons_per_layer),
        ("pallet", layers_per_pallet),
    ] {
        // The chain stops at the first missing rung: a pallet of unknown layers
        // is unknown however many cartons are in a layer.
        let Some(rung) = rung else { break };
        let units = running.unwrap_or(1) * i64::from(rung);
        running = Some(units);
        out.push(PackLevel {
            level: level.into(),
            units,
            item_packing_config_id: Some(id),
            // Filled in by `screen`, once every line on the truck can be asked
            // about together.
            baseline: None,
        });
    }
    out
}

const OWNERS: &str = "SELECT DISTINCT s.owner_id, party.name
                        FROM stock s
                        JOIN party ON party.id = s.owner_id
                       WHERE s.item_id = $1 AND s.site_id = $2 AND s.quantity > 0
                       ORDER BY party.name
                       LIMIT 4";

const LEVELS: &str = "SELECT id, units_per_inner, inners_per_carton,
                             cartons_per_layer, layers_per_pallet
                        FROM item_packing_config
                       WHERE item_id = $1
                         AND effective_from <= current_date
                       ORDER BY effective_from DESC, id DESC
                       LIMIT 1";

static LINES: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!(
        "WITH {picture}
     SELECT es.id,
            es.item_id,
            i.code,
            i.description,
            po.order_number,
            party.name,
            es.expected_from,
            es.quantity_expected::bigint,
            es.quantity_received::bigint,
            es.quantity_outstanding::bigint,
            po.supplier_party_id,
            es.owner_id,
            p.digest,
            p.source
       FROM expected_supply es
       JOIN item i ON i.id = es.item_id
       LEFT JOIN purchase_order_line pol ON pol.id = es.purchase_order_line_id
       LEFT JOIN purchase_order po ON po.id = pol.purchase_order_id
       LEFT JOIN party ON party.id = po.supplier_party_id
       LEFT JOIN picture p ON p.item_id = es.item_id
      WHERE es.site_id = $1
        AND es.closed_at IS NULL
        AND es.quantity_outstanding > 0
      -- Soonest first, and a promise with no date last: a date nobody stated is
      -- not a date in the distant future.
      ORDER BY es.expected_from NULLS LAST, po.order_number, i.code
      LIMIT $2",
        picture = pictures::PICTURE_CTE
    )
});
