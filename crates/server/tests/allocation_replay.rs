//! Q174: a retried allocation is a replay, not a second claim.
//!
//! `POST /allocations` was the one write path with no retry protection. A
//! handheld that timed out and resubmitted wrote a second `stock_allocation`,
//! and D12 makes that worse than a duplicate row: coverage is a fold of
//! allocations (J31), so the line reads as covered twice, `uncovered_quantity`
//! goes negative, and the partial index that finds under-covered lines drops it
//! — correct for the index and exactly wrong as a way to find out.
//!
//! The fix is the identifier rather than an act envelope. An allocation is an
//! Intention and S19 asks facts for a `client_event`; what D5 actually leans on
//! is *"every entry carries an identifier the handheld generates. Sending the
//! same entry twice changes nothing."*
//!
//! Rolls back. Reaches the fixture through the stable purchase order line.

use chrono::Utc;
use spork_server::client_events::{self, PriorAllocation};
use uuid::Uuid;

mod common;
use common::{connect, url_and_role};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";

/// The mismatch guard, as a pure function. No database needed.
#[test]
fn a_reused_id_with_a_different_body_is_refused() {
    let prior = PriorAllocation {
        id: Uuid::from_u128(1),
        stock_id: Some(Uuid::from_u128(2)),
        fulfilment_line_id: Some(Uuid::from_u128(3)),
        quantity: 5,
        firm: false,
    };
    // The same act arriving twice.
    assert!(client_events::reject_allocation_mismatch(
        &prior,
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        5
    )
    .is_ok());

    // A different act wearing the first one's name. Returning the stored row
    // here would discard the difference silently, which is the whole objection.
    for (stock, line, qty) in [
        (Uuid::from_u128(9), Uuid::from_u128(3), 5),
        (Uuid::from_u128(2), Uuid::from_u128(9), 5),
        (Uuid::from_u128(2), Uuid::from_u128(3), 6),
    ] {
        assert!(
            client_events::reject_allocation_mismatch(&prior, stock, line, qty).is_err(),
            "a claim differing on one field must not read as a replay"
        );
    }
}

/// A second submission of one claim writes nothing and reads back the first.
#[tokio::test]
async fn a_resubmitted_claim_does_not_commit_the_cell_twice() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let claim = Uuid::now_v7();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // A cell with stock, and the line it will be claimed for.
    let cell = tx
        .query_one(
            "SELECT id, item_id FROM stock
              WHERE quantity > 0 AND holder_location_id IS NOT NULL
              ORDER BY quantity DESC LIMIT 1",
            &[],
        )
        .await
        .expect("a location-held cell");
    let stock_id: Uuid = cell.get(0);
    let line_id: Uuid = tx
        .query_one(
            "SELECT fl.id FROM fulfilment_line fl
               JOIN order_line ol ON ol.id = fl.order_line_id
              WHERE ol.item_id = $1 LIMIT 1",
            &[&cell.get::<_, Uuid>(1)],
        )
        .await
        .expect("a fulfilment line for that item")
        .get(0);

    let before: i64 = tx
        .query_one(
            "SELECT count(*) FROM stock_allocation WHERE fulfilment_line_id = $1",
            &[&line_id],
        )
        .await
        .unwrap()
        .get(0);

    let insert = "INSERT INTO stock_allocation (
                      id, tenant_id, stock_id, fulfilment_line_id,
                      quantity, state, firm, bound_at)
                  VALUES ($1, $2, $3, $4, $5, 'allocated', false, $6)
                  ON CONFLICT (id) DO NOTHING";
    let now = Utc::now();

    let first = tx
        .execute(insert, &[&claim, &tenant, &stock_id, &line_id, &1i64, &now])
        .await
        .unwrap();
    assert_eq!(first, 1, "the first submission writes the claim");

    let second = tx
        .execute(insert, &[&claim, &tenant, &stock_id, &line_id, &1i64, &now])
        .await
        .unwrap();
    assert_eq!(second, 0, "the retry writes nothing");

    let after: i64 = tx
        .query_one(
            "SELECT count(*) FROM stock_allocation WHERE fulfilment_line_id = $1",
            &[&line_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(after, before + 1, "one claim, however many times it is sent");

    // And the replay path finds it, scoped to the tenant.
    let prior = client_events::prior_allocation(&tx, tenant, claim)
        .await
        .unwrap()
        .expect("the claim is there");
    assert_eq!(prior.id, claim);
    assert_eq!(prior.quantity, 1);
    client_events::reject_allocation_mismatch(&prior, stock_id, line_id, 1).unwrap();

    // A stranger's tenant cannot see it, which is what makes "already in use and
    // not yours" answerable rather than a duplicate-key 500.
    let other = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
    assert!(
        client_events::prior_allocation(&tx, other, claim)
            .await
            .unwrap()
            .is_none(),
        "the lookup is tenant-scoped, not left to RLS alone"
    );

    tx.rollback().await.unwrap();
}

/// The coverage fold must not count the claim a retry is retrying.
///
/// **This is the failure the endpoint had to be restructured around.** The
/// coverage read happens before the write, so on a second submission it already
/// includes the first — and running the over-cover check against that total
/// refuses a legitimate retry as though the line were doubly committed. The
/// replay is therefore settled before the checks rather than after them.
#[tokio::test]
async fn a_retry_of_a_fully_covering_claim_is_not_read_as_over_cover() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let claim = Uuid::now_v7();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let cell = tx
        .query_one(
            "SELECT id, item_id FROM stock
              WHERE quantity > 0 AND holder_location_id IS NOT NULL
              ORDER BY quantity DESC LIMIT 1",
            &[],
        )
        .await
        .expect("a location-held cell");
    let stock_id: Uuid = cell.get(0);
    let line = tx
        .query_one(
            "SELECT fl.id, fl.quantity FROM fulfilment_line fl
               JOIN order_line ol ON ol.id = fl.order_line_id
              WHERE ol.item_id = $1 LIMIT 1",
            &[&cell.get::<_, Uuid>(1)],
        )
        .await
        .expect("a fulfilment line");
    let line_id: Uuid = line.get(0);
    let line_qty: i64 = line.get(1);

    // Cover the line exactly, under a minted id.
    tx.execute(
        "INSERT INTO stock_allocation (
             id, tenant_id, stock_id, fulfilment_line_id, quantity, state, firm, bound_at)
         VALUES ($1, $2, $3, $4, $5, 'allocated', false, now())
         ON CONFLICT (id) DO NOTHING",
        &[&claim, &tenant, &stock_id, &line_id, &line_qty],
    )
    .await
    .unwrap();

    // What the endpoint reads on the retry: coverage now includes this claim.
    let covered: i64 = tx
        .query_one(
            "SELECT coalesce(sum(quantity), 0)::bigint FROM stock_allocation
              WHERE fulfilment_line_id = $1
                AND state IN ('allocated','picking','picked','packed','fulfilled')",
            &[&line_id],
        )
        .await
        .unwrap()
        .get(0);
    assert!(
        covered + line_qty > line_qty,
        "the retry's naive re-check would over-cover; this is why replay comes first"
    );

    // The replay lookup succeeds and agrees, so the endpoint returns before it
    // ever builds a ProposedAllocation.
    let prior = client_events::prior_allocation(&tx, tenant, claim)
        .await
        .unwrap()
        .expect("the claim is there");
    client_events::reject_allocation_mismatch(&prior, stock_id, line_id, line_qty).unwrap();

    tx.rollback().await.unwrap();
}
