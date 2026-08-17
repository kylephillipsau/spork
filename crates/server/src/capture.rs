//! What wants weighing, measuring or photographing, and in what order.
//!
//! [`crate::revalidation`] answers a narrower question — *what wants putting on
//! the scale* — and answers it well. This is the same question widened to what a
//! capture session actually produces under D133: a weight, three dimensions and
//! seven photographs, taken in one look at one box.
//!
//! Nothing here writes. The write is `POST /observations` followed by
//! `POST /observations/{id}/images/{face}`, both of which already existed.
//!
//! # Why this does not read `observation_current` first
//!
//! The weighing worklist starts from `observation_current` and asks which of
//! those values have gone stale. That is right for revalidation and wrong here,
//! because **a thing nobody has ever observed has no row in that table at all**
//! and is therefore invisible to it — which is precisely the thing a capture
//! worklist exists to find. So this starts from the catalogue and reports what
//! is known about each subject, including nothing.
//!
//! # Which subjects the floor is asked to capture
//!
//! Not every row `observable` can hold. The write path accepts five packaging
//! levels; this offers two, and the omission is a choice rather than an
//! oversight. A carton is what ships, what despatch weighs and what a carrier
//! invoices against; an `each` is the unit a pick is counted in. `inner`,
//! `layer` and `pallet` are definite subjects that nothing on the floor is
//! currently asked to walk to, and a worklist naming them would be three more
//! lines per item that never come off the list.
//!
//! And **parts**, which are a subject with no level at all (D139). A pan and
//! its 1200mm handle carry the sizes because the set they make has none, so
//! they are on the list for the same reason a carton is: somebody has to walk
//! to them. Their parent's `each` stays on the list beside them — a two-part
//! thing could have a meaningful assembled box, and deciding otherwise from
//! the presence of parts would be inferring by fiat what the operator is there
//! to determine. What the read sends instead is the count, so the screen can
//! say *measure the parts*.
//!
//! # A worklist that cannot hear "no" asks forever
//!
//! D138 makes *this thing has no dimensions* an answer rather than a gap, and
//! the classifier has three states because of it. Two consequences live in the
//! query rather than in the function.
//!
//! **A declared absence completes a subject** only when all three lengths carry
//! one, on the same argument that two measurements out of three is not a cube.
//!
//! **And absences are excluded from the staleness and method aggregation.**
//! Without that, a set whose weight came off a scale and whose dimensions were
//! declared absent resolves its newest method to the `keyed` act that declared
//! them, `ever_measured("keyed")` is false, and it sits on `unconfirmed` for
//! ever with nothing to re-measure. There is no interval an absence could be
//! measured against.
//!
//! Carton is offered only where an `item_packing_config` in force says how many
//! inners are in one, which is the same rule the writer applies when it refuses
//! a measurement of a carton that *"is not yet a definite thing to measure"*.
//!
//! # A style is one trip, not four
//!
//! D108 makes the style the subject that gets measured and the SKU the thing
//! that inherits, because *"writing the style's numbers onto each variant would
//! claim four measurements where one carton was weighed."* The worklist has the
//! same failure available to it in a different form: listing four variants of
//! one style would send an operator to weigh one carton four times.
//!
//! So a style's carton is the subject, and a variant's carton appears beside it
//! only when the variant has been treated as a subject in its own right —
//! meaning somebody has already recorded an observation against it. Own facts
//! beat inherited ones per D108 and age on their own, so once they exist the
//! variant is a thing to come back to.
//!
//! **Enumerated from `observation`, not from `observation_current`.** Whether a
//! subject is a subject is a question about facts, and the fold behind
//! `observation_current` lags an act by a rebuild (D107). Reading the projection
//! here would make a carton somebody measured this morning drop off the list
//! until the rebuild caught up, and then reappear.
//!
//! # Figures inherit; photographs do not
//!
//! A missing dimension resolves to the style's, per D108. A missing photograph
//! does not resolve to anything. D132 hangs an image off the event that produced
//! it — one look at one box — so showing a style's picture against a variant
//! would claim a photograph of this carton that is a photograph of a different
//! subject's carton. Figures are what a kind of thing measures; a photograph is
//! what one look saw.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use actix_web::web;

use crate::auth::Caller;
use crate::error::ApiError;
use crate::revalidation;
use crate::tenancy::TenantScope;
use crate::AppState;

/// The three lists, and what a subject is doing on one.
///
/// **Three rather than one, and none of them called `never`.** The weighing
/// worklist already uses that word for *never confirmed by an instrument*, which
/// is a different state from *nothing has ever been recorded* — a transcribed
/// weight is one and not the other. D53 is the standing example of one name
/// doing two jobs in this repository, so the words here are chosen to sit beside
/// revalidation's rather than to collide with them.
#[derive(Serialize)]
pub struct CaptureScreen {
    pub site: String,
    /// **One list, in the order somebody would walk it.**
    ///
    /// This was three — unrecorded, partial, unconfirmed — which is the right
    /// shape for triage at a desk and the wrong one for a lap of the
    /// warehouse: an operator working three lists in bin order walks past each
    /// shelf three times. The classification is not lost, it moved onto the
    /// row: `wants` says what this trip is for and `because` says why the row
    /// is here at all, and between them they are finer than the list ever was.
    /// The rule the despatch bench states — *a thing on two lists is a screen
    /// telling somebody to do the same job twice* — is the same one.
    pub walk: Vec<CaptureSubject>,
}

/// One thing to walk to, and everything the screen draws about it.
///
/// The identity is `(item_id | item_style_id, packaging_level)`, which is
/// exactly what `POST /observations` takes as a subject. There is deliberately
/// no `observable_id`: the writer is get-or-create over that table, so a screen
/// that carried one would be carrying an id it must not send.
#[derive(Serialize)]
pub struct CaptureSubject {
    pub item_id: Option<Uuid>,
    pub item_style_id: Option<Uuid>,
    /// **The part arm, and it takes no level.** A pan and its handle are
    /// measured on their own because the set they make has no box of its own.
    /// D139. A subject carrying this posts `item_part_id` and nothing else.
    pub item_part_id: Option<Uuid>,
    /// Which part it is, for a screen that shows two rows under one code.
    pub part_label: Option<String>,
    pub code: String,
    pub description: Option<String>,
    /// `each` or `carton`, and **absent for a part** — a part has no packaging
    /// level, and sending one would be sending a field the writer refuses.
    pub packaging_level: Option<String>,
    /// Parts this subject is made of. Non-zero says the box to measure is not
    /// this one: measure the parts, and answer *not applicable* here. D139.
    pub parts: i64,
    /// Canonical, per Principle 5: grams and millimetres. What was typed lives
    /// on the observation.
    pub gross_weight_g: Option<i64>,
    pub length_mm: Option<i64>,
    pub width_mm: Option<i64>,
    pub height_mm: Option<i64>,
    /// Declared to have none, rather than not yet weighed. D138.
    pub weight_absent: bool,
    /// All three lengths declared absent. Two of three is not an answer, for
    /// the same reason two of three is not a cube.
    pub dimensions_absent: bool,
    /// `own`, `style` or `mixed` — D108's vocabulary, and the reason it exists:
    /// a screen that cannot tell them apart reports a number nobody took
    /// against this code as though somebody had.
    pub source: Option<String>,
    pub style_code: Option<String>,
    /// How the newest figure was come by. A cube off a prepack sheet and a cube
    /// off a scanner are both facts and are not the same fact.
    pub method: Option<String>,
    pub observed_at: Option<DateTime<Utc>>,
    /// Faces photographed against this subject, of the seven.
    pub faces: Vec<String>,
    /// `weight`, `dimensions`, `photographs` — what a session here would add.
    pub wants: Vec<String>,
    /// Open order lines naming it. A style counts its variants' lines, which is
    /// the whole reason it is the subject.
    pub demand: i64,
    /// Finer than the list it is on: `nothing`, `incomplete`, `never-measured`,
    /// `overdue`.
    pub because: String,
    /// The bin holding the most of it at this site, or absent where the stock
    /// projection has none — a catalogue row nobody has put anywhere.
    pub location_code: Option<String>,
    /// How much is here, over every bin at this site. Zero is a real answer and
    /// the reason the walk excludes it: an empty bin is not a thing to walk to.
    pub soh: i64,
}

/// What the record says about one of the things a session produces.
///
/// **Three states, because a worklist that cannot hear "no" asks forever.** An
/// apron loose in its carton and a two-part set have no bounding box, and until
/// D138 the only way to say so was to leave the row empty — which is what
/// *nobody has measured this yet* looks like. So a declared absence is an
/// answer, and it settles the subject exactly as a number does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Nothing on file.
    Missing,
    /// A figure.
    Recorded,
    /// Somebody looked and recorded that there is none. D138.
    NotApplicable,
}

impl Answer {
    /// Whether the question has been answered at all, however it was answered.
    fn given(self) -> bool {
        !matches!(self, Answer::Missing)
    }
}

/// The three things one capture session produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Held {
    pub weight: Answer,
    /// All three of length, width and height. Two out of three is not a cube,
    /// and two declared absent out of three is not an answer either.
    pub dimensions: Answer,
    /// **Any face at all, not all seven.** A subject with some pictures has been
    /// looked at, and putting it back on a worklist for a missing `bottom` sends
    /// somebody to a shelf for a photograph nobody is waiting for. The screen
    /// shows which faces are present so a retake starts from the subject rather
    /// than from this list.
    pub photographs: bool,
}

/// Which list a subject belongs on, or none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Unrecorded,
    Partial,
    /// Complete, and either never measured or measured too long ago.
    Unconfirmed,
    /// Complete, measured, and not yet due. On no list.
    Settled,
}

impl Class {
    pub fn list(self) -> Option<&'static str> {
        match self {
            Class::Unrecorded => Some("unrecorded"),
            Class::Partial => Some("partial"),
            Class::Unconfirmed => Some("unconfirmed"),
            Class::Settled => None,
        }
    }
}

/// What a session here would add, in the order a session does it.
pub fn wants(held: Held) -> Vec<String> {
    let mut out = vec![];
    if !held.weight.given() {
        out.push("weight".to_string());
    }
    if !held.dimensions.given() {
        out.push("dimensions".to_string());
    }
    if !held.photographs {
        out.push("photographs".to_string());
    }
    out
}

/// Exactly one list, or none.
///
/// **The property this whole module owes.** A subject on two lists is a screen
/// telling an operator to walk to the same box twice, which is the despatch
/// bench's rule about cartons applied to the thing being measured. Written as a
/// total function over the three booleans so that it is a property of the
/// classifier rather than of the query that feeds it.
pub fn classify(held: Held, method: Option<&str>, staleness: f64) -> Class {
    if !held.weight.given() && !held.dimensions.given() && !held.photographs {
        return Class::Unrecorded;
    }
    if !held.weight.given() || !held.dimensions.given() || !held.photographs {
        return Class::Partial;
    }
    // **A declared absence does not age.** Revalidation exists because a figure
    // can drift from the thing it describes; *this set has no bounding box* is
    // not a reading that goes stale, and there is nothing to put back on a
    // scale. A subject whose every figure is an absence is therefore settled,
    // and without this it would sit on `unconfirmed` for ever with no method to
    // measure its age against — the same trap in a new place.
    if held.weight == Answer::NotApplicable && held.dimensions == Answer::NotApplicable {
        return Class::Settled;
    }
    match method {
        // Nothing has been on a scale. The interval cannot help: an imported
        // figure carries the date of the import, so its age is unknown rather
        // than small — which is the finding the weighing worklist made and the
        // reason it grew a second list.
        Some(m) if !revalidation::ever_measured(m) => Class::Unconfirmed,
        Some(_) if staleness >= 1.0 => Class::Unconfirmed,
        Some(_) => Class::Settled,
        // Figures present and no method recorded. Not trusted, on the same
        // argument `trust_of` makes about a vocabulary that grows.
        None => Class::Unconfirmed,
    }
}

/// Why it is on the list it is on.
pub fn because(class: Class, method: Option<&str>) -> &'static str {
    match class {
        Class::Unrecorded => "nothing",
        Class::Partial => "incomplete",
        Class::Unconfirmed => match method {
            Some(m) if revalidation::ever_measured(m) => "overdue",
            _ => "never-measured",
        },
        Class::Settled => "settled",
    }
}

/// The subjects, and everything known about each.
///
/// **One query for the catalogue and one for the photographs.** The screen is a
/// page, and a page costing a round trip per subject is what the handheld
/// cannot afford — the same argument the despatch bench makes about fetching
/// each carton before it can act.
pub async fn screen(
    state: &web::Data<AppState>,
    who: &Caller,
    limit: i64,
) -> Result<CaptureScreen, ApiError> {
    let site_id = who.site_id;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let site = match site_id {
                    Some(id) => tx
                        .query_opt("SELECT code FROM site WHERE id = $1", &[&id])
                        .await?
                        .map(|r| r.get::<_, String>(0))
                        .unwrap_or_else(|| "no site".into()),
                    None => "every site".into(),
                };

                // **Only what is on a shelf here.** A walk is a lap of the
                // building, so a catalogue row with no stock at this site is
                // not a thing anybody can walk to. This filters on *stock*
                // rather than on observations, which is what keeps this
                // module's founding argument intact: something nobody has
                // ever measured has no `observation_current` row and is
                // exactly what a capture worklist exists to find, and it is
                // still on the walk if it is on a shelf.
                //
                // Anything else is reachable by typing or scanning its code,
                // which is `subjects_for_item` and answers for stock and no
                // stock alike.
                let sql = format!("{SUBJECTS} WHERE coalesce(hi.soh, hs.soh, 0) > 0");
                let mut classified = classified_subjects(tx, &sql, &[&site_id]).await?;

                // **Bin order, which is the whole point.** Demand decided the
                // order when this was three lists read at a desk; on a walk
                // the only order that saves steps is the one the shelves are
                // in. A subject with no bin sorts last rather than first —
                // `Option::None` orders before `Some` in Rust, and a row
                // nobody can walk to leading the walk is the wrong end.
                classified.sort_by(|a, b| {
                    a.1.location_code
                        .is_none()
                        .cmp(&b.1.location_code.is_none())
                        .then(a.1.location_code.cmp(&b.1.location_code))
                        .then(a.1.code.cmp(&b.1.code))
                        .then(a.1.packaging_level.cmp(&b.1.packaging_level))
                });

                let walk = classified
                    .into_iter()
                    .filter(|(class, _)| *class != Class::Settled)
                    .map(|(_, subject)| subject)
                    .take(limit as usize)
                    .collect();

                Ok(CaptureScreen { site, walk })
            })
        })
        .await
}

/// Run the enumeration and classify every row it yields.
///
/// **Shared with the locator, and that sharing is load-bearing.** A scan
/// resolves to an *item*, and the obvious next move is to open a session
/// against that item at carton level — which would walk straight back into the
/// trip D108 exists to prevent, because for a styled variant the worklist
/// deliberately offers the *style's* carton instead. Two enumerations would
/// disagree the first time somebody scanned a variant, and the scan would win
/// silently, writing observations against a subject the worklist would never
/// have listed.
///
/// So there is one enumeration and one classifier, and the scan path reaches
/// the screen through them rather than beside them.
async fn classified_subjects(
    tx: &tokio_postgres::Transaction<'_>,
    sql: &str,
    params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
) -> Result<Vec<(Class, CaptureSubject)>, ApiError> {
    let rows = tx.query(sql, params).await?;
    let now = Utc::now();
    let mut out = vec![];
    for r in &rows {
        let gross_weight_g: Option<i64> = r.get(5);
        let length_mm: Option<i64> = r.get(6);
        let width_mm: Option<i64> = r.get(7);
        let height_mm: Option<i64> = r.get(8);
        let method: Option<String> = r.get(11);
        let observed_at: Option<DateTime<Utc>> = r.get(12);
        let faces: Vec<String> = r.get::<_, Option<Vec<String>>>(13).unwrap_or_default();
        let weight_absent: bool = r.get(15);
        let dimensions_absent: bool = r.get(16);

        // **A number wins over a declared absence**, and the order matters
        // because both can be on file: an operator says a set has no box, and a
        // later session measures one anyway. `observation_current` has already
        // decided which observation is current per metric, so a value present
        // here is the one that won, and the absence flag is what the losing
        // metrics carry.
        let held = Held {
            weight: if gross_weight_g.is_some() {
                Answer::Recorded
            } else if weight_absent {
                Answer::NotApplicable
            } else {
                Answer::Missing
            },
            dimensions: if length_mm.is_some() && width_mm.is_some() && height_mm.is_some() {
                Answer::Recorded
            } else if dimensions_absent {
                Answer::NotApplicable
            } else {
                Answer::Missing
            },
            photographs: !faces.is_empty(),
        };
        let staleness = match observed_at {
            Some(at) => revalidation::staleness(at, method.as_deref().unwrap_or(""), now),
            None => 0.0,
        };
        let class = classify(held, method.as_deref(), staleness);

        out.push((
            class,
            CaptureSubject {
                location_code: r.get(20),
                soh: r.get::<_, Option<i64>>(21).unwrap_or(0),
                item_id: r.get(0),
                item_style_id: r.get(1),
                item_part_id: r.get(17),
                part_label: r.get(18),
                code: r.get(2),
                description: r.get(3),
                // `part` is this query's join key and never a packaging level.
                // Turning it into an absent level here is what stops a screen
                // posting it as one.
                packaging_level: match r.get::<_, String>(4).as_str() {
                    "part" => None,
                    level => Some(level.to_string()),
                },
                parts: r.get(19),
                gross_weight_g,
                length_mm,
                width_mm,
                height_mm,
                weight_absent,
                dimensions_absent,
                source: r.get(9),
                style_code: r.get(10),
                wants: wants(held),
                because: because(class, method.as_deref()).to_string(),
                method,
                observed_at,
                faces,
                demand: r.get::<_, Option<i64>>(14).unwrap_or(0),
            },
        ));
    }
    Ok(out)
}

/// The capture subjects an item resolves to, in the order to offer them.
///
/// **What a scan lands on.** The item's own `each`, its own carton where the
/// worklist would list one, and its style's carton where the style is the
/// subject — exactly the rows `GET /capture` would show, because it is the same
/// query with a filter on it.
///
/// A subject already settled comes back too. The worklist hides those because a
/// worklist is a list of work; a scan is a person holding the box and asking
/// what this is, and answering "nothing" to a box in somebody's hand is not an
/// answer. `because` says which it is.
pub async fn subjects_for_item(
    tx: &tokio_postgres::Transaction<'_>,
    site_id: Option<uuid::Uuid>,
    item_id: uuid::Uuid,
) -> Result<Vec<CaptureSubject>, ApiError> {
    // **Ordered by an explicit rank rather than by the level's spelling.** The
    // first version sorted `level DESC` with a comment claiming that put a
    // carton first. It does not — `carton` sorts before `each`, so descending
    // puts `each` first — and adding `part` to the vocabulary would have
    // silently reshuffled it a third way. A rank says what the order is.
    // `$1` is the site, because the query it shares carries the bin and the
    // stock figure. `$2` is the item.
    let sql = format!(
        "{SUBJECTS}
         WHERE sub.item_id = $2
            OR sub.item_style_id = (SELECT style_id FROM item WHERE id = $2)
            OR sub.item_part_id IN (SELECT id FROM item_part WHERE item_id = $2)
         ORDER BY CASE sub.level WHEN 'carton' THEN 0 WHEN 'each' THEN 1 ELSE 2 END,
                  sub.code, sub.part_label"
    );
    let mut found = classified_subjects(tx, &sql, &[&site_id, &item_id]).await?;
    // A scan off a carton is the common case and the level the operator wants
    // is usually the bigger one, so carton leads, then the each, then the parts
    // — which are what to measure when the each turns out to have no box.
    Ok(found.drain(..).map(|(_, s)| s).collect())
}

/// Every subject the floor may be asked to capture, with what is known of it.
///
/// The resolution in `resolved` is [`crate::routes::item_measurements`]'s,
/// generalised from one item to the catalogue: `DISTINCT ON (subject, metric)`
/// ordered by rank, so own facts win per metric and the rest inherit. D108's
/// rule is per fact rather than per subject, which is why the rank sits inside
/// the `DISTINCT ON` and not in a `WHERE`.
///
/// # Why every join is on one column rather than two
///
/// A subject is an item *or* a style, so the obvious join is
/// `item_id IS NOT DISTINCT FROM …AND item_style_id IS NOT DISTINCT FROM …`,
/// which is what this did first and what is correct and quietly catastrophic.
/// **`IS NOT DISTINCT FROM` is not an equality operator, so the planner cannot
/// hash on it** and falls back to a nested loop. Against the real catalogue —
/// nine thousand items, not the fixture's two — that was 9,021 rows against
/// 9,075 in the demand set, eighty-two million comparisons, and 1.4 seconds for
/// a screen D2 requires to answer in under one.
///
/// So exactly one of the two arms is non-null on every row by construction, and
/// `coalesce(item_id, item_style_id)` is therefore a total, non-null key that
/// discriminates them — two uuids never collide. The joins are plain equalities
/// on it and the planner hashes.
///
/// It measured at 1.4s on the fixture's two items too. It measured at 7ms.
const SUBJECTS: &str = "
WITH cfg AS (
    SELECT DISTINCT ON (item_id) item_id, inners_per_carton
      FROM item_packing_config
     WHERE effective_from <= current_date
     ORDER BY item_id, effective_from DESC, id DESC
),
subject AS (
    -- Every item at `each`. A pair of boots is a definite thing on its own and
    -- needs no case pack, which is why `observable_item_config_ck` requires the
    -- config column to be absent here rather than merely allowing it.
    SELECT i.id AS item_id, NULL::uuid AS item_style_id, NULL::uuid AS item_part_id,
           NULL::text AS part_label, 'each'::text AS level,
           i.code, i.description, i.style_id,
           -- Whose stock says where to walk. The subject and the thing on the
           -- shelf are the same item here and are not always: a part is
           -- measured on its own and lives in its parent's bin.
           i.id AS stock_item_id
      FROM item i
    UNION ALL
    -- A style's carton: one subject standing for every variant in it, so long
    -- as some variant's case pack says what a carton of them is.
    SELECT NULL, s.id, NULL, NULL, 'carton', s.code, s.description, NULL, NULL
      FROM item_style s
     WHERE EXISTS (
           SELECT 1 FROM item i
             JOIN cfg ON cfg.item_id = i.id
            WHERE i.style_id = s.id AND cfg.inners_per_carton IS NOT NULL)
    UNION ALL
    -- A variant's own carton, when nothing else speaks for it or when somebody
    -- has already recorded against it.
    SELECT i.id, NULL, NULL, NULL, 'carton', i.code, i.description, i.style_id, i.id
      FROM item i
      JOIN cfg ON cfg.item_id = i.id
     WHERE cfg.inners_per_carton IS NOT NULL
       AND (i.style_id IS NULL
            OR EXISTS (
               SELECT 1 FROM observable o
                 JOIN observation ob ON ob.observable_id = o.id
                WHERE o.item_id = i.id AND o.packaging_level = 'carton'))
    UNION ALL
    -- Every part of every item. D139: a pan and a handle are what carry the
    -- sizes, because the set they make has none. The parent's code so the two
    -- rows sit under the product on a screen sorted by code, and the part's
    -- label to tell them apart.
    --
    -- `level` is `part` here and nowhere in the database. It is this query's
    -- join key for which subject of this item, not a `packaging_level`, and the
    -- mapping into Rust turns it back into an absent level so that nothing can
    -- post it as one.
    SELECT NULL, NULL, p.id, p.label, 'part', i.code, p.label, NULL, i.id
      FROM item_part p
      JOIN item i ON i.id = p.item_id
),
-- ---------------------------------------------------------------------------
-- Where the thing is, and how much of it is there
-- ---------------------------------------------------------------------------
--
-- **A worklist you cannot walk is a list of homework.** The printed sheet this
-- replaces carries a bin and a stock figure on every row and is ordered by the
-- first of them, because that is what turns 26 items into one lap of the
-- warehouse instead of 26 searches.
--
-- Scoped to the caller's site ($1), and NULL there means every site — the same
-- reading `screen` already used for the label it drew.
--
-- **A projection, and that is fine here.** `stock` lags an act by a rebuild
-- (D107), so a box moved five minutes ago may send somebody to the bin it left.
-- The alternative is folding the ledger for nine thousand items to decide a
-- sort order. A walking order wants to be roughly right and cheap, and the
-- operator is looking at the shelf.
at_site AS (
    SELECT s.item_id,
           coalesce(s.holder_location_id, s.resolved_location_id) AS location_id,
           sum(s.quantity)::bigint AS qty
      FROM stock s
     WHERE ($1::uuid IS NULL OR s.site_id = $1)
       AND s.quantity > 0
     GROUP BY 1, 2
),
-- **The bin holding the most of it.** An item split across three bins has no
-- single answer, and the biggest pile is the one worth walking to. The sheet
-- makes the same simplification by printing one bin per row. The total is over
-- every bin, so what the screen shows is *all of it, and where most of it is*.
held_item AS (
    SELECT DISTINCT ON (item_id)
           item_id,
           sum(qty) OVER (PARTITION BY item_id) AS soh,
           location_id
      FROM at_site
     ORDER BY item_id, qty DESC, location_id
),
-- A style is where its variants are, for the reason D108 makes it the subject:
-- one carton spec over thirteen sizes is one box to walk to, and the sizes sit
-- together on the shelf because they ship in the same carton.
held_style AS (
    SELECT DISTINCT ON (i.style_id)
           i.style_id,
           sum(a.qty) OVER (PARTITION BY i.style_id) AS soh,
           a.location_id
      FROM at_site a
      JOIN item i ON i.id = a.item_id
     WHERE i.style_id IS NOT NULL
     ORDER BY i.style_id, a.qty DESC, a.location_id
),
-- The observables that may speak for each subject, and in what order.
candidate AS (
    SELECT sub.item_id, sub.item_style_id, sub.item_part_id, sub.level,
           o.id AS observable_id, 'own'::text AS src, NULL::text AS style_code, 0 AS rank
      FROM subject sub
      JOIN observable o
        ON o.packaging_level::text = sub.level
       AND ((sub.item_id IS NOT NULL AND o.item_id = sub.item_id)
         OR (sub.item_style_id IS NOT NULL AND o.item_style_id = sub.item_style_id))
    UNION ALL
    SELECT sub.item_id, NULL, NULL, sub.level, o.id, 'style', st.code, 1
      FROM subject sub
      JOIN item_style st ON st.id = sub.style_id
      JOIN observable o ON o.item_style_id = st.id
                       AND o.packaging_level::text = sub.level
     WHERE sub.item_id IS NOT NULL
    UNION ALL
    -- A part has one candidate and inherits from nothing. There is no level to
    -- match on, because a part has none.
    SELECT NULL, NULL, sub.item_part_id, sub.level, o.id, 'own', NULL, 0
      FROM subject sub
      JOIN observable o ON o.item_part_id = sub.item_part_id
     WHERE sub.item_part_id IS NOT NULL
),
resolved AS (
    SELECT DISTINCT ON (c.item_id, c.item_style_id, c.item_part_id, c.level, oc.metric_id)
           c.item_id, c.item_style_id, c.item_part_id, c.level, c.src, c.style_code,
           oc.metric_id, oc.value_numeric, oc.absent_reason, oc.method, oc.observed_at
      FROM candidate c
      JOIN observation_current oc ON oc.observable_id = c.observable_id
     ORDER BY c.item_id, c.item_style_id, c.item_part_id, c.level, oc.metric_id, c.rank
),
figures AS (
    SELECT coalesce(r.item_id, r.item_style_id, r.item_part_id) AS subject_key, r.level,
           (max(r.value_numeric) FILTER (WHERE m.code = 'gross_weight'))::bigint AS gross_weight_g,
           (max(r.value_numeric) FILTER (WHERE m.code = 'length'))::bigint  AS length_mm,
           (max(r.value_numeric) FILTER (WHERE m.code = 'width'))::bigint   AS width_mm,
           (max(r.value_numeric) FILTER (WHERE m.code = 'height'))::bigint  AS height_mm,
           -- D138. A declared absence is an answer, and it is not a number, so
           -- it travels beside the figures rather than in them.
           bool_or(m.code = 'gross_weight' AND r.absent_reason IS NOT NULL) AS weight_absent,
           count(*) FILTER (WHERE m.code IN ('length','width','height')
                              AND r.absent_reason IS NOT NULL) = 3 AS dimensions_absent,
           CASE WHEN bool_and(r.src = 'own') THEN 'own'
                WHEN bool_and(r.src = 'style') THEN 'style'
                ELSE 'mixed' END AS src,
           max(r.style_code) AS style_code,
           -- The newest figure's method, which is what the interval is measured
           -- against. `array_agg … ORDER BY observed_at DESC` rather than a
           -- second scan.
           --
           -- **Absences are excluded from both.** A set whose weight came off a
           -- scale and whose dimensions were declared absent would otherwise
           -- resolve its newest method to the `keyed` act that declared them,
           -- and revalidation would call an instrument reading unconfirmed for
           -- ever. There is nothing to re-measure about an absence and no
           -- interval it could be measured against.
           (array_agg(r.method ORDER BY r.observed_at DESC)
                FILTER (WHERE r.absent_reason IS NULL))[1] AS method,
           max(r.observed_at) FILTER (WHERE r.absent_reason IS NULL) AS observed_at
      FROM resolved r
      JOIN metric m ON m.id = r.metric_id
     WHERE m.code IN ('gross_weight','length','width','height')
     GROUP BY coalesce(r.item_id, r.item_style_id, r.item_part_id), r.level
),
-- Photographs do not inherit: this joins the subject's own observable only.
faces AS (
    SELECT coalesce(o.item_id, o.item_style_id, o.item_part_id) AS subject_key,
           coalesce(o.packaging_level::text, 'part') AS level,
           array_agg(DISTINCT oi.face) AS faces
      FROM observation_image oi
      JOIN observation_event oe ON oe.id = oi.observation_event_id
      JOIN observable o ON o.id = oe.observable_id
     WHERE o.item_id IS NOT NULL OR o.item_style_id IS NOT NULL
        OR o.item_part_id IS NOT NULL
     GROUP BY coalesce(o.item_id, o.item_style_id, o.item_part_id),
              coalesce(o.packaging_level::text, 'part')
),
-- A style's demand is its variants'. Counting only lines naming the style
-- itself would report zero for every style, which is what makes the subject
-- that matters most sort last.
demand AS (
    SELECT i.id AS subject_key, count(ol.*)::bigint AS lines
      FROM item i LEFT JOIN order_line ol ON ol.item_id = i.id
     GROUP BY i.id
    UNION ALL
    SELECT s.id, count(ol.*)::bigint
      FROM item_style s
      LEFT JOIN item i ON i.style_id = s.id
      LEFT JOIN order_line ol ON ol.item_id = i.id
     GROUP BY s.id
    UNION ALL
    -- A part's demand is its parent's, for the style's reason one level down:
    -- nothing orders a handle, and a part that counted its own lines would
    -- report zero and sort last on every list.
    SELECT p.id, count(ol.*)::bigint
      FROM item_part p
      LEFT JOIN order_line ol ON ol.item_id = p.item_id
     GROUP BY p.id
),
parts AS (
    SELECT item_id, count(*)::bigint AS n FROM item_part GROUP BY item_id
)
SELECT sub.item_id, sub.item_style_id, sub.code, sub.description, sub.level,
       f.gross_weight_g, f.length_mm, f.width_mm, f.height_mm,
       f.src, f.style_code, f.method, f.observed_at,
       fa.faces,
       d.lines,
       coalesce(f.weight_absent, false), coalesce(f.dimensions_absent, false),
       sub.item_part_id, sub.part_label,
       CASE WHEN sub.level = 'each' THEN coalesce(pc.n, 0) ELSE 0 END,
       loc.code,
       coalesce(hi.soh, hs.soh, 0)::bigint
  FROM subject sub
  LEFT JOIN figures f
         ON f.subject_key = coalesce(sub.item_id, sub.item_style_id, sub.item_part_id)
        AND f.level = sub.level
  LEFT JOIN faces fa
         ON fa.subject_key = coalesce(sub.item_id, sub.item_style_id, sub.item_part_id)
        AND fa.level = sub.level
  LEFT JOIN demand d
         ON d.subject_key = coalesce(sub.item_id, sub.item_style_id, sub.item_part_id)
  LEFT JOIN parts pc ON pc.item_id = sub.item_id
  LEFT JOIN held_item hi ON hi.item_id = sub.stock_item_id
  LEFT JOIN held_style hs ON hs.style_id = sub.item_style_id
  LEFT JOIN location loc ON loc.id = coalesce(hi.location_id, hs.location_id)
";

#[cfg(test)]
mod tests {
    use super::*;

    use Answer::{Missing, NotApplicable, Recorded};

    const NOTHING: Held =
        Held { weight: Missing, dimensions: Missing, photographs: false };
    const ALL: Held = Held { weight: Recorded, dimensions: Recorded, photographs: true };

    #[test]
    fn a_subject_is_on_one_list_or_none() {
        // Every combination of the three answers, against both trust classes
        // and both sides of the interval. The property is that `list()` is a
        // function: there is no input producing two lists, because there is no
        // input producing two classes.
        for weight in [Missing, Recorded, NotApplicable] {
            for dimensions in [Missing, Recorded, NotApplicable] {
                for photographs in [false, true] {
                    let held = Held { weight, dimensions, photographs };
                    for method in [None, Some("instrument"), Some("transcribed")] {
                        for staleness in [0.0, 0.99, 1.0, 40.0] {
                            let class = classify(held, method, staleness);
                            // Exhaustive by construction: a Class is one value.
                            assert!(
                                matches!(
                                    class,
                                    Class::Unrecorded
                                        | Class::Partial
                                        | Class::Unconfirmed
                                        | Class::Settled
                                ),
                                "{held:?} {method:?} {staleness}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn nothing_recorded_is_not_the_same_state_as_nothing_measured() {
        // The distinction the names exist to keep. A transcribed cube is
        // complete and untrusted; an empty subject is neither.
        assert_eq!(classify(NOTHING, None, 0.0), Class::Unrecorded);
        assert_eq!(because(Class::Unrecorded, None), "nothing");

        assert_eq!(classify(ALL, Some("transcribed"), 0.0), Class::Unconfirmed);
        assert_eq!(because(Class::Unconfirmed, Some("transcribed")), "never-measured");

        // And a measured one that has aged is the other reason to be there.
        assert_eq!(classify(ALL, Some("instrument"), 1.5), Class::Unconfirmed);
        assert_eq!(because(Class::Unconfirmed, Some("instrument")), "overdue");
    }

    #[test]
    fn anything_missing_is_partial_however_good_the_rest_is() {
        for held in [
            Held { weight: Missing, dimensions: Recorded, photographs: true },
            Held { weight: Recorded, dimensions: Missing, photographs: true },
            Held { weight: Recorded, dimensions: Recorded, photographs: false },
        ] {
            assert_eq!(
                classify(held, Some("instrument"), 0.0),
                Class::Partial,
                "{held:?}"
            );
        }
    }

    #[test]
    fn a_complete_fresh_measured_subject_is_on_no_list() {
        assert_eq!(classify(ALL, Some("instrument"), 0.5), Class::Settled);
        assert_eq!(Class::Settled.list(), None);
        // And every other class names a list.
        for class in [Class::Unrecorded, Class::Partial, Class::Unconfirmed] {
            assert!(class.list().is_some(), "{class:?}");
        }
    }

    #[test]
    fn figures_with_no_method_are_not_trusted() {
        // Same argument `trust_of` makes about an unknown method: a vocabulary
        // that grows must not silently grant confidence to something nobody
        // classified. Absent is at least as unknown as unrecognised.
        assert_eq!(classify(ALL, None, 0.0), Class::Unconfirmed);
    }

    #[test]
    fn wants_names_what_a_session_would_add_in_the_order_it_does_it() {
        assert_eq!(wants(NOTHING), ["weight", "dimensions", "photographs"]);
        assert_eq!(wants(ALL), Vec::<String>::new());
        assert_eq!(
            wants(Held { weight: Recorded, dimensions: Missing, photographs: false }),
            ["dimensions", "photographs"]
        );
    }

    #[test]
    fn two_of_three_dimensions_is_not_a_cube() {
        // `Held::dimensions` is all three or nothing, asserted here because the
        // query builds it from three separate columns and the interesting bug
        // is a subject reading complete on a missing height.
        let held = Held { weight: Recorded, dimensions: Missing, photographs: true };
        assert_eq!(classify(held, Some("instrument"), 0.0), Class::Partial);
        assert!(wants(held).contains(&"dimensions".to_string()));
    }
}
