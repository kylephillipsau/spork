//! One delivery, from the supplier's message to stock on a pallet, as the app.
//!
//! **Every other test here asserts a property. This one performs a day's work.**
//! Forty-eight migrations and ninety-four decisions describe an inbound path that
//! had never been executed end to end by the role that would execute it, and D94
//! is what that costs: a function nobody could call, and a grant nobody had taken
//! away, both live for four migrations while every structural check passed.
//!
//! The walk is the roadmap in executable form. Where it stops, something is
//! missing, and the assertions at the bottom name what — in S42's idiom, so that
//! building the missing piece makes this file fail and demand to be extended.
//!
//! Everything runs after `SET LOCAL ROLE nylonite_app`, inside one transaction
//! that is rolled back. Skips rather than fails without `DATABASE_URL`.

use postgres::Transaction;

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const SUPPLIER: &str = "9a247000-0000-0000-0000-000000000003";
const OWNER: &str = "9a247000-0000-0000-0000-000000000001";
const RECEIVER: &str = "77770000-0000-0000-0000-000000000001";
const GLOVE: &str = "17e10000-0000-0000-0000-000000000001";
const CONFIG: &str = "9ac40000-0000-0000-0000-000000000001";
const BIN: &str = "10c00000-0000-0000-0000-000000000001";
const GOOD: &str = "57a70000-0000-0000-0000-000000000001";
const PO: &str = "90000000-0000-0000-0000-000000000001";
const SHIPMENT: &str = "1b500000-0000-0000-0000-000000000001";
const POLICY_VERSION: &str = "4ec00000-0000-0000-0000-000000000002";

/// Ten cartons of ten, declared and counted, so the conversion is visible.
const DECLARED_BASE: i64 = 100;
const CARTONS: i64 = 10;

fn step(tx: &mut Transaction, what: &str, sql: &str) {
    if let Err(e) = tx.batch_execute(sql) {
        let detail = e
            .as_db_error()
            .map(|d| d.message().to_string())
            .unwrap_or_else(|| e.to_string());
        panic!("the inbound walk stops at {what}: {detail}");
    }
}

#[test]
fn a_delivery_reaches_stock_without_leaving_the_application_role() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = nylonite_invariants::connect_exclusive(&url);
    let mut tx = c.transaction().expect("begin");
    tx.batch_execute("SET LOCAL ROLE nylonite_app").expect("become the app");
    tx.execute("SELECT set_config('nylonite.tenant_id', $1, true)", &[&TENANT])
        .expect("name the tenant");

    // ---------------------------------------------------------------------
    // The message arrives. Nothing here is our fact; all of it is their claim.
    // ---------------------------------------------------------------------
    step(&mut tx, "recording the ingestion event", &format!(
        "INSERT INTO client_event (tenant_id, client_event_id, submitted_at, automation_key)
         VALUES ('{TENANT}', 'ce000000-0000-0000-0000-0000000000f1', now(), 'edi-ingest')"));

    step(&mut tx, "storing the artefact", &format!(
        "INSERT INTO party_message (id, tenant_id, party_id, direction, channel, payload,
             byte_count, content_hash, occurred_at, recorded_at, client_event_id)
         VALUES ('9a550000-0000-0000-0000-0000000000f1', '{TENANT}', '{SUPPLIER}', 'inbound',
             'api', '<DESADV/>', 9, 'walkwalkwalk', now(), now(),
             'ce000000-0000-0000-0000-0000000000f1')"));

    step(&mut tx, "the despatch advice itself", &format!(
        "INSERT INTO assertion (id, tenant_id, kind, direction, author_party_id, site_id,
             author_reference, author_version, asserted_at, received_at, party_message_id,
             client_event_id)
         VALUES ('a55e0000-0000-0000-0000-0000000000f1', '{TENANT}', 'despatch_advice',
             'inbound', '{SUPPLIER}', '{SITE}', 'ASN-WALK', '1', now(), now(),
             '9a550000-0000-0000-0000-0000000000f1', 'ce000000-0000-0000-0000-0000000000f1');
         INSERT INTO despatch_advice (assertion_id, tenant_id, inbound_shipment_id, granularity)
         VALUES ('a55e0000-0000-0000-0000-0000000000f1', '{TENANT}', '{SHIPMENT}', 'pallet')"));

    // D96: `resolved_physical` is our reading of their level vocabulary, and a
    // pallet is a thing on the dock rather than a level of the document.
    step(&mut tx, "the declared pallet", &format!(
        "INSERT INTO asserted_unit (id, tenant_id, assertion_id, level_code, sscc,
             sequence, resolved_physical)
         VALUES ('a5010000-0000-0000-0000-0000000000f1', '{TENANT}',
             'a55e0000-0000-0000-0000-0000000000f1', 'pallet', '393123450000000025', 1, true)"));

    // D93: they say cartons, we keep the word and read it as a packaging level.
    step(&mut tx, "the declared contents", &format!(
        "INSERT INTO asserted_unit_content (id, tenant_id, asserted_unit_id, raw_gtin,
             quantity, entered_quantity, raw_unit_code, resolved_packaging_level,
             item_packing_config_id, lot_code, expiry_date)
         VALUES ('a5c00000-0000-0000-0000-0000000000f1', '{TENANT}',
             'a5010000-0000-0000-0000-0000000000f1', '09312345000012',
             {DECLARED_BASE}, {CARTONS}, 'CT', 'carton', '{CONFIG}',
             'L2026-WALK', '2027-01-31')"));

    step(&mut tx, "taking a position on the claim", &format!(
        "INSERT INTO assertion_stance (id, tenant_id, assertion_id, stance, reason_code,
             occurred_at, recorded_at, client_event_id, automation_key)
         VALUES ('a55c0000-0000-0000-0000-0000000000f1', '{TENANT}',
             'a55e0000-0000-0000-0000-0000000000f1', 'in_force', 'parsed_clean',
             now(), now(), 'ce000000-0000-0000-0000-0000000000f1', 'edi-ingest')"));

    // D90 and D94: our annotation on their words, through the function that
    // refuses once anything has compared against it.
    step(&mut tx, "resolving the GTIN to an item", &format!(
        "SELECT asserted_unit_content_resolve('a5c00000-0000-0000-0000-0000000000f1',
             '{GLOVE}', NULL, '{RECEIVER}', 'gtin_exact')"));

    // ---------------------------------------------------------------------
    // The truck arrives. From here everything is ours and observed.
    // ---------------------------------------------------------------------
    step(&mut tx, "the receiver's event", &format!(
        "INSERT INTO client_event (tenant_id, client_event_id, submitted_at, recorded_by_id)
         VALUES ('{TENANT}', 'ce000000-0000-0000-0000-0000000000f2', now(), '{RECEIVER}')"));

    step(&mut tx, "opening the receipt", &format!(
        "INSERT INTO goods_receipt (id, tenant_id, site_id, purchase_order_id, received_at,
             recorded_at, client_event_id, recorded_by_id)
         VALUES ('92c00000-0000-0000-0000-0000000000f1', '{TENANT}', '{SITE}', '{PO}',
             now(), now(), 'ce000000-0000-0000-0000-0000000000f2', '{RECEIVER}')"));

    step(&mut tx, "the lot on the carton", &format!(
        "INSERT INTO lot (id, tenant_id, item_id, code, expiry_date)
         VALUES ('10700000-0000-0000-0000-0000000000f1', '{TENANT}', '{GLOVE}',
             'L2026-WALK', '2027-01-31')"));

    // D91 links the count to the claim; D92 stores it in base units.
    step(&mut tx, "counting the line", &format!(
        "INSERT INTO goods_receipt_line (id, tenant_id, goods_receipt_id, item_id, quantity,
             entered_quantity, entered_packaging_level, item_packing_config_id, lot_id,
             asserted_unit_content_id, recorded_at, client_event_id, recorded_by_id)
         VALUES ('92c10000-0000-0000-0000-0000000000f1', '{TENANT}',
             '92c00000-0000-0000-0000-0000000000f1', '{GLOVE}', {DECLARED_BASE}, {CARTONS},
             'carton', '{CONFIG}', '10700000-0000-0000-0000-0000000000f1',
             'a5c00000-0000-0000-0000-0000000000f1', now(),
             'ce000000-0000-0000-0000-0000000000f2', '{RECEIVER}')"));

    // D89 and D94: the disposition is a mediated write and refuses a second one.
    step(&mut tx, "dispositioning under the policy", &format!(
        "SELECT goods_receipt_line_dispose('92c10000-0000-0000-0000-0000000000f1',
             '{POLICY_VERSION}', true, NULL, '{RECEIVER}', NULL)"));

    // ---------------------------------------------------------------------
    // The pallet becomes a physical object. J34: minted from a scan, never from
    // the claim — which is why `source` is `operator_scan` and not `asn`.
    // ---------------------------------------------------------------------
    step(&mut tx, "the pallet's identity", &format!(
        "INSERT INTO package (id, tenant_id) VALUES ('9ac00000-0000-0000-0000-0000000000f1', '{TENANT}')"));

    step(&mut tx, "scanning the SSCC", &format!(
        "INSERT INTO package_event (id, tenant_id, package_id, kind, source, occurred_at,
             recorded_at, client_event_id, recorded_by_id, sscc)
         VALUES ('9ae00000-0000-0000-0000-0000000000f1', '{TENANT}',
             '9ac00000-0000-0000-0000-0000000000f1', 'identified', 'operator_scan',
             now(), now(), 'ce000000-0000-0000-0000-0000000000f2', '{RECEIVER}',
             '393123450000000025')"));

    step(&mut tx, "putting the pallet down", &format!(
        "INSERT INTO package_event (id, tenant_id, package_id, kind, source, occurred_at,
             recorded_at, client_event_id, recorded_by_id, location_id)
         VALUES ('9ae00000-0000-0000-0000-0000000000f2', '{TENANT}',
             '9ac00000-0000-0000-0000-0000000000f1', 'placed', 'operator_scan',
             now(), now(), 'ce000000-0000-0000-0000-0000000000f2', '{RECEIVER}', '{BIN}')"));

    // The ledger entry that makes it stock. Held by the pallet rather than by the
    // bin, which is what `package_content` reads.
    step(&mut tx, "the receipt movement", &format!(
        "INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
             to_package_id, to_lot_id, to_status_id, to_owner_id, reason, occurred_at,
             recorded_at, recorded_by_id, entered_quantity, entered_packaging_level,
             item_packing_config_id, goods_receipt_line_id)
         VALUES ('5b000000-0000-0000-0000-0000000000f1', '{TENANT}',
             'ce000000-0000-0000-0000-0000000000f2', '{GLOVE}', {DECLARED_BASE},
             '9ac00000-0000-0000-0000-0000000000f1', '10700000-0000-0000-0000-0000000000f1',
             '{GOOD}', '{OWNER}', 'receipt', now(), now(), '{RECEIVER}',
             {CARTONS}, 'carton', '{CONFIG}', '92c10000-0000-0000-0000-0000000000f1')"));

    // D96. The pallet they declared and the pallet we scanned are the same pallet,
    // and this is where the walk used to stop. The package was minted from a scan
    // rather than from the claim, so the collapse records a match rather than
    // creating the thing it matches.
    step(&mut tx, "collapsing the declared pallet onto the scanned one", &format!(
        "SELECT asserted_unit_collapse('a5010000-0000-0000-0000-0000000000f1',
             '9ac00000-0000-0000-0000-0000000000f1', '{RECEIVER}')"));

    let collapsed: bool = tx
        .query_one(
            "SELECT resolved_package_id = '9ac00000-0000-0000-0000-0000000000f1'
               FROM asserted_unit WHERE id = 'a5010000-0000-0000-0000-0000000000f1'",
            &[],
        )
        .expect("the declared pallet")
        .get(0);
    assert!(collapsed, "the declared unit names the package the receiver scanned");

    // Freezes on first use, D21's rule and D90's shape. Inside a savepoint,
    // because the refusal is an exception and the walk has further to go.
    {
        let mut sp = tx.savepoint("second_collapse").expect("savepoint");
        let again = sp.execute(
            "SELECT asserted_unit_collapse('a5010000-0000-0000-0000-0000000000f1',
                 '9ac00000-0000-0000-0000-0000000000f1', ($1)::text::uuid)",
            &[&RECEIVER],
        );
        assert!(
            again.is_err(),
            "a second collapse would rewrite what a receipt already compared against"
        );
        sp.rollback().expect("release the savepoint");
    }

    // The comparison D21 describes, on the line we just walked.
    let variance: Option<i64> = tx
        .query_one(
            "SELECT base_quantity_variance::bigint FROM goods_receipt_variance(
                 '92c00000-0000-0000-0000-0000000000f1')",
            &[],
        )
        .expect("the variance")
        .get(0);
    assert_eq!(
        variance,
        Some(0),
        "declared and counted agree on this delivery, so the variance is zero rather than absent"
    );

    tx.rollback().expect("rollback");
}

/// **Where the walk stops, stated as an absence.**
///
/// S42's idiom: a pending check names an object and asserts it is still missing,
/// so a gap cannot go on reading as deliberate after it has been filled. These
/// are the two boundaries the walk above runs into.
#[test]
fn the_walk_stops_where_the_schema_stops() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = nylonite_invariants::connect_exclusive(&url);

    // 1. **This guard was wrong and said nothing for a whole decision.**
    //
    // It watched for a column called `package_id`, because that is the name D24's
    // sketch used. D96 built it as `resolved_package_id`, following the raw/resolved
    // convention the table already had — so the guard went on passing while the
    // very thing it was placed to catch happened. A check that names the wrong
    // object cannot fail, which is S42's whole subject arriving in a test that
    // S42 does not cover.
    //
    // It now asks the question by shape rather than by name: is there a foreign
    // key from the declared tree to the physical one? Renaming the column cannot
    // silence that, and neither can adding the link under a third spelling.
    let links: i64 = c
        .query_one(
            "SELECT count(*) FROM pg_constraint
              WHERE conrelid = 'asserted_unit'::regclass
                AND contype = 'f' AND confrelid = 'package'::regclass",
            &[],
        )
        .expect("catalogue")
        .get(0);
    assert_eq!(
        links, 1,
        "the declared tree should reach the physical one exactly once; D96 made that \
         asserted_unit.resolved_package_id, and the walk above collapses through it"
    );

    // 2. The application cannot fold its own writes into a projection.
    //
    // `projection_run_all` is granted to the scheduler, the platform and the
    // projection owner, and deliberately not to the app — D25 keeps the
    // maintainer away from the writer. The consequence is unstated anywhere: a
    // receiver finishes a delivery and `stock.quantity` does not move until
    // something else runs, and with no triggers (S7 examines zero) there is
    // nothing that would make it. What that latency may be is nobody's decision
    // yet.
    let app_may_fold: bool = c
        .query_one(
            "SELECT has_function_privilege('nylonite_app', 'projection_run_all(uuid)', 'EXECUTE')",
            &[],
        )
        .expect("catalogue")
        .get(0);
    assert!(
        !app_may_fold,
        "the app can now run projections, which reverses D25's separation between \
         the writer and the maintainer and wants a decision behind it"
    );
}
