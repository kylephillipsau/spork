//! Handing cartons to a carrier, and the count that existed nowhere.
//!
//! The walkthrough's second observation about the data:
//!
//! > The quantity gap is a real model mismatch. NetSuite holds one line per
//! > product/box type; MachShip needs a package count. The count is currently
//! > re-entered by hand in MachShip and exists in neither system beforehand.
//! > Any replacement needs a package-level model, not just a line-item model.
//!
//! Packages are real rows here, so the count is a fold. What the tests below are
//! actually for is the two ways that fold misleads: cartons of one preset with
//! different dimensions, which a carrier would quote at the smallest, and a
//! carton with no weight, which a carrier will weigh and invoice for.

use chrono::Utc;
use spork_server::client_events::{self, NewClientEvent};
use uuid::Uuid;

mod common;
use common::{connect, url_and_role};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const PERSON: &str = "77770000-0000-0000-0000-000000000001";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const DOCK: &str = "10c00000-0000-0000-0000-000000000003";
const FULFILMENT: &str = "f01f0000-0000-0000-0000-000000000001";
const SWIFT_NBD: &str = "ca450000-0000-0000-0000-000000000001";
const SWIFT: &str = "ca440000-0000-0000-0000-000000000002";
const SMALL_BOX: &str = "9a7e0000-0000-0000-0000-0000000000b1";

/// The carrier-line fold the endpoint runs.
const CARRIER_LINES: &str = "
    SELECT pt.name, count(*)::bigint, sum(p.gross_weight_g)::bigint,
           count(DISTINCT (p.length_mm, p.width_mm, p.height_mm)) = 1
      FROM consignment_package cp
      JOIN package p ON p.id = cp.package_id
      LEFT JOIN package_type pt ON pt.id = p.package_type_id
     WHERE cp.consignment_id = $1
     GROUP BY pt.name
     ORDER BY pt.name NULLS LAST";

/// Seal a carton with a preset and dimensions, ready to consign.
#[allow(clippy::too_many_arguments)]
async fn sealed_carton(
    tx: &tokio_postgres::Transaction<'_>,
    tenant: Uuid,
    person: Uuid,
    site: Uuid,
    dock: Uuid,
    fulfilment: Uuid,
    sequence: i32,
    height_mm: i32,
    weight_g: Option<i64>,
) -> Uuid {
    let carton = Uuid::now_v7();
    let ce = Uuid::now_v7();
    let now = Utc::now();
    client_events::claim_act(
        tx,
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
    tx.execute(
        "INSERT INTO package (id, tenant_id, fulfilment_id, sequence, package_type_id,
                              length_mm, width_mm, height_mm, gross_weight_g,
                              dimensions_source, sealed_at)
         VALUES ($1, $2, $3, $4, $5, 320, 240, $6, $7, 'confirmed', now())",
        &[
            &carton,
            &tenant,
            &fulfilment,
            &sequence,
            &Uuid::parse_str(SMALL_BOX).unwrap(),
            &height_mm,
            &weight_g,
        ],
    )
    .await
    .expect("carton");
    // The status column is a fold of package_event, so sealing is an event.
    for kind in ["created", "sealed"] {
        tx.execute(
            "INSERT INTO package_event (
                 tenant_id, client_event_id, package_id, kind, source,
                 occurred_at, recorded_by_id, location_id)
             VALUES ($1, $2, $3, $4, 'operator_scan', $5, $6, $7)",
            &[&tenant, &ce, &carton, &kind, &now, &person, &dock],
        )
        .await
        .unwrap_or_else(|e| panic!("{kind} event: {e}"));
    }
    carton
}

#[tokio::test]
async fn the_carton_count_is_a_fold_not_a_number_somebody_retypes() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let person = Uuid::parse_str(PERSON).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();
    let dock = Uuid::parse_str(DOCK).unwrap();
    let fulfilment = Uuid::parse_str(FULFILMENT).unwrap();
    let consignment = Uuid::now_v7();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Three identical small boxes.
    let mut cartons = vec![];
    for seq in 101..104 {
        cartons.push(
            sealed_carton(&tx, tenant, person, site, dock, fulfilment, seq, 180, Some(4200))
                .await,
        );
    }

    tx.execute(
        "INSERT INTO consignment (id, tenant_id, carrier_id, carrier_service_id)
         VALUES ($1, $2, $3, $4)",
        &[
            &consignment,
            &tenant,
            &Uuid::parse_str(SWIFT).unwrap(),
            &Uuid::parse_str(SWIFT_NBD).unwrap(),
        ],
    )
    .await
    .unwrap();
    for c in &cartons {
        tx.execute(
            "INSERT INTO consignment_package (tenant_id, consignment_id, package_id)
             VALUES ($1, $2, $3)",
            &[&tenant, &consignment, c],
        )
        .await
        .unwrap();
    }

    let rows = tx.query(CARRIER_LINES, &[&consignment]).await.unwrap();
    assert_eq!(rows.len(), 1, "one preset, one carrier line");
    assert_eq!(
        rows[0].get::<_, i64>(1),
        3,
        "three cartons — the number that exists in neither system today"
    );
    assert_eq!(rows[0].get::<_, Option<i64>>(2), Some(12_600), "and their weight");
    assert!(
        rows[0].get::<_, Option<bool>>(3).unwrap_or(false),
        "all three are the same size, so one line quotes them correctly"
    );

    tx.rollback().await.unwrap();
}

/// Cartons of one preset with different sizes must not read as one line.
#[tokio::test]
async fn cartons_that_differ_are_reported_rather_than_averaged() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let person = Uuid::parse_str(PERSON).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();
    let dock = Uuid::parse_str(DOCK).unwrap();
    let fulfilment = Uuid::parse_str(FULFILMENT).unwrap();
    let consignment = Uuid::now_v7();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    // Same preset, two heights — an overfilled carton, which is ordinary.
    let a = sealed_carton(&tx, tenant, person, site, dock, fulfilment, 111, 180, Some(4200)).await;
    let b = sealed_carton(&tx, tenant, person, site, dock, fulfilment, 112, 260, Some(6100)).await;

    tx.execute(
        "INSERT INTO consignment (id, tenant_id) VALUES ($1, $2)",
        &[&consignment, &tenant],
    )
    .await
    .unwrap();
    for c in [a, b] {
        tx.execute(
            "INSERT INTO consignment_package (tenant_id, consignment_id, package_id)
             VALUES ($1, $2, $3)",
            &[&tenant, &consignment, &c],
        )
        .await
        .unwrap();
    }

    let rows = tx.query(CARRIER_LINES, &[&consignment]).await.unwrap();
    assert_eq!(rows.len(), 1, "the preset groups them");
    assert_eq!(rows[0].get::<_, i64>(1), 2);
    assert!(
        !rows[0].get::<_, Option<bool>>(3).unwrap_or(true),
        "and they are NOT uniform, which is what the warning is for: a carrier \
         given one line would be quoted the smaller and invoice the difference"
    );

    tx.rollback().await.unwrap();
}

/// A carton is collected by one carrier, and the database says so.
#[tokio::test]
async fn a_carton_cannot_be_put_on_two_trucks() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let person = Uuid::parse_str(PERSON).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();
    let dock = Uuid::parse_str(DOCK).unwrap();
    let fulfilment = Uuid::parse_str(FULFILMENT).unwrap();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('spork.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let carton =
        sealed_carton(&tx, tenant, person, site, dock, fulfilment, 121, 180, Some(4200)).await;
    let first = Uuid::now_v7();
    let second = Uuid::now_v7();
    for c in [first, second] {
        tx.execute(
            "INSERT INTO consignment (id, tenant_id) VALUES ($1, $2)",
            &[&c, &tenant],
        )
        .await
        .unwrap();
    }

    tx.execute(
        "INSERT INTO consignment_package (tenant_id, consignment_id, package_id)
         VALUES ($1, $2, $3)",
        &[&tenant, &first, &carton],
    )
    .await
    .expect("the first truck takes it");

    // **Migration 69.** Without the unique index this succeeds, and the carton is
    // on two consignments with nothing complaining — a check in the handler
    // cannot prevent it, because two requests both read no row before either
    // writes.
    let err = tx
        .execute(
            "INSERT INTO consignment_package (tenant_id, consignment_id, package_id)
             VALUES ($1, $2, $3)",
            &[&tenant, &second, &carton],
        )
        .await
        .expect_err("a carton goes on one truck");
    assert_eq!(
        err.code(),
        Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION),
        "refused by the database rather than by a handler that would race itself"
    );

    tx.rollback().await.unwrap();
}
