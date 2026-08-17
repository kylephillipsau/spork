//! The second spine table gets a write path, and J12 keeps holding.
//!
//! Architecture names two tables that carry everything. Twenty-five endpoints
//! wrote `stock_movement` and none wrote `observation`, which is why stage 4 of
//! the recorded pack process — *"the only stage with genuinely variable,
//! physically-measured input"* — was the one stage the API could not perform.
//!
//! The property worth testing is not that a row lands. It is that recording a
//! measurement against an unsealed package leaves J12 satisfied, because J12
//! compares with `IS DISTINCT FROM` and a package carrying NULL against a
//! recorded observation fails exactly as loudly as one carrying a wrong number.

use chrono::Utc;
use nylonite_server::client_events::{self, NewClientEvent};
use nylonite_server::observing::{self, Factor};
use uuid::Uuid;

mod common;
use common::{connect, url_and_role};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const PERSON: &str = "77770000-0000-0000-0000-000000000001";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const DOCK: &str = "10c00000-0000-0000-0000-000000000003";
const FULFILMENT: &str = "f01f0000-0000-0000-0000-000000000001";

/// The unit table's factors are the ones the writer converts by.
///
/// Not a restatement of the unit test: this reads the real rows, so a seeded
/// factor that disagrees with the arithmetic here is caught rather than assumed.
/// S22 keeps every canonical unit at 1/1, which is what makes `mm` and `g` the
/// identity.
#[tokio::test]
async fn the_seeded_factors_convert_as_the_writer_expects() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let client = connect(&u, assume).await;

    for (code, entered, expect) in [
        ("kg", "12.1", 12_100i64),
        ("g", "845", 845),
        ("m", "1.2", 1_200),
        ("mm", "1200", 1_200),
        ("in", "1.5", 38),
    ] {
        let row = client
            .query_one(
                "SELECT factor_num, factor_den, offset_num FROM unit WHERE code = $1",
                &[&code],
            )
            .await
            .unwrap_or_else(|_| panic!("unit {code} is seeded"));
        let offset: i64 = row.get(2);
        assert_eq!(offset, 0, "S22: no canonical-dimension unit carries an offset");
        let f = Factor {
            num: row.get(0),
            den: row.get(1),
        };
        let e = observing::parse_entered(entered).unwrap();
        assert_eq!(
            observing::to_canonical(e, f).unwrap(),
            expect,
            "{entered} {code} in base units"
        );
    }
}

/// Measuring a pallet is one act, and it leaves J12 satisfied.
#[tokio::test]
async fn measuring_an_unsealed_package_agrees_with_the_dimension_cache() {
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
    let carton = Uuid::now_v7();
    let ce = Uuid::now_v7();
    let now = Utc::now();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('nylonite.tenant_id', $1::text, true)",
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

    tx.execute(
        "INSERT INTO package (id, tenant_id, fulfilment_id, sequence)
         VALUES ($1, $2, $3, 41)",
        &[&carton, &tenant, &fulfilment],
    )
    .await
    .unwrap();
    tx.execute(
        "INSERT INTO package_event (
             tenant_id, client_event_id, package_id, kind, source,
             occurred_at, recorded_by_id, location_id)
         VALUES ($1, $2, $3, 'created', 'operator_scan', $4, $5, $6)",
        &[&tenant, &ce, &carton, &now, &person, &dock],
    )
    .await
    .unwrap();

    // One subject, one moment, four metrics — `observation_event`'s own shape.
    let observable_id: Uuid = {
        tx.execute(
            "INSERT INTO observable (tenant_id, package_id) VALUES ($1, $2)
             ON CONFLICT (tenant_id, package_id) WHERE package_id IS NOT NULL DO NOTHING",
            &[&tenant, &carton],
        )
        .await
        .unwrap();
        tx.query_one(
            "SELECT id FROM observable WHERE tenant_id = $1 AND package_id = $2",
            &[&tenant, &carton],
        )
        .await
        .expect("get-or-create found the subject")
        .get(0)
    };

    let event_id: Uuid = tx
        .query_one(
            "INSERT INTO observation_event (
                 tenant_id, client_event_id, observable_id, observed_at,
                 recorded_by_id, method, ingestion_channel)
             VALUES ($1, $2, $3, $4, $5, 'instrument', 'scale')
             RETURNING id",
            &[&tenant, &ce, &observable_id, &now, &person],
        )
        .await
        .unwrap()
        .get(0);

    // 1.2 m, 0.8 m, 1.45 m and 312.5 kg — none of which is a double.
    let measured = [
        ("length", "1.2", "m", 1_200i64, "length_mm"),
        ("width", "0.8", "m", 800, "width_mm"),
        ("height", "1.45", "m", 1_450, "height_mm"),
        ("gross_weight", "312.5", "kg", 312_500, "gross_weight_g"),
    ];

    for (metric_code, entered, unit_code, expect, column) in measured {
        let m = tx
            .query_one(
                "SELECT id, dimension_id, applies_to FROM metric WHERE code = $1",
                &[&metric_code],
            )
            .await
            .unwrap();
        let metric_id: Uuid = m.get(0);
        let metric_dim: Option<Uuid> = m.get(1);
        let applies: Vec<String> = m.get::<_, Option<Vec<String>>>(2).unwrap_or_default();
        assert!(
            applies.iter().any(|k| k == "package"),
            "{metric_code} applies to a package"
        );

        let un = tx
            .query_one(
                "SELECT id, dimension_id, factor_num, factor_den FROM unit WHERE code = $1",
                &[&unit_code],
            )
            .await
            .unwrap();
        let unit_id: Uuid = un.get(0);
        let unit_dim: Uuid = un.get(1);
        assert_eq!(metric_dim, Some(unit_dim), "the dimensions must agree");

        let canonical = observing::to_canonical(
            observing::parse_entered(entered).unwrap(),
            Factor {
                num: un.get(2),
                den: un.get(3),
            },
        )
        .unwrap();
        assert_eq!(canonical, expect, "{entered} {unit_code}");

        tx.execute(
            "INSERT INTO observation (
                 tenant_id, observation_event_id, observable_id, observed_at,
                 client_event_id, metric_id, result_kind, dimension_id,
                 value_numeric, entered_value, entered_unit_id)
             VALUES ($1, $2, $3, $4, $5, $6, 'quantity', $7, $8, $9::text::numeric, $10)",
            &[
                &tenant,
                &event_id,
                &observable_id,
                &now,
                &ce,
                &metric_id,
                &unit_dim,
                &canonical,
                &entered,
                &unit_id,
            ],
        )
        .await
        .unwrap_or_else(|e| panic!("recording {metric_code}: {e}"));

        // The cache J12 compares against.
        // `$1::bigint`: the three length columns are `integer` and the
        // observation is `bigint`, so the assignment cast is what narrows.
        let sql = format!(
            "UPDATE package SET {column} = $1::bigint, dimensions_source = 'confirmed'
              WHERE id = $2"
        );
        tx.execute(sql.as_str(), &[&canonical, &carton])
            .await
            .unwrap();
    }

    // What was typed survives beside what is stored — Principle 5's whole point.
    let kept: String = tx
        .query_one(
            "SELECT o.entered_value::text FROM observation o
               JOIN metric m ON m.id = o.metric_id
              WHERE o.observation_event_id = $1 AND m.code = 'gross_weight'",
            &[&event_id],
        )
        .await
        .unwrap()
        .get(0);
    assert!(kept.starts_with("312.5"), "the operator typed 312.5, not 312500");

    // Rebuild observation_current, then run J12's own comparison.
    // **The app may not call a maintainer**, which is D25 and D107 working: the
    // only refresh it holds EXECUTE on is the rate-limited wrapper. If that
    // returns NULL the rebuild was declined, and the assertions below would be
    // reading a stale projection rather than a wrong one.
    let refreshed: Option<i64> = tx
        .query_one("SELECT projection_refresh_tenant($1)", &[&tenant])
        .await
        .expect("the app may ask for a refresh")
        .get(0);
    if refreshed.is_none() {
        eprintln!("refresh rate-limited; skipping the projection half");
        tx.rollback().await.unwrap();
        return;
    }

    let disagreements: i64 = tx
        .query_one(
            "SELECT count(*)
               FROM package p
               JOIN observable o ON o.package_id = p.id
               JOIN observation_current oc ON oc.observable_id = o.id
               JOIN metric m ON m.id = oc.metric_id
              WHERE p.id = $1
                AND p.sealed_at IS NULL
                AND m.code IN ('length','width','height','gross_weight')
                AND (CASE m.code WHEN 'length' THEN p.length_mm
                                 WHEN 'width'  THEN p.width_mm
                                 WHEN 'height' THEN p.height_mm
                                 WHEN 'gross_weight' THEN p.gross_weight_g END)
                    IS DISTINCT FROM oc.value_numeric",
            &[&carton],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        disagreements, 0,
        "J12: an unsealed package's dimensions equal its current observations"
    );

    // And the check is not passing because it examined nothing.
    let examined: i64 = tx
        .query_one(
            "SELECT count(*) FROM observation_current oc
               JOIN observable o ON o.id = oc.observable_id
              WHERE o.package_id = $1",
            &[&carton],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(examined, 4, "four metrics reached observation_current");

    tx.rollback().await.unwrap();
}

/// Writing the observation and *not* the cache is what J12 exists to catch.
///
/// Demonstrated rather than assumed, in the register's idiom: the check has to
/// be shown firing on the state it forbids, or a passing run proves only that
/// the fixture is clean.
#[tokio::test]
async fn an_uncached_measurement_is_exactly_what_j12_reports() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let mut client = connect(&u, assume).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let person = Uuid::parse_str(PERSON).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();
    let fulfilment = Uuid::parse_str(FULFILMENT).unwrap();
    let carton = Uuid::now_v7();
    let ce = Uuid::now_v7();
    let now = Utc::now();

    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('nylonite.tenant_id', $1::text, true)",
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
    tx.execute(
        "INSERT INTO package (id, tenant_id, fulfilment_id, sequence) VALUES ($1, $2, $3, 42)",
        &[&carton, &tenant, &fulfilment],
    )
    .await
    .unwrap();
    tx.execute(
        "INSERT INTO observable (tenant_id, package_id) VALUES ($1, $2)",
        &[&tenant, &carton],
    )
    .await
    .unwrap();
    let observable_id: Uuid = tx
        .query_one(
            "SELECT id FROM observable WHERE tenant_id = $1 AND package_id = $2",
            &[&tenant, &carton],
        )
        .await
        .unwrap()
        .get(0);
    let event_id: Uuid = tx
        .query_one(
            "INSERT INTO observation_event (
                 tenant_id, client_event_id, observable_id, observed_at,
                 recorded_by_id, method, ingestion_channel)
             VALUES ($1, $2, $3, $4, $5, 'keyed', 'keyed') RETURNING id",
            &[&tenant, &ce, &observable_id, &now, &person],
        )
        .await
        .unwrap()
        .get(0);

    let m = tx
        .query_one(
            "SELECT id, dimension_id FROM metric WHERE code = 'height'",
            &[],
        )
        .await
        .unwrap();
    let unit = tx
        .query_one("SELECT id, dimension_id FROM unit WHERE code = 'mm'", &[])
        .await
        .unwrap();
    tx.execute(
        "INSERT INTO observation (
             tenant_id, observation_event_id, observable_id, observed_at,
             client_event_id, metric_id, result_kind, dimension_id,
             value_numeric, entered_value, entered_unit_id)
         VALUES ($1, $2, $3, $4, $5, $6, 'quantity', $7, 1450, 1450, $8)",
        &[
            &tenant,
            &event_id,
            &observable_id,
            &now,
            &ce,
            &m.get::<_, Uuid>(0),
            &m.get::<_, Option<Uuid>>(1),
            &unit.get::<_, Uuid>(0),
        ],
    )
    .await
    .unwrap();

    // Deliberately not touching package.height_mm.
    let refreshed: Option<i64> = tx
        .query_one("SELECT projection_refresh_tenant($1)", &[&tenant])
        .await
        .expect("the app may ask for a refresh")
        .get(0);
    if refreshed.is_none() {
        eprintln!("refresh rate-limited; skipping");
        tx.rollback().await.unwrap();
        return;
    }

    let disagreements: i64 = tx
        .query_one(
            "SELECT count(*)
               FROM package p
               JOIN observable o ON o.package_id = p.id
               JOIN observation_current oc ON oc.observable_id = o.id
               JOIN metric m ON m.id = oc.metric_id
              WHERE p.id = $1 AND p.sealed_at IS NULL AND m.code = 'height'
                AND p.height_mm IS DISTINCT FROM oc.value_numeric",
            &[&carton],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        disagreements, 1,
        "a measurement with no cache behind it is a finding, which is why the \
         writer sets both in one act"
    );

    tx.rollback().await.unwrap();
}

/// The presets a pack screen reads, shared and tenant-owned in one list.
#[tokio::test]
async fn package_types_return_shipped_and_tenant_presets() {
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

    // No WHERE tenant_id: the read policy pairs shared rows with this tenant's,
    // which is what lets one list serve a screen.
    let rows = tx
        .query(
            "SELECT name, dimensions_fixed, tenant_id IS NOT NULL, width_mm, height_mm
               FROM package_type
              WHERE effective_from <= CURRENT_DATE
              ORDER BY tenant_id IS NULL, name",
            &[],
        )
        .await
        .expect("read presets");

    let names: Vec<String> = rows.iter().map(|r| r.get::<_, String>(0)).collect();
    for want in ["PALLET", "SKID", "small box", "medium box", "large box"] {
        assert!(
            names.iter().any(|n| n == want),
            "{want} is a preset the pack screen can offer"
        );
    }

    // The pallet is the case `dimensions_fixed` exists for: a standard footprint,
    // and a stack height only the scale knows.
    let pallet = rows
        .iter()
        .find(|r| r.get::<_, String>(0) == "PALLET")
        .expect("the shipped pallet");
    assert!(!pallet.get::<_, bool>(1), "a pallet's dimensions are not fixed");
    assert!(!pallet.get::<_, bool>(2), "and it is shipped, not this tenant's");
    assert_eq!(
        pallet.get::<_, Option<i32>>(3),
        Some(1165),
        "the Australian standard footprint"
    );
    assert_eq!(
        pallet.get::<_, Option<i32>>(4),
        None,
        "the stack height is measured, not preset"
    );

    // A box is the other case, and migration 67's CHECK means a preset claiming
    // a fixed size cannot be missing one.
    let boxes: Vec<_> = rows
        .iter()
        .filter(|r| r.get::<_, String>(0).ends_with("box"))
        .collect();
    assert_eq!(boxes.len(), 3);
    for b in boxes {
        let name: String = b.get(0);
        assert!(b.get::<_, bool>(1), "{name} claims fixed dimensions");
        assert!(b.get::<_, bool>(2), "{name} is this tenant's, not a standard");
        assert!(
            b.get::<_, Option<i32>>(4).is_some(),
            "{name} states the height it claims is fixed"
        );
    }

    tx.rollback().await.unwrap();
}
