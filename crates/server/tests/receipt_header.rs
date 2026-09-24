//! Q172: one delivery is one goods_receipt — multi-line join and act replay.
//!
//! Looks up expected_supply by the stable purchase_order_line id (never by a
//! uuidv7 that seed regenerates). Rolls back; does not use the compose DB as a
//! permanent store.

use chrono::Utc;
use spork_server::client_events::{self, ActInsert, NewClientEvent};
use uuid::Uuid;

mod common;
use common::{connect, url_and_role};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const PERSON: &str = "77770000-0000-0000-0000-000000000001";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const POL: &str = "901e0000-0000-0000-0000-000000000001";
const PO: &str = "90000000-0000-0000-0000-000000000001";
const LOT: &str = "10700000-0000-0000-0000-000000000001";
const DOCK: &str = "10c00000-0000-0000-0000-000000000003";
const OWNER: &str = "9a247000-0000-0000-0000-000000000001";

#[tokio::test]
async fn two_lines_share_one_header() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let person = Uuid::parse_str(PERSON).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();
    let po = Uuid::parse_str(PO).unwrap();
    let lot = Uuid::parse_str(LOT).unwrap();
    let dock = Uuid::parse_str(DOCK).unwrap();
    let owner = Uuid::parse_str(OWNER).unwrap();
    let header = Uuid::now_v7();
    let ce1 = Uuid::now_v7();
    let ce2 = Uuid::now_v7();
    let now = Utc::now();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let supply: Uuid = tx
        .query_one(
            "SELECT id FROM expected_supply
              WHERE purchase_order_line_id = $1",
            &[&Uuid::parse_str(POL).unwrap()],
        )
        .await
        .expect("supply from stable POL")
        .get(0);

    let item: Uuid = tx
        .query_one(
            "SELECT item_id FROM expected_supply WHERE id = $1",
            &[&supply],
        )
        .await
        .unwrap()
        .get(0);

    // Line 1: open header with client-minted id.
    assert_eq!(
        client_events::claim_act(
            &tx,
            &NewClientEvent {
                tenant_id: tenant,
                client_event_id: ce1,
                site_id: Some(site),
                recorded_by_id: person,
                submitted_at: now,
            },
        )
        .await
        .unwrap(),
        ActInsert::Fresh
    );
    let (gr1, created1) = client_events::ensure_goods_receipt(
        &tx,
        &client_events::NewGoodsReceipt {
            tenant_id: tenant,
            goods_receipt_id: Some(header),
            site_id: Some(site),
            purchase_order_id: Some(po),
            received_at: now,
            client_event_id: ce1,
            recorded_by_id: person,
        },
    )
    .await
    .unwrap();
    assert_eq!(gr1, header);
    assert!(created1);

    let line1: Uuid = tx
        .query_one(
            "INSERT INTO goods_receipt_line (
                 tenant_id, goods_receipt_id, item_id, expected_supply_id,
                 expected_quantity, quantity, entered_quantity,
                 entered_packaging_level, lot_id,
                 client_event_id, recorded_by_id)
             VALUES ($1, $2, $3, $4, 10, 10, 10, 'each', $5, $6, $7)
             RETURNING id",
            &[&tenant, &header, &item, &supply, &lot, &ce1, &person],
        )
        .await
        .unwrap()
        .get(0);

    // Line 2: join same header under a new act.
    assert_eq!(
        client_events::claim_act(
            &tx,
            &NewClientEvent {
                tenant_id: tenant,
                client_event_id: ce2,
                site_id: Some(site),
                recorded_by_id: person,
                submitted_at: now,
            },
        )
        .await
        .unwrap(),
        ActInsert::Fresh
    );
    let (gr2, created2) = client_events::ensure_goods_receipt(
        &tx,
        &client_events::NewGoodsReceipt {
            tenant_id: tenant,
            goods_receipt_id: Some(header),
            site_id: Some(site),
            purchase_order_id: Some(po),
            received_at: now,
            client_event_id: ce2,
            recorded_by_id: person,
        },
    )
    .await
    .unwrap();
    assert_eq!(gr2, header);
    assert!(!created2);

    let line2: Uuid = tx
        .query_one(
            "INSERT INTO goods_receipt_line (
                 tenant_id, goods_receipt_id, item_id, expected_supply_id,
                 expected_quantity, quantity, entered_quantity,
                 entered_packaging_level, lot_id,
                 client_event_id, recorded_by_id)
             VALUES ($1, $2, $3, $4, 5, 5, 5, 'each', $5, $6, $7)
             RETURNING id",
            &[&tenant, &header, &item, &supply, &lot, &ce2, &person],
        )
        .await
        .unwrap()
        .get(0);

    let n_headers: i64 = tx
        .query_one(
            "SELECT count(*) FROM goods_receipt WHERE id = $1",
            &[&header],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(n_headers, 1);

    let n_lines: i64 = tx
        .query_one(
            "SELECT count(*) FROM goods_receipt_line WHERE goods_receipt_id = $1",
            &[&header],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(n_lines, 2);
    assert_ne!(line1, line2);

    // Replay line 2 by act — finds that line, not the first line on the header.
    let (gr_r, line_r, _, _, _, _) =
        client_events::require_receipt_facts(&tx, ce2).await.unwrap();
    assert_eq!(gr_r, header);
    assert_eq!(line_r, line2);

    let (gr_r1, line_r1, _, _, _, _) =
        client_events::require_receipt_facts(&tx, ce1).await.unwrap();
    assert_eq!(gr_r1, header);
    assert_eq!(line_r1, line1);

    // PO mismatch: header for PO, different demand document refused.
    let other_po = Uuid::now_v7();
    // Insert a throwaway PO so FK allows a conflicting header check path:
    // ensure_goods_receipt compares against purchase_order_id on the line.
    let err = client_events::ensure_goods_receipt(
        &tx,
        &client_events::NewGoodsReceipt {
            tenant_id: tenant,
            goods_receipt_id: Some(header),
            site_id: Some(site),
            purchase_order_id: Some(other_po),
            received_at: now,
            client_event_id: Uuid::now_v7(),
            recorded_by_id: person,
        },
    )
    .await
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("purchase order") || msg.contains("demand"),
        "expected PO mismatch, got {msg}"
    );

    // Silence unused
    let _ = (dock, owner);

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn omit_header_id_still_opens_one_line_receipt() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let person = Uuid::parse_str(PERSON).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();
    let ce = Uuid::now_v7();
    let now = Utc::now();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

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

    let (id, created) = client_events::ensure_goods_receipt(
        &tx,
        &client_events::NewGoodsReceipt {
            tenant_id: tenant,
            goods_receipt_id: None,
            site_id: Some(site),
            purchase_order_id: None,
            received_at: now,
            client_event_id: ce,
            recorded_by_id: person,
        },
    )
    .await
    .unwrap();
    assert!(created);
    assert_ne!(id, Uuid::nil());

    tx.rollback().await.unwrap();
}

/// Two lines of one delivery arriving together must not collide on the header.
///
/// **This is a regression test for a defect that was live**, and the reason it
/// needs two connections is the reason the defect existed: check-then-insert
/// looks correct from one. With the old body, the second session read zero
/// header rows, inserted, and took `duplicate key value violates unique
/// constraint "goods_receipt_pkey"` — a 500 on the dock, on precisely the
/// multi-line traffic Q172 widened the endpoint to carry.
///
/// The first session has to commit for the second to see the conflict at all,
/// so this is the one test here that writes durably. Ids are minted, so a leak
/// is an orphan nobody collides with, and the rows are removed at the end.
#[tokio::test]
async fn a_second_line_joins_a_header_it_raced_to_open() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut a = connect(&u, assume).await;
    let mut b = connect(&u, assume).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let person = Uuid::parse_str(PERSON).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();
    let header = Uuid::now_v7();
    let ce_a = Uuid::now_v7();
    let ce_b = Uuid::now_v7();
    let now = Utc::now();

    let header_for = |ce: Uuid| client_events::NewGoodsReceipt {
        tenant_id: tenant,
        goods_receipt_id: Some(header),
        site_id: Some(site),
        purchase_order_id: None,
        received_at: now,
        client_event_id: ce,
        recorded_by_id: person,
    };

    // The act envelopes go inside each transaction, because `client_event` is
    // RLS-protected and the tenant is a `SET LOCAL` — an insert on the bare
    // connection has no tenant and is refused by the write policy, which is the
    // boundary working rather than a problem with it.
    let act = |ce: Uuid| {
        (
            "INSERT INTO client_event (tenant_id, client_event_id, site_id,
                 recorded_by_id, submitted_at, received_at)
             VALUES ($1, $2, $3, $4, now(), now())",
            ce,
        )
    };

    // A opens the delivery and commits, which is what makes B's insert conflict.
    let ta = a.transaction().await.unwrap();
    ta.execute("SELECT set_config('spork.tenant_id', $1::text, true)", &[&ALPHA])
        .await
        .unwrap();
    let (sql, ce) = act(ce_a);
    ta.execute(sql, &[&tenant, &ce, &site, &person])
        .await
        .expect("A's act envelope");
    let (id_a, created_a) = client_events::ensure_goods_receipt(&ta, &header_for(ce_a))
        .await
        .expect("A opens the header");
    assert_eq!(id_a, header);
    assert!(created_a, "the first act opened the delivery");
    ta.commit().await.unwrap();

    // B names the same header. Before the fix this was a duplicate-key error.
    let tb = b.transaction().await.unwrap();
    tb.execute("SELECT set_config('spork.tenant_id', $1::text, true)", &[&ALPHA])
        .await
        .unwrap();
    let (sql, ce) = act(ce_b);
    tb.execute(sql, &[&tenant, &ce, &site, &person])
        .await
        .expect("B's act envelope");
    let (id_b, created_b) = client_events::ensure_goods_receipt(&tb, &header_for(ce_b))
        .await
        .expect("B must join rather than fail");
    assert_eq!(id_b, header);
    assert!(!created_b, "the second act joined; it did not open");
    tb.commit().await.unwrap();

    for client in [&a, &b] {
        let _ = client.execute("RESET ROLE", &[]).await;
    }

    let n: i64 = a
        .query_one("SELECT count(*) FROM goods_receipt WHERE id = $1", &[&header])
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 1, "one delivery, one header");

    a.execute("DELETE FROM goods_receipt WHERE id = $1", &[&header])
        .await
        .ok();
    a.execute(
        "DELETE FROM client_event WHERE client_event_id = ANY($1)",
        &[&vec![ce_a, ce_b]],
    )
    .await
    .ok();
}
