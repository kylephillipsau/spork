//! WP1: client_event act idempotency — double-submit must not double facts.
//!
//! Exercises [`spork_server::client_events`] against a live DB the same way
//! handlers claim the envelope and short-circuit on replay.

use chrono::Utc;
use spork_server::client_events::{self, ActInsert, NewClientEvent};
use uuid::Uuid;

mod common;
use common::{connect, url_and_role};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const PERSON: &str = "77770000-0000-0000-0000-000000000001";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const BIN_A: &str = "10c00000-0000-0000-0000-000000000001";
const DOCK: &str = "10c00000-0000-0000-0000-000000000003";

#[tokio::test]
async fn claim_act_second_submit_is_replay() {
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

    let ev = NewClientEvent {
        tenant_id: tenant,
        client_event_id: ce,
        site_id: Some(site),
        recorded_by_id: person,
        submitted_at: now,
    };

    assert_eq!(
        client_events::claim_act(&tx, &ev).await.unwrap(),
        ActInsert::Fresh
    );
    assert_eq!(
        client_events::claim_act(&tx, &ev).await.unwrap(),
        ActInsert::Replay
    );

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn double_movement_insert_blocked_by_replay_path() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let person = Uuid::parse_str(PERSON).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();
    let bin = Uuid::parse_str(BIN_A).unwrap();
    let dock = Uuid::parse_str(DOCK).unwrap();
    let ce = Uuid::now_v7();
    let now = Utc::now();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Item + status + owner from a live stock cell at bin A.
    let cell = tx
        .query_one(
            "SELECT item_id, status_id, owner_id, lot_id FROM stock
              WHERE holder_location_id = $1 AND quantity >= 1
              ORDER BY quantity DESC LIMIT 1",
            &[&bin],
        )
        .await
        .expect("fixture stock");
    let item: Uuid = cell.get(0);
    let status: Uuid = cell.get(1);
    let owner: Uuid = cell.get(2);
    let lot: Option<Uuid> = cell.get(3);

    let ev = NewClientEvent {
        tenant_id: tenant,
        client_event_id: ce,
        site_id: Some(site),
        recorded_by_id: person,
        submitted_at: now,
    };

    // First submit: claim + movement (handler shape).
    assert_eq!(
        client_events::claim_act(&tx, &ev).await.unwrap(),
        ActInsert::Fresh
    );
    let mid1: Uuid = tx
        .query_one(
            "INSERT INTO stock_movement (
                 tenant_id, client_event_id, item_id, quantity,
                 from_location_id, from_lot_id, from_status_id, from_owner_id,
                 to_location_id, to_lot_id, to_status_id, to_owner_id,
                 reason, occurred_at, recorded_by_id)
             VALUES (
                 $1, $2, $3, 1,
                 $4, $5, $6, $7,
                 $8, $5, $6, $7,
                 'move', $9, $10)
             RETURNING id",
            &[
                &tenant, &ce, &item, &bin, &lot, &status, &owner, &dock, &now, &person,
            ],
        )
        .await
        .unwrap()
        .get(0);

    // Replay: claim is Replay; must not insert a second movement.
    assert_eq!(
        client_events::claim_act(&tx, &ev).await.unwrap(),
        ActInsert::Replay
    );
    let (mid2, qty) = client_events::require_one_movement(&tx, ce).await.unwrap();
    assert_eq!(mid1, mid2);
    assert_eq!(qty, 1);
    client_events::reject_quantity_mismatch(qty, 1).unwrap();
    assert!(client_events::reject_quantity_mismatch(qty, 2).is_err());

    let n: i64 = tx
        .query_one(
            "SELECT count(*) FROM stock_movement WHERE client_event_id = $1",
            &[&ce],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 1, "replay must not insert a second movement");

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn incomplete_act_is_hard() {
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

    let ev = NewClientEvent {
        tenant_id: tenant,
        client_event_id: ce,
        site_id: Some(site),
        recorded_by_id: person,
        submitted_at: now,
    };
    assert_eq!(
        client_events::claim_act(&tx, &ev).await.unwrap(),
        ActInsert::Fresh
    );
    // No facts — require_one_movement must fail.
    assert!(client_events::require_one_movement(&tx, ce).await.is_err());

    tx.rollback().await.unwrap();
}
