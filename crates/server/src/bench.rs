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

use std::collections::{BTreeMap, HashMap};

use actix_web::web;
use serde::Serialize;
use uuid::Uuid;

use crate::auth::Caller;
use crate::baseline::{self, Grams};
use crate::error::ApiError;
use crate::pictures::{self, Picture};
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
    /// The site's pack location (migration 97), absent until the site says.
    pub dock_id: Option<Uuid>,
    /// Where goods picked elsewhere are put down before they are boxed (D172).
    /// The same pack location: the bench is where they are put down.
    pub staging_id: Option<Uuid>,
    /// What this site has not set up that packing here needs, said as a
    /// sentence, or absent when nothing is missing. A bench that cannot start a
    /// carton says why before somebody tries.
    pub unready: Option<String>,
    pub lines: Vec<BenchLine>,
}

#[derive(Serialize)]
pub struct BenchLine {
    pub line_id: Uuid,
    /// So a screen can open the item's properties from the line.
    pub item_id: Uuid,
    pub item_code: String,
    pub description: Option<String>,
    /// Still to do at the bench: committed less the larger of what this system
    /// picked and what has gone into a carton (D172).
    pub remaining: i64,
    /// What the line commits, all of it: what the whole order view counts
    /// every unit against, packed or not (D202).
    pub committed: i64,
    pub cells: Vec<Cell>,
    /// Present when another system says this line was picked there (D172).
    pub elsewhere: Option<PickedElsewhere>,
    /// Present when a carton of this item has a known count, so whole cartons
    /// of it can ship as they are (migration 98).
    pub own_carton: Option<OwnCarton>,
    /// What to look for (D141): its box drawn, or its front, its own before
    /// its family's, and saying whose.
    pub picture: Option<Picture>,
    /// One of it at each level it can leave at, for the suggested arrangement
    /// (D195): an each; an inner pack and a carton when the case pack says
    /// how many are in one. Whether each ships as it is (D196) says whether it
    /// goes into a box at all.
    pub packs: Vec<PackUnit>,
    /// The kit it is a part of, when it is one (D223). The kit's own line is
    /// no work and isn't a line here; its parts say what was ordered.
    pub kit: Option<KitOf>,
}

/// The kit a line is a part of, as the order has it (D223).
#[derive(Serialize)]
pub struct KitOf {
    pub item_code: String,
    pub description: Option<String>,
    /// How many of the kit were ordered.
    pub ordered: i64,
}

/// One of an item at a packaging level, as a suggested arrangement places it
/// (D195): what it measures, what it weighs, and its sides to draw it with.
///
/// **What is recorded, and nothing guessed.** An each with no size is not
/// given its carton's size divided by the count: it is left out of the
/// arrangement and listed, so the packer can measure it there and then.
#[derive(Serialize)]
pub struct PackUnit {
    /// `each`, `inner` or `carton`.
    pub level: String,
    /// **Of what NetSuite counts, how many are in one of it** (D218): one of
    /// the item's unit is one, its carton so many of them. A level below the
    /// unit is never one of these: nobody orders one glove from a box.
    pub units: i64,
    /// It goes to the carrier as it is rather than into a box (D196), said or
    /// by default: a carton does, an each or an inner pack does not.
    pub ships_as_is: bool,
    /// It stays the way up it stands (D200): the arrangement turns it round,
    /// never onto its side.
    pub upright: bool,
    /// All three lengths, or nothing.
    pub size: Option<StatedSize>,
    /// Somebody said it has no size to measure (D138): a soft thing that goes
    /// in round the rest rather than taking a place of its own.
    pub no_size: bool,
    pub gross_weight_g: Option<i64>,
    /// `own`, `style` or `mixed` (D108): a family's figure says it is one.
    pub source: String,
    pub style_code: Option<String>,
    /// Its sides cut from photographs (D176), by face, to draw it with.
    pub faces: BTreeMap<String, String>,
}

/// The product's own carton, as its case pack and its carton's measurements say.
///
/// **Offered only when the count is known.** A carton nobody has said the
/// contents of cannot be filled to a known number, and filling it to a guess is
/// the case pack the prepack loader refused to invent.
#[derive(Serialize)]
pub struct OwnCarton {
    pub item_packing_config_id: Uuid,
    /// Units one carton holds: units per inner times inners per carton.
    pub units: i64,
    /// What a carton of it measures, when that is recorded.
    pub size: Option<StatedSize>,
    /// What a carton of it weighs by the record. **A listed figure, not a
    /// weighing of any carton on this bench**, which is why it is not
    /// `expected`: that is built from weighings and says how many.
    pub listed_weight_g: Option<i64>,
    /// How the listed figures were come by, `transcribed` from a prepack list
    /// for most, and whether they are this code's or its family's (D108).
    pub method: Option<String>,
    pub source: Option<String>,
    pub style_code: Option<String>,
}

/// A line picked elsewhere: what is reported, and what has been handed over.
#[derive(Serialize)]
pub struct PickedElsewhere {
    /// What the other system reports picked, now: each external line's newest.
    pub reported: i64,
    /// Handed over against it so far, net of corrections.
    pub handed: i64,
    /// The document a person quotes: `IF270947`.
    pub document: String,
    /// Who picked, in the other system's words.
    pub picked_by: Option<String>,
    /// Where it was picked, as a sentence (D114): "Picked in NetSuite ·
    /// IF270947 · by Casual Melbourne".
    pub provenance: String,
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
    /// The box's inside, when it claims a fixed size and states all three: what
    /// a suggested arrangement fits goods into (D195). A pallet or a skid has
    /// none here, fixed or not: it carries cartons rather than holding goods,
    /// and fitted against, the biggest "box" would always be a pallet.
    pub size: Option<StatedSize>,
    /// The suggestion may choose it (D196). A shovel box is the smallest box
    /// three rolls fit in, and nobody sends rolls in one.
    pub suggested: bool,
    /// The most the goods in it may weigh, when the workspace says (D199).
    pub max_payload_g: Option<i64>,
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
    /// The item, when this is one of it as it is rather than a box type
    /// (migration 98, D196).
    pub own_carton_of: Option<String>,
    /// Which of it: `carton`, `inner` or `each` (D196).
    pub own_level: Option<String>,
    /// A product's own carton's weight by the record, when there is one. Listed,
    /// never weighed here: see [`OwnCarton::listed_weight_g`].
    pub listed_weight_g: Option<i64>,
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
    pub item_id: Uuid,
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

                // **Where the site says it packs, and nowhere else** (migration
                // 97). The first version took the first location by code for
                // the carton and the first `staging` one for the goods, which
                // on a real bin list are a rack bin and a rack's top level.
                let (pack_at, owned): (Option<Uuid>, bool) = match tx
                    .query_opt(
                        "SELECT pack_location_id, owner_party_id IS NOT NULL
                           FROM site WHERE id = $1",
                        &[&site_id],
                    )
                    .await?
                {
                    Some(r) => (r.get(0), r.get(1)),
                    None => (None, false),
                };
                let dock_id = pack_at;
                let staging_id = pack_at;
                let site_code: String = head.get(2);
                let unready = match (pack_at.is_some(), owned) {
                    (true, true) => None,
                    (false, true) => Some(format!(
                        "{site_code} doesn't say where it packs yet. Set it in Workspace."
                    )),
                    (true, false) => Some(format!(
                        "{site_code} doesn't say who owns the stock it holds, so goods picked \
                         elsewhere can't be handed over. Set it in Workspace."
                    )),
                    (false, false) => Some(format!(
                        "{site_code} doesn't say where it packs or who owns its stock yet. Set \
                         both in Workspace."
                    )),
                };

                let lines = tx
                    .query(
                        "SELECT fl.id, i.code, i.description, ol.item_id,
                                fl.quantity - greatest(fl.picked_quantity, bx.q), fl.quantity,
                                ki.code, ki.description, kl.quantity_ordered
                           FROM fulfilment_line fl
                           JOIN order_line ol ON ol.id = fl.order_line_id
                           JOIN item i ON i.id = ol.item_id
                           -- The kit it is a part of (D223).
                           LEFT JOIN order_line kl ON kl.id = ol.kit_line_id
                           LEFT JOIN item ki ON ki.id = kl.item_id
                           LEFT JOIN LATERAL (
                               SELECT coalesce(sum(v.effective_quantity), 0)::bigint AS q
                                 FROM stock_movement m
                                 JOIN stock_movement_effective v
                                   ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
                                WHERE m.fulfilment_line_id = fl.id
                                  AND m.to_package_id IS NOT NULL) bx ON true
                          WHERE fl.fulfilment_id = $1
                          -- A kit's parts together, where the kit's code sorts.
                          ORDER BY coalesce(ki.code, i.code), kl.id NULLS FIRST, i.code",
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
                    // Picked elsewhere: the level live from the reports (the
                    // column may lag the scheduler), the newest report's
                    // document and picker, and what is handed over already.
                    let line_id: Uuid = l.get(0);
                    let elsewhere = tx
                        .query_opt(
                            "SELECT (SELECT coalesce(sum(quantity), 0)::bigint FROM
                                       (SELECT DISTINCT ON (external_line) quantity
                                          FROM external_pick WHERE fulfilment_line_id = $1
                                         ORDER BY external_line, observed_at DESC,
                                                  recorded_at DESC, id DESC) newest),
                                    (SELECT coalesce(sum(v.effective_quantity), 0)::bigint
                                       FROM stock_movement m
                                       JOIN stock_movement_effective v
                                         ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
                                      WHERE m.fulfilment_line_id = $1
                                        AND m.external_pick_id IS NOT NULL),
                                    ep.document, ep.picked_by,
                                    (SELECT name FROM source_channel WHERE id = ep.source_channel_id)
                               FROM external_pick ep
                              WHERE ep.fulfilment_line_id = $1
                              ORDER BY ep.observed_at DESC, ep.recorded_at DESC, ep.id DESC
                              LIMIT 1",
                            &[&line_id],
                        )
                        .await?
                        .map(|r| PickedElsewhere {
                            reported: r.get(0),
                            handed: r.get(1),
                            provenance: crate::packing::picked_elsewhere(
                                r.get(4),
                                r.get(2),
                                r.get(3),
                            ),
                            document: r.get(2),
                            picked_by: r.get(3),
                        });
                    let measured = crate::routes::measurements_of(tx, item_id).await?;
                    let case = case_pack(tx, item_id).await?;
                    // What NetSuite counts one of (D218): what was ordered is in it.
                    let unit: String = tx
                        .query_opt("SELECT level::text FROM item_unit_level WHERE item_id = $1", &[&item_id])
                        .await?
                        .map(|r| r.get(0))
                        .unwrap_or_else(|| "each".into());
                    out.push(BenchLine {
                        elsewhere,
                        own_carton: own_carton(case.as_ref(), &measured, &unit),
                        picture: None,
                        packs: packs_of(case.as_ref(), &measured, &unit),
                        line_id: l.get(0),
                        item_id,
                        item_code: l.get(1),
                        description: l.get(2),
                        remaining: l.get(4),
                        committed: l.get(5),
                        kit: l.get::<_, Option<String>>(6).map(|item_code| KitOf {
                            item_code,
                            description: l.get(7),
                            ordered: l.get(8),
                        }),
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

                looks(tx, &mut out).await?;
                Ok(Bench {
                    reference: head.get(0),
                    order_reference: head.get(4),
                    customer: head.get(1),
                    site: head.get(2),
                    dock_id,
                    staging_id,
                    unready,
                    lines: out,
                })
            })
        })
        .await
}

/// The case pack in force: the newest by `effective_from`, the rule
/// `receiving` reads it by. Its id, units per inner and inners per carton.
struct CasePack {
    id: Uuid,
    per_inner: Option<i32>,
    inners: Option<i32>,
}

async fn case_pack(
    tx: &tokio_postgres::Transaction<'_>,
    item_id: Uuid,
) -> Result<Option<CasePack>, ApiError> {
    Ok(tx
        .query_opt(
            "SELECT id, units_per_inner, inners_per_carton
               FROM item_packing_config
              WHERE item_id = $1 AND effective_from <= CURRENT_DATE
              ORDER BY effective_from DESC, id DESC
              LIMIT 1",
            &[&item_id],
        )
        .await?
        .map(|c| CasePack {
            id: c.get(0),
            per_inner: c.get(1),
            inners: c.get(2),
        }))
}

/// How many of what NetSuite counts are in one of each level (D218), each,
/// inner, carton: one of the unit is one; a carton is so many of it; a level
/// below the unit has none, because it never leaves on its own. A count
/// nobody has said is none, and a level with none is not offered.
fn per_level(case: Option<&CasePack>, unit: &str) -> [(&'static str, Option<i64>); 3] {
    let per_inner = case.and_then(|c| c.per_inner).map(i64::from).filter(|n| *n > 0);
    let inners = case.and_then(|c| c.inners).map(i64::from).filter(|n| *n > 0);
    match unit {
        // The carton is the unit: one of it is one, whatever it holds.
        "carton" => [("each", None), ("inner", None), ("carton", Some(1))],
        // The pack is the unit: a carton is so many packs, however many each
        // pack holds, said or not.
        "inner" => [("each", None), ("inner", Some(1)), ("carton", inners)],
        _ => [
            ("each", Some(1)),
            ("inner", per_inner.filter(|n| *n > 1)),
            ("carton", per_inner.zip(inners).map(|(p, i)| p * i)),
        ],
    }
}

/// A carton of this item, when it has one of a known count: in what NetSuite
/// counts, so a carton that is the unit is one (D218).
///
/// The carton's figures are [`crate::routes::measurements_of`]'s, so the bench
/// and the item page cannot disagree about what a carton measures.
fn own_carton(
    case: Option<&CasePack>,
    measured: &[crate::routes::ItemMeasurements],
    unit: &str,
) -> Option<OwnCarton> {
    let c = case?;
    let units = per_level(case, unit)[2].1?;
    let carton = measured.iter().find(|m| m.packaging_level == "carton");
    Some(OwnCarton {
        item_packing_config_id: c.id,
        units,
        size: carton.and_then(size_of),
        listed_weight_g: carton.and_then(|m| m.gross_weight_g),
        method: carton.and_then(|m| m.method.clone()),
        source: carton.map(|m| m.source.clone()),
        style_code: carton.and_then(|m| m.style_code.clone()),
    })
}

/// One of it at each level it can leave at (D195, D196, D218): its unit
/// always, the one NetSuite counts; a pack or a carton above it when the case
/// pack counts one. Each with what is recorded at that level, and nothing
/// guessed: the arrangement lists a level with no size as not measured. Its
/// sides, whether it has no size, and whether it ships as it is are filled in
/// by [`looks`], for every line at once.
fn packs_of(case: Option<&CasePack>, measured: &[crate::routes::ItemMeasurements], unit: &str) -> Vec<PackUnit> {
    per_level(case, unit)
        .into_iter()
        .filter_map(|(level, units)| {
            let units = units?;
            let m = measured.iter().find(|m| m.packaging_level == level);
            Some(PackUnit {
                level: level.to_string(),
                units,
                ships_as_is: level == "carton",
                upright: false,
                size: m.and_then(size_of),
                no_size: false,
                gross_weight_g: m.and_then(|m| m.gross_weight_g),
                source: m.map(|m| m.source.clone()).unwrap_or_else(|| "own".into()),
                style_code: m.and_then(|m| m.style_code.clone()),
                faces: BTreeMap::new(),
            })
        })
        .collect()
}

/// Each line's picture, and its eaches' and inners' sides and whether they
/// have no size, in three reads for the whole screen rather than three a line.
async fn looks(tx: &tokio_postgres::Transaction<'_>, lines: &mut [BenchLine]) -> Result<(), ApiError> {
    let ids: Vec<Uuid> = lines.iter().map(|l| l.item_id).collect();
    let pictured = pictures::of(tx, &ids).await?;
    // The newest cut of each side of its own each or inner (D176), not one
    // moved to another subject. Only cut faces: an uncut photo is the bench
    // behind the box as much as the box.
    let mut faces: HashMap<(Uuid, String), BTreeMap<String, String>> = HashMap::new();
    for r in tx
        .query(
            "SELECT DISTINCT ON (o.item_id, o.packaging_level, oi.face)
                    o.item_id, o.packaging_level::text, oi.face, x.digest
               FROM observable o
               JOIN observation_event e ON e.observable_id = o.id
               JOIN observation_image oi ON oi.observation_event_id = e.id
               JOIN LATERAL (
                    SELECT c.digest FROM observation_image_cut c
                     WHERE c.observation_image_id = oi.id
                     ORDER BY c.recorded_at DESC, c.id DESC
                     LIMIT 1) x ON true
              WHERE o.item_id = ANY($1)
                AND o.packaging_level IN ('each', 'inner')
                AND oi.face IN ('front', 'back', 'left', 'right', 'top', 'bottom')
                AND NOT EXISTS (SELECT 1 FROM observation_image_move mv
                                 WHERE mv.observation_image_id = oi.id)
              ORDER BY o.item_id, o.packaging_level, oi.face, oi.captured_at DESC, oi.id DESC",
            &[&ids],
        )
        .await?
    {
        faces.entry((r.get(0), r.get(1))).or_default().insert(r.get(2), r.get(3));
    }
    // "It has no size to measure" is all three lengths absent (D138), the
    // reading `capture` gives it.
    let sizeless: Vec<(Uuid, String)> = tx
        .query(
            "SELECT o.item_id, o.packaging_level::text
               FROM observable o
               JOIN observation_current oc ON oc.observable_id = o.id
               JOIN metric m ON m.id = oc.metric_id
              WHERE o.item_id = ANY($1) AND o.packaging_level IN ('each', 'inner')
                AND m.code IN ('length', 'width', 'height')
              GROUP BY o.item_id, o.packaging_level
             HAVING count(*) FILTER (WHERE oc.absent_reason IS NOT NULL) = 3",
            &[&ids],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    // Whether each level ships as it is (D196) and stays the way up it stands
    // (D200): said, inherited or the default.
    let handling: HashMap<(Uuid, String), (bool, bool)> = tx
        .query(
            "SELECT i.id, l.level, s.as_it_is, u.upright
               FROM unnest($1::uuid[]) AS i(id)
              CROSS JOIN unnest(ARRAY['each', 'inner', 'carton']) AS l(level)
              CROSS JOIN LATERAL ships_as_is(i.id, NULL, NULL, NULL, l.level::packaging_level) s
              CROSS JOIN LATERAL keeps_upright(i.id, NULL, NULL, NULL, l.level::packaging_level) u",
            &[&ids],
        )
        .await?
        .iter()
        .map(|r| ((r.get(0), r.get(1)), (r.get(2), r.get(3))))
        .collect();
    for line in lines.iter_mut() {
        line.picture = pictured.get(&line.item_id).cloned();
        for p in line.packs.iter_mut() {
            let key = (line.item_id, p.level.clone());
            if let Some((as_is, upright)) = handling.get(&key) {
                p.ships_as_is = *as_is;
                p.upright = *upright;
            }
            p.faces = faces.remove(&key).unwrap_or_default();
            p.no_size = p.size.is_none() && sizeless.contains(&key);
        }
    }
    Ok(())
}

/// All three lengths, or nothing: two of three is not a carton's size.
fn size_of(m: &crate::routes::ItemMeasurements) -> Option<StatedSize> {
    let whole = |v: Option<i64>| v.and_then(|v| i32::try_from(v).ok());
    Some(StatedSize {
        length_mm: whole(m.length_mm)?,
        width_mm: whole(m.width_mm)?,
        height_mm: whole(m.height_mm)?,
    })
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
                        "SELECT id, name,
                                dimensions_fixed AND coalesce(carrier_package_code, '')
                                    NOT IN ('PAL', 'SKI', 'SKD'),
                                length_mm, width_mm, height_mm, suggested, max_payload_g
                           FROM package_type
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
                        size: match (r.get::<_, bool>(2), r.get(3), r.get(4), r.get(5)) {
                            (true, Some(l), Some(w), Some(h)) => Some(StatedSize {
                                length_mm: l,
                                width_mm: w,
                                height_mm: h,
                            }),
                            _ => None,
                        },
                        suggested: r.get(6),
                        max_payload_g: r.get(7),
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
                                pt.height_mm, pt.tare_weight_g, p.own_item_id, own_item.code,
                                p.own_level::text
                           FROM package p
                           LEFT JOIN package_type pt ON pt.id = p.package_type_id
                           LEFT JOIN item own_item ON own_item.id = p.own_item_id
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
                    // One of a product as it is states the size its item is
                    // recorded at, at that level, read here rather than copied
                    // onto it (D196).
                    let own_item: Option<Uuid> = r.get(11);
                    let own_level: Option<String> = r.get(13);
                    let own_figures = match (own_item, own_level.as_deref()) {
                        (Some(item), Some(level)) => crate::routes::measurements_of(tx, item)
                            .await?
                            .into_iter()
                            .find(|m| m.packaging_level == level),
                        _ => None,
                    };
                    out.push(CartonSummary {
                        id,
                        sequence: r.get(1),
                        package_type: r.get(2),
                        own_carton_of: r.get(12),
                        own_level,
                        listed_weight_g: own_figures.as_ref().and_then(|m| m.gross_weight_g),
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
                            _ => own_figures.as_ref().and_then(size_of),
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
                                    item_id: c.get(4),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn case(per_inner: Option<i32>, inners: Option<i32>) -> CasePack {
        CasePack { id: Uuid::nil(), per_inner, inners }
    }

    /// What one of each level is, in what NetSuite counts (D218).
    fn units(c: Option<CasePack>, unit: &str) -> Vec<(String, i64)> {
        packs_of(c.as_ref(), &[], unit).into_iter().map(|p| (p.level, p.units)).collect()
    }

    fn of(levels: &[(&str, i64)]) -> Vec<(String, i64)> {
        levels.iter().map(|(l, n)| (l.to_string(), *n)).collect()
    }

    #[test]
    fn a_carton_sold_as_one_is_one_whatever_it_holds() {
        // Gloves, "ctn 1000": an order for two is two cartons, not two gloves.
        assert_eq!(units(Some(case(Some(1), Some(1000))), "carton"), of(&[("carton", 1)]));
        // Its count unsaid, it is still one carton.
        assert_eq!(units(Some(case(None, None)), "carton"), of(&[("carton", 1)]));
        assert_eq!(units(None, "carton"), of(&[("carton", 1)]));
    }

    #[test]
    fn a_pack_sold_as_one_fills_a_carton_by_the_pack() {
        // Earplugs, a box of 100 in cartons of 10 boxes; no single pair.
        assert_eq!(units(Some(case(Some(100), Some(10))), "inner"), of(&[("inner", 1), ("carton", 10)]));
        // How many are in a box unsaid: the carton is still ten boxes.
        assert_eq!(units(Some(case(None, Some(10))), "inner"), of(&[("inner", 1), ("carton", 10)]));
        assert_eq!(units(None, "inner"), of(&[("inner", 1)]));
    }

    #[test]
    fn an_each_sold_as_one_is_as_it_was() {
        assert_eq!(units(Some(case(Some(1), Some(6))), "each"), of(&[("each", 1), ("carton", 6)]));
        assert_eq!(units(Some(case(Some(24), Some(6))), "each"), of(&[("each", 1), ("inner", 24), ("carton", 144)]));
        assert_eq!(units(Some(case(None, Some(6))), "each"), of(&[("each", 1)]), "a carton of an unsaid count is not filled");
        assert_eq!(units(None, "each"), of(&[("each", 1)]));
    }

    #[test]
    fn a_carton_of_its_own_counts_what_netsuite_counts() {
        let carton = |c: CasePack, unit| own_carton(Some(&c), &[], unit).map(|o| o.units);
        assert_eq!(carton(case(Some(1), Some(1000)), "carton"), Some(1));
        assert_eq!(carton(case(Some(100), Some(10)), "inner"), Some(10));
        assert_eq!(carton(case(Some(1), Some(6)), "each"), Some(6));
        assert_eq!(carton(case(None, Some(6)), "each"), None);
    }
}
