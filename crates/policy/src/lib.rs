//! The compiled policy registry: which dimension outranks which, per kind.
//!
//! D22 resolves a policy by matching every binding whose non-null dimensions are
//! at-or-above the request's node on each axis, then ordering the matches by
//! their **depth vector, compared lexicographically in a precedence order
//! declared per kind**. This module is that declaration.
//!
//! # Why the orderings are here and justified rather than merely declared
//!
//! Question 93 (and 79, its duplicate) has been open since D22 was adopted:
//!
//! > *"Eleven orderings of six dimensions, declared in a Rust const. Counterparty
//! > over product for shelf life, product over space for putaway — both
//! > defensible, neither obvious, and a manager who assumes wrong misconfigures
//! > confidently. Each needs a written justification, not just a declaration."*
//!
//! The research note calls it *"the highest-leverage undocumented number in the
//! system"*, and it is right for a reason worth stating: **the ordering is
//! invisible in the UI and decides the answer.** Two managers can configure the
//! same intent and get different stock shipped, and neither will see why. So each
//! ordering below carries the argument for it, and the argument is the deliverable
//! — the `const` was always going to exist.
//!
//! # The rule that generates them
//!
//! An ordering is not a ranking of how *important* the dimensions are. It answers
//! one question:
//!
//! > **When two people have configured this kind at different depths on different
//! > axes, whose statement was more likely meant as an override?**
//!
//! From which:
//!
//! - **Commercial terms belong to the counterparty.** Anything a customer or
//!   supplier negotiates — shelf life on arrival, delivery tolerance, receiving
//!   discipline — is a promise to a named party, and a promise beats a default.
//! - **Physical handling belongs to the product.** Where a thing may be stored,
//!   how it is sampled, how tightly it is counted, follow from what it is.
//! - **Measurement rules belong to the metric.** A statement about temperature is
//!   a statement about temperature, whatever it is measured on.
//! - **Space and Ownership are filters far more often than they are authors.** A
//!   site rarely means "and I intend to override the customer's contract"; it
//!   usually means "here is the default where nothing else applies". They sit last
//!   almost everywhere, and where they do not, the entry says why.
//!
//! **Tenancy is not declarable and is index 0 of every ordering.** D22: without
//! it, a per-kind order ranking Product above Tenancy lets a platform-shipped
//! default outrank a tenant's own configuration — *"a correctness hole, not a
//! support surface."* S15 asserts it.
//!
//! # What is deliberately not decided here
//!
//! **Ownership sits last in all eleven.** For a third-party logistics client the
//! stock's owner may well be an author rather than a filter — their goods, their
//! rules — and that is questions 65 and 81, which are open. When they settle,
//! some of these orderings will move, and the entry for each says whether it is a
//! candidate.

use std::fmt;

/// The version of *what resolution means*.
///
/// Not a release number. It moves when an answer legitimately changes: a
/// precedence order is re-argued, a clamp direction flips, a specificity constant
/// is redefined, a dimension is added to the lattice.
///
/// **J22 is the reason it exists.** Everything that decides which stock ships now
/// runs through code whose inputs keep moving — D81's eleven orderings, D83's
/// clamp directions, the depth constants, the taxonomy the closures fold from —
/// and a quiet change to any of them changes shipped goods with nothing failing.
/// Every other guard in this repository is a rule about structure; the golden
/// snapshot is the only one that watches *meaning*, and this constant is how a
/// change to meaning is declared rather than discovered.
///
/// Bumping it is the explicit act. It does not make the snapshot pass — it makes
/// the suite demand a regenerated one, so the diff is read by whoever changed the
/// behaviour rather than by whoever next runs the tests.
pub const RESOLVER_VERSION: u32 = 1;

/// The six axes of D22's lattice, in the order they are declared on a binding.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Dimension {
    /// Not declarable. Index 0 of every ordering, always.
    Tenancy,
    Product,
    Counterparty,
    Space,
    Ownership,
    Metric,
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Dimension::Tenancy => "tenancy",
            Dimension::Product => "product",
            Dimension::Counterparty => "counterparty",
            Dimension::Space => "space",
            Dimension::Ownership => "ownership",
            Dimension::Metric => "metric",
        };
        f.write_str(s)
    }
}

use Dimension::{Counterparty, Metric, Ownership, Product, Space, Tenancy};

/// D22's eleven kinds. Three have value tables; the rest are declared and unbuilt,
/// which S13 reports as a count rather than a violation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum PolicyKind {
    Allocation,
    Putaway,
    Receiving,
    OrderTolerance,
    CountTolerance,
    ShelfLife,
    Sampling,
    CycleCount,
    Specification,
    ObservationPrecedence,
    ObservationAcceptance,
}

pub const ALL: &[PolicyKind] = &[
    PolicyKind::Allocation,
    PolicyKind::Putaway,
    PolicyKind::Receiving,
    PolicyKind::OrderTolerance,
    PolicyKind::CountTolerance,
    PolicyKind::ShelfLife,
    PolicyKind::Sampling,
    PolicyKind::CycleCount,
    PolicyKind::Specification,
    PolicyKind::ObservationPrecedence,
    PolicyKind::ObservationAcceptance,
];

impl PolicyKind {
    /// The `policy_kind` enum label, which is also the `%_policy` table prefix.
    pub fn as_str(self) -> &'static str {
        match self {
            PolicyKind::Allocation => "allocation",
            PolicyKind::Putaway => "putaway",
            PolicyKind::Receiving => "receiving",
            PolicyKind::OrderTolerance => "order_tolerance",
            PolicyKind::CountTolerance => "count_tolerance",
            PolicyKind::ShelfLife => "shelf_life",
            PolicyKind::Sampling => "sampling",
            PolicyKind::CycleCount => "cycle_count",
            PolicyKind::Specification => "specification",
            PolicyKind::ObservationPrecedence => "observation_precedence",
            PolicyKind::ObservationAcceptance => "observation_acceptance",
        }
    }

    /// The precedence order for this kind, most significant first.
    ///
    /// Each arm's justification is on [`PolicyKind::rationale`], beside it, so a
    /// reader who changes an ordering has to walk past the argument for the one
    /// they are replacing.
    pub fn dimensions(self) -> &'static [Dimension] {
        match self {
            PolicyKind::Allocation => {
                &[Tenancy, Counterparty, Product, Space, Ownership, Metric]
            }
            PolicyKind::Putaway => &[Tenancy, Product, Space, Counterparty, Ownership, Metric],
            PolicyKind::Receiving => {
                &[Tenancy, Counterparty, Product, Space, Ownership, Metric]
            }
            PolicyKind::OrderTolerance => {
                &[Tenancy, Counterparty, Product, Space, Ownership, Metric]
            }
            PolicyKind::CountTolerance => {
                &[Tenancy, Product, Space, Counterparty, Ownership, Metric]
            }
            PolicyKind::ShelfLife => {
                &[Tenancy, Counterparty, Product, Space, Ownership, Metric]
            }
            PolicyKind::Sampling => &[Tenancy, Product, Counterparty, Space, Ownership, Metric],
            PolicyKind::CycleCount => {
                &[Tenancy, Product, Space, Counterparty, Ownership, Metric]
            }
            PolicyKind::Specification => {
                &[Tenancy, Metric, Product, Counterparty, Space, Ownership]
            }
            PolicyKind::ObservationPrecedence => {
                &[Tenancy, Metric, Counterparty, Product, Space, Ownership]
            }
            PolicyKind::ObservationAcceptance => {
                &[Tenancy, Counterparty, Metric, Product, Space, Ownership]
            }
        }
    }

    /// Why this kind is ordered the way it is, and what would change it.
    ///
    /// Question 93 asked for exactly this: *"each needs a written justification,
    /// not just a declaration."* It is a method rather than a comment so the
    /// explain UI can show a manager the reasoning next to the answer — which is
    /// what 79 means by *"the explain UI must ship with the resolver, not after
    /// it."*
    pub fn rationale(self) -> &'static str {
        match self {
            PolicyKind::Allocation =>
                "Counterparty first. Which stock may be picked for a demand is mostly a promise to a \
                 named customer -- no mixed lots, minimum life on arrival, this brand only -- and a \
                 promise beats a product default. Product is second because the rest of allocation is \
                 about what the goods are. Ownership last today and the first candidate to move: for a \
                 3PL client the owner authors the rule, which is questions 65 and 81.",
            PolicyKind::Putaway =>
                "Product over Space, which is one of the two orderings the record already asserts. \
                 Where a thing may be put follows from what it is -- temperature, hazard, weight, \
                 velocity -- and the site constrains what is available rather than what is suitable. \
                 A zone rule that beat a product rule would let a layout change silently relax a \
                 handling requirement.",
            PolicyKind::Receiving =>
                "Counterparty first, because receiving discipline is a statement about a supplier: \
                 this one short-ships, count every pallet; that one is trusted, sample. Product second \
                 for what the goods demand on arrival. A site cannot know a supplier is unreliable, \
                 which is why Space is third rather than second.",
            PolicyKind::OrderTolerance =>
                "Counterparty first. An over- or under-delivery tolerance is a commercial term, agreed \
                 with one party and quoted back in a dispute, so it must not be overridden by a product \
                 default. This is the same argument as ShelfLife and the two should move together.",
            PolicyKind::CountTolerance =>
                "Product first, and this one is deliberately not Counterparty. How much variance a \
                 count may show is a property of the goods -- countability, unit value, whether they \
                 are weighed or scanned -- and stock being counted is ours whoever sold it to us. \
                 Space is second because a bulk zone and a pick face are counted differently. \
                 Counterparty is nearly inert here and is third only to keep it declarable.",
            PolicyKind::ShelfLife =>
                "Counterparty over Product, the other ordering the record already asserts. A customer \
                 requiring ninety days remaining means it for every line they buy, and a product's \
                 default life is what applies when nobody has negotiated. Reversing this would let a \
                 catalogue edit quietly break a contract.",
            PolicyKind::Sampling =>
                "Product first, because a sampling regime follows the hazard: an allergen or a chilled \
                 line is sampled on what it is, not on who sent it. Counterparty second and close \
                 behind -- supplier risk is the standard modifier, and a new supplier is sampled \
                 harder -- which is why this is the one ordering where the top two are genuinely \
                 arguable and the argument is recorded rather than settled by taste.",
            PolicyKind::CycleCount =>
                "Product first for value and velocity, Space second for how the location behaves. \
                 Counting cadence is an internal discipline; no counterparty has a view on it, so \
                 Counterparty sits below both.",
            PolicyKind::Specification =>
                "Metric first, and this is the ordering most likely to surprise. A specification is a \
                 statement about one attribute -- two to eight degrees, under fifteen percent moisture \
                 -- so a binding that names the attribute is speaking about the thing being specified, \
                 and one that does not is a default for everything measurable. Product second, because \
                 the attribute is a property of the goods. A statutory limit a customer must not be \
                 able to loosen is not modelled by precedence at all: it is a platform-shipped clamp, \
                 which D22 provides.",
            PolicyKind::ObservationPrecedence =>
                "Metric first, straight from D23's own example: 'trust supplier dimensions for items \
                 we have never measured, but never trust their weight over our scale.' That is a rule \
                 about which metric, and it cannot be expressed if Counterparty outranks Metric. \
                 Counterparty second, because the rest of the sentence is about whose number it is. \
                 D78's maintainer applies a default in place of this until the kind is built.",
            PolicyKind::ObservationAcceptance =>
                "Counterparty first, inverting the kind above it, and the inversion is the point. \
                 Whether a declared value needs a human to accept it is a question about trust in a \
                 party -- this supplier's weights go straight in, that one's are checked -- where \
                 which value wins once accepted is a question about the metric.",
        }
    }
}

/// Every ordering names each dimension exactly once, and starts with Tenancy.
///
/// S15 asserts the second half against the compiled registry. The first half is
/// asserted here, because a permutation that dropped an axis would make some
/// bindings unorderable and the failure would look like a tie rather than like a
/// missing declaration.
pub fn ordering_is_well_formed(kind: PolicyKind) -> Result<(), String> {
    let dims = kind.dimensions();
    if dims.first() != Some(&Tenancy) {
        return Err(format!(
            "{}: tenancy is not index 0, so a platform default could outrank a tenant's own \
             configuration",
            kind.as_str()
        ));
    }
    let mut seen: Vec<Dimension> = dims.to_vec();
    seen.sort();
    seen.dedup();
    if seen.len() != dims.len() {
        return Err(format!("{}: a dimension is named twice", kind.as_str()));
    }
    if seen.len() != 6 {
        return Err(format!(
            "{}: names {} dimensions, not six, so some bindings cannot be ordered",
            kind.as_str(),
            seen.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_declares_a_well_formed_ordering() {
        for &k in ALL {
            ordering_is_well_formed(k).unwrap_or_else(|e| panic!("{e}"));
        }
    }

    #[test]
    fn every_kind_justifies_its_ordering() {
        // Question 93 is about the justification, not the const. An entry that
        // declares an order and says nothing is the state this settles.
        for &k in ALL {
            let why = k.rationale();
            assert!(
                why.len() > 120,
                "{} has no real justification for its ordering",
                k.as_str()
            );
        }
    }

    #[test]
    fn the_two_orderings_the_record_already_asserts_are_the_ones_declared() {
        // "Counterparty over product for shelf life, product over space for
        // putaway" -- mechanism-design.md, and question 93 quoting it. If either
        // ever flips, it flips here and against the sentence that named it.
        let sl = PolicyKind::ShelfLife.dimensions();
        let pos = |d: Dimension, v: &[Dimension]| v.iter().position(|x| *x == d).unwrap();
        assert!(pos(Counterparty, sl) < pos(Product, sl));
        let pu = PolicyKind::Putaway.dimensions();
        assert!(pos(Product, pu) < pos(Space, pu));
    }
}

/// One binding's depth vector, as `policy_candidate` returns it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Candidate {
    /// Compared only to break ties deterministically, never to rank specificity.
    pub binding: u128,
    pub tenancy: i32,
    pub product: i32,
    pub counterparty: i32,
    pub space: i32,
    pub ownership: i32,
    pub metric: i32,
}

impl Candidate {
    fn on(&self, d: Dimension) -> i32 {
        match d {
            Dimension::Tenancy => self.tenancy,
            Dimension::Product => self.product,
            Dimension::Counterparty => self.counterparty,
            Dimension::Space => self.space,
            Dimension::Ownership => self.ownership,
            Dimension::Metric => self.metric,
        }
    }
}

/// The winner, and enough to explain it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub winner: Candidate,
    /// The dimension that decided it, and against whom. `None` when there was
    /// only one candidate — nothing decided anything.
    pub decided_on: Option<Dimension>,
    pub runner_up: Option<Candidate>,
    /// **Equal vectors.** D22 makes these structurally impossible within a
    /// dimension, so this is defence in depth rather than an expected branch: the
    /// floor takes the lower binding id and never stops, and the caller raises
    /// `discrepancy.kind = 'policy_ambiguous'`.
    pub ambiguous_with: Vec<Candidate>,
}

/// Order the candidates by depth vector, lexicographically, in the kind's
/// declared precedence order.
///
/// D22: *"specificity is a vector, not a number. Collapsing a componentwise
/// comparison to one integer lets a large count in a low-weight component beat a
/// small count in a high-weight one. CSS is twenty years of proof; there is no
/// `specificity` column."*
///
/// Returns `None` when nothing matched, which is a real answer: no policy applies
/// and the caller falls back to whatever it does with no policy — never to a
/// guess made here.
pub fn resolve(kind: PolicyKind, candidates: &[Candidate]) -> Option<Resolution> {
    let dims = kind.dimensions();
    let better = |a: &Candidate, b: &Candidate| -> std::cmp::Ordering {
        for &d in dims {
            match b.on(d).cmp(&a.on(d)) {
                std::cmp::Ordering::Equal => continue,
                other => return other,
            }
        }
        // Equal on every declared axis. Lower id wins so the floor is
        // deterministic; the caller is told it happened.
        a.binding.cmp(&b.binding)
    };

    let mut ranked: Vec<Candidate> = candidates.to_vec();
    ranked.sort_by(better);
    let winner = *ranked.first()?;

    let runner_up = ranked.get(1).copied();
    let decided_on = runner_up.and_then(|r| dims.iter().find(|&&d| winner.on(d) != r.on(d)).copied());
    let ambiguous_with = ranked
        .iter()
        .skip(1)
        .filter(|c| dims.iter().all(|&d| c.on(d) == winner.on(d)))
        .copied()
        .collect();

    Some(Resolution { winner, decided_on, runner_up, ambiguous_with })
}

/// A sentence a manager can read, beside the answer.
///
/// Question 79: *"the explain UI must ship **with** the resolver, not after it."*
/// Shipping the resolver without this is how the ordering stays invisible, which
/// is the whole failure 93 describes.
pub fn explain(kind: PolicyKind, r: &Resolution) -> String {
    match (r.decided_on, r.runner_up) {
        _ if !r.ambiguous_with.is_empty() => format!(
            "{} bindings are equally specific for {}; the lowest id was taken and this is a \
             discrepancy, not a decision",
            r.ambiguous_with.len() + 1,
            kind.as_str()
        ),
        (Some(d), Some(_)) => format!(
            "won on {d}, which outranks the rest for {} because {}",
            kind.as_str(),
            kind.rationale()
        ),
        _ => format!("the only binding matching this request for {}", kind.as_str()),
    }
}

#[cfg(test)]
mod resolve_tests {
    use super::*;

    fn c(binding: u128, tenancy: i32, product: i32, counterparty: i32) -> Candidate {
        Candidate { binding, tenancy, product, counterparty, space: 0, ownership: 0, metric: 0 }
    }

    #[test]
    fn a_tenant_binding_always_beats_a_platform_one() {
        // The correctness hole S15 exists to prevent, exercised rather than
        // assumed: the platform row is more specific on Product and still loses.
        let platform = c(1, 0, 1000, 0);
        let tenant = c(2, 1, 0, 0);
        for &k in ALL {
            let r = resolve(k, &[platform, tenant]).unwrap();
            assert_eq!(r.winner, tenant, "{} let a platform default win", k.as_str());
        }
    }

    #[test]
    fn shelf_life_takes_the_customer_over_the_product() {
        // D81's ordering, as behaviour. A customer's contract beats a catalogue
        // default even when the catalogue names the exact item.
        let by_item = c(1, 1, 1000, 0);
        let by_customer = c(2, 1, 0, 1);
        let r = resolve(PolicyKind::ShelfLife, &[by_item, by_customer]).unwrap();
        assert_eq!(r.winner, by_customer);
        assert_eq!(r.decided_on, Some(Dimension::Counterparty));
    }

    #[test]
    fn count_tolerance_takes_the_product_over_the_customer() {
        // The same two bindings, a different kind, the opposite answer -- which
        // is the whole reason the ordering is per-kind and question 93 mattered.
        let by_item = c(1, 1, 1000, 0);
        let by_customer = c(2, 1, 0, 1);
        let r = resolve(PolicyKind::CountTolerance, &[by_item, by_customer]).unwrap();
        assert_eq!(r.winner, by_item);
        assert_eq!(r.decided_on, Some(Dimension::Product));
    }

    #[test]
    fn a_vector_is_not_a_number() {
        // D22's argument, made concrete: a big count on a low-weight axis must
        // not beat a small one on a high-weight axis. Summing these would give
        // the loser 1000 against the winner's 1.
        let deep_but_low = Candidate {
            binding: 1, tenancy: 1, product: 0, counterparty: 0,
            space: 2, ownership: 1, metric: 1,
        };
        let shallow_but_high = c(2, 1, 1, 0);
        let r = resolve(PolicyKind::Putaway, &[deep_but_low, shallow_but_high]).unwrap();
        assert_eq!(r.winner, shallow_but_high);
    }

    #[test]
    fn equal_vectors_are_deterministic_and_reported() {
        let a = c(7, 1, 1, 0);
        let b = c(3, 1, 1, 0);
        let r = resolve(PolicyKind::Allocation, &[a, b]).unwrap();
        assert_eq!(r.winner.binding, 3, "the floor must never stop, so the lower id wins");
        assert_eq!(r.ambiguous_with.len(), 1);
        assert!(explain(PolicyKind::Allocation, &r).contains("discrepancy"));
    }

    #[test]
    fn nothing_matching_is_an_answer_rather_than_a_guess() {
        assert!(resolve(PolicyKind::Receiving, &[]).is_none());
    }

    #[test]
    fn the_explanation_names_the_dimension_and_the_reason() {
        let r = resolve(PolicyKind::ShelfLife, &[c(1, 1, 1000, 0), c(2, 1, 0, 1)]).unwrap();
        let why = explain(PolicyKind::ShelfLife, &r);
        assert!(why.contains("counterparty"));
        assert!(why.contains("negotiated"), "the explanation must carry D81's argument");
    }
}

/// Which way a clamped field may not move.
///
/// D22 puts this *"on the Rust value type"* and it belongs beside the precedence
/// orderings for the same reason D81 gives: **a clamp direction is as invisible
/// and as consequential as a precedence order.** A manager who believes a
/// customer can shorten a shelf-life floor is wrong in exactly the way one who
/// believes product outranks counterparty is wrong.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Clamp {
    /// The result is at least every less-specific match. Booleans order
    /// `false < true`, so a floor on one means "no less strict".
    Floor,
    /// The result is at most every less-specific match.
    Ceiling,
}

impl PolicyKind {
    /// The fields that clamp, and which way.
    ///
    /// **`allocation` declares none, and that is the decision rather than an
    /// omission.** D22: *"you must not take `weight_rotation` from a customer
    /// binding and `weight_travel` from a site binding, because weights are only
    /// meaningful relative to each other."* The whole row wins or none of it
    /// does.
    ///
    /// A kind with no value table yet declares nothing, because a clamp
    /// direction invented before the field exists is a guess that will be
    /// obeyed.
    pub fn clamped_fields(self) -> &'static [(&'static str, Clamp)] {
        match self {
            // D14, in D22's words: "a site floor raises a customer rule". A
            // customer may not negotiate less shelf life than our own rule
            // requires; they may always ask for more.
            PolicyKind::ShelfLife => &[
                ("min_shelf_life_days", Clamp::Floor),
                ("min_shelf_life_pct", Clamp::Floor),
            ],
            // Tolerances are permissions, so the less specific rule is the
            // outer bound: a tenant cannot accept a larger over-delivery than
            // the platform allows, nor take longer to respond. `require_lot` is
            // a floor because a platform requiring lot capture must not be
            // relaxed underneath it.
            PolicyKind::Receiving => &[
                ("respond_by_hours", Clamp::Ceiling),
                ("tolerance_over_pct", Clamp::Ceiling),
                ("tolerance_under_pct", Clamp::Ceiling),
                ("require_lot", Clamp::Floor),
            ],
            // D85. Both are permissions to trust somebody else's number, so the
            // less specific rule is the outer bound: a tenant may be stricter
            // than the platform about whose measurement counts and never looser.
            // `prefer_own` floors because true is the strict end.
            PolicyKind::ObservationPrecedence => &[
                ("prefer_own", Clamp::Floor),
                ("accept_counterparty", Clamp::Ceiling),
            ],
            _ => &[],
        }
    }
}

/// A field the winner did not get to decide alone.
#[derive(Clone, Debug, PartialEq)]
pub struct Clamped {
    pub field: &'static str,
    pub direction: Clamp,
    pub winner_said: f64,
    pub applied: f64,
    /// The binding whose value bound it.
    pub bound_by: u128,
}

/// Apply the kind's clamps to the winner's values.
///
/// `values` is every candidate's `(binding, field, value)`, as `policy_value`
/// returns it. Everything that is not the winner is a less-specific match by
/// construction — [`resolve`] has already ordered them.
///
/// Returns the fields that moved. D22: **"the resolver returns an explanation,
/// not a value — winner, what clamped it"**, so a field that was overridden by a
/// floor is reportable rather than silently different from what the manager
/// typed.
pub fn apply_clamps(
    kind: PolicyKind,
    winner: u128,
    values: &[(u128, &str, f64)],
) -> Vec<Clamped> {
    let mut out = vec![];
    for &(field, direction) in kind.clamped_fields() {
        let Some(winner_said) = values
            .iter()
            .find(|(b, f, _)| *b == winner && *f == field)
            .map(|(_, _, v)| *v)
        else {
            continue;
        };
        let mut applied = winner_said;
        let mut bound_by = winner;
        for &(b, f, v) in values {
            if b == winner || f != field {
                continue;
            }
            let binds = match direction {
                Clamp::Floor => v > applied,
                Clamp::Ceiling => v < applied,
            };
            if binds {
                applied = v;
                bound_by = b;
            }
        }
        if applied != winner_said {
            out.push(Clamped { field, direction, winner_said, applied, bound_by });
        }
    }
    out
}

#[cfg(test)]
mod clamp_tests {
    use super::*;

    #[test]
    fn a_less_specific_floor_raises_the_winner() {
        // The fixture's shape: a customer negotiated 60 days and a product rule
        // requires 120. The customer's binding wins on Counterparty, and still
        // ships 120 -- which is what D22 means by "a site floor raises a
        // customer rule".
        let values = [(2u128, "min_shelf_life_days", 60.0), (1u128, "min_shelf_life_days", 120.0)];
        let moved = apply_clamps(PolicyKind::ShelfLife, 2, &values);
        assert_eq!(moved.len(), 1);
        assert_eq!(moved[0].winner_said, 60.0);
        assert_eq!(moved[0].applied, 120.0);
        assert_eq!(moved[0].bound_by, 1);
    }

    #[test]
    fn a_winner_already_stricter_than_the_floor_is_untouched() {
        let values = [(2u128, "min_shelf_life_days", 150.0), (1u128, "min_shelf_life_days", 120.0)];
        assert!(apply_clamps(PolicyKind::ShelfLife, 2, &values).is_empty());
    }

    #[test]
    fn a_ceiling_stops_a_tenant_exceeding_a_platform_permission() {
        // D22's stated purpose: "a commercial product a platform-shipped ceiling
        // no tenant can exceed."
        let values = [(2u128, "tolerance_over_pct", 25.0), (1u128, "tolerance_over_pct", 10.0)];
        let moved = apply_clamps(PolicyKind::Receiving, 2, &values);
        assert_eq!(moved[0].applied, 10.0);
        assert_eq!(moved[0].direction, Clamp::Ceiling);
    }

    #[test]
    fn a_boolean_floor_means_no_less_strict() {
        // require_lot: the platform requires lot capture, the tenant tried not to.
        let values = [(2u128, "require_lot", 0.0), (1u128, "require_lot", 1.0)];
        let moved = apply_clamps(PolicyKind::Receiving, 2, &values);
        assert_eq!(moved[0].applied, 1.0);
    }

    #[test]
    fn allocation_clamps_nothing_because_its_weights_are_only_meaningful_together() {
        let values = [
            (2u128, "weight_rotation", 1.0),
            (1u128, "weight_rotation", 9.0),
            (1u128, "weight_travel", 9.0),
        ];
        assert!(apply_clamps(PolicyKind::Allocation, 2, &values).is_empty());
        assert!(PolicyKind::Allocation.clamped_fields().is_empty());
    }

    #[test]
    fn a_kind_with_no_value_table_declares_no_clamps() {
        // A clamp direction invented before the field exists is a guess that
        // will be obeyed.
        for k in [PolicyKind::Sampling, PolicyKind::Specification, PolicyKind::Putaway] {
            assert!(k.clamped_fields().is_empty(), "{} guessed a clamp", k.as_str());
        }
    }
}
