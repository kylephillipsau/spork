//! What is physically in a carton, named back to the order line it serves.
//!
//! First on architecture.md's list of what the system being replaced cannot do:
//! *"Cartons are real records and their contents link back to order lines, so a
//! packing list is per carton."* NetSuite holds one line per product with no
//! carton count, so that number exists nowhere until somebody types it into the
//! freight system.
//!
//! The property worth testing is the one that would be silently wrong: **a
//! corrected pick must stop reading as packed.** D103 put the netting in
//! `stock_movement_effective` as the single definition its readers share, and a
//! reader that folds `stock_movement` directly counts a reversed pick twice —
//! once as the pick and once as its correction, in the wrong direction.

use chrono::Utc;
use spork_server::client_events::{self, NewClientEvent};
use uuid::Uuid;

mod common;
use common::{connect, url_and_role};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const PERSON: &str = "77770000-0000-0000-0000-000000000001";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const FULFILMENT: &str = "f01f0000-0000-0000-0000-000000000001";
const LINE: &str = "f11e0000-0000-0000-0000-000000000001";
const ITEM: &str = "17e10000-0000-0000-0000-000000000001";
const BIN: &str = "10c00000-0000-0000-0000-000000000001";
const LOT: &str = "10700000-0000-0000-0000-000000000001";
const GOOD: &str = "57a70000-0000-0000-0000-000000000001";
const OWNER: &str = "9a247000-0000-0000-0000-000000000001";

/// Looked up by code: `adjustment_reason` ids are generated, not hand-written,
/// so pinning one would be the mistake `receipt_disposition.rs` already made.
async fn miscount(tx: &tokio_postgres::Transaction<'_>) -> Uuid {
    tx.query_one(
        "SELECT id FROM adjustment_reason WHERE code = 'miscount'",
        &[],
    )
    .await
    .expect("the fixture ships a miscount reason")
    .get(0)
}

/// The query the packing list runs, so the test exercises the shipped SQL rather
/// than a paraphrase of it.
const PACKING_LIST: &str = "
    SELECT i.code, l.code, sum(e.effective_quantity)::bigint,
           m.fulfilment_line_id, o.confirmation_number
      FROM stock_movement m
      JOIN stock_movement_effective e ON e.movement_id = m.id
      JOIN item i ON i.id = m.item_id
      JOIN fulfilment_line fl ON fl.id = m.fulfilment_line_id
      JOIN order_line ol ON ol.id = fl.order_line_id
      JOIN \"order\" o ON o.id = ol.order_id
      LEFT JOIN lot l ON l.id = m.to_lot_id
     WHERE m.to_package_id = $1 AND m.fulfilment_line_id IS NOT NULL
     GROUP BY i.code, l.code, m.fulfilment_line_id, o.confirmation_number
    HAVING sum(e.effective_quantity) <> 0
     ORDER BY i.code";

#[tokio::test]
async fn a_carton_says_what_is_in_it_and_who_it_is_for() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let person = Uuid::parse_str(PERSON).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();
    let fulfilment = Uuid::parse_str(FULFILMENT).unwrap();
    let line = Uuid::parse_str(LINE).unwrap();
    let item = Uuid::parse_str(ITEM).unwrap();
    let bin = Uuid::parse_str(BIN).unwrap();
    let lot = Uuid::parse_str(LOT).unwrap();
    let good = Uuid::parse_str(GOOD).unwrap();
    let owner = Uuid::parse_str(OWNER).unwrap();
    let carton = Uuid::now_v7();
    let ce_pick = Uuid::now_v7();
    let ce_fix = Uuid::now_v7();
    let now = Utc::now();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    for ce in [ce_pick, ce_fix] {
        client_events::claim_act(
            &tx,
            &NewClientEvent {
                tenant_id: tenant,
                client_event_id: ce,
                site_id: Some(site),
                recorded_by_id: person,
                submitted_at: now,
            },
        )
        .await
        .unwrap();
    }

    tx.execute(
        "INSERT INTO package (id, tenant_id, fulfilment_id, sequence) VALUES ($1, $2, $3, 71)",
        &[&carton, &tenant, &fulfilment],
    )
    .await
    .unwrap();

    // Twelve picked into the carton, naming the line it serves. D53's column.
    let pick: Uuid = tx
        .query_one(
            "INSERT INTO stock_movement (
                 tenant_id, client_event_id, item_id, quantity,
                 from_location_id, from_lot_id, from_status_id, from_owner_id,
                 to_package_id, to_lot_id, to_status_id, to_owner_id,
                 reason, occurred_at, recorded_by_id, fulfilment_line_id)
             VALUES ($1, $2, $3, 12, $4, $5, $6, $7, $8, $5, $6, $7,
                     'pick', $9, $10, $11)
             RETURNING id",
            &[
                &tenant, &ce_pick, &item, &bin, &lot, &good, &owner, &carton, &now, &person,
                &line,
            ],
        )
        .await
        .expect("the pick")
        .get(0);

    let rows = tx.query(PACKING_LIST, &[&carton]).await.unwrap();
    assert_eq!(rows.len(), 1, "one item and lot in this carton");
    assert_eq!(rows[0].get::<_, i64>(2), 12, "twelve units");
    assert_eq!(
        rows[0].get::<_, Uuid>(3),
        line,
        "and the line it was picked for"
    );
    assert_eq!(
        rows[0].get::<_, Option<String>>(4).as_deref(),
        Some("S260041"),
        "named by what a person quotes on the phone, not by a uuid"
    );

    // Five taken back out. **A correction carries its target's sides unchanged**
    // -- swapping them is `Problem::DoesNotMirror` -- so it lands with the same
    // `to_package_id` and looks exactly like a second pick to a naive fold.
    let reason = miscount(&tx).await;
    tx.execute(
        "INSERT INTO stock_movement (
             tenant_id, client_event_id, item_id, quantity,
             from_location_id, from_lot_id, from_status_id, from_owner_id,
             to_package_id, to_lot_id, to_status_id, to_owner_id,
             reason, occurred_at, recorded_by_id, fulfilment_line_id,
             reverses_movement_id, adjustment_reason_id)
         VALUES ($1, $2, $3, 5, $4, $5, $6, $7, $8, $5, $6, $7,
                 'adjustment', $9, $10, $11, $12, $13)",
        &[
            &tenant, &ce_fix, &item, &bin, &lot, &good, &owner, &carton, &now, &person, &line,
            &pick, &reason,
        ],
    )
    .await
    .expect("the correction");

    // **The property.** A reader folding `stock_movement` directly would see
    // 12 + 5 = 17, because the correction mirrors the pick's sides and so looks
    // like a second pick. Through `stock_movement_effective` it is 7.
    let rows = tx.query(PACKING_LIST, &[&carton]).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get::<_, i64>(2),
        7,
        "twelve picked with five taken back is seven in the carton, not seventeen"
    );

    let naive: i64 = tx
        .query_one(
            "SELECT coalesce(sum(quantity),0)::bigint FROM stock_movement
              WHERE to_package_id = $1 AND fulfilment_line_id IS NOT NULL",
            &[&carton],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        naive, 17,
        "the reading D103 exists to stop, stated so the difference is visible"
    );

    tx.rollback().await.unwrap();
}

/// A carton emptied by corrections drops off the list rather than showing zero.
#[tokio::test]
async fn a_fully_reversed_pick_leaves_no_line_on_the_list() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let person = Uuid::parse_str(PERSON).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();
    let fulfilment = Uuid::parse_str(FULFILMENT).unwrap();
    let line = Uuid::parse_str(LINE).unwrap();
    let item = Uuid::parse_str(ITEM).unwrap();
    let bin = Uuid::parse_str(BIN).unwrap();
    let lot = Uuid::parse_str(LOT).unwrap();
    let good = Uuid::parse_str(GOOD).unwrap();
    let owner = Uuid::parse_str(OWNER).unwrap();
    let carton = Uuid::now_v7();
    let ce_pick = Uuid::now_v7();
    let ce_fix = Uuid::now_v7();
    let now = Utc::now();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();
    for ce in [ce_pick, ce_fix] {
        client_events::claim_act(
            &tx,
            &NewClientEvent {
                tenant_id: tenant,
                client_event_id: ce,
                site_id: Some(site),
                recorded_by_id: person,
                submitted_at: now,
            },
        )
        .await
        .unwrap();
    }
    tx.execute(
        "INSERT INTO package (id, tenant_id, fulfilment_id, sequence) VALUES ($1, $2, $3, 72)",
        &[&carton, &tenant, &fulfilment],
    )
    .await
    .unwrap();

    let pick: Uuid = tx
        .query_one(
            "INSERT INTO stock_movement (
                 tenant_id, client_event_id, item_id, quantity,
                 from_location_id, from_lot_id, from_status_id, from_owner_id,
                 to_package_id, to_lot_id, to_status_id, to_owner_id,
                 reason, occurred_at, recorded_by_id, fulfilment_line_id)
             VALUES ($1, $2, $3, 4, $4, $5, $6, $7, $8, $5, $6, $7,
                     'pick', $9, $10, $11)
             RETURNING id",
            &[
                &tenant, &ce_pick, &item, &bin, &lot, &good, &owner, &carton, &now, &person,
                &line,
            ],
        )
        .await
        .unwrap()
        .get(0);
    let reason = miscount(&tx).await;
    tx.execute(
        "INSERT INTO stock_movement (
             tenant_id, client_event_id, item_id, quantity,
             from_location_id, from_lot_id, from_status_id, from_owner_id,
             to_package_id, to_lot_id, to_status_id, to_owner_id,
             reason, occurred_at, recorded_by_id, fulfilment_line_id,
             reverses_movement_id, adjustment_reason_id)
         VALUES ($1, $2, $3, 4, $4, $5, $6, $7, $8, $5, $6, $7,
                 'adjustment', $9, $10, $11, $12, $13)",
        &[
            &tenant, &ce_fix, &item, &bin, &lot, &good, &owner, &carton, &now, &person, &line,
            &pick, &reason,
        ],
    )
    .await
    .unwrap();

    let rows = tx.query(PACKING_LIST, &[&carton]).await.unwrap();
    assert!(
        rows.is_empty(),
        "a line reversed to nothing is not a line on the packing list; the HAVING \
         is what stops a printed zero"
    );

    tx.rollback().await.unwrap();
}
