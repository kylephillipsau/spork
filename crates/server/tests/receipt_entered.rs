//! Q173: receipt lines preserve entered packaging and convert to base units.

use nylonite_server::receiving::{self, PackingConfig, PackagingLevel};
use tokio_postgres::NoTls;
use uuid::Uuid;

mod common;
use common::{url_and_role};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const POL: &str = "901e0000-0000-0000-0000-000000000001";
const CONFIG: &str = "9ac40000-0000-0000-0000-000000000001";
const ITEM: &str = "17e10000-0000-0000-0000-000000000001";

#[tokio::test]
async fn packing_factor_matches_sql() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let (client, connection) = tokio_postgres::connect(&u, NoTls).await.expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    if assume {
        client.batch_execute("SET ROLE nylonite_app").await.unwrap();
    }
    let mut client = client;
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('nylonite.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let cfg_id = Uuid::parse_str(CONFIG).unwrap();
    let item = Uuid::parse_str(ITEM).unwrap();
    let row = tx
        .query_one(
            "SELECT units_per_inner, inners_per_carton, cartons_per_layer, layers_per_pallet
               FROM item_packing_config WHERE id = $1",
            &[&cfg_id],
        )
        .await
        .unwrap();
    let cfg = PackingConfig {
        id: cfg_id,
        item_id: item,
        units_per_inner: row.get(0),
        inners_per_carton: row.get(1),
        cartons_per_layer: row.get(2),
        layers_per_pallet: row.get(3),
    };

    // **Every level, not just the one.** Principle 5 has the writer convert and
    // the database store the result, so this Rust cascade and SQL `packing_factor`
    // are two implementations of one rule -- and `layer` and `pallet` are the two
    // with the longest chains and the most room to disagree. Checking `carton`
    // alone tests two of the four multiplications.
    for (level, label) in [
        (PackagingLevel::Each, "each"),
        (PackagingLevel::Inner, "inner"),
        (PackagingLevel::Carton, "carton"),
        (PackagingLevel::Layer, "layer"),
        (PackagingLevel::Pallet, "pallet"),
    ] {
        let sql_factor: Option<i64> = tx
            .query_one(
                // `$2::text::packaging_level`, not `$2::packaging_level`: the single
                // cast makes Postgres infer the parameter *as the enum*, which
                // tokio-postgres has no `&str` binding for. Same shape as the
                // `$8::numeric` defect — the cast in the text does not decide
                // what the driver may send.
                "SELECT packing_factor($1, $2::text::packaging_level)",
                &[&cfg_id, &label],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(
            receiving::packing_factor(Some(&cfg), level),
            sql_factor,
            "Rust and SQL disagree on the factor for {label}"
        );
    }

    // The fixture's cascade, stated so a changed config is visible here rather
    // than only as a silent agreement between two wrong implementations.
    assert_eq!(receiving::packing_factor(Some(&cfg), PackagingLevel::Carton), Some(10));
    assert_eq!(receiving::packing_factor(Some(&cfg), PackagingLevel::Layer), Some(80));
    assert_eq!(receiving::packing_factor(Some(&cfg), PackagingLevel::Pallet), Some(400));

    let (base, entered) =
        receiving::resolve_count(None, Some(12), Some("carton"), Some(&cfg), item).unwrap();
    assert_eq!(base, 120);
    assert_eq!(entered.level, PackagingLevel::Carton);

    // Fixture supply still reachable by stable POL.
    let _: Uuid = tx
        .query_one(
            "SELECT id FROM expected_supply WHERE purchase_order_line_id = $1",
            &[&Uuid::parse_str(POL).unwrap()],
        )
        .await
        .unwrap()
        .get(0);

    tx.rollback().await.unwrap();
}

/// The line INSERT accepts a **bound** packaging level.
///
/// **Nothing exercised this and that is why it was wrong twice.** The statement
/// lives in `record_receipt`, which has no HTTP-level test, so a level that
/// could not be bound compiled cleanly and failed only against a real server.
/// It went wrong in both directions: first as a `format!` interpolating the
/// label into the SQL text, then — fixing that — as `$8::packaging_level`, which
/// makes Postgres infer the parameter as the enum and rejects a `&str` exactly
/// as `$8::numeric` rejected an integer. Only `::text::packaging_level` lets the
/// driver send what it has.
///
/// This asserts the statement shape the route uses, with every level, so the
/// next person to touch that cast finds out here.
#[tokio::test]
async fn the_line_insert_binds_every_packaging_level() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let (client, connection) = tokio_postgres::connect(&u, NoTls).await.expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    if assume {
        client.batch_execute("SET ROLE nylonite_app").await.unwrap();
    }
    let mut client = client;
    let tx = client.transaction().await.unwrap();
    tx.execute(
        "SELECT set_config('nylonite.tenant_id', $1::text, true)",
        &[&ALPHA],
    )
    .await
    .unwrap();

    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let item = Uuid::parse_str(ITEM).unwrap();
    let cfg = Uuid::parse_str(CONFIG).unwrap();
    let person = Uuid::parse_str("77770000-0000-0000-0000-000000000001").unwrap();
    let site = Uuid::parse_str("a5170000-0000-0000-0000-000000000001").unwrap();
    let supply: Uuid = tx
        .query_one(
            "SELECT id FROM expected_supply WHERE purchase_order_line_id = $1",
            &[&Uuid::parse_str(POL).unwrap()],
        )
        .await
        .unwrap()
        .get(0);

    for level in ["each", "inner", "carton", "layer", "pallet"] {
        let ce = Uuid::now_v7();
        tx.execute(
            "INSERT INTO client_event (tenant_id, client_event_id, site_id,
                 recorded_by_id, submitted_at, received_at)
             VALUES ($1, $2, $3, $4, now(), now())",
            &[&tenant, &ce, &site, &person],
        )
        .await
        .unwrap();
        let header: Uuid = tx
            .query_one(
                "INSERT INTO goods_receipt (tenant_id, site_id, received_at,
                     client_event_id, recorded_by_id)
                 VALUES ($1, $2, now(), $3, $4) RETURNING id",
                &[&tenant, &site, &ce, &person],
            )
            .await
            .unwrap()
            .get(0);

        // `each` needs no config; every other level does (D58's implication
        // CHECKs), so the config goes on exactly where the route would put it.
        let config_id = (level != "each").then_some(cfg);
        let level_owned = level.to_string();
        let id: Uuid = tx
            .query_one(
                "INSERT INTO goods_receipt_line (
                     tenant_id, goods_receipt_id, item_id, expected_supply_id,
                     expected_quantity, quantity, entered_quantity,
                     entered_packaging_level, item_packing_config_id, lot_id,
                     client_event_id, recorded_by_id)
                 VALUES (
                     $1, $2, $3, $4, $5, $6, $7, $8::text::packaging_level,
                     $9, NULL, $10, $11)
                 RETURNING id",
                &[
                    &tenant,
                    &header,
                    &item,
                    &supply,
                    &10i64,
                    &10i64,
                    &1i64,
                    &level_owned,
                    &config_id,
                    &ce,
                    &person,
                ],
            )
            .await
            .unwrap_or_else(|e| panic!("binding level {level}: {e}"))
            .get(0);

        let stored: String = tx
            .query_one(
                "SELECT entered_packaging_level::text FROM goods_receipt_line WHERE id = $1",
                &[&id],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(stored, level, "the level bound is the level stored");
    }

    tx.rollback().await.unwrap();
}
