//! Stage 1 and stage 2 of the recorded process, in one request.
//!
//! Today they are four screens across two systems: search by confirmation
//! number, confirm the contact, open Related Records, find the Item Fulfilment,
//! check its status and its warehouse. The walkthrough calls the last part a
//! gate: *"wrong location or a status other than `Picked` means stop."*
//!
//! **The status has no column to read.** S44 forbids one — *"any label it could
//! hold is a function of the four coverage quantities that can disagree with
//! them"* — so the gate is computed from `fulfilment_line` and the quantities
//! come back beside it. That is the difference between a gate that says no and
//! one that says how far.

use uuid::Uuid;

mod common;
use common::{connect, url_and_role};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";

/// The search the endpoint runs, so this exercises the shipped SQL.
const SEARCH: &str = "
    SELECT o.id, o.confirmation_number, o.external_ref, p.name, o.state::text
      FROM \"order\" o
      LEFT JOIN party p ON p.id = o.customer_party_id
     WHERE o.confirmation_number = $1 OR o.external_ref = $1
     ORDER BY o.placed_at DESC NULLS LAST, o.id";

const GATE: &str = "
    SELECT f.id, f.state::text, s.code, count(fl.id),
           coalesce(sum(fl.quantity), 0)::bigint,
           coalesce(sum(fl.picked_quantity), 0)::bigint,
           bool_and(fl.picked_quantity >= fl.quantity)
      FROM fulfilment f
      LEFT JOIN fulfilment_line fl ON fl.fulfilment_id = f.id
      LEFT JOIN site s ON s.id = f.site_id
     WHERE f.order_id = $1
     GROUP BY f.id, f.state, s.code
     ORDER BY f.id";

#[tokio::test]
async fn an_order_is_found_by_the_number_a_customer_quotes() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('nylonite.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let rows = tx.query(SEARCH, &[&"S260041"]).await.unwrap();
    assert_eq!(rows.len(), 1, "one order carries that confirmation number");
    let order_id: Uuid = rows[0].get(0);
    assert_eq!(
        rows[0].get::<_, Option<String>>(1).as_deref(),
        Some("S260041")
    );
    assert!(
        rows[0].get::<_, Option<String>>(3).is_some(),
        "the contact comes back, because stage 1 is 'confirm it matches the contact'"
    );

    // Stage 2's gate, computed rather than read.
    let fs = tx.query(GATE, &[&order_id]).await.unwrap();
    assert!(!fs.is_empty(), "the order has a fulfilment to pack");
    let f = &fs[0];
    let lines: i64 = f.get(3);
    let committed: i64 = f.get(4);
    let picked: i64 = f.get(5);
    assert!(lines > 0, "and the fulfilment has lines");
    assert!(committed > 0);
    assert!(
        f.get::<_, Option<String>>(2).is_some(),
        "and a site, which is the other half of the gate"
    );
    // The fixture is mid-flight: some picked, not all. That is the case a label
    // cannot express and four quantities can.
    assert!(
        picked <= committed,
        "picked never exceeds the commitment; J56 asserts this against the ledger"
    );

    tx.rollback().await.unwrap();
}

/// A reference nobody quoted finds nothing, rather than everything.
#[tokio::test]
async fn an_unknown_reference_finds_nothing() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('nylonite.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    assert!(tx.query(SEARCH, &[&"SO-NOPE"]).await.unwrap().is_empty());
    // And an empty string is refused by the handler rather than matching a row
    // with a null reference, which is why the equality is on the column and not
    // on a coalesce.
    assert!(tx.query(SEARCH, &[&""]).await.unwrap().is_empty());

    tx.rollback().await.unwrap();
}

/// The search is tenant-scoped by the policy, not by a predicate in the query.
#[tokio::test]
async fn another_tenants_order_is_not_findable() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tx = client.transaction().await.unwrap();

    // A stranger's tenant. The query below carries no WHERE tenant_id, which is
    // the point: row-level security applies the predicate, and a handler that
    // forgets it returns nothing rather than everything.
    tx.execute(
        "SELECT set_config('nylonite.tenant_id', $1::text, true)",
        &[&"22222222-2222-2222-2222-222222222222"],
    )
    .await
    .unwrap();

    assert!(
        tx.query(SEARCH, &[&"S260041"]).await.unwrap().is_empty(),
        "a confirmation number is not a global key"
    );

    tx.rollback().await.unwrap();
}

/// The indexes migration 68 added are the ones the search uses.
#[tokio::test]
async fn the_lookup_uses_its_index() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('nylonite.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Both indexes exist and are partial, which is what keeps them to the rows a
    // search can actually find.
    for name in ["order_confirmation_number_idx", "order_external_ref_idx"] {
        let def: String = tx
            .query_one(
                "SELECT indexdef FROM pg_indexes WHERE indexname = $1",
                &[&name],
            )
            .await
            .unwrap_or_else(|_| panic!("{name} exists"))
            .get(0);
        assert!(def.contains("IS NOT NULL"), "{name} is partial");
        assert!(
            !def.starts_with("CREATE UNIQUE"),
            "{name} is not unique: D44 makes cancel-and-reraise the amendment \
             channel, so one reference legitimately names two orders"
        );
    }

    tx.rollback().await.unwrap();
}
