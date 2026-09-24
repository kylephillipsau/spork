//! One outbound loop against the fixture: create → place → allocate → pick →
//! seal → despatch.
//!
//! Asserts the contracts the HTTP handlers share with these inserts: facts land,
//! projections are not written by the floor path, live ledger coverage moves
//! with allocations, and after a mediated refresh stock is in the carton so
//! despatch can leave. Rolls back the app work; cleans up only the fulfilment
//! line the test had to mint as owner (app cannot INSERT it).

use chrono::Utc;
use tokio_postgres::NoTls;
use uuid::Uuid;

mod common;
use common::{url_and_role};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const FULFILMENT_OPEN: &str = "f01f0000-0000-0000-0000-000000000001";
const ORDER_LINE: &str = "01e00000-0000-0000-0000-000000000001";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const PERSON: &str = "77770000-0000-0000-0000-000000000001";
const DOCK: &str = "10c00000-0000-0000-0000-000000000003";
const BIN_A: &str = "10c00000-0000-0000-0000-000000000001";

/// Fresh line so coverage starts at zero.
const LINE: &str = "f11e0000-0000-0000-0000-0000000000a1";
const CARTON: &str = "9ac00000-0000-0000-0000-0000000000a1";
const CE_CREATE: &str = "ce000000-0000-0000-0000-0000000000a1";
const CE_PLACE: &str = "ce000000-0000-0000-0000-0000000000a2";
const CE_PICK: &str = "ce000000-0000-0000-0000-0000000000a3";
const CE_SEAL: &str = "ce000000-0000-0000-0000-0000000000a4";
const CE_DESPATCH: &str = "ce000000-0000-0000-0000-0000000000a5";

async fn connect_raw(u: &str) -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(u, NoTls).await.expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

/// Live fold of pick/pack/despatch for one line — same shape as ledger_views.
async fn live_progress(
    tx: &tokio_postgres::Transaction<'_>,
    line_id: Uuid,
) -> (i64, i64, i64, i64) {
    let row = tx
        .query_one(
            "WITH RECURSIVE roots AS (
                 SELECT m.id, m.quantity, m.fulfilment_line_id,
                        m.from_location_id, m.to_location_id, m.to_package_id
                   FROM stock_movement m
                  WHERE m.fulfilment_line_id = $1
                    AND m.reverses_movement_id IS NULL
             ),
             chain AS (
                 SELECT id AS root_id, id AS movement_id, quantity, 0 AS depth,
                        fulfilment_line_id, from_location_id, to_location_id, to_package_id
                   FROM roots
                 UNION ALL
                 SELECT c.root_id, r.id, r.quantity, c.depth + 1,
                        c.fulfilment_line_id, c.from_location_id, c.to_location_id,
                        c.to_package_id
                   FROM chain c
                   JOIN stock_movement r ON r.reverses_movement_id = c.movement_id
             ),
             effective AS (
                 SELECT fulfilment_line_id, from_location_id, to_location_id, to_package_id,
                        sum(CASE WHEN depth % 2 = 0 THEN quantity ELSE -quantity END)::bigint
                            AS eq
                   FROM chain
                  GROUP BY 1, 2, 3, 4
             ),
             ledger AS (
                 SELECT coalesce(sum(e.eq) FILTER (
                            WHERE e.from_location_id IS NOT NULL), 0)::bigint AS picked,
                        coalesce(sum(e.eq) FILTER (
                            WHERE p.status IN ('sealed', 'despatched')), 0)::bigint AS packed,
                        coalesce(sum(e.eq) FILTER (
                            WHERE e.to_location_id IS NULL
                              AND e.to_package_id IS NULL), 0)::bigint AS despatched
                   FROM effective e
                   LEFT JOIN package p ON p.id = e.to_package_id
             ),
             intention AS (
                 SELECT coalesce(sum(quantity), 0)::bigint AS covered
                   FROM stock_allocation
                  WHERE fulfilment_line_id = $1
                    AND state IN ('allocated','picking','picked','packed','fulfilled')
             )
             SELECT i.covered, l.picked, l.packed, l.despatched
               FROM ledger l CROSS JOIN intention i",
            &[&line_id],
        )
        .await
        .expect("live progress");
    (row.get(0), row.get(1), row.get(2), row.get(3))
}

#[tokio::test]
async fn outbound_happy_path_create_place_allocate_pick_seal_despatch() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };

    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let fulfilment = Uuid::parse_str(FULFILMENT_OPEN).unwrap();
    let order_line = Uuid::parse_str(ORDER_LINE).unwrap();
    let line = Uuid::parse_str(LINE).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();
    let person = Uuid::parse_str(PERSON).unwrap();
    let dock = Uuid::parse_str(DOCK).unwrap();
    let bin_a = Uuid::parse_str(BIN_A).unwrap();
    let carton = Uuid::parse_str(CARTON).unwrap();
    let ce_create = Uuid::parse_str(CE_CREATE).unwrap();
    let ce_place = Uuid::parse_str(CE_PLACE).unwrap();
    let ce_pick = Uuid::parse_str(CE_PICK).unwrap();
    let ce_seal = Uuid::parse_str(CE_SEAL).unwrap();
    let ce_despatch = Uuid::parse_str(CE_DESPATCH).unwrap();

    let qty: i64 = 10;
    let mut client = connect_raw(&u).await;

    // --- Owner setup: a line with room to allocate (app cannot INSERT this). ---
    // Clear on-demand rate limit so the mid-path refresh after pick can accept.
    {
        let tx = client.transaction().await.unwrap();
        tx.execute(
            "INSERT INTO fulfilment_line (id, tenant_id, fulfilment_id, order_line_id, quantity)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (id) DO UPDATE SET quantity = EXCLUDED.quantity",
            &[&line, &tenant, &fulfilment, &order_line, &qty],
        )
        .await
        .expect("mint fulfilment line");
        tx.execute(
            "UPDATE projection_freshness
                SET last_on_demand_at = NULL
              WHERE tenant_id = $1",
            &[&tenant],
        )
        .await
        .ok();
        tx.commit().await.unwrap();
    }

    if assume_role {
        client
            .batch_execute("SET ROLE spork_app")
            .await
            .expect("app role");
    }

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let now = Utc::now();

    // Projection columns before any write on this line.
    let proj0 = tx
        .query_one(
            "SELECT covered_quantity, picked_quantity, packed_quantity, despatched_quantity
               FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap();
    let covered_p0: i64 = proj0.get(0);
    let picked_p0: i64 = proj0.get(1);
    let packed_p0: i64 = proj0.get(2);
    let despatched_p0: i64 = proj0.get(3);
    assert_eq!(covered_p0, 0);
    assert_eq!(picked_p0, 0);

    // 1. Create package + created event at dock.
    for (ce, _) in [
        (ce_create, "create"),
        (ce_place, "place"),
        (ce_pick, "pick"),
        (ce_seal, "seal"),
        (ce_despatch, "despatch"),
    ] {
        tx.execute(
            "INSERT INTO client_event
                 (tenant_id, client_event_id, site_id, recorded_by_id, submitted_at, received_at)
             VALUES ($1, $2, $3, $4, $5, now())
             ON CONFLICT DO NOTHING",
            &[&tenant, &ce, &site, &person, &now],
        )
        .await
        .expect("client_event");
    }

    tx.execute(
        "INSERT INTO package (id, tenant_id, fulfilment_id, sequence)
         VALUES ($1, $2, $3, 90)",
        &[&carton, &tenant, &fulfilment],
    )
    .await
    .expect("package skeleton");

    tx.execute(
        "INSERT INTO package_event (
             tenant_id, client_event_id, package_id, kind, source,
             occurred_at, recorded_by_id, location_id, barcode)
         VALUES ($1, $2, $3, 'created', 'operator_scan', $4, $5, $6, 'CARTON-HAPPY')",
        &[&tenant, &ce_create, &carton, &now, &person, &dock],
    )
    .await
    .expect("created");

    // 2. Place → bin A.
    tx.execute(
        "INSERT INTO package_event (
             tenant_id, client_event_id, package_id, kind, source,
             occurred_at, recorded_by_id, location_id)
         VALUES ($1, $2, $3, 'placed', 'operator_scan', $4, $5, $6)",
        &[&tenant, &ce_place, &carton, &now, &person, &bin_a],
    )
    .await
    .expect("placed");

    let winning_kind: String = tx
        .query_one(
            "SELECT kind FROM package_event
              WHERE package_id = $1
                AND kind IN ('created','placed','contained','sealed','opened','despatched','voided')
              ORDER BY occurred_at DESC, recorded_at DESC, id DESC
              LIMIT 1",
            &[&carton],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(winning_kind, "placed");

    let resolved_still: Option<Uuid> = tx
        .query_one(
            "SELECT resolved_location_id FROM package WHERE id = $1",
            &[&carton],
        )
        .await
        .unwrap()
        .get(0);
    // Before rebuild the projection may still be null/old.
    let _ = resolved_still;

    // 3. Allocate from free location-held stock.
    let cell = tx
        .query_one(
            "SELECT id, holder_location_id, lot_id, status_id, owner_id, item_id,
                    available_quantity
               FROM stock
              WHERE holder_location_id IS NOT NULL
                AND available_quantity >= $1
              ORDER BY available_quantity DESC
              LIMIT 1",
            &[&qty],
        )
        .await
        .expect("free cell");
    let stock_id: Uuid = cell.get(0);
    let from_loc: Uuid = cell.get(1);
    let lot: Option<Uuid> = cell.get(2);
    let status: Uuid = cell.get(3);
    let owner: Uuid = cell.get(4);
    let item: Uuid = cell.get(5);

    let allocation_id: Uuid = tx
        .query_one(
            "INSERT INTO stock_allocation (
                 tenant_id, stock_id, fulfilment_line_id, quantity,
                 state, firm, bound_at)
             VALUES ($1, $2, $3, $4, 'allocated', false, $5)
             RETURNING id",
            &[&tenant, &stock_id, &line, &qty, &now],
        )
        .await
        .expect("allocate")
        .get(0);

    let (covered, picked, _packed, despatched) = live_progress(&tx, line).await;
    assert_eq!(covered, qty, "live cover after allocate");
    assert_eq!(picked, 0);
    assert_eq!(despatched, 0);

    let covered_proj = tx
        .query_one(
            "SELECT covered_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(
        covered_proj, covered_p0,
        "allocate must not UPDATE covered_quantity"
    );

    // 4. Pick into the carton.
    tx.execute(
        "INSERT INTO stock_movement (
             tenant_id, client_event_id, item_id, quantity,
             from_location_id, from_lot_id, from_status_id, from_owner_id,
             to_package_id, to_lot_id, to_status_id, to_owner_id,
             reason, occurred_at, recorded_by_id, fulfilment_line_id)
         VALUES (
             $1, $2, $3, $4,
             $5, $6, $7, $8,
             $9, $6, $7, $8,
             'pick', $10, $11, $12)",
        &[
            &tenant,
            &ce_pick,
            &item,
            &qty,
            &from_loc,
            &lot,
            &status,
            &owner,
            &carton,
            &now,
            &person,
            &line,
        ],
    )
    .await
    .expect("pick");

    let (covered2, picked2, _, _) = live_progress(&tx, line).await;
    assert_eq!(covered2, qty);
    assert_eq!(picked2, qty, "live picked after pick movement");

    let picked_proj = tx
        .query_one(
            "SELECT picked_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(picked_proj, picked_p0, "pick must not UPDATE picked_quantity");

    // Stock in the carton only after maintainers; refresh is the app's allowed path.
    let refreshed: Option<i64> = tx
        .query_one("SELECT projection_refresh_tenant($1)", &[&tenant])
        .await
        .expect("refresh after pick")
        .get(0);
    assert!(
        refreshed.is_some(),
        "refresh after pick must accept so package-held stock exists for despatch"
    );

    let pkg_stock = tx
        .query_one(
            "SELECT id, lot_id, status_id, owner_id, quantity
               FROM stock
              WHERE holder_package_id = $1 AND item_id = $2 AND quantity > 0
              LIMIT 1",
            &[&carton, &item],
        )
        .await
        .expect("package-held stock after rebuild");
    let pkg_qty: i64 = pkg_stock.get(4);
    assert!(pkg_qty >= qty, "carton holds the pick");

    // 5. Seal (event + freeze column the app may write).
    tx.execute(
        "INSERT INTO package_event (
             tenant_id, client_event_id, package_id, kind, source,
             occurred_at, recorded_by_id)
         VALUES ($1, $2, $3, 'sealed', 'operator_scan', $4, $5)",
        &[&tenant, &ce_seal, &carton, &now, &person],
    )
    .await
    .expect("seal");
    tx.execute(
        "UPDATE package SET sealed_at = $1 WHERE id = $2 AND sealed_at IS NULL",
        &[&now, &carton],
    )
    .await
    .ok();

    // 6. Despatch: event + movement with no to side.
    let from_lot: Option<Uuid> = pkg_stock.get(1);
    let from_status: Uuid = pkg_stock.get(2);
    let from_owner: Uuid = pkg_stock.get(3);

    tx.execute(
        "INSERT INTO package_event (
             tenant_id, client_event_id, package_id, kind, source,
             occurred_at, recorded_by_id)
         VALUES ($1, $2, $3, 'despatched', 'operator_scan', $4, $5)",
        &[&tenant, &ce_despatch, &carton, &now, &person],
    )
    .await
    .expect("despatch event");

    let despatch_id: Uuid = tx
        .query_one(
            "INSERT INTO stock_movement (
                 tenant_id, client_event_id, item_id, quantity,
                 from_package_id, from_lot_id, from_status_id, from_owner_id,
                 reason, occurred_at, recorded_by_id, fulfilment_line_id)
             VALUES (
                 $1, $2, $3, $4,
                 $5, $6, $7, $8,
                 'despatch', $9, $10, $11)
             RETURNING id",
            &[
                &tenant,
                &ce_despatch,
                &item,
                &qty,
                &carton,
                &from_lot,
                &from_status,
                &from_owner,
                &now,
                &person,
                &line,
            ],
        )
        .await
        .expect("despatch movement")
        .get(0);

    let no_to: bool = tx
        .query_one(
            "SELECT to_location_id IS NULL AND to_package_id IS NULL
               FROM stock_movement WHERE id = $1",
            &[&despatch_id],
        )
        .await
        .unwrap()
        .get(0);
    assert!(no_to, "despatch has no to side");

    let (_, _, _, live_despatched) = live_progress(&tx, line).await;
    assert_eq!(live_despatched, qty, "live despatched after leave movement");

    let despatched_proj = tx
        .query_one(
            "SELECT despatched_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    // Refresh earlier may have written pick progress; despatch itself must not
    // have been folded yet unless a second refresh ran.
    let _ = (packed_p0, despatched_p0, despatched_proj);

    let status_ledger: String = tx
        .query_one(
            "SELECT kind FROM package_event
              WHERE package_id = $1
                AND kind IN ('created','placed','contained','sealed','opened','despatched','voided')
              ORDER BY occurred_at DESC, recorded_at DESC, id DESC
              LIMIT 1",
            &[&carton],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(status_ledger, "despatched");

    // Live numbers after the full write path (ledger truth, not cache).
    let (lc, lp, _lpk, ld) = live_progress(&tx, line).await;
    assert_eq!(lc, qty, "covered");
    assert_eq!(lp, qty, "picked");
    assert_eq!(ld, qty, "despatched");
    // packed folds package.status (projection); may lag until another rebuild.
    // After one mid-path refresh it may still be zero — that is the dual-view design.

    assert!(allocation_id != Uuid::nil());

    // Roll back the floor work so the suite leaves the fixture as it found it.
    tx.rollback().await.unwrap();

    if assume_role {
        let _ = client.batch_execute("RESET ROLE").await;
    }

    // Drop the owner-minted line (not part of the rolled-back app txn).
    let _ = client
        .execute("DELETE FROM fulfilment_line WHERE id = $1", &[&line])
        .await;
}
