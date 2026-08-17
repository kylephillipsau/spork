//! The whole chain, on a real line: resolve, clamp, decide, dispose.
//!
//! Each half has been tested alone since D82 and none of it had ever run end to
//! end against a row somebody could ship. This is that run.
//!
//! Skips rather than fails without `DATABASE_URL`.

use nylonite_policy::{apply_clamps, resolve, Candidate, PolicyKind};
use nylonite_server::receiving::{disposition, CountedLine, ReceivingPolicy};
use uuid::Uuid;

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const GLOVE: &str = "17e10000-0000-0000-0000-000000000001";
const GLOVECO: &str = "9a247000-0000-0000-0000-000000000002";
const ACTOR: &str = "77770000-0000-0000-0000-000000000001";
const AT: &str = "2026-08-04T12:00:00Z";

fn hex_to_uuid(id: u128) -> Uuid {
    Uuid::from_u128(id)
}

/// Resolve the receiving policy for this supplier and item, clamped.
fn resolved(c: &mut postgres::Client) -> ReceivingPolicy {
    let rows = c
        .query(
            "SELECT c.policy_binding_id::text, c.tenancy, c.product, c.counterparty,
                    c.space, c.ownership, c.metric
               FROM policy_candidate(($1)::text::uuid, 'receiving', ($2)::text::uuid,
                                     ($3)::text::uuid, NULL, NULL, NULL, NULL,
                                     ($4)::text::timestamptz) c",
            &[&TENANT, &GLOVE, &GLOVECO, &AT],
        )
        .expect("candidates");
    let candidates: Vec<Candidate> = rows
        .iter()
        .map(|r| {
            let id: String = r.get(0);
            Candidate {
                binding: u128::from_str_radix(&id.replace('-', ""), 16).unwrap(),
                tenancy: r.get(1),
                product: r.get(2),
                counterparty: r.get(3),
                space: r.get(4),
                ownership: r.get(5),
                metric: r.get(6),
            }
        })
        .collect();
    let r = resolve(PolicyKind::Receiving, &candidates).expect("a winner");

    let ids: Vec<String> = candidates
        .iter()
        .map(|c| hex_to_uuid(c.binding).to_string())
        .collect();
    let vals = c
        .query(
            "SELECT policy_binding_id::text, field, value::float8
               FROM policy_value('receiving', ($1::text[])::uuid[], ($2)::text::timestamptz)
              WHERE value IS NOT NULL",
            &[&ids, &AT],
        )
        .expect("values");
    let owned: Vec<(u128, String, f64)> = vals
        .iter()
        .map(|r| {
            let id: String = r.get(0);
            (
                u128::from_str_radix(&id.replace('-', ""), 16).unwrap(),
                r.get(1),
                r.get(2),
            )
        })
        .collect();
    let values: Vec<(u128, &str, f64)> =
        owned.iter().map(|(b, f, v)| (*b, f.as_str(), *v)).collect();

    let raw = |field: &str| -> f64 {
        values
            .iter()
            .find(|(b, f, _)| *b == r.winner.binding && *f == field)
            .map(|(_, _, v)| *v)
            .expect("the winner states this field")
    };
    let mut tolerance_over_pct = raw("tolerance_over_pct");
    let mut require_lot = raw("require_lot") != 0.0;
    for moved in apply_clamps(PolicyKind::Receiving, r.winner.binding, &values) {
        match moved.field {
            "tolerance_over_pct" => tolerance_over_pct = moved.applied,
            "require_lot" => require_lot = moved.applied != 0.0,
            _ => {}
        }
    }

    // The winning binding's *version* is what gets stamped, not the binding.
    let version: String = c
        .query_one(
            "SELECT v.id::text FROM receiving_policy v
              WHERE v.policy_binding_id = ($1)::text::uuid
                AND v.effective @> ($2)::text::timestamptz",
            &[&hex_to_uuid(r.winner.binding).to_string(), &AT],
        )
        .expect("a version in force")
        .get(0);

    ReceivingPolicy {
        version: version.parse().expect("uuid"),
        // Optional since migration 65's sibling change: the column is nullable,
        // so a version stating no over-delivery bound is a real configuration
        // and `disposition` reads the absence rather than reading it as zero.
        tolerance_over_pct: Some(tolerance_over_pct),
        require_lot,
    }
}

#[test]
fn a_line_is_disposed_of_under_the_policy_that_governed_it() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = nylonite_invariants::connect_exclusive(&url);

    // **D94 made this line necessary and that is the point of it.**
    // `goods_receipt_line_dispose` is now a definer owned by a role with neither
    // SUPERUSER nor BYPASSRLS, so its body is inside row-level security even when
    // the caller is a superuser. Before D94 the function ran as whoever called it
    // and this test passed without ever naming a tenant — which is exactly how a
    // definer owned by `postgres` would have passed too, while quietly being a
    // tenancy escape.
    c.execute("SET nylonite.tenant_id = '11111111-1111-1111-1111-111111111111'", &[])
        .expect("a caller names its tenant");

    let policy = resolved(&mut c);

    // The tenant asked for twenty-five percent over-delivery and the platform
    // ceiling brought it to ten. This is the number that reaches the dock.
    assert_eq!(policy.tolerance_over_pct, Some(10.0), "the clamp reached the floor");
    assert!(policy.require_lot, "the platform requires lot capture and a tenant may not relax it");

    // A line nobody has disposed of yet, counted over the tolerance.
    //
    // **Twelve cartons, and the numbers are the point.** Before D92 this test
    // entered 125 with an expectation of 100 and called it a twenty-five percent
    // over-delivery — but the column is a carton count and the config is ten to a
    // carton, so the line it inserted was a 1250-unit delivery against a promise
    // of 100, and it passed for the wrong reason. Twelve cartons is 120 base
    // against 100, which is the over-delivery the assertion below describes.
    let line_id = Uuid::from_u128(0x92c1_0000_0000_0000_0000_0000_0000_00ff);
    c.execute(
        "INSERT INTO goods_receipt_line (id, tenant_id, goods_receipt_id, item_id,
             expected_quantity, quantity, entered_quantity, entered_packaging_level,
             item_packing_config_id, lot_id, recorded_at, client_event_id,
             recorded_by_id)
         SELECT ($1)::text::uuid, l.tenant_id, l.goods_receipt_id, l.item_id,
                100, 120, 12, l.entered_packaging_level, l.item_packing_config_id,
                l.lot_id, now(), l.client_event_id, l.recorded_by_id
           FROM goods_receipt_line l
          WHERE l.entered_packaging_level IS NOT NULL LIMIT 1",
        &[&line_id.to_string()],
    )
    .expect("a fresh line");

    // What the dock passes in is the base quantity the row now stores, and J65
    // checks that it is the conversion of the entered form rather than a number
    // the client believed.
    let counted = CountedLine {
        expected_quantity: 100,
        quantity: 120,
        has_lot: true,
    };
    let d = disposition(&policy, &counted);
    assert!(d.accept, "an over-delivery is recorded, not refused");
    assert_eq!(d.raise, Some("over_receipt"));

    let discrepancy: Option<String> = c
        .query_one(
            "SELECT goods_receipt_line_dispose(($1)::text::uuid, ($2)::text::uuid,
                        $3, $4, ($5)::text::uuid, NULL)::text",
            &[
                &line_id.to_string(),
                &d.receiving_policy_id.to_string(),
                &d.accept,
                &d.raise,
                &ACTOR,
            ],
        )
        .expect("dispose")
        .get(0);
    assert!(discrepancy.is_some(), "an over-receipt is a row somebody can chase");

    let (accepted, stamped): (bool, String) = {
        let r = c
            .query_one(
                "SELECT accepted_at IS NOT NULL, receiving_policy_id::text
                   FROM goods_receipt_line WHERE id = ($1)::text::uuid",
                &[&line_id.to_string()],
            )
            .expect("the line");
        (r.get(0), r.get(1))
    };
    assert!(accepted);
    assert_eq!(
        stamped,
        d.receiving_policy_id.to_string(),
        "J63: the act names the version that governed it"
    );

    // A second disposition is refused: a change of mind is a correction.
    let again = c.query_one(
        "SELECT goods_receipt_line_dispose(($1)::text::uuid, ($2)::text::uuid, true, NULL,
                    ($3)::text::uuid, NULL)::text",
        &[&line_id.to_string(), &d.receiving_policy_id.to_string(), &ACTOR],
    );
    assert!(again.is_err(), "a line disposed of twice would rewrite what the first one meant");

    // Leave the fixture as the rest of the suite expects it.
    c.execute(
        "DELETE FROM discrepancy WHERE detail LIKE ($1)",
        &[&format!("receipt line {line_id}%")],
    )
    .expect("clean");
    c.execute(
        "DELETE FROM goods_receipt_line WHERE id = ($1)::text::uuid",
        &[&line_id.to_string()],
    )
    .expect("clean");
}
