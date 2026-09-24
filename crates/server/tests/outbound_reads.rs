//! The first read surface for outbound progress, against the fixture.
//!
//! These assert that the numbers D99–D103 fold are visible under a real tenant
//! scope as the application role — the same contract the HTTP handlers use.
//! Actix wiring is thin enough that the SQL and the tenancy boundary are what
//! need a database; the handlers are the same queries with a JSON envelope.

use tokio_postgres::NoTls;
use uuid::Uuid;

mod common;
use common::{connect, url_and_role};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const BETA: &str = "22222222-2222-2222-2222-222222222222";
/// The fixture line that was picked, packed, despatched, and partly reversed.
const LINE_SHIPPED: &str = "f11e0000-0000-0000-0000-000000000002";
const FULFILMENT_SHIPPED: &str = "f01f0000-0000-0000-0000-000000000002";
const CARTON_D: &str = "9ac00000-0000-0000-0000-00000000000d";

#[tokio::test]
async fn the_shipped_line_reads_the_fold_numbers() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Same columns the GET /fulfilment-lines/{id} handler selects.
    let row = tx
        .query_one(
            "SELECT fl.quantity, fl.covered_quantity, fl.picked_quantity,
                    fl.packed_quantity, fl.despatched_quantity, fl.uncovered_quantity,
                    i.code
               FROM fulfilment_line fl
               JOIN order_line ol ON ol.id = fl.order_line_id
               JOIN item i ON i.id = ol.item_id
              WHERE fl.id = $1",
            &[&Uuid::parse_str(LINE_SHIPPED).unwrap()],
        )
        .await
        .expect("the shipped line");

    assert_eq!(row.get::<_, i64>(0), 20, "commitment quantity");
    assert_eq!(row.get::<_, i64>(1), 20, "covered");
    assert_eq!(row.get::<_, i64>(2), 20, "picked");
    assert_eq!(row.get::<_, i64>(3), 20, "packed");
    assert_eq!(
        row.get::<_, i64>(4),
        15,
        "despatched nets the partial reverse"
    );
    assert_eq!(row.get::<_, i64>(5), 0, "uncovered");
    assert_eq!(row.get::<_, String>(6), "GLOVE-M");
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn a_fulfilment_lists_its_lines_and_a_stranger_sees_none() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let fid = Uuid::parse_str(FULFILMENT_SHIPPED).unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();
    let n: i64 = tx
        .query_one(
            "SELECT count(*) FROM fulfilment_line WHERE fulfilment_id = $1",
            &[&fid],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 1, "alpha's shipped fulfilment has one line");
    tx.commit().await.unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&BETA],
    )
    .await
    .unwrap();
    let n: i64 = tx
        .query_one(
            "SELECT count(*) FROM fulfilment_line WHERE fulfilment_id = $1",
            &[&fid],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 0, "beta must not see alpha's fulfilment lines");
    let found = tx
        .query_opt("SELECT 1 FROM fulfilment WHERE id = $1", &[&fid])
        .await
        .unwrap();
    assert!(found.is_none(), "beta must not see alpha's fulfilment row");
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn the_carton_reads_despatched_and_still_holds_five() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Same shape as GET /packages/{id}.
    let row = tx
        .query_one(
            "SELECT p.status, p.barcode,
                    coalesce((SELECT sum(s.quantity) FROM stock s
                               WHERE s.holder_package_id = p.id), 0)::bigint
               FROM package p WHERE p.id = $1",
            &[&Uuid::parse_str(CARTON_D).unwrap()],
        )
        .await
        .expect("carton D");

    assert_eq!(row.get::<_, Option<String>>(0).as_deref(), Some("despatched"));
    assert_eq!(row.get::<_, Option<String>>(1).as_deref(), Some("CARTON-D"));
    assert_eq!(
        row.get::<_, i64>(2),
        5,
        "partial reverse left five units package-held"
    );
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn open_lines_at_the_site_include_the_unfinished_commitment() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let line_open = Uuid::parse_str("f11e0000-0000-0000-0000-000000000001").unwrap();
    let line_partial = Uuid::parse_str(LINE_SHIPPED).unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Same filter as GET /sites/{id}/open-lines.
    let rows = tx
        .query(
            "SELECT fl.id
               FROM fulfilment_line fl
               JOIN fulfilment f ON f.id = fl.fulfilment_id
              WHERE f.site_id = $1
                AND f.state <> 'cancelled'
                AND fl.despatched_quantity < fl.quantity",
            &[&site],
        )
        .await
        .unwrap();
    let ids: Vec<Uuid> = rows.iter().map(|r| r.get(0)).collect();
    assert!(
        ids.contains(&line_open),
        "the unpicked 40-unit line is open work: {ids:?}"
    );
    assert!(
        ids.contains(&line_partial),
        "the partly despatched line is still open (15 of 20): {ids:?}"
    );
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn recording_a_pick_writes_the_ledger_and_not_the_projection() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    // Mint the carton as the connecting role (app has SELECT-only on package).
    // Then become the app for the pick INSERT — the real write path.
    let (client, connection) = tokio_postgres::connect(&u, NoTls).await.expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let mut client = client;

    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let line = Uuid::parse_str("f11e0000-0000-0000-0000-000000000001").unwrap();
    let fulfilment = Uuid::parse_str("f01f0000-0000-0000-0000-000000000001").unwrap();
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let picker = Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap();
    let event = Uuid::parse_str("ce000000-0000-0000-0000-0000000000b1").unwrap();
    let carton = Uuid::parse_str("9ac00000-0000-0000-0000-0000000000b1").unwrap();
    let movement = Uuid::parse_str("5b000000-0000-0000-0000-0000000000b1").unwrap();
    let dock = Uuid::parse_str("10c00000-0000-0000-0000-000000000003").unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
             submitted_at, received_at)
         VALUES ($1, $2, $3, $4, now(), now())
         ON CONFLICT DO NOTHING",
        &[&tenant, &event, &site, &picker],
    )
    .await
    .expect("client_event");
    tx.execute(
        "INSERT INTO package (id, tenant_id, fulfilment_id, sequence, barcode)
         VALUES ($1, $2, $3, 2, 'CARTON-PICK-TEST')
         ON CONFLICT (id) DO NOTHING",
        &[&carton, &tenant, &fulfilment],
    )
    .await
    .expect("carton");
    tx.execute(
        "INSERT INTO package_event (id, tenant_id, package_id, kind, source, occurred_at,
             recorded_at, client_event_id, recorded_by_id, location_id)
         VALUES ('9ae00000-0000-0000-0000-0000000000b1', $1, $2, 'created', 'operator_scan',
                 now(), now(), $3, $4, $5)
         ON CONFLICT (id) DO NOTHING",
        &[&tenant, &carton, &event, &picker, &dock],
    )
    .await
    .expect("placement");
    tx.commit().await.unwrap();

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

    let picked_before: i64 = tx
        .query_one(
            "SELECT picked_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap()
        .get(0);

    let cell = tx
        .query_one(
            "SELECT id, holder_location_id, lot_id, status_id, owner_id, item_id
               FROM stock
              WHERE holder_location_id IS NOT NULL AND available_quantity >= 5
              ORDER BY quantity DESC LIMIT 1",
            &[],
        )
        .await
        .expect("a location-held cell with stock");

    let from_loc: Uuid = cell.get(1);
    let lot: Option<Uuid> = cell.get(2);
    let status: Uuid = cell.get(3);
    let owner: Uuid = cell.get(4);
    let item: Uuid = cell.get(5);

    tx.execute(
        "INSERT INTO stock_movement (
             id, tenant_id, client_event_id, item_id, quantity,
             from_location_id, from_lot_id, from_status_id, from_owner_id,
             to_package_id, to_lot_id, to_status_id, to_owner_id,
             reason, occurred_at, recorded_by_id, fulfilment_line_id)
         VALUES (
             $1, $2, $3, $4, 5,
             $5, $6, $7, $8,
             $9, $6, $7, $8,
             'pick', now(), $10, $11)",
        &[
            &movement,
            &tenant,
            &event,
            &item,
            &from_loc,
            &lot,
            &status,
            &owner,
            &carton,
            &picker,
            &line,
        ],
    )
    .await
    .expect("pick movement");

    let n: i64 = tx
        .query_one(
            "SELECT count(*) FROM stock_movement
              WHERE id = $1 AND reason = 'pick' AND fulfilment_line_id = $2",
            &[&movement, &line],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 1, "the pick is on the ledger");

    let picked_after: i64 = tx
        .query_one(
            "SELECT picked_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        picked_after, picked_before,
        "the handler must not UPDATE the projection; the fold does that"
    );

    tx.rollback().await.unwrap();

    if assume_role {
        let _ = client.batch_execute("RESET ROLE").await;
    }
    // **The pointer, then the folds, then the facts.**
    //
    // `package.placement_event_id` references the `package_event` this deletes,
    // so removing the event first is a foreign key violation — and this cleanup
    // did exactly that, under a `let _ =` that threw the error away. It looked
    // like it worked because the pointer is set by the placement fold: on a run
    // where the maintainer had not caught up, the column was still null and the
    // delete succeeded. On a run where it had, the carton stayed in the database
    // forever and the suite said nothing.
    //
    // That is the second time an ignored cleanup error on this table has hidden
    // a leak, and both times the symptom was a count that moved on some runs and
    // not others.
    client
        .batch_execute(
            "UPDATE package SET placement_event_id = NULL, placement_occurred_at = NULL
              WHERE id = '9ac00000-0000-0000-0000-0000000000b1';
             DELETE FROM package_containment
              WHERE package_id = '9ac00000-0000-0000-0000-0000000000b1';
             DELETE FROM package_event WHERE id = '9ae00000-0000-0000-0000-0000000000b1';
             DELETE FROM package WHERE id = '9ac00000-0000-0000-0000-0000000000b1';
             DELETE FROM client_event WHERE client_event_id = 'ce000000-0000-0000-0000-0000000000b1';",
        )
        .await
        .expect("the test removes the carton it committed");
}

#[tokio::test]
async fn stock_includes_the_package_held_cell() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Same shape as GET /stock.
    let rows = tx
        .query(
            "SELECT i.code, l.code, p.barcode, s.quantity
               FROM stock s
               JOIN item i ON i.id = s.item_id
               LEFT JOIN location l ON l.id = s.resolved_location_id
               LEFT JOIN package p ON p.id = s.holder_package_id
              WHERE s.quantity <> 0 AND p.barcode = 'CARTON-D'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "one package-held cell in CARTON-D");
    assert_eq!(rows[0].get::<_, i64>(3), 5);
    assert_eq!(
        rows[0].get::<_, Option<String>>(1).as_deref(),
        Some("DOCK-1"),
        "resolved location is the carton's placement"
    );
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn the_app_can_create_a_package_with_a_created_event() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let fulfilment = Uuid::parse_str("f01f0000-0000-0000-0000-000000000001").unwrap();
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let picker = Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap();
    let event = Uuid::parse_str("ce000000-0000-0000-0000-0000000000c1").unwrap();
    let carton = Uuid::parse_str("9ac00000-0000-0000-0000-0000000000c1").unwrap();
    let dock = Uuid::parse_str("10c00000-0000-0000-0000-000000000003").unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    tx.execute(
        "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
             submitted_at, received_at)
         VALUES ($1, $2, $3, $4, now(), now())
         ON CONFLICT DO NOTHING",
        &[&tenant, &event, &site, &picker],
    )
    .await
    .expect("client_event");

    // Same two inserts POST /packages performs — as the application role.
    tx.execute(
        "INSERT INTO package (id, tenant_id, fulfilment_id, sequence)
         VALUES ($1, $2, $3, 3)",
        &[&carton, &tenant, &fulfilment],
    )
    .await
    .expect("package skeleton");

    let event_id: Uuid = tx
        .query_one(
            "INSERT INTO package_event (
                 tenant_id, client_event_id, package_id, kind, source,
                 occurred_at, recorded_by_id, location_id, barcode)
             VALUES ($1, $2, $3, 'created', 'operator_scan', now(), $4, $5, 'CARTON-CREATE-TEST')
             RETURNING id",
            &[&tenant, &event, &carton, &picker, &dock],
        )
        .await
        .expect("created event")
        .get(0);

    let n: i64 = tx
        .query_one(
            "SELECT count(*) FROM package p
              JOIN package_event e ON e.package_id = p.id AND e.kind = 'created'
             WHERE p.id = $1 AND e.id = $2",
            &[&carton, &event_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 1);

    // Projection columns stay empty until the maintainers run — that is the design.
    let status: Option<String> = tx
        .query_one("SELECT status FROM package WHERE id = $1", &[&carton])
        .await
        .unwrap()
        .get(0);
    assert!(
        status.is_none(),
        "status is a projection; create must not write it"
    );

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn sealing_writes_an_event_and_not_status() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let picker = Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap();
    let event = Uuid::parse_str("ce000000-0000-0000-0000-0000000000d1").unwrap();
    // Fresh package for seal (CARTON-D is already despatched).
    let carton = Uuid::parse_str("9ac00000-0000-0000-0000-0000000000d1").unwrap();
    let fulfilment = Uuid::parse_str("f01f0000-0000-0000-0000-000000000001").unwrap();
    let dock = Uuid::parse_str("10c00000-0000-0000-0000-000000000003").unwrap();

    // Setup package as owner if needed
    if assume_role {
        let (setup, conn) = tokio_postgres::connect(&u, NoTls).await.unwrap();
        tokio::spawn(async move {
            let _ = conn.await;
        });
        setup
            .batch_execute(&format!(
                "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id, submitted_at, received_at)
                 VALUES ('{ALPHA}', '{event}', '{site}', '{picker}', now(), now()) ON CONFLICT DO NOTHING;
                 INSERT INTO package (id, tenant_id, fulfilment_id, sequence)
                 VALUES ('{carton}', '{ALPHA}', '{fulfilment}', 9) ON CONFLICT DO NOTHING;
                 INSERT INTO package_event (id, tenant_id, client_event_id, package_id, kind, source, occurred_at, recorded_by_id, location_id)
                 VALUES ('9ae00000-0000-0000-0000-0000000000d1', '{ALPHA}', '{event}', '{carton}',
                         'created', 'operator_scan', now(), '{picker}', '{dock}') ON CONFLICT DO NOTHING;"
            ))
            .await
            .expect("setup carton");
    }

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let status_before: Option<String> = tx
        .query_one("SELECT status FROM package WHERE id = $1", &[&carton])
        .await
        .unwrap()
        .get(0);

    tx.execute(
        "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
             submitted_at, received_at)
         VALUES ($1, $2, $3, $4, now(), now())
         ON CONFLICT DO NOTHING",
        &[&tenant, &event, &site, &picker],
    )
    .await
    .ok();

    // Same as POST /packages/{id}/seal
    let event_id: Uuid = tx
        .query_one(
            "INSERT INTO package_event (
                 tenant_id, client_event_id, package_id, kind, source,
                 occurred_at, recorded_by_id)
             VALUES ($1, $2, $3, 'sealed', 'operator_scan', now(), $4)
             RETURNING id",
            &[&tenant, &event, &carton, &picker],
        )
        .await
        .expect("seal event")
        .get(0);

    tx.execute(
        "UPDATE package SET sealed_at = now() WHERE id = $1 AND sealed_at IS NULL",
        &[&carton],
    )
    .await
    .expect("sealed_at");

    let status_after: Option<String> = tx
        .query_one("SELECT status FROM package WHERE id = $1", &[&carton])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        status_before, status_after,
        "seal must not UPDATE status; the fold does that"
    );
    assert!(event_id != Uuid::nil());

    tx.rollback().await.unwrap();

    if assume_role {
        client.batch_execute("RESET ROLE").await.ok();
        client
            .batch_execute(
                // Same ordering as the other cleanups here, and for the same
                // reason: `projection_package_stamp` writes
                // `package.placement_event_id`, so once a maintainer has run the
                // package points at its own event and deleting the event first
                // violates a foreign key. Checked rather than discarded — `.ok()`
                // turned that into a row left behind on some runs and nothing
                // said so.
                "UPDATE package SET placement_event_id = NULL,
                                    placement_occurred_at = NULL
                  WHERE id = '9ac00000-0000-0000-0000-0000000000d1';
                 DELETE FROM package_containment WHERE package_id = '9ac00000-0000-0000-0000-0000000000d1'
                                                    OR parent_package_id = '9ac00000-0000-0000-0000-0000000000d1';
                 DELETE FROM stock WHERE holder_package_id = '9ac00000-0000-0000-0000-0000000000d1';
                 DELETE FROM package_event WHERE package_id = '9ac00000-0000-0000-0000-0000000000d1';
                 DELETE FROM package WHERE id = '9ac00000-0000-0000-0000-0000000000d1';
                 DELETE FROM client_event WHERE client_event_id = 'ce000000-0000-0000-0000-0000000000d1';",
            )
            .await
            .expect("the test removes what it committed");
    }
}

#[tokio::test]
async fn opening_writes_opened_event_and_clears_sealed_at() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let picker = Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap();
    let event = Uuid::parse_str("ce000000-0000-0000-0000-0000000000d2").unwrap();
    let carton = Uuid::parse_str("9ac00000-0000-0000-0000-0000000000d2").unwrap();
    let fulfilment = Uuid::parse_str("f01f0000-0000-0000-0000-000000000001").unwrap();
    let dock = Uuid::parse_str("10c00000-0000-0000-0000-000000000003").unwrap();

    if assume_role {
        let (setup, conn) = tokio_postgres::connect(&u, NoTls).await.unwrap();
        tokio::spawn(async move {
            let _ = conn.await;
        });
        setup
            .batch_execute(&format!(
                "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id, submitted_at, received_at)
                 VALUES ('{ALPHA}', '{event}', '{site}', '{picker}', now(), now()) ON CONFLICT DO NOTHING;
                 INSERT INTO package (id, tenant_id, fulfilment_id, sequence, sealed_at)
                 VALUES ('{carton}', '{ALPHA}', '{fulfilment}', 12, now()) ON CONFLICT DO NOTHING;
                 INSERT INTO package_event (id, tenant_id, client_event_id, package_id, kind, source, occurred_at, recorded_by_id, location_id)
                 VALUES ('9ae00000-0000-0000-0000-0000000000d2', '{ALPHA}', '{event}', '{carton}',
                         'created', 'operator_scan', now(), '{picker}', '{dock}') ON CONFLICT DO NOTHING;
                 INSERT INTO package_event (id, tenant_id, client_event_id, package_id, kind, source, occurred_at, recorded_by_id)
                 VALUES ('9ae00000-0000-0000-0000-0000000000d3', '{ALPHA}', '{event}', '{carton}',
                         'sealed', 'operator_scan', now(), '{picker}') ON CONFLICT DO NOTHING;
                 UPDATE package SET status = 'sealed' WHERE id = '{carton}';"
            ))
            .await
            .expect("setup open carton");
    }

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // If status was projected sealed, open should clear sealed_at without touching status.
    let sealed_before: Option<chrono::DateTime<chrono::Utc>> = tx
        .query_one("SELECT sealed_at FROM package WHERE id = $1", &[&carton])
        .await
        .unwrap()
        .get(0);
    assert!(sealed_before.is_some(), "setup left sealed_at set");

    let status_before: Option<String> = tx
        .query_one("SELECT status FROM package WHERE id = $1", &[&carton])
        .await
        .unwrap()
        .get(0);

    tx.execute(
        "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
             submitted_at, received_at)
         VALUES ($1, $2, $3, $4, now(), now())
         ON CONFLICT DO NOTHING",
        &[&tenant, &event, &site, &picker],
    )
    .await
    .ok();

    let event_id: Uuid = tx
        .query_one(
            "INSERT INTO package_event (
                 tenant_id, client_event_id, package_id, kind, source,
                 occurred_at, recorded_by_id)
             VALUES ($1, $2, $3, 'opened', 'operator_scan', now(), $4)
             RETURNING id",
            &[&tenant, &event, &carton, &picker],
        )
        .await
        .expect("open event")
        .get(0);

    tx.execute(
        "UPDATE package SET sealed_at = NULL WHERE id = $1",
        &[&carton],
    )
    .await
    .expect("clear sealed_at");

    let sealed_after: Option<chrono::DateTime<chrono::Utc>> = tx
        .query_one("SELECT sealed_at FROM package WHERE id = $1", &[&carton])
        .await
        .unwrap()
        .get(0);
    assert!(sealed_after.is_none(), "open clears sealed_at");

    let status_after: Option<String> = tx
        .query_one("SELECT status FROM package WHERE id = $1", &[&carton])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        status_before, status_after,
        "open must not UPDATE status; the fold does that"
    );

    let kind: String = tx
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
    assert_eq!(kind, "opened");
    assert!(event_id != Uuid::nil());

    tx.rollback().await.unwrap();

    if assume_role {
        client.batch_execute("RESET ROLE").await.ok();
        client
            .batch_execute(
                // Same ordering as the other cleanups here, and for the same
                // reason: `projection_package_stamp` writes
                // `package.placement_event_id`, so once a maintainer has run the
                // package points at its own event and deleting the event first
                // violates a foreign key. Checked rather than discarded — `.ok()`
                // turned that into a row left behind on some runs and nothing
                // said so.
                "UPDATE package SET placement_event_id = NULL,
                                    placement_occurred_at = NULL
                  WHERE id = '9ac00000-0000-0000-0000-0000000000d2';
                 DELETE FROM package_containment WHERE package_id = '9ac00000-0000-0000-0000-0000000000d2'
                                                    OR parent_package_id = '9ac00000-0000-0000-0000-0000000000d2';
                 DELETE FROM stock WHERE holder_package_id = '9ac00000-0000-0000-0000-0000000000d2';
                 DELETE FROM package_event WHERE package_id = '9ac00000-0000-0000-0000-0000000000d2';
                 DELETE FROM package WHERE id = '9ac00000-0000-0000-0000-0000000000d2';
                 DELETE FROM client_event WHERE client_event_id = 'ce000000-0000-0000-0000-0000000000d2';",
            )
            .await
            .expect("the test removes what it committed");
    }
}

#[tokio::test]
async fn despatch_writes_event_and_movement_without_to_side() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let line = Uuid::parse_str(LINE_SHIPPED).unwrap();
    let carton = Uuid::parse_str(CARTON_D).unwrap();
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let picker = Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap();
    let event = Uuid::parse_str("ce000000-0000-0000-0000-0000000000e1").unwrap();
    let movement = Uuid::parse_str("5b000000-0000-0000-0000-0000000000e1").unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let despatched_before: i64 = tx
        .query_one(
            "SELECT despatched_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap()
        .get(0);

    // Stock still in CARTON-D (5 units from fixture partial reverse).
    let stock = tx
        .query_one(
            "SELECT id, lot_id, status_id, owner_id, item_id, quantity
               FROM stock WHERE holder_package_id = $1 AND quantity > 0 LIMIT 1",
            &[&carton],
        )
        .await
        .expect("package-held stock");
    let lot: Option<Uuid> = stock.get(1);
    let status: Uuid = stock.get(2);
    let owner: Uuid = stock.get(3);
    let item: Uuid = stock.get(4);
    let qty: i64 = stock.get(5);

    tx.execute(
        "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
             submitted_at, received_at)
         VALUES ($1, $2, $3, $4, now(), now())
         ON CONFLICT DO NOTHING",
        &[&tenant, &event, &site, &picker],
    )
    .await
    .expect("client_event");

    tx.execute(
        "INSERT INTO package_event (
             tenant_id, client_event_id, package_id, kind, source,
             occurred_at, recorded_by_id)
         VALUES ($1, $2, $3, 'despatched', 'operator_scan', now(), $4)",
        &[&tenant, &event, &carton, &picker],
    )
    .await
    .expect("despatch event");

    tx.execute(
        "INSERT INTO stock_movement (
             id, tenant_id, client_event_id, item_id, quantity,
             from_package_id, from_lot_id, from_status_id, from_owner_id,
             reason, occurred_at, recorded_by_id, fulfilment_line_id)
         VALUES (
             $1, $2, $3, $4, $5,
             $6, $7, $8, $9,
             'despatch', now(), $10, $11)",
        &[
            &movement,
            &tenant,
            &event,
            &item,
            &qty,
            &carton,
            &lot,
            &status,
            &owner,
            &picker,
            &line,
        ],
    )
    .await
    .expect("despatch movement");

    let no_to: bool = tx
        .query_one(
            "SELECT to_location_id IS NULL AND to_package_id IS NULL
               FROM stock_movement WHERE id = $1",
            &[&movement],
        )
        .await
        .unwrap()
        .get(0);
    assert!(no_to, "despatch has no to side");

    let despatched_after: i64 = tx
        .query_one(
            "SELECT despatched_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        despatched_after, despatched_before,
        "despatch must not UPDATE the projection"
    );

    tx.rollback().await.unwrap();
}

// ---------------------------------------------------------------------------
// D107 Phase A: live ledger view matches the fixture fold numbers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn live_ledger_progress_matches_the_shipped_line_fold() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let line = Uuid::parse_str(LINE_SHIPPED).unwrap();
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Same recursive fold as ledger_views::line_progress_ledger (D99/D103 scoped).
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
            &[&line],
        )
        .await
        .expect("live ledger fold");

    assert_eq!(row.get::<_, i64>(0), 20, "covered");
    assert_eq!(row.get::<_, i64>(1), 20, "picked");
    assert_eq!(row.get::<_, i64>(2), 20, "packed");
    assert_eq!(row.get::<_, i64>(3), 15, "despatched nets the reverse");
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn package_status_ledger_reads_the_winning_event() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let carton = Uuid::parse_str(CARTON_D).unwrap();
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let kind: String = tx
        .query_one(
            "SELECT kind
               FROM package_event
              WHERE package_id = $1
                AND kind IN ('created','placed','contained','sealed','opened','despatched','voided')
              ORDER BY occurred_at DESC, recorded_at DESC, id DESC
              LIMIT 1",
            &[&carton],
        )
        .await
        .expect("winning event")
        .get(0);
    assert_eq!(kind, "despatched");
    tx.commit().await.unwrap();
}

// ---------------------------------------------------------------------------
// D107 Phase B: dirty mark + Phase C: refresh rate limit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mark_dirty_and_refresh_tenant_as_the_app() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Clear any prior dirty / on-demand stamp for a clean rate-limit window.
    // App has DELETE on projection_dirty; last_on_demand_at is updated by the
    // definer, not by the app directly.
    tx.execute("DELETE FROM projection_dirty WHERE tenant_id = $1", &[&tenant])
        .await
        .ok();

    tx.execute(
        "SELECT projection_mark_dirty($1, 'test')",
        &[&tenant],
    )
    .await
    .expect("app may mark dirty");

    let dirty: i64 = tx
        .query_one(
            "SELECT count(*) FROM projection_dirty WHERE tenant_id = $1",
            &[&tenant],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(dirty, 1, "tenant is in the dirty set");

    // Accepted refresh (or rate-limited NULL if something else just refreshed).
    let n: Option<i64> = tx
        .query_one("SELECT projection_refresh_tenant($1)", &[&tenant])
        .await
        .expect("app may call the wrapper")
        .get(0);

    if n.is_some() {
        // Accepted: dirty cleared.
        let remaining: i64 = tx
            .query_one(
                "SELECT count(*) FROM projection_dirty WHERE tenant_id = $1",
                &[&tenant],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(remaining, 0, "accepted refresh clears dirty");

        // Immediate second call must be rate-limited (NULL).
        let second: Option<i64> = tx
            .query_one("SELECT projection_refresh_tenant($1)", &[&tenant])
            .await
            .unwrap()
            .get(0);
        assert!(
            second.is_none(),
            "second call within 5s returns NULL (rate limited), got {second:?}"
        );
    } else {
        // Already rate-limited from a prior run; still a success for the contract.
        eprintln!("refresh was already rate-limited; dirty may remain until gap elapses");
    }

    // Cannot rebuild another tenant.
    let other = Uuid::parse_str(BETA).unwrap();
    let cross = tx
        .query_one("SELECT projection_refresh_tenant($1)", &[&other])
        .await;
    assert!(
        cross.is_err(),
        "refresh must refuse another tenant"
    );

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn the_app_cannot_execute_projection_run_all() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();
    let forbidden = tx
        .query("SELECT projection_run_all($1)", &[&tenant])
        .await;
    assert!(
        forbidden.is_err(),
        "app must not hold EXECUTE on projection_run_all (D95)"
    );
    tx.rollback().await.unwrap();
}

// ---------------------------------------------------------------------------
// Corrections, moves, receipts, scheduler drain
// ---------------------------------------------------------------------------

#[tokio::test]
async fn correction_mirrors_and_does_not_update_the_target() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let person = Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap();
    let event = Uuid::parse_str("ce000000-0000-0000-0000-0000000000e1").unwrap();
    // The fixture putaway/move of 40 from A-01-1 to B-01-1.
    let target = Uuid::parse_str("5b000000-0000-0000-0000-000000000002").unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let reason: Uuid = tx
        .query_one(
            "SELECT id FROM adjustment_reason WHERE code = 'miscount' AND tenant_id IS NULL",
            &[],
        )
        .await
        .unwrap()
        .get(0);

    let target_row = tx
        .query_one(
            "SELECT item_id, quantity, occurred_at,
                    from_location_id, from_package_id, from_lot_id, from_status_id, from_owner_id,
                    to_location_id, to_package_id, to_lot_id, to_status_id, to_owner_id
               FROM stock_movement WHERE id = $1",
            &[&target],
        )
        .await
        .expect("target move");

    let item: Uuid = target_row.get(0);
    let qty: i64 = 2; // reverse 2 of 40
    let occurred: chrono::DateTime<chrono::Utc> = target_row.get(2);
    let from_loc: Option<Uuid> = target_row.get(8); // mirror: from = target.to
    let from_pkg: Option<Uuid> = target_row.get(9);
    let from_lot: Option<Uuid> = target_row.get(10);
    let from_status: Option<Uuid> = target_row.get(11);
    let from_owner: Option<Uuid> = target_row.get(12);
    let to_loc: Option<Uuid> = target_row.get(3);
    let to_pkg: Option<Uuid> = target_row.get(4);
    let to_lot: Option<Uuid> = target_row.get(5);
    let to_status: Option<Uuid> = target_row.get(6);
    let to_owner: Option<Uuid> = target_row.get(7);

    tx.execute(
        "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
             submitted_at, received_at)
         VALUES ($1, $2, $3, $4, now(), now()) ON CONFLICT DO NOTHING",
        &[&tenant, &event, &site, &person],
    )
    .await
    .unwrap();

    let id: Uuid = tx
        .query_one(
            "INSERT INTO stock_movement (
                 tenant_id, client_event_id, item_id, quantity,
                 from_location_id, from_package_id, from_lot_id, from_status_id, from_owner_id,
                 to_location_id, to_package_id, to_lot_id, to_status_id, to_owner_id,
                 reason, occurred_at, recorded_by_id,
                 reverses_movement_id, adjustment_reason_id)
             VALUES (
                 $1, $2, $3, $4,
                 $5, $6, $7, $8, $9,
                 $10, $11, $12, $13, $14,
                 'adjustment', $15, $16, $17, $18)
             RETURNING id",
            &[
                &tenant,
                &event,
                &item,
                &qty,
                &from_loc,
                &from_pkg,
                &from_lot,
                &from_status,
                &from_owner,
                &to_loc,
                &to_pkg,
                &to_lot,
                &to_status,
                &to_owner,
                &occurred,
                &person,
                &target,
                &reason,
            ],
        )
        .await
        .expect("correction insert")
        .get(0);

    let n: i64 = tx
        .query_one(
            "SELECT count(*) FROM stock_movement
              WHERE id = $1 AND reverses_movement_id = $2 AND quantity = 2",
            &[&id, &target],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 1);

    let target_qty: i64 = tx
        .query_one("SELECT quantity FROM stock_movement WHERE id = $1", &[&target])
        .await
        .unwrap()
        .get(0);
    assert_eq!(target_qty, 40, "correction must not UPDATE the target");

    tx.execute(
        "SELECT projection_mark_dirty($1, 'correction')",
        &[&tenant],
    )
    .await
    .unwrap();

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn move_writes_ledger_and_marks_dirty() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let person = Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap();
    let event = Uuid::parse_str("ce000000-0000-0000-0000-0000000000e2").unwrap();
    let dock = Uuid::parse_str("10c00000-0000-0000-0000-000000000003").unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Cell with available stock on A-01-1 (fixture).
    let cell = tx
        .query_one(
            "SELECT id, item_id, holder_location_id, lot_id, status_id, owner_id
               FROM stock
              WHERE holder_location_id = '10c00000-0000-0000-0000-000000000001'
                AND available_quantity >= 5
              ORDER BY available_quantity DESC LIMIT 1",
            &[],
        )
        .await
        .expect("source cell");
    let stock_id: Uuid = cell.get(0);
    let item: Uuid = cell.get(1);
    let from_loc: Uuid = cell.get(2);
    let lot: Option<Uuid> = cell.get(3);
    let status: Uuid = cell.get(4);
    let owner: Uuid = cell.get(5);

    tx.execute(
        "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
             submitted_at, received_at)
         VALUES ($1, $2, $3, $4, now(), now()) ON CONFLICT DO NOTHING",
        &[&tenant, &event, &site, &person],
    )
    .await
    .unwrap();

    let movement_id: Uuid = tx
        .query_one(
            "INSERT INTO stock_movement (
                 tenant_id, client_event_id, item_id, quantity,
                 from_location_id, from_lot_id, from_status_id, from_owner_id,
                 to_location_id, to_lot_id, to_status_id, to_owner_id,
                 reason, occurred_at, recorded_by_id)
             VALUES (
                 $1, $2, $3, 5,
                 $4, $5, $6, $7,
                 $8, $5, $6, $7,
                 'putaway', now(), $9)
             RETURNING id",
            &[
                &tenant, &event, &item, &from_loc, &lot, &status, &owner, &dock, &person,
            ],
        )
        .await
        .expect("putaway")
        .get(0);
    assert!(movement_id != Uuid::nil());
    // stock_id only used to choose the cell; movement does not name it.
    let _ = stock_id;

    tx.execute(
        "SELECT projection_mark_dirty($1, 'putaway')",
        &[&tenant],
    )
    .await
    .unwrap();
    let dirty: i64 = tx
        .query_one(
            "SELECT count(*) FROM projection_dirty WHERE tenant_id = $1",
            &[&tenant],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(dirty, 1);

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn scheduler_role_drains_dirty_tenants() {
    let Some((u, _)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    // Connect as owner/superuser so we can SET ROLE to scheduler and app.
    let (client, connection) = tokio_postgres::connect(&u, NoTls).await.expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let client = client;
    let tenant = Uuid::parse_str(ALPHA).unwrap();

    client
        .batch_execute("SET ROLE spork_app")
        .await
        .expect("app");
    client
        .execute(
            "SELECT set_config('spork.tenant_id', $1::text, false)",
            &[&ALPHA],
        )
        .await
        .unwrap();
    client
        .execute(
            "SELECT projection_mark_dirty($1, 'scheduler_test')",
            &[&tenant],
        )
        .await
        .expect("mark dirty");

    client.batch_execute("RESET ROLE").await.ok();
    client
        .batch_execute("SET ROLE spork_scheduler")
        .await
        .expect("scheduler");

    // The drain walks tenants under SET LOCAL inside the definer; the scheduler
    // role itself does not scan projection_dirty without a tenant (S9).
    let _n: i64 = client
        .query_one("SELECT projection_run_dirty()", &[])
        .await
        .expect("drain")
        .get(0);

    client.batch_execute("RESET ROLE").await.ok();
    client
        .batch_execute("SET ROLE spork_app")
        .await
        .expect("app again");
    client
        .execute(
            "SELECT set_config('spork.tenant_id', $1::text, false)",
            &[&ALPHA],
        )
        .await
        .unwrap();
    let dirty_after: i64 = client
        .query_one(
            "SELECT count(*) FROM projection_dirty WHERE tenant_id = $1",
            &[&tenant],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(dirty_after, 0, "drain clears the tenant");

    client.batch_execute("RESET ROLE").await.ok();
}

// ---------------------------------------------------------------------------
// Allocation (directed claim: cell → fulfilment line)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn allocating_claims_a_cell_and_does_not_write_covered() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    // New fulfilment line so coverage starts at zero — app cannot INSERT fulfilment.
    let (setup, connection) = tokio_postgres::connect(&u, NoTls).await.expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let fulfilment = Uuid::parse_str("f01f0000-0000-0000-0000-000000000001").unwrap();
    let order_line = Uuid::parse_str("01e00000-0000-0000-0000-000000000001").unwrap();
    let line = Uuid::parse_str("f11e0000-0000-0000-0000-0000000000a1").unwrap();

    setup
        .batch_execute(&format!(
            "INSERT INTO fulfilment_line (id, tenant_id, fulfilment_id, order_line_id, quantity)
             VALUES ('{line}', '{ALPHA}', '{fulfilment}', '{order_line}', 15)
             ON CONFLICT (id) DO NOTHING;"
        ))
        .await
        .expect("new line for allocation test");

    let mut client = connect(&u, assume_role).await;
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Location-held cell with free stock (A-01-1 after fixture folds).
    let cell = tx
        .query_one(
            "SELECT id, available_quantity, item_id
               FROM stock
              WHERE holder_location_id = '10c00000-0000-0000-0000-000000000001'
                AND available_quantity >= 10
              ORDER BY available_quantity DESC
              LIMIT 1",
            &[],
        )
        .await
        .expect("available cell");
    let stock_id: Uuid = cell.get(0);
    let available: i64 = cell.get(1);

    let covered_before: i64 = tx
        .query_one(
            "SELECT covered_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap()
        .get(0);

    let allocation_id: Uuid = tx
        .query_one(
            "INSERT INTO stock_allocation (
                 tenant_id, stock_id, fulfilment_line_id, quantity,
                 state, firm, bound_at)
             VALUES ($1, $2, $3, 10, 'allocated', false, now())
             RETURNING id",
            &[&tenant, &stock_id, &line],
        )
        .await
        .expect("allocation insert")
        .get(0);

    // Live coverage from the intention table (same fold as ledger_views).
    let live_covered: i64 = tx
        .query_one(
            "SELECT coalesce(sum(quantity), 0)::bigint
               FROM stock_allocation
              WHERE fulfilment_line_id = $1
                AND state IN ('allocated','picking','picked','packed','fulfilled')",
            &[&line],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(live_covered, 10, "live cover includes the new claim");

    let covered_after: i64 = tx
        .query_one(
            "SELECT covered_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        covered_after, covered_before,
        "handler must not UPDATE the projection; the fold does that"
    );

    tx.execute(
        "SELECT projection_mark_dirty($1, 'allocation')",
        &[&tenant],
    )
    .await
    .unwrap();

    assert!(allocation_id != Uuid::nil());
    assert!(available >= 10);

    tx.rollback().await.unwrap();

    if assume_role {
        let _ = client.batch_execute("RESET ROLE").await;
    }
    let _ = client
        .batch_execute(&format!(
            "DELETE FROM stock_allocation WHERE fulfilment_line_id = '{line}';
             DELETE FROM fulfilment_line WHERE id = '{line}';"
        ))
        .await;
}

#[tokio::test]
async fn over_cover_is_refused_by_the_pure_check() {
    use spork_server::allocating::{self, ProposedAllocation};

    let line = allocating::FulfilmentLine {
        id: Uuid::nil(),
        tenant_id: Uuid::nil(),
        item_id: Uuid::from_u128(1),
        quantity: 20,
        covered_quantity: 20,
        fulfilment_cancelled: false,
    };
    let cell = allocating::StockCell {
        id: Uuid::nil(),
        tenant_id: Uuid::nil(),
        item_id: Uuid::from_u128(1),
        holder_location_id: Some(Uuid::from_u128(2)),
        holder_package_id: None,
        quantity: 100,
        available_quantity: 100,
    };
    let proposed = ProposedAllocation {
        tenant_id: Uuid::nil(),
        fulfilment_line_id: Uuid::nil(),
        stock_id: Uuid::nil(),
        quantity: 1,
        firm: false,
        bound_at: chrono::Utc::now(),
    };
    let problems = allocating::check(&proposed, Some(&line), Some(&cell));
    assert!(
        problems
            .iter()
            .any(|p| matches!(p, allocating::Problem::OverCovers { .. })),
        "{problems:?}"
    );
    assert!(problems.iter().any(allocating::is_hard));
}

#[tokio::test]
async fn releasing_an_allocated_claim_sets_released_and_not_covered() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    // Fixture soft allocation on the shipped line (20 units, state allocated).
    let allocation = Uuid::parse_str("a110c000-0000-0000-0000-000000000002").unwrap();
    let line = Uuid::parse_str(LINE_SHIPPED).unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let state_before: String = tx
        .query_one(
            "SELECT state FROM stock_allocation WHERE id = $1",
            &[&allocation],
        )
        .await
        .expect("fixture allocation")
        .get(0);
    assert_eq!(state_before, "allocated");

    let covered_proj_before: i64 = tx
        .query_one(
            "SELECT covered_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap()
        .get(0);

    // Same act POST /allocations/{id}/release performs.
    let n = tx
        .execute(
            "UPDATE stock_allocation SET state = 'released' WHERE id = $1 AND state = 'allocated'",
            &[&allocation],
        )
        .await
        .expect("release");
    assert_eq!(n, 1);

    let state_after: String = tx
        .query_one(
            "SELECT state FROM stock_allocation WHERE id = $1",
            &[&allocation],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(state_after, "released");

    let live_covered: i64 = tx
        .query_one(
            "SELECT coalesce(sum(quantity), 0)::bigint
               FROM stock_allocation
              WHERE fulfilment_line_id = $1
                AND state IN ('allocated','picking','picked','packed','fulfilled')",
            &[&line],
        )
        .await
        .unwrap()
        .get(0);
    // Fixture had 20 covering + 5 released; after releasing the 20, only the
    // pre-existing released row remains out of cover — live cover is 0.
    assert_eq!(live_covered, 0, "released claims cover nothing");

    let covered_proj_after: i64 = tx
        .query_one(
            "SELECT covered_quantity FROM fulfilment_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        covered_proj_after, covered_proj_before,
        "release must not UPDATE the projection column"
    );

    tx.execute(
        "SELECT projection_mark_dirty($1, 'allocation_release')",
        &[&tenant],
    )
    .await
    .unwrap();

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn firm_release_requires_force_in_pure_check() {
    use spork_server::allocating::{self, Allocation, ProposedRelease};

    let a = Allocation {
        id: Uuid::nil(),
        tenant_id: Uuid::nil(),
        state: "allocated".into(),
        firm: true,
        fulfilment_line_id: None,
        quantity: 5,
    };
    let (p, _) = allocating::check_release(
        &ProposedRelease {
            tenant_id: Uuid::nil(),
            allocation_id: Uuid::nil(),
            force: false,
        },
        Some(&a),
    );
    assert!(p.contains(&allocating::ReleaseProblem::FirmWithoutForce));
}

#[tokio::test]
async fn placing_writes_a_placed_event_and_not_resolved_location() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let person = Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap();
    // Minted per run rather than fixed. The rows below are committed on a second
    // connection so the carton outlives this test's rollback, and the cleanup at
    // the bottom is best-effort: an assertion that fires skips it. With a fixed
    // id that leak is permanent and shared — this test held the same
    // `client_event` id as `inbound_walk.rs`, so one leaked row failed that walk
    // on every subsequent run until somebody deleted it by hand. Minted ids make
    // a leak an orphan nobody collides with.
    let event = Uuid::now_v7();
    let carton = Uuid::now_v7();
    let created_event = Uuid::now_v7();
    let fulfilment = Uuid::parse_str("f01f0000-0000-0000-0000-000000000001").unwrap();
    let dock = Uuid::parse_str("10c00000-0000-0000-0000-000000000003").unwrap();
    let bin_a = Uuid::parse_str("10c00000-0000-0000-0000-000000000001").unwrap();

    // Setup package as owner if app cannot INSERT package freely in all envs.
    if assume_role {
        let (setup, conn) = tokio_postgres::connect(&u, NoTls).await.unwrap();
        tokio::spawn(async move {
            let _ = conn.await;
        });
        setup
            .batch_execute(&format!(
                "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id, submitted_at, received_at)
                 VALUES ('{ALPHA}', '{event}', '{site}', '{person}', now(), now()) ON CONFLICT DO NOTHING;
                 INSERT INTO package (id, tenant_id, fulfilment_id, sequence)
                 VALUES ('{carton}', '{ALPHA}', '{fulfilment}', 11) ON CONFLICT DO NOTHING;
                 INSERT INTO package_event (id, tenant_id, client_event_id, package_id, kind, source, occurred_at, recorded_by_id, location_id)
                 VALUES ('{created_event}', '{ALPHA}', '{event}', '{carton}',
                         'created', 'operator_scan', now(), '{person}', '{dock}') ON CONFLICT DO NOTHING;"
            ))
            .await
            .expect("setup place carton");
    }

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let resolved_before: Option<Uuid> = tx
        .query_one(
            "SELECT resolved_location_id FROM package WHERE id = $1",
            &[&carton],
        )
        .await
        .unwrap()
        .get(0);

    tx.execute(
        "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
             submitted_at, received_at)
         VALUES ($1, $2, $3, $4, now(), now())
         ON CONFLICT DO NOTHING",
        &[&tenant, &event, &site, &person],
    )
    .await
    .ok();

    let event_id: Uuid = tx
        .query_one(
            "INSERT INTO package_event (
                 tenant_id, client_event_id, package_id, kind, source,
                 occurred_at, recorded_by_id, location_id)
             VALUES ($1, $2, $3, 'placed', 'operator_scan', now(), $4, $5)
             RETURNING id",
            &[&tenant, &event, &carton, &person, &bin_a],
        )
        .await
        .expect("placed event")
        .get(0);

    let kind: String = tx
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
    assert_eq!(kind, "placed");

    let resolved_after: Option<Uuid> = tx
        .query_one(
            "SELECT resolved_location_id FROM package WHERE id = $1",
            &[&carton],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        resolved_before, resolved_after,
        "place must not UPDATE resolved_location_id; the fold does that"
    );
    assert!(event_id != Uuid::nil());

    tx.execute(
        "SELECT projection_mark_dirty($1, 'package_place')",
        &[&tenant],
    )
    .await
    .unwrap();

    tx.rollback().await.unwrap();

    if assume_role {
        client.batch_execute("RESET ROLE").await.ok();
        // **The projections point back at what they were folded from.**
        // `projection_package_stamp` writes `package.placement_event_id`, so once
        // a maintainer has run the package references its own event and deleting
        // the event first violates a foreign key; `package_containment` is folded
        // from the same log. Cleared in dependency order — the pointer, then the
        // folds, then the facts — and the result is checked rather than
        // discarded. `.ok()` here hid this as an intermittent leak: the test
        // panicked, its cleanup never ran, and a carton stayed behind on roughly
        // one run in three. A cleanup nobody watches is what poisoned this
        // database once already.
        client
            .batch_execute(&format!(
                "UPDATE package SET placement_event_id = NULL, placement_occurred_at = NULL
                  WHERE id = '{carton}';
                 DELETE FROM package_containment WHERE package_id = '{carton}'
                                                    OR parent_package_id = '{carton}';
                 DELETE FROM stock WHERE holder_package_id = '{carton}';
                 DELETE FROM package_event WHERE package_id = '{carton}';
                 DELETE FROM package WHERE id = '{carton}';
                 DELETE FROM client_event WHERE client_event_id = '{event}';"
            ))
            .await
            .expect("the test removes what it committed");
    }
}

#[tokio::test]
async fn containing_writes_contained_event_with_parent() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let person = Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap();
    let event = Uuid::parse_str("ce000000-0000-0000-0000-0000000000c9").unwrap();
    let pallet = Uuid::parse_str("9ac00000-0000-0000-0000-0000000000c9").unwrap();
    let carton = Uuid::parse_str("9ac00000-0000-0000-0000-0000000000ca").unwrap();
    let fulfilment = Uuid::parse_str("f01f0000-0000-0000-0000-000000000001").unwrap();
    let dock = Uuid::parse_str("10c00000-0000-0000-0000-000000000003").unwrap();

    if assume_role {
        let (setup, conn) = tokio_postgres::connect(&u, NoTls).await.unwrap();
        tokio::spawn(async move {
            let _ = conn.await;
        });
        setup
            .batch_execute(&format!(
                "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id, submitted_at, received_at)
                 VALUES ('{ALPHA}', '{event}', '{site}', '{person}', now(), now()) ON CONFLICT DO NOTHING;
                 INSERT INTO package (id, tenant_id, fulfilment_id, sequence)
                 VALUES ('{pallet}', '{ALPHA}', '{fulfilment}', 20),
                        ('{carton}', '{ALPHA}', '{fulfilment}', 21)
                 ON CONFLICT DO NOTHING;
                 INSERT INTO package_event (id, tenant_id, client_event_id, package_id, kind, source, occurred_at, recorded_by_id, location_id)
                 VALUES ('9ae00000-0000-0000-0000-0000000000c9', '{ALPHA}', '{event}', '{pallet}',
                         'created', 'operator_scan', now(), '{person}', '{dock}'),
                        ('9ae00000-0000-0000-0000-0000000000ca', '{ALPHA}', '{event}', '{carton}',
                         'created', 'operator_scan', now(), '{person}', '{dock}')
                 ON CONFLICT DO NOTHING;"
            ))
            .await
            .expect("setup contain packages");
    }

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let parent_before: Option<Uuid> = tx
        .query_one(
            "SELECT parent_package_id FROM package WHERE id = $1",
            &[&carton],
        )
        .await
        .unwrap()
        .get(0);

    tx.execute(
        "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
             submitted_at, received_at)
         VALUES ($1, $2, $3, $4, now(), now())
         ON CONFLICT DO NOTHING",
        &[&tenant, &event, &site, &person],
    )
    .await
    .ok();

    let event_id: Uuid = tx
        .query_one(
            "INSERT INTO package_event (
                 tenant_id, client_event_id, package_id, kind, source,
                 occurred_at, recorded_by_id, parent_package_id)
             VALUES ($1, $2, $3, 'contained', 'operator_scan', now(), $4, $5)
             RETURNING id",
            &[&tenant, &event, &carton, &person, &pallet],
        )
        .await
        .expect("contained event")
        .get(0);

    let parent_on_event: Uuid = tx
        .query_one(
            "SELECT parent_package_id FROM package_event WHERE id = $1",
            &[&event_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(parent_on_event, pallet);

    let kind: String = tx
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
    assert_eq!(kind, "contained");

    let parent_after: Option<Uuid> = tx
        .query_one(
            "SELECT parent_package_id FROM package WHERE id = $1",
            &[&carton],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        parent_before, parent_after,
        "contain must not UPDATE parent_package_id; the fold does that"
    );

    tx.execute(
        "SELECT projection_mark_dirty($1, 'package_contain')",
        &[&tenant],
    )
    .await
    .unwrap();

    tx.rollback().await.unwrap();

    if assume_role {
        client.batch_execute("RESET ROLE").await.ok();
        client
            .batch_execute(
                "DELETE FROM package_event WHERE package_id IN (
                     '9ac00000-0000-0000-0000-0000000000c9',
                     '9ac00000-0000-0000-0000-0000000000ca');
                 DELETE FROM package WHERE id IN (
                     '9ac00000-0000-0000-0000-0000000000c9',
                     '9ac00000-0000-0000-0000-0000000000ca');
                 DELETE FROM client_event WHERE client_event_id = 'ce000000-0000-0000-0000-0000000000c9';",
            )
            .await
            .ok();
    }
}

#[tokio::test]
async fn inventory_adjust_posts_delta_as_world_event_movement() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let person = Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap();
    let event = Uuid::parse_str("ce000000-0000-0000-0000-0000000000ad").unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let cell = tx
        .query_one(
            "SELECT id, quantity, holder_location_id, holder_package_id, lot_id,
                    status_id, owner_id, item_id
               FROM stock
              WHERE quantity >= 10 AND holder_location_id IS NOT NULL
              ORDER BY quantity DESC
              LIMIT 1",
            &[],
        )
        .await
        .expect("cell to count");
    let stock_id: Uuid = cell.get(0);
    let system: i64 = cell.get(1);
    let from_loc: Option<Uuid> = cell.get(2);
    let from_pkg: Option<Uuid> = cell.get(3);
    let lot: Option<Uuid> = cell.get(4);
    let status: Uuid = cell.get(5);
    let owner: Uuid = cell.get(6);
    let item: Uuid = cell.get(7);

    let reason: Uuid = tx
        .query_one(
            "SELECT id FROM adjustment_reason
              WHERE code = 'damaged' AND tenant_id IS NULL",
            &[],
        )
        .await
        .unwrap()
        .get(0);

    // Count 3 short of system → decrease movement of 3.
    let counted = system - 3;
    assert!(counted >= 0);

    tx.execute(
        "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
             submitted_at, received_at)
         VALUES ($1, $2, $3, $4, now(), now())
         ON CONFLICT DO NOTHING",
        &[&tenant, &event, &site, &person],
    )
    .await
    .unwrap();

    let movement_id: Uuid = tx
        .query_one(
            "INSERT INTO stock_movement (
                 tenant_id, client_event_id, item_id, quantity,
                 from_location_id, from_package_id, from_lot_id,
                 from_status_id, from_owner_id,
                 reason, occurred_at, recorded_by_id, adjustment_reason_id)
             VALUES (
                 $1, $2, $3, 3,
                 $4, $5, $6, $7, $8,
                 'adjustment', now(), $9, $10)
             RETURNING id",
            &[
                &tenant, &event, &item, &from_loc, &from_pkg, &lot, &status, &owner, &person,
                &reason,
            ],
        )
        .await
        .expect("adjustment movement")
        .get(0);

    let n: i64 = tx
        .query_one(
            "SELECT count(*) FROM stock_movement
              WHERE id = $1 AND reason = 'adjustment'
                AND reverses_movement_id IS NULL
                AND quantity = 3
                AND adjustment_reason_id = $2",
            &[&movement_id, &reason],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 1);

    let qty_after: i64 = tx
        .query_one("SELECT quantity FROM stock WHERE id = $1", &[&stock_id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        qty_after, system,
        "adjust must not UPDATE stock.quantity; the fold does that"
    );

    // Pure check: counted match is soft no-op.
    use spork_server::adjusting::{self, ProposedAdjustment};
    let (problems, dir) = adjusting::check(
        &ProposedAdjustment {
            tenant_id: tenant,
            stock_id,
            counted_quantity: system,
            reason: adjusting::AdjustmentReason {
                id: reason,
                class: "world_event".into(),
                code: "damaged".into(),
            },
            discrepancy_id: None,
        },
        Some(&adjusting::StockCell {
            id: stock_id,
            tenant_id: tenant,
            item_id: item,
            holder_location_id: from_loc,
            holder_package_id: from_pkg,
            lot_id: lot,
            status_id: status,
            owner_id: owner,
            quantity: system,
        }),
        None,
    );
    assert!(dir.is_none());
    assert!(problems
        .iter()
        .any(|p| matches!(p, adjusting::Problem::ZeroVariance { .. })));
    let _ = counted;

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn stock_count_raises_variance_finding_without_ledger_write() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let person = Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap();
    let event = Uuid::parse_str("ce000000-0000-0000-0000-0000000000cc").unwrap();

    let tx = client.transaction().await.unwrap();
    // stock_count may not exist if migration 63 not applied.
    let has_table: bool = tx
        .query_one(
            "SELECT EXISTS (
                 SELECT 1 FROM information_schema.tables
                  WHERE table_schema = 'public' AND table_name = 'stock_count')",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    if !has_table {
        eprintln!("stock_count missing: skip (apply migration 63)");
        tx.rollback().await.ok();
        return;
    }

    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let cell = tx
        .query_one(
            "SELECT id, quantity, item_id, holder_location_id, holder_package_id,
                    lot_id, status_id, owner_id
               FROM stock
              WHERE quantity >= 5 AND holder_location_id IS NOT NULL
              ORDER BY quantity DESC LIMIT 1",
            &[],
        )
        .await
        .expect("cell");
    let stock_id: Uuid = cell.get(0);
    let system: i64 = cell.get(1);
    let item: Uuid = cell.get(2);
    let loc: Option<Uuid> = cell.get(3);
    let pkg: Option<Uuid> = cell.get(4);
    let lot: Option<Uuid> = cell.get(5);
    let status: Uuid = cell.get(6);
    let owner: Uuid = cell.get(7);
    let counted = system - 2;
    assert!(counted >= 0);

    let movements_before: i64 = tx
        .query_one("SELECT count(*) FROM stock_movement", &[])
        .await
        .unwrap()
        .get(0);

    tx.execute(
        "INSERT INTO client_event (tenant_id, client_event_id, site_id, recorded_by_id,
             submitted_at, received_at)
         VALUES ($1, $2, $3, $4, now(), now())
         ON CONFLICT DO NOTHING",
        &[&tenant, &event, &site, &person],
    )
    .await
    .unwrap();

    let count_id: Uuid = tx
        .query_one(
            "INSERT INTO stock_count (
                 tenant_id, stock_id, item_id,
                 holder_location_id, holder_package_id, lot_id, status_id, owner_id,
                 counted_quantity, system_quantity,
                 counted_at, client_event_id, recorded_by_id, blind)
             VALUES (
                 $1, $2, $3, $4, $5, $6, $7, $8,
                 $9, $10, now(), $11, $12, false)
             RETURNING id",
            &[
                &tenant, &stock_id, &item, &loc, &pkg, &lot, &status, &owner, &counted, &system,
                &event, &person,
            ],
        )
        .await
        .expect("stock_count")
        .get(0);

    let disc_id: Uuid = tx
        .query_one(
            "INSERT INTO discrepancy (
                 tenant_id, kind,
                 item_id, holder_location_id, holder_package_id, lot_id, status_id, owner_id,
                 expected_quantity, observed_quantity,
                 stock_count_id, detail,
                 detected_at, detected_by_id, state)
             VALUES (
                 $1, 'count_variance',
                 $2, $3, $4, $5, $6, $7,
                 -- Through bigint, not straight to numeric. `$8::numeric` makes
                 -- Postgres infer the parameter as numeric, which tokio-postgres
                 -- has no i64 mapping for and no ToSql for String either, so the
                 -- bind was rejected client-side before the statement was ever
                 -- sent. The double cast lets the driver send a bigint.
                 $8::bigint::numeric, $9::bigint::numeric,
                 $10, 'test variance',
                 now(), $11, 'open')
             RETURNING id",
            &[
                &tenant,
                &item,
                &loc,
                &pkg,
                &lot,
                &status,
                &owner,
                &system,
                &counted,
                &count_id,
                &person,
            ],
        )
        .await
        .expect("discrepancy")
        .get(0);

    let movements_after: i64 = tx
        .query_one("SELECT count(*) FROM stock_movement", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        movements_after, movements_before,
        "a count must not write the ledger"
    );

    let kind: String = tx
        .query_one(
            "SELECT kind::text FROM discrepancy WHERE id = $1",
            &[&disc_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(kind, "count_variance");

    let src: Uuid = tx
        .query_one(
            "SELECT stock_count_id FROM discrepancy WHERE id = $1",
            &[&disc_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(src, count_id);

    let qty: i64 = tx
        .query_one("SELECT quantity FROM stock WHERE id = $1", &[&stock_id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(qty, system, "count must not change stock.quantity");

    tx.rollback().await.unwrap();
}


#[tokio::test]
async fn open_discrepancies_list_is_tenant_scoped() {
    let Some((u, assume_role)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume_role).await;
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Same filter as GET /discrepancies?state=open
    let rows = tx
        .query(
            "SELECT d.id, d.kind::text, d.state::text
               FROM discrepancy d
              WHERE d.state::text = ANY ($1)
              ORDER BY d.detected_at DESC
              LIMIT 100",
            &[&vec!["open".to_string()]],
        )
        .await
        .expect("list open findings");
    // May be empty; the shape and RLS are what matter.
    for r in &rows {
        let state: String = r.get(2);
        assert_eq!(state, "open");
    }

    // Beta sees none of alpha's ids.
    let alpha_ids: Vec<Uuid> = rows.iter().map(|r| r.get(0)).collect();
    tx.commit().await.unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&BETA],
    )
    .await
    .unwrap();
    for id in alpha_ids {
        let found = tx
            .query_opt("SELECT 1 FROM discrepancy WHERE id = $1", &[&id])
            .await
            .unwrap();
        assert!(found.is_none(), "beta must not see alpha finding {id}");
    }
    tx.commit().await.unwrap();
}
