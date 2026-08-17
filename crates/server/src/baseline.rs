//! What a thing has weighed before, and whether a fresh figure agrees with it.
//!
//! # The problem this answers
//!
//! A count and a scale are two observations of one pallet. The count is what
//! somebody believes; the weight is what the floor actually holds — and if we
//! know what one carton of this item weighs, the two can be checked against
//! each other without asking anybody to trust their memory. That check is the
//! only reason a receiver's *46* is defensible weeks later against a supplier's
//! *48*.
//!
//! Nothing here is a specification. `specification_policy` is where a declared
//! tolerance will live, and migration 35 is explicit that neither it nor its
//! resolver exists yet — which is why `observation_current.in_breach` is not a
//! column. **A baseline is evidence, not a limit.** It says what happened
//! before; it does not say what is allowed, and nothing in this module raises a
//! finding.
//!
//! # Derived, never stored
//!
//! Same argument [`crate::revalidation`] makes for staleness: a
//! `baseline_weight_g` column would be a second thing that can disagree with
//! the observations behind it, which is S44's shape. It is worked out on read.
//!
//! # The median, not the mean
//!
//! The corpus is small and hand-entered. A scale read as 18.4 and typed as 184
//! is one keystroke, and it moves a mean of ten by a factor of two — so the
//! statistic that is supposed to catch a mistyped weight would itself be
//! wrecked by one. The median is unmoved by any single figure however wrong,
//! which is the whole property wanted here.
//!
//! # Which observations count is not this module's question
//!
//! It is the query's, and the query mirrors the eligibility rules in
//! `projection_observation_current_rebuild` (migration 35) rather than stating
//! its own. This module takes the grams it is given.

/// Grams, which is the only unit an observation of mass is ever in.
///
/// `observation.value_numeric` has no unit column beside it — Principle 5 — so
/// a mass value is canonical by construction and a conversion here would be a
/// second answer to a question the writer already settled.
pub type Grams = i64;

/// The fewest weighings that make a baseline rather than a reading.
///
/// One figure is a reading. Two that disagree are a disagreement with no way to
/// say which one is wrong. Three is the fewest where a mistyped weight is
/// outvoted rather than averaged in, and it is the point of using a median at
/// all.
///
/// A placeholder with a defensible shape rather than a researched number, in
/// exactly the sense [`crate::revalidation::MEASURED_DAYS`] is one. When
/// `specification_policy` arrives this is the kind of figure it carries.
pub const ENOUGH: usize = 3;

/// Whose weighings a figure was built from.
///
/// Migration 73: one carton spec covers every size of a boot, so a code nobody
/// has weighed inherits its style's figure. **The borrowed figure says it is
/// borrowed** — the same thing D141 does for a picture taken of another size,
/// and for the same reason: a number presented as this item's own, when what
/// was put on the scale was a different size in the same box, is a fact copied
/// to where it was convenient and thereafter indistinguishable from one
/// observed there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Somebody weighed this code.
    Own,
    /// Nobody has weighed this code; these are its style's weighings.
    Style,
}

/// What a subject has weighed, and how much evidence that is.
///
/// `n` travels with `grams` and is not an implementation detail: *0.40 kg a
/// carton* means one thing across fourteen receipts and something else across
/// two, and a screen that shows the figure without the count invites the reader
/// to believe both equally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Baseline {
    pub grams: Grams,
    pub n: usize,
    pub source: Source,
}

impl Baseline {
    /// Whether there is enough behind this to put in front of an operator.
    ///
    /// The caller decides what to do about it. A thin baseline is still worth
    /// returning — the weighing queue is ranked by exactly this shortage — but
    /// it should not be rendered as though it settled anything.
    pub fn is_established(&self) -> bool {
        self.n >= ENOUGH
    }
}

/// The median of what a thing has weighed, or nothing if it never has.
///
/// **The lower of the two middles on an even count, not their mean.** The mean
/// of two middles is a figure nobody ever read off a scale, and this column is
/// whole grams, so taking it would round evidence into something that only
/// looks like a measurement. The lower middle is one of the observations. At
/// the counts this runs on the difference is a gram or two, and being able to
/// say *every figure on this screen came off a scale* is worth more than that.
///
/// Sorts in place, because the caller owns the vector it just read out of the
/// database and copying it to preserve an order nobody depends on is work for
/// nothing.
///
/// `source` is a parameter rather than something inferred, so that the two call
/// sites have to say which pile of weighings they handed over.
pub fn from_weights(weights: &mut [Grams], source: Source) -> Option<Baseline> {
    if weights.is_empty() {
        return None;
    }
    weights.sort_unstable();
    // `(len - 1) / 2` is the middle of an odd count and the lower of the two
    // middles of an even one: index 2 of five, index 1 of four.
    let middle = (weights.len() - 1) / 2;
    Some(Baseline {
        grams: weights[middle],
        n: weights.len(),
        source,
    })
}

/// Most specific wins: this code's own weighings, or failing that its style's.
///
/// Migration 73 states the rule and the reason — *"an observation against the
/// SKU beats an observation against its style, because somebody measured
/// **this** one"* — and it is D22's lattice resolution at one level, which is
/// why it needs no closure table.
///
/// **The two piles are never pooled.** Merging them would add a style's
/// thirteen weighings to a SKU's one and report fourteen, which is the lie
/// migration 73 was written to prevent: *"four rows claiming to be measured
/// when one carton was put on a scale."* One weighing of this code is better
/// evidence about this code than thirteen of its siblings, and it is also only
/// one weighing; both of those stay true only if the piles stay apart.
pub fn resolve(own: Option<Baseline>, style: Option<Baseline>) -> Option<Baseline> {
    own.or(style)
}

/// How many of something a weight accounts for.
///
/// *18.4 kg is 46 cartons* — the arithmetic behind the receive screen's third
/// witness. Rounded to nearest rather than truncated: 45.6 cartons is a
/// disagreement about the last carton, and reporting 45 would turn it into a
/// disagreement about the count.
///
/// `None` when there is no per-unit figure to divide by. A zero or negative
/// baseline is not a thin baseline, it is a broken one, and dividing by it
/// would produce a number the screen has no business showing.
pub fn implied_units(weighed: Grams, per_unit: Grams) -> Option<i64> {
    if per_unit <= 0 || weighed < 0 {
        return None;
    }
    // Rounded to nearest, in integers: add half the divisor before dividing.
    Some((weighed + per_unit / 2) / per_unit)
}

/// The gap between what was weighed and what was expected.
///
/// Both figures, because they answer different questions: *104.9 against 104.6*
/// is three hundred grams, and whether three hundred grams matters depends
/// entirely on whether the pallet is a hundred kilos or two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delta {
    /// Observed minus expected. Negative means light.
    pub grams: i64,
    /// The same gap against the expected figure, in parts per thousand.
    ///
    /// Per mille rather than a percentage with a decimal point, because a
    /// percentage fine enough to be useful here needs one and this is integer
    /// arithmetic all the way down. `None` when the expected figure is zero and
    /// there is no proportion to take.
    pub per_mille: Option<i32>,
}

/// Compare a fresh reading against what was expected.
pub fn delta(observed: Grams, expected: Grams) -> Delta {
    let grams = observed.saturating_sub(expected);
    let per_mille = if expected == 0 {
        None
    } else {
        // i128 so the multiply cannot overflow before the divide brings it back
        // into range. A pallet in grams times a thousand is comfortably inside
        // i64, but the type should not be the reason that is true.
        let scaled = (grams as i128 * 1000) / expected as i128;
        Some(scaled.clamp(i32::MIN as i128, i32::MAX as i128) as i32)
    };
    Delta { grams, per_mille }
}

/// One line of a pallet: how many, and what one of them weighs.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub quantity: i64,
    pub each: Baseline,
}

/// What a built pallet should weigh, and how thin the evidence behind it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Expected {
    pub grams: Grams,
    /// **The fewest weighings behind any line, not the total across them.**
    ///
    /// Adding them up would let two well-measured lines vouch for a third
    /// nobody has ever weighed, and the third is the one that makes the figure
    /// wrong. The weakest line is what the whole sum is worth — the same
    /// reason `receiving_list` stops its packaging chain at the first missing
    /// rung rather than skipping it.
    pub n: usize,
    /// Whether any line's figure came from a style rather than the code itself.
    ///
    /// The same weakest-link reading as `n`: one borrowed line makes the sum a
    /// partly borrowed figure, and a screen that showed it as measured would be
    /// making exactly the claim [`Source`] exists to keep it from making.
    pub borrowed: bool,
}

impl Expected {
    /// Whether the thinnest line behind this sum clears [`ENOUGH`].
    ///
    /// Here rather than at the call site for the reason [`Baseline`] has the
    /// same method: "is this enough" answered in two places is two answers, and
    /// the one an operator sees would depend on which screen they opened.
    pub fn is_established(&self) -> bool {
        self.n >= ENOUGH
    }
}

/// Sum a pallet's lines, plus whatever the empty packaging weighs.
///
/// `tare` is the package type's own weight — migration 9's `tare_weight_g` on
/// the preset. Without it the comparison is out by a pallet board on every
/// pallet, in the same direction, which is the kind of constant error that gets
/// absorbed into a tolerance and hides everything else.
///
/// `None` when there are no lines: a pallet with nothing on it has no expected
/// weight, and returning the bare tare would state that an empty pallet is the
/// answer to a question about a full one.
pub fn expected(lines: &[Line], tare: Grams) -> Option<Expected> {
    if lines.is_empty() {
        return None;
    }
    let mut grams = tare as i128;
    let mut n = usize::MAX;
    let mut borrowed = false;
    for line in lines {
        grams += line.quantity as i128 * line.each.grams as i128;
        n = n.min(line.each.n);
        borrowed |= line.each.source == Source::Style;
    }
    Some(Expected {
        grams: grams.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
        n,
        borrowed,
    })
}

/// A baseline as it goes over the wire.
///
/// **One shape, because two screens ask the same question.** The bench wants
/// what a carton should weigh and the dock wants what one carton weighs; both
/// need the figure, how many weighings are behind it, whether any of them
/// belong to a style rather than this code, and whether that is enough. Two
/// structs carrying those four fields is the drift this codebase's register is
/// mostly made of — `bench::ExpectedWeight` flattens this and adds the gap
/// against the scale, which is the only thing it has that the dock does not.
///
/// `established` is computed here rather than left to the client for the reason
/// [`Baseline::is_established`] gives: two renderers answering *is this enough*
/// separately would eventually answer it differently.
#[derive(serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeightBaseline {
    pub grams: Grams,
    pub n: usize,
    pub borrowed: bool,
    pub established: bool,
}

impl From<Baseline> for WeightBaseline {
    fn from(b: Baseline) -> Self {
        WeightBaseline {
            grams: b.grams,
            n: b.n,
            borrowed: b.source == Source::Style,
            established: b.is_established(),
        }
    }
}

impl From<Expected> for WeightBaseline {
    fn from(e: Expected) -> Self {
        WeightBaseline {
            grams: e.grams,
            n: e.n,
            borrowed: e.borrowed,
            established: e.is_established(),
        }
    }
}

// ---------------------------------------------------------------------------
// The read
// ---------------------------------------------------------------------------
//
// Everything above this line is arithmetic and is tested without a database.
// Everything below it is I/O and has no judgement in it — the same split
// `prepack`, `receiving` and `observing` make, and it is why the tests at the
// bottom of this file need no `DATABASE_URL`.
//
// It lives in this file rather than in `bench` because the pack bench and the
// receive screen both ask this question, and `bench`'s own header says why that
// matters: two modules holding one query is two answers to one fact, free to
// disagree.

use std::collections::HashMap;

use actix_web::web;
use uuid::Uuid;

use crate::auth::Caller;
use crate::error::ApiError;
use crate::revalidation::MEASURED_METHODS;
use crate::tenancy::TenantScope;
use crate::AppState;

/// What each of these items has weighed, at one packaging level.
///
/// **One query for the whole pallet, not one per line.** D10's batch-loading
/// argument, and the packing station is the hot path open decision 20 names
/// when it applies that argument to measurements.
///
/// Items with nothing eligible are absent from the map rather than present with
/// a zero — a thing nobody has weighed has no baseline, and a zero is a figure.
///
/// # Which observations are eligible
///
/// The first two rules are copied from `projection_observation_current_rebuild`
/// in migration 35, deliberately and with its comments, because they are the
/// same question — *what counts as a value* — and this codebase's register is
/// mostly entries about two places answering that separately. They are meant to
/// agree; if that maintainer's rules change, these change with them.
///
/// The third rule there — a counterparty's observation needs an acceptance — is
/// moot here and stronger: this takes **ours only, off an instrument.** A
/// supplier's asserted weight is not an independent witness to our count, it is
/// the same paperwork the count is being checked against. That is D23's own
/// example, which the migration quotes: *"never trust their weight over our
/// scale."*
///
/// There is no recency window. A weight that has gone stale is
/// [`crate::revalidation`]'s question and it already answers it; taking a
/// second view of the same thing here is the drift this file exists not to add.
///
/// # Both arms, in one trip
///
/// Migration 73 put carton specs on the style, so the observations that answer
/// *what does one carton weigh* are mostly not against the SKU at all. Reading
/// only the item arm returns nothing for exactly the catalogue this was built
/// for — 58 measured styles standing for 252 sellable codes — so both arms are
/// fetched and [`resolve`] picks per item.
///
/// # A case pack is part of the subject, not a detail of it
///
/// `observable_item_idx` is keyed `(item_id, packaging_level,
/// item_packing_config_id)`, and migration 7 says why: *"a carton is only a
/// definite physical object relative to a case pack, and because
/// `item_packing_config` is versioned, a corrected case pack cannot silently
/// rewrite the dimensions of cartons shipped last year."*
///
/// So the caller names the config it is counting against — `goods_receipt_line`
/// and the bench both hold one — and weighings of a **different** case pack are
/// not this subject's weighings. Pooling them would put a carton of twelve and
/// a carton of twenty-four in one median and report the count as though every
/// figure were about the same box. `each` carries no config by constraint, so
/// `None` there is the ordinary case rather than a missing argument.
pub async fn for_items(
    state: &web::Data<AppState>,
    who: &Caller,
    // Each item, with the case pack the caller is counting against.
    items: &[(Uuid, Option<Uuid>)],
    level: &str,
) -> Result<HashMap<Uuid, Baseline>, ApiError> {
    if items.is_empty() {
        return Ok(HashMap::new());
    }
    let item_ids: Vec<Uuid> = items.iter().map(|(i, _)| *i).collect();
    let ids = item_ids.clone();
    let level = level.to_owned();
    let methods: Vec<String> = MEASURED_METHODS.iter().map(|m| (*m).to_owned()).collect();

    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let (styles, rows) = scope
        .run(move |tx| {
            Box::pin(async move {
                // Which of these codes belongs to a style. Nullable and usually
                // null — migration 73: "most items are not part of a style".
                let styles = tx
                    .query(
                        "SELECT id, style_id FROM item
                          WHERE id = ANY($1) AND style_id IS NOT NULL",
                        &[&ids],
                    )
                    .await?;
                let style_ids: Vec<Uuid> = styles.iter().map(|r| r.get(1)).collect();

                let rows = tx
                    .query(
                        "SELECT ob.item_id, ob.item_style_id,
                                ob.item_packing_config_id, o.value_numeric
                           FROM observation o
                           JOIN observation_event e ON e.id = o.observation_event_id
                           JOIN observable ob ON ob.id = o.observable_id
                           JOIN metric m ON m.id = o.metric_id
                          WHERE m.code = 'gross_weight'
                            AND (ob.item_id = ANY($1) OR ob.item_style_id = ANY($4))
                            AND ob.packaging_level::text = $2
                            AND o.value_numeric IS NOT NULL
                            AND o.absent_reason IS NULL
                            -- ours, and off an instrument
                            AND e.asserted_by_party_id IS NULL
                            AND e.method::text = ANY($3)
                            -- (1) a retraction is not a value, and neither is
                            -- what it retracts
                            AND o.retracts_observation_id IS NULL
                            AND NOT EXISTS (SELECT 1 FROM observation r
                                             WHERE r.retracts_observation_id = o.id)
                            -- (2) a corrected row loses to its correction
                            AND NOT EXISTS (SELECT 1 FROM observation c
                                             WHERE c.corrects_observation_id = o.id)",
                        &[&item_ids, &level, &methods, &style_ids],
                    )
                    .await?;
                Ok((styles, rows))
            })
        })
        .await?;

    let style_of: HashMap<Uuid, Uuid> = styles.iter().map(|r| (r.get(0), r.get(1))).collect();

    // Keyed on the case pack as well as the subject, so two eras of one code
    // never land in one median.
    type Key = (Uuid, Option<Uuid>);
    let mut own: HashMap<Key, Vec<Grams>> = HashMap::new();
    let mut by_style: HashMap<Key, Vec<Grams>> = HashMap::new();
    for r in &rows {
        let config: Option<Uuid> = r.get(2);
        let grams: Grams = r.get(3);
        match (r.get::<_, Option<Uuid>>(0), r.get::<_, Option<Uuid>>(1)) {
            (Some(item), _) => own.entry((item, config)).or_default().push(grams),
            (_, Some(style)) => by_style.entry((style, config)).or_default().push(grams),
            _ => {}
        }
    }

    let mut out = HashMap::new();
    for (item, config) in items {
        let mine = own
            .get_mut(&(*item, *config))
            .and_then(|w| from_weights(w, Source::Own));

        // The style arm's config is the style's own, and migration 73's whole
        // premise is that it is *one* spec covering every size — so it need not
        // be the config this SKU was counted under, and requiring a match would
        // discard the only weighings that exist. Where a style carries more than
        // one, though, there is no rule here that picks between them: that is a
        // re-versioned carton spec, and guessing which era applies is the error
        // this key was added to prevent. Nothing is returned, and the SKU reads
        // as never weighed until somebody weighs it or names the spec.
        let theirs = style_of.get(item).and_then(|s| {
            let mut found = by_style.iter_mut().filter(|((st, _), _)| st == s);
            match (found.next(), found.next()) {
                (Some((_, w)), None) => from_weights(w, Source::Style),
                _ => None,
            }
        });

        if let Some(b) = resolve(mine, theirs) {
            out.insert(*item, b);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thing_never_weighed_has_no_baseline() {
        assert_eq!(from_weights(&mut [], Source::Own), None);
    }

    #[test]
    fn one_mistyped_weight_does_not_move_the_figure() {
        // A scale read as 400 g and typed as 4000 is one missed decimal point.
        // The mean of these is 1100; the median is the figure four of the five
        // scales agree on, which is the entire reason this is not a mean.
        let mut weights = [400, 398, 4000, 401, 399];
        let got = from_weights(&mut weights, Source::Own).unwrap();
        assert_eq!(got.grams, 400);
        assert_eq!(got.n, 5);
    }

    #[test]
    fn an_even_count_takes_the_lower_middle_rather_than_inventing_a_figure() {
        let mut weights = [400, 402, 404, 406];
        // Not 403, which no scale ever read.
        assert_eq!(from_weights(&mut weights, Source::Own).unwrap().grams, 402);
    }

    #[test]
    fn a_baseline_is_not_established_until_a_mistyped_weight_can_be_outvoted() {
        let mut two = [400, 402];
        assert!(!from_weights(&mut two, Source::Own).unwrap().is_established());
        let mut three = [400, 402, 404];
        assert!(from_weights(&mut three, Source::Own).unwrap().is_established());
    }

    #[test]
    fn a_code_somebody_weighed_beats_its_styles_thirteen_siblings() {
        let mine = from_weights(&mut [402], Source::Own);
        let theirs = from_weights(&mut [380, 381, 379, 382], Source::Style);
        let got = resolve(mine, theirs).unwrap();
        assert_eq!(got.grams, 402);
        assert_eq!(got.source, Source::Own);
        // And it is still only one weighing. Most specific wins does not make
        // the evidence thicker.
        assert_eq!(got.n, 1);
        assert!(!got.is_established());
    }

    #[test]
    fn a_code_nobody_weighed_inherits_and_says_so() {
        let got = resolve(None, from_weights(&mut [380, 381, 379], Source::Style)).unwrap();
        assert_eq!(got.grams, 380);
        assert_eq!(got.source, Source::Style);
        assert_eq!(got.n, 3);
    }

    #[test]
    fn a_code_with_no_weighings_and_no_style_has_nothing() {
        assert_eq!(resolve(None, None), None);
    }

    #[test]
    fn the_scale_agrees_with_the_count() {
        // The receive artboard's figures: 18.4 kg over a 400 g carton.
        assert_eq!(implied_units(18_400, 400), Some(46));
    }

    #[test]
    fn a_part_carton_rounds_to_the_nearer_whole_one() {
        // 45.6 cartons is a disagreement about one carton, not about six.
        assert_eq!(implied_units(18_240, 400), Some(46));
        assert_eq!(implied_units(18_000, 400), Some(45));
    }

    #[test]
    fn a_broken_per_unit_figure_yields_no_count_at_all() {
        assert_eq!(implied_units(18_400, 0), None);
        assert_eq!(implied_units(18_400, -400), None);
    }

    #[test]
    fn the_gap_is_reported_both_ways() {
        // The bench artboard: 104.9 weighed against 104.6 expected.
        let d = delta(104_900, 104_600);
        assert_eq!(d.grams, 300);
        assert_eq!(d.per_mille, Some(2));
    }

    #[test]
    fn a_light_pallet_reads_negative() {
        let d = delta(214, 268);
        assert_eq!(d.grams, -54);
        // The findings panel's -20.1 %, to the nearest part per thousand.
        assert_eq!(d.per_mille, Some(-201));
    }

    #[test]
    fn nothing_to_compare_against_is_not_a_proportion() {
        assert_eq!(delta(400, 0).per_mille, None);
    }

    #[test]
    fn a_pallet_is_its_lines_plus_the_pallet_itself() {
        let lines = [
            Line { quantity: 6, each: Baseline { grams: 400, n: 14, source: Source::Own } },
            Line { quantity: 6, each: Baseline { grams: 380, n: 9, source: Source::Own } },
            Line { quantity: 4, each: Baseline { grams: 5_000, n: 41, source: Source::Own } },
        ];
        // 2400 + 2280 + 20000, on a 25 kg pallet.
        let got = expected(&lines, 25_000).unwrap();
        assert_eq!(got.grams, 49_680);
    }

    #[test]
    fn one_borrowed_line_makes_the_whole_figure_borrowed() {
        let lines = [
            Line { quantity: 6, each: Baseline { grams: 400, n: 9, source: Source::Own } },
            Line { quantity: 4, each: Baseline { grams: 380, n: 9, source: Source::Style } },
        ];
        assert!(expected(&lines, 0).unwrap().borrowed);

        let mine = [Line { quantity: 6, each: Baseline { grams: 400, n: 9, source: Source::Own } }];
        assert!(!expected(&mine, 0).unwrap().borrowed);
    }

    #[test]
    fn the_thinnest_line_is_what_the_whole_figure_is_worth() {
        let lines = [
            Line { quantity: 6, each: Baseline { grams: 400, n: 41, source: Source::Own } },
            Line { quantity: 1, each: Baseline { grams: 380, n: 1, source: Source::Own } },
        ];
        // Not 42, and not 21. One line has been weighed once and it is on the
        // pallet, so the sum is worth one weighing.
        assert_eq!(expected(&lines, 0).unwrap().n, 1);
    }

    #[test]
    fn an_empty_pallet_is_not_an_answer_about_a_full_one() {
        assert_eq!(expected(&[], 25_000), None);
    }
}
