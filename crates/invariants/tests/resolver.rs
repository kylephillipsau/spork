//! The two halves of D22's resolver, joined.
//!
//! `policy_candidate` matches over the closures and returns a depth vector;
//! `spork_policy::resolve` orders it in the kind's declared precedence order.
//! Neither half is the resolver, and a test of either alone would pass while the
//! join was wrong — which is the only way this can fail in practice.
//!
//! Skips rather than fails without `DATABASE_URL`, like the rest of the suite.

use spork_policy::{explain, resolve, Candidate, Dimension, PolicyKind};
use postgres::Client;

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const GLOVE: &str = "17e10000-0000-0000-0000-000000000001";
const GLOVECO: &str = "9a247000-0000-0000-0000-000000000002";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";

fn candidates(c: &mut Client, kind: &str, at: &str) -> Vec<(Candidate, String)> {
    c.query(
        "SELECT c.policy_binding_id::text, b.note, c.tenancy, c.product,
                c.counterparty, c.space, c.ownership, c.metric
           FROM policy_candidate(($1)::text::uuid, ($2)::text::policy_kind,
                                 ($3)::text::uuid, ($4)::text::uuid,
                                 ($5)::text::uuid, NULL, NULL, NULL,
                                 ($6)::text::timestamptz) c
           JOIN policy_binding b ON b.id = c.policy_binding_id",
        &[&TENANT, &kind, &GLOVE, &GLOVECO, &SITE, &at],
    )
    .expect("candidates")
    .iter()
    .map(|r| {
        let id: String = r.get(0);
        (
            Candidate {
                binding: u128::from_str_radix(&id.replace('-', ""), 16).expect("uuid"),
                tenancy: r.get(2),
                product: r.get(3),
                counterparty: r.get(4),
                space: r.get(5),
                ownership: r.get(6),
                metric: r.get(7),
            },
            r.get::<_, String>(1),
        )
    })
    .collect()
}

#[test]
fn the_customer_contract_beats_the_catalogue_default() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = spork_invariants::connect_exclusive(&url);

    let found = candidates(&mut c, "shelf_life", "2026-08-04T12:00:00Z");
    assert!(
        found.len() >= 4,
        "the fixture should offer the platform default, two product bindings and a \
         counterparty one, got {found:?}"
    );

    let only: Vec<Candidate> = found.iter().map(|(c, _)| *c).collect();
    let r = resolve(PolicyKind::ShelfLife, &only).expect("a winner");

    let note = &found
        .iter()
        .find(|(c, _)| *c == r.winner)
        .expect("the winner is one of the candidates")
        .1;

    // D81's shelf_life ordering, as behaviour on real rows: the binding scoped to
    // the counterparty class wins over the one naming the exact item, because a
    // promise to a customer beats a catalogue default.
    assert_eq!(note, "tenant binding, counterparty class", "{}", explain(PolicyKind::ShelfLife, &r));
    assert_eq!(r.decided_on, Some(Dimension::Counterparty));

    // And the platform default loses to every tenant binding, which is S15's
    // property observed rather than asserted.
    assert_eq!(r.winner.tenancy, 1);
}

#[test]
fn the_same_request_under_a_different_kind_answers_differently() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = spork_invariants::connect_exclusive(&url);

    let only: Vec<Candidate> = candidates(&mut c, "shelf_life", "2026-08-04T12:00:00Z")
        .into_iter()
        .map(|(c, _)| c)
        .collect();

    // The candidate set is a property of the scope lattice; the answer is a
    // property of the kind. Resolving the same rows under count_tolerance's
    // ordering flips the winner from the customer to the item -- which is the
    // whole reason question 93 was worth settling before this was built.
    let sl = resolve(PolicyKind::ShelfLife, &only).expect("winner");
    let ct = resolve(PolicyKind::CountTolerance, &only).expect("winner");
    assert_ne!(sl.winner, ct.winner);
    assert_eq!(ct.decided_on, Some(Dimension::Product));
}

#[test]
fn a_binding_whose_version_has_expired_is_not_a_candidate() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = spork_invariants::connect_exclusive(&url);

    // Before any tenant version came into force, only the platform default
    // applies. This is the half D70 said no epoch can see: nothing was written
    // when the range opened, and the resolver still has to get it right.
    let early = candidates(&mut c, "shelf_life", "2026-02-01T00:00:00Z");
    assert!(
        early.iter().all(|(c, _)| c.tenancy == 0),
        "a tenant version effective from August must not match in February: {early:?}"
    );
}

/// The whole path: candidates, winner, value, clamp.
///
/// This is the assertion the four migrations were for. The fixture's customer
/// negotiated sixty days of shelf life and a rule on the item itself requires a
/// hundred and twenty, so the customer's binding **wins** and the goods still
/// ship at a hundred and twenty. D22: *"a site floor raises a customer rule."*
#[test]
fn the_customer_wins_and_is_still_raised_by_a_floor_it_did_not_set() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = spork_invariants::connect_exclusive(&url);
    const AT: &str = "2026-08-04T12:00:00Z";

    let found = candidates(&mut c, "shelf_life", AT);
    let only: Vec<Candidate> = found.iter().map(|(c, _)| *c).collect();
    let r = resolve(PolicyKind::ShelfLife, &only).expect("a winner");

    // Fetch every candidate's fields, which is what makes the clamp possible:
    // the losers' values are the floors.
    let ids: Vec<String> = found
        .iter()
        .map(|(c, _)| {
            let h = format!("{:032x}", c.binding);
            format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
        })
        .collect();
    let rows = c
        .query(
            "SELECT policy_binding_id::text, field, value::float8
               FROM policy_value(($1)::text::policy_kind,
                                 ($2::text[])::uuid[], ($3)::text::timestamptz)
              WHERE value IS NOT NULL",
            &[&"shelf_life", &ids, &AT],
        )
        .expect("values");
    let owned: Vec<(u128, String, f64)> = rows
        .iter()
        .map(|r| {
            let id: String = r.get(0);
            (
                u128::from_str_radix(&id.replace('-', ""), 16).expect("uuid"),
                r.get(1),
                r.get(2),
            )
        })
        .collect();
    let values: Vec<(u128, &str, f64)> =
        owned.iter().map(|(b, f, v)| (*b, f.as_str(), *v)).collect();

    let winner_said = values
        .iter()
        .find(|(b, f, _)| *b == r.winner.binding && *f == "min_shelf_life_days")
        .map(|(_, _, v)| *v)
        .expect("the winner has a shelf life");
    assert_eq!(winner_said, 60.0, "the counterparty binding is the one that won");

    let moved = spork_policy::apply_clamps(PolicyKind::ShelfLife, r.winner.binding, &values);
    assert_eq!(moved.len(), 1, "the floor should have moved exactly one field");
    assert_eq!(moved[0].field, "min_shelf_life_days");
    assert_eq!(moved[0].winner_said, 60.0);
    assert_eq!(
        moved[0].applied, 120.0,
        "a customer may not negotiate less shelf life than our own rule requires"
    );

    // And the platform's 30 did not win the floor: a floor takes the strictest
    // of the less specific, not the least specific of them.
    assert_ne!(moved[0].applied, 30.0);
}

/// D85. The resolver's answer drives a projection.
///
/// This is the join D78 could not make: the maintainer no longer decides whose
/// number wins, it is told. The policy is resolved here exactly as any caller
/// would — candidates, ordering, values, clamps — and the answer is handed to
/// `projection_observation_current_rebuild`.
#[test]
fn a_resolved_precedence_policy_decides_whose_number_is_current() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = spork_invariants::connect_exclusive(&url);
    const AT: &str = "2026-08-04T12:00:00Z";

    let rows = c
        .query(
            "SELECT c.policy_binding_id::text, c.tenancy, c.product, c.counterparty,
                    c.space, c.ownership, c.metric
               FROM policy_candidate(($1)::text::uuid, 'observation_precedence',
                                     NULL, NULL, NULL, NULL, NULL, NULL,
                                     ($2)::text::timestamptz) c",
            &[&TENANT, &AT],
        )
        .expect("candidates");
    assert!(!rows.is_empty(), "the fixture declares a precedence policy");

    let candidates: Vec<Candidate> = rows
        .iter()
        .map(|r| {
            let id: String = r.get(0);
            Candidate {
                binding: u128::from_str_radix(&id.replace('-', ""), 16).expect("uuid"),
                tenancy: r.get(1),
                product: r.get(2),
                counterparty: r.get(3),
                space: r.get(4),
                ownership: r.get(5),
                metric: r.get(6),
            }
        })
        .collect();
    let r = resolve(PolicyKind::ObservationPrecedence, &candidates).expect("a winner");

    let ids: Vec<String> = candidates
        .iter()
        .map(|c| {
            let h = format!("{:032x}", c.binding);
            format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
        })
        .collect();
    let vals = c
        .query(
            "SELECT policy_binding_id::text, field, value::float8
               FROM policy_value('observation_precedence', ($1::text[])::uuid[],
                                 ($2)::text::timestamptz)
              WHERE value IS NOT NULL",
            &[&ids, &AT],
        )
        .expect("values");
    let winner_hex = format!("{:032x}", r.winner.binding);
    let field = |name: &str| -> bool {
        vals.iter()
            .find(|row| {
                row.get::<_, String>(0).replace('-', "") == winner_hex
                    && row.get::<_, String>(1) == name
            })
            .map(|row| row.get::<_, f64>(2) != 0.0)
            .expect("the winner states this field")
    };
    let prefer_own = field("prefer_own");
    assert!(prefer_own, "the fixture's policy says our scale decides");

    // Hand the resolved answer to the maintainer, and to its opposite, and watch
    // the current value move. Nothing about the observations changes.
    let current = |c: &mut postgres::Client| -> (f64, bool) {
        let row = c
            .query_one(
                "SELECT oc.value_numeric::float8, oc.asserted_by_party_id IS NULL
                   FROM observation_current oc JOIN metric m ON m.id = oc.metric_id
                  WHERE m.code = 'net_weight'",
                &[],
            )
            .expect("a current net weight");
        (row.get(0), row.get(1))
    };

    let decisions = |prefer_own: bool| -> String {
        format!("ARRAY[(NULL, {prefer_own}, true, NULL)]::observation_precedence_decision[]")
    };
    c.execute(
        &format!(
            "SELECT projection_observation_current_rebuild(($1)::text::uuid, {})",
            decisions(prefer_own)
        ),
        &[&TENANT],
    )
    .expect("rebuild under the resolved policy");
    assert_eq!(current(&mut c), (500.0, true), "our scale reading is current");

    c.execute(
        &format!(
            "SELECT projection_observation_current_rebuild(($1)::text::uuid, {})",
            decisions(false)
        ),
        &[&TENANT],
    )
    .expect("rebuild under the opposite");
    assert_eq!(
        current(&mut c),
        (505.0, false),
        "with prefer_own off, the supplier's fresher number wins -- which is the \
         half of D23's sentence D78 could not express"
    );

    // Leave the fixture as the rest of the suite expects it.
    c.execute(
        "SELECT projection_observation_current_rebuild(($1)::text::uuid)",
        &[&TENANT],
    )
    .expect("restore");
}
