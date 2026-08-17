//! The pack bench, as data.
//!
//! These reads were written inside `web/` when the bench was a server-rendered
//! page and nothing else needed them. D113 gives the screen to React, and a
//! React screen needs the same figures over JSON — so the choice was to write
//! the queries a second time against the same tables, or to move them here and
//! have both renderers read one definition.
//!
//! **This codebase has a register for what the first option costs.** Every
//! drift it records has the same shape: two things stating one fact, free to
//! disagree. A packing list computing "what is left" one way while the screen
//! that filled the carton computed it another is that shape exactly, and it
//! would surface as a carton the page says is complete and the paperwork says
//! is short.
//!
//! So `web/` and `routes.rs` both call in here, and neither holds SQL of its
//! own for the bench. Nothing in this module writes.

use actix_web::web;
use serde::Serialize;
use uuid::Uuid;

use crate::auth::Caller;
use crate::baseline::{self, Grams};
use crate::error::ApiError;
use crate::tenancy::TenantScope;
use crate::AppState;

/// Everything the pack screen needs, in one round trip.
///
/// **One request rather than five.** D2's bar is *no page loads, sub-second*,
/// and a screen that opens by fetching a header, then lines, then cartons, then
/// contents, then presets has five chances to be slow and five orderings to get
/// wrong. The page is one thing; so is the read behind it.
#[derive(Serialize)]
pub struct BenchScreen {
    #[serde(flatten)]
    pub bench: Bench,
    pub cartons: Vec<CartonSummary>,
    pub presets: Vec<Preset>,
    /// The reason a correction carries when units went in the wrong box. Sent
    /// with the screen because the alternative is the client holding a code
    /// string and looking it up at the moment somebody is trying to undo
    /// something — see `take_out` in the client.
    pub wrong_box_reason_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct Bench {
    /// The item fulfilment number, which is what the job is called. Migration 71.
    pub reference: String,
    /// The order behind it, which is a different number and a different thing.
    pub order_reference: String,
    pub customer: String,
    pub site: String,
    /// Where a new carton comes into existence. D97: `created` asserts placement.
    pub dock_id: Option<Uuid>,
    pub lines: Vec<BenchLine>,
}

#[derive(Serialize)]
pub struct BenchLine {
    pub line_id: Uuid,
    pub item_code: String,
    pub description: Option<String>,
    pub remaining: i64,
    pub cells: Vec<Cell>,
}

/// A place the item actually is, with what is free to claim.
///
/// **The operator picks the cell because nothing else can yet.** Question 26 —
/// who allocates, and when — is deferred against building the allocator, so
/// `/allocations` is directed only and the screen has to offer the choice. When
/// 26 is answered this list becomes a default rather than a question.
#[derive(Serialize)]
pub struct Cell {
    pub stock_id: Uuid,
    pub location: String,
    pub lot: Option<String>,
    pub available: i64,
}

#[derive(Serialize)]
pub struct Preset {
    pub id: Uuid,
    pub name: String,
}

/// The preset's answer to how big a carton is, when it has one.
///
/// A named struct rather than the tuple this was: the tuple serialised as a
/// bare three-element array, which is a shape a client has to be told the
/// meaning of. Length and width before height matters, and nothing in
/// `[1165, 1165, 1840]` says so.
#[derive(Serialize, Clone, Copy)]
pub struct StatedSize {
    pub length_mm: i32,
    pub width_mm: i32,
    pub height_mm: i32,
}

#[derive(Serialize)]
pub struct CartonSummary {
    pub id: Uuid,
    pub sequence: String,
    pub package_type: Option<String>,
    pub sealed: bool,
    pub gross_weight_g: Option<i64>,
    pub height_mm: Option<i32>,
    /// A box states its size; a pallet's height is the stack and only the
    /// measurement knows it. Migration 67 is the column.
    pub stated_size: Option<StatedSize>,
    /// What the contents and the empty box have weighed before, when there is
    /// enough to say. `None` for an empty carton, and for one whose lines have
    /// never been on a scale.
    pub expected: Option<ExpectedWeight>,
    pub contents: Vec<PackedRow>,
}

/// What is physically in one carton, carried out of the read transaction so the
/// weights for every code on the screen can be fetched in one go afterwards.
struct Packed {
    carton: Uuid,
    tare: Grams,
    /// `(item, base units)`, one per content row.
    rows: Vec<(Uuid, i64)>,
}

/// What this carton should weigh, and how much that claim is worth.
///
/// **Every field beside `grams` is there to stop the figure being read as more
/// than it is.** `n` is the fewest weighings behind any line on the carton and
/// `borrowed` says whether any of them came from an item's style rather than
/// the code itself — [`crate::baseline`] argues both at length. A screen given
/// only the number would present a sum built on one weighing of a boot in
/// another size exactly as it presents one built on forty of this code.
#[derive(Serialize)]
pub struct ExpectedWeight {
    /// The figure and what it rests on, in the shape the dock reads too.
    #[serde(flatten)]
    pub baseline: crate::baseline::WeightBaseline,
    /// Weighed minus expected, and the same gap in parts per thousand. Present
    /// only once somebody has put this carton on a scale.
    pub delta_g: Option<i64>,
    pub delta_per_mille: Option<i32>,
}

#[derive(Serialize)]
pub struct PackedRow {
    pub item_code: String,
    pub description: Option<String>,
    pub lot_code: Option<String>,
    pub quantity: i64,
    /// The picks behind this row, newest first, with what is left of each after
    /// its own corrections. Taking units back out reverses these in order —
    /// a row is not one movement, and pretending it is would fail on the second
    /// pick of the same item into the same carton.
    pub picks: Vec<(Uuid, i64)>,
}

/// The whole screen, in one transaction per read.
pub async fn screen(
    state: &web::Data<AppState>,
    who: &Caller,
    fulfilment_id: Uuid,
) -> Result<BenchScreen, ApiError> {
    Ok(BenchScreen {
        bench: bench_view(state, who, fulfilment_id).await?,
        cartons: cartons_on(state, who, fulfilment_id).await?,
        presets: presets(state, who).await?,
        wrong_box_reason_id: reason_id(state, who, "wrong_location").await?,
    })
}

pub async fn bench_view(
    state: &web::Data<AppState>,
    who: &Caller,
    fulfilment_id: Uuid,
) -> Result<Bench, ApiError> {
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let head = tx
                    .query_opt(
                        "SELECT coalesce(f.reference, o.confirmation_number,
                                         o.external_ref, '—'),
                                coalesce(p.name, 'no customer named'),
                                coalesce(s.code, 'no site'), f.site_id,
                                coalesce(o.confirmation_number, o.external_ref, '—')
                           FROM fulfilment f
                           JOIN \"order\" o ON o.id = f.order_id
                           LEFT JOIN party p ON p.id = o.customer_party_id
                           LEFT JOIN site s ON s.id = f.site_id
                          WHERE f.id = $1",
                        &[&fulfilment_id],
                    )
                    .await?
                    .ok_or(ApiError::NotFound)?;
                let site_id: Option<Uuid> = head.get(3);

                // A dock to bring cartons into being at. Any location at the
                // site will do for the demo; D97 only requires that `created`
                // says where.
                let dock_id: Option<Uuid> = tx
                    .query_opt(
                        "SELECT id FROM location WHERE site_id = $1 ORDER BY code LIMIT 1",
                        &[&site_id],
                    )
                    .await?
                    .map(|r| r.get(0));

                let lines = tx
                    .query(
                        "SELECT fl.id, i.code, i.description, ol.item_id,
                                fl.quantity - fl.picked_quantity
                           FROM fulfilment_line fl
                           JOIN order_line ol ON ol.id = fl.order_line_id
                           JOIN item i ON i.id = ol.item_id
                          WHERE fl.fulfilment_id = $1
                          ORDER BY i.code",
                        &[&fulfilment_id],
                    )
                    .await?;

                let mut out = vec![];
                for l in &lines {
                    let item_id: Uuid = l.get(3);
                    // Location-held only: a cell already inside a carton is not
                    // somewhere to pick from.
                    let cells = tx
                        .query(
                            "SELECT st.id, loc.code, lot.code, st.available_quantity
                               FROM stock st
                               JOIN location loc ON loc.id = st.holder_location_id
                               LEFT JOIN lot ON lot.id = st.lot_id
                              WHERE st.item_id = $1
                                AND st.holder_location_id IS NOT NULL
                                AND st.available_quantity > 0
                                AND ($2::uuid IS NULL OR st.site_id = $2)
                              ORDER BY st.available_quantity DESC
                              LIMIT 8",
                            &[&item_id, &site_id],
                        )
                        .await?;
                    out.push(BenchLine {
                        line_id: l.get(0),
                        item_code: l.get(1),
                        description: l.get(2),
                        remaining: l.get(4),
                        cells: cells
                            .iter()
                            .map(|c| Cell {
                                stock_id: c.get(0),
                                location: c.get(1),
                                lot: c.get(2),
                                available: c.get(3),
                            })
                            .collect(),
                    });
                }

                Ok(Bench {
                    reference: head.get(0),
                    order_reference: head.get(4),
                    customer: head.get(1),
                    site: head.get(2),
                    dock_id,
                    lines: out,
                })
            })
        })
        .await
}

/// An `adjustment_reason` by code, for the acts that require one.
pub async fn reason_id(
    state: &web::Data<AppState>,
    who: &Caller,
    code: &str,
) -> Result<Option<Uuid>, ApiError> {
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let code = code.to_string();
    scope
        .run(move |tx| {
            Box::pin(async move {
                Ok(tx
                    .query_opt(
                        "SELECT id FROM adjustment_reason WHERE code = $1 ORDER BY tenant_id
                          NULLS LAST LIMIT 1",
                        &[&code],
                    )
                    .await?
                    .map(|r| r.get(0)))
            })
        })
        .await
}

pub async fn presets(state: &web::Data<AppState>, who: &Caller) -> Result<Vec<Preset>, ApiError> {
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        "SELECT id, name FROM package_type
                          WHERE effective_from <= CURRENT_DATE
                          ORDER BY tenant_id IS NULL, name",
                        &[],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| Preset {
                        id: r.get(0),
                        name: r.get(1),
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await
}

/// The cartons on a fulfilment, each with what it should weigh.
///
/// # Two reads, because the second one is somebody else's question
///
/// The baselines come from [`crate::baseline::for_items`] after this
/// transaction closes rather than from SQL restated in it. `screen` already
/// opens a scope per read for exactly this reason — the alternative is a second
/// copy of the eligibility rules living here, and the whole argument of this
/// module's header is that two places stating one fact is how they come to
/// disagree.
pub async fn cartons_on(
    state: &web::Data<AppState>,
    who: &Caller,
    fulfilment_id: Uuid,
) -> Result<Vec<CartonSummary>, ApiError> {
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let (mut out, packed) = scope
        .run(move |tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        // **The winning event, not `package.status`.** That
                        // column is a fold and lags the act that just happened,
                        // so a carton sealed or voided a second ago still reads
                        // by its previous state — which put a voided carton back
                        // on the bench and would have shown a sealed one as open.
                        // The log is the answer now; the projection catches up.
                        "SELECT p.id, coalesce(p.sequence::text, '—'), pt.name,
                                w.kind = 'sealed', p.gross_weight_g, p.height_mm,
                                pt.dimensions_fixed, pt.length_mm, pt.width_mm,
                                pt.height_mm, pt.tare_weight_g
                           FROM package p
                           LEFT JOIN package_type pt ON pt.id = p.package_type_id
                           LEFT JOIN LATERAL (
                               SELECT e.kind FROM package_event e
                                WHERE e.package_id = p.id
                                  AND e.kind IN ('created','placed','contained','sealed',
                                                 'opened','despatched','voided')
                                ORDER BY e.occurred_at DESC, e.recorded_at DESC, e.id DESC
                                LIMIT 1) w ON true
                          WHERE p.fulfilment_id = $1
                            AND w.kind IS DISTINCT FROM 'voided'
                          ORDER BY p.sequence NULLS LAST, p.id",
                        &[&fulfilment_id],
                    )
                    .await?;
                let mut out = vec![];
                // What is in each carton, in base units, kept beside the
                // summaries so the weights can be looked up in one go once the
                // transaction is done with.
                let mut packed: Vec<Packed> = vec![];
                for r in &rows {
                    let id: Uuid = r.get(0);
                    // Netted through `stock_movement_effective`, like the
                    // packing list: a reversed pick must stop reading as packed.
                    let contents = tx
                        .query(
                            "SELECT i.code, i.description, l.code,
                                    sum(e.effective_quantity)::bigint,
                                    m.item_id,
                                    array_agg(m.id ORDER BY m.recorded_at DESC),
                                    array_agg(e.effective_quantity ORDER BY m.recorded_at DESC)
                               FROM stock_movement m
                               JOIN stock_movement_effective e ON e.movement_id = m.id
                               JOIN item i ON i.id = m.item_id
                               LEFT JOIN lot l ON l.id = m.to_lot_id
                              WHERE m.to_package_id = $1
                                AND m.fulfilment_line_id IS NOT NULL
                              -- Still grouped by the fulfilment line as well as
                              -- the item: two lines of one code packed into one
                              -- carton are two rows, and were before `item_id`
                              -- joined the select list.
                              GROUP BY i.code, i.description, l.code,
                                       m.fulfilment_line_id, m.item_id
                             HAVING sum(e.effective_quantity) <> 0
                              ORDER BY i.code",
                            &[&id],
                        )
                        .await?;
                    packed.push(Packed {
                        carton: id,
                        // A preset with no stated tare weighs nothing, which is
                        // wrong but is the only figure available; the box is
                        // then missing from the expectation rather than the
                        // expectation from the screen. Worth knowing when the
                        // bench reads light by a kilo.
                        tare: r.get::<_, Option<Grams>>(10).unwrap_or(0),
                        rows: contents
                            .iter()
                            .map(|c| (c.get::<_, Uuid>(4), c.get::<_, i64>(3)))
                            .collect(),
                    });
                    out.push(CartonSummary {
                        id,
                        sequence: r.get(1),
                        package_type: r.get(2),
                        sealed: r.get::<_, Option<bool>>(3).unwrap_or(false),
                        gross_weight_g: r.get(4),
                        height_mm: r.get(5),
                        // Only when the preset claims a fixed size *and* states
                        // all three. Migration 67's constraint makes the second
                        // follow from the first, and reading it that way means a
                        // preset that ever gets one without the other shows
                        // nothing rather than a partial size.
                        stated_size: match (
                            r.get::<_, Option<bool>>(6),
                            r.get::<_, Option<i32>>(7),
                            r.get::<_, Option<i32>>(8),
                            r.get::<_, Option<i32>>(9),
                        ) {
                            (Some(true), Some(l), Some(w), Some(h)) => Some(StatedSize {
                                length_mm: l,
                                width_mm: w,
                                height_mm: h,
                            }),
                            _ => None,
                        },
                        // Filled in below, once every item on the screen can be
                        // asked about together.
                        expected: None,
                        contents: contents
                            .iter()
                            .map(|c| {
                                let ids: Vec<Uuid> = c.get(5);
                                let left: Vec<i64> = c.get(6);
                                PackedRow {
                                    item_code: c.get(0),
                                    description: c.get(1),
                                    lot_code: c.get(2),
                                    quantity: c.get(3),
                                    // Only what still has something to take back.
                                    picks: ids
                                        .into_iter()
                                        .zip(left)
                                        .filter(|(_, q)| *q > 0)
                                        .collect(),
                                }
                            })
                            .collect(),
                    });
                }
                Ok((out, packed))
            })
        })
        .await?;

    // **One lookup for the screen, not one per carton.** D10's batch-loading
    // argument again: a bench with eight cartons of the same four codes would
    // otherwise ask the same question eight times.
    //
    // `each`, and no case pack: `stock_movement.quantity` is base units by
    // migration 46, `packing_factor` puts `each` at one base unit, and
    // `observable_item_config_ck` says an each carries no config.
    let items: Vec<(Uuid, Option<Uuid>)> = packed
        .iter()
        .flat_map(|p| p.rows.iter().map(|(item, _)| (*item, None)))
        .collect();
    let weights = baseline::for_items(state, who, &items, "each").await?;

    for Packed { carton, tare, rows } in &packed {
        // A line whose item has never been weighed is not a line worth nothing —
        // it is a carton whose weight cannot be worked out at all, so the figure
        // is withheld rather than computed from the lines that happen to have one.
        let Some(lines) = rows
            .iter()
            .map(|(item, quantity)| {
                weights.get(item).map(|each| baseline::Line {
                    quantity: *quantity,
                    each: *each,
                })
            })
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let Some(e) = baseline::expected(&lines, *tare) else {
            continue;
        };
        if let Some(c) = out.iter_mut().find(|c| c.id == *carton) {
            let d = c.gross_weight_g.map(|w| baseline::delta(w, e.grams));
            c.expected = Some(ExpectedWeight {
                baseline: e.into(),
                delta_g: d.map(|d| d.grams),
                delta_per_mille: d.and_then(|d| d.per_mille),
            });
        }
    }
    Ok(out)
}
