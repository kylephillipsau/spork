//! WP2: receipt disposition chain — resolve receiving policy, clamp, decide,
//! dispose via mediated function (no app-stamped accepted_at).

use chrono::Utc;
use spork_server::client_events::{self, NewClientEvent};
use spork_server::receiving::{self, CountedLine};
use spork_server::tenancy::TenantScope;
use tokio_postgres::NoTls;
use uuid::Uuid;

mod common;
use common::{url_and_role};

const ALPHA: &str = "11111111-1111-1111-1111-111111111111";
const PERSON: &str = "77770000-0000-0000-0000-000000000001";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const LOT: &str = "10700000-0000-0000-0000-000000000001";
const ITEM: &str = "17e10000-0000-0000-0000-000000000001";
const SUPPLIER: &str = "9a247000-0000-0000-0000-000000000002";

/// The fixture's purchase order line, which is hand-written and therefore stable.
///
/// **`expected_supply` is not.** The fixture folds the issued order into a promise
/// rather than inserting one with a fixed id, so the row takes `uuidv7()`'s
/// default and gets a different identifier every time `seed.sql` is loaded. The
/// first version of this file pinned the id one such load happened to produce,
/// which made the test pass on the machine it was written on and fail on a fresh
/// database with a foreign key violation. `seed.sql` reaches the same row the
/// same way — `WHERE e.purchase_order_line_id = ...` — twice.
const PO_LINE: &str = "901e0000-0000-0000-0000-000000000001";

async fn pool(u: &str) -> deadpool_postgres::Pool {
    use std::str::FromStr;
    let pg = tokio_postgres::Config::from_str(u).expect("url");
    let mut cfg = deadpool_postgres::Config::new();
    cfg.host = pg.get_hosts().first().map(|h| match h {
        tokio_postgres::config::Host::Tcp(s) => s.clone(),
        #[cfg(unix)]
        tokio_postgres::config::Host::Unix(p) => p.to_string_lossy().into_owned(),
    });
    cfg.port = pg.get_ports().first().copied();
    cfg.user = pg.get_user().map(str::to_string);
    cfg.password = pg
        .get_password()
        .map(|p| String::from_utf8_lossy(p).into_owned());
    cfg.dbname = pg.get_dbname().map(str::to_string);
    cfg.create_pool(Some(deadpool_postgres::Runtime::Tokio1), NoTls)
        .expect("pool")
}

#[tokio::test]
async fn resolve_receiving_applies_platform_clamp() {
    let Some((u, _)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let pool = pool(&u).await;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let item = Uuid::parse_str(ITEM).unwrap();
    let supplier = Uuid::parse_str(SUPPLIER).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();

    let mut scope = TenantScope::begin(&pool, tenant).await.expect("scope");
    let resolved = scope
        .run(|tx| {
            Box::pin(async move {
                receiving::resolve_receiving_policy(
                    tx,
                    tenant,
                    item,
                    Some(supplier),
                    Some(site),
                    Utc::now(),
                )
                .await
            })
        })
        .await
        .expect("resolve");

    // Fixture: tenant asks 25%, platform ceiling 10%; require_lot from platform.
    assert_eq!(resolved.policy.tolerance_over_pct, Some(10.0));
    assert!(resolved.policy.require_lot);

    // The platform default now ships in migration 65 and the tenant binding in
    // the fixture. They differ on the counterparty axis, so the pair resolves
    // rather than tying — which is what stops migration 65 from turning every
    // receipt into a `policy_ambiguous` finding.
    assert_eq!(
        resolved.ambiguity, None,
        "the shipped default and the tenant binding must not be equally specific"
    );
}

#[tokio::test]
async fn over_receipt_disposes_with_finding() {
    let Some((u, assume)) = url_and_role() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let (client, connection) = tokio_postgres::connect(&u, NoTls).await.expect("connect");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    if assume {
        client
            .batch_execute("SET ROLE spork_app")
            .await
            .expect("role");
    }
    let mut client = client;
    let tenant = Uuid::parse_str(ALPHA).unwrap();
    let person = Uuid::parse_str(PERSON).unwrap();
    let site = Uuid::parse_str(SITE).unwrap();
    let lot = Uuid::parse_str(LOT).unwrap();
    let item = Uuid::parse_str(ITEM).unwrap();
    let supplier = Uuid::parse_str(SUPPLIER).unwrap();
    let po_line = Uuid::parse_str(PO_LINE).unwrap();
    let ce = Uuid::now_v7();
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
            "SELECT id FROM expected_supply WHERE purchase_order_line_id = $1",
            &[&po_line],
        )
        .await
        .expect("the fixture's promise against PO line 1")
        .get(0);

    let resolved = receiving::resolve_receiving_policy(
        &tx,
        tenant,
        item,
        Some(supplier),
        Some(site),
        now,
    )
    .await
    .expect("policy");

    let d = receiving::disposition(
        &resolved.policy,
        &CountedLine {
            expected_quantity: 130,
            quantity: 160,
            has_lot: true,
        },
    );
    assert!(d.accept);
    assert_eq!(d.raise, Some("over_receipt"));

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

    let gr: Uuid = tx
        .query_one(
            "INSERT INTO goods_receipt (
                 tenant_id, site_id, received_at, client_event_id, recorded_by_id)
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
            &[&tenant, &site, &now, &ce, &person],
        )
        .await
        .unwrap()
        .get(0);

    let line: Uuid = tx
        .query_one(
            "INSERT INTO goods_receipt_line (
                 tenant_id, goods_receipt_id, item_id, expected_supply_id,
                 expected_quantity, quantity, entered_quantity,
                 entered_packaging_level, lot_id,
                 client_event_id, recorded_by_id)
             VALUES (
                 $1, $2, $3, $4, 130, 160, 160, 'each', $5, $6, $7)
             RETURNING id",
            &[&tenant, &gr, &item, &supply, &lot, &ce, &person],
        )
        .await
        .unwrap()
        .get(0);

    // Must not have stamped accept before dispose.
    let pre: bool = tx
        .query_one(
            "SELECT accepted_at IS NOT NULL FROM goods_receipt_line WHERE id = $1",
            &[&line],
        )
        .await
        .unwrap()
        .get(0);
    assert!(!pre);

    let disc: Option<Uuid> = tx
        .query_one(
            "SELECT goods_receipt_line_dispose($1, $2, true, $3, $4, $5)",
            &[
                &line,
                &d.receiving_policy_id,
                &Some("over_receipt"),
                &person,
                &ce,
            ],
        )
        .await
        .unwrap()
        .get(0);
    assert!(disc.is_some(), "over_receipt must raise a discrepancy");

    let (accepted, stamped): (bool, Uuid) = {
        let r = tx
            .query_one(
                "SELECT accepted_at IS NOT NULL, receiving_policy_id
                   FROM goods_receipt_line WHERE id = $1",
                &[&line],
            )
            .await
            .unwrap();
        (r.get(0), r.get(1))
    };
    assert!(accepted);
    assert_eq!(stamped, d.receiving_policy_id);

    let d2 = receiving::disposition(
        &resolved.policy,
        &CountedLine {
            expected_quantity: 130,
            quantity: 100,
            has_lot: false,
        },
    );
    assert!(!d2.accept);
    assert_eq!(d2.raise, Some("lot_missing"));

    tx.rollback().await.unwrap();
}
