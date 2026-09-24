//! One order, from placement to the truck, as the app.
//!
//! The counterpart to `inbound_walk.rs` and written for the same reason: a path
//! nobody has executed end to end as the role that would execute it is a path
//! whose gaps are invisible to structural checking. Inbound found two that way.
//!
//! **This walk completes, and what it found was not a missing column.** Outbound
//! progress — picked, packed, despatched — was folded from `stock_allocation.state`
//! rather than from the ledger, so a pick recorded in `stock_movement` moved no
//! progress number and nothing compared the two. The assertions at the bottom
//! demonstrated that rather than describing it, and became question 165.
//!
//! **Three migrations later they assert the agreement instead.** D99 gave the ledger
//! its outbound cause arm, so the pick and the despatch below say which commitment
//! they served; D100 moved the fold onto the facts. The walk's statements never
//! changed — the same pick, seal and despatch — and what moved is where the numbers
//! come from. `covered_quantity` still folds the allocation, because coverage is an
//! intention and the other three are claims about what happened.
//!
//! Everything runs after `SET LOCAL ROLE spork_app`, inside one transaction
//! that is rolled back. Skips rather than fails without `DATABASE_URL`.

use postgres::Transaction;

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";
const CUSTOMER: &str = "9a247000-0000-0000-0000-000000000002";
const PICKER: &str = "77770000-0000-0000-0000-000000000001";
const GLOVE: &str = "17e10000-0000-0000-0000-000000000001";
const CHANNEL: &str = "5c000000-0000-0000-0000-000000000003";
/// Where a carton comes into existence. D97: an event asserting a placement says
/// where, and packing a fresh carton is the case that found the rule missing.
const BENCH: &str = "10c00000-0000-0000-0000-000000000001";

const EVENT: &str = "ce000000-0000-0000-0000-0000000000e1";
const ORDER: &str = "04de0000-0000-0000-0000-0000000000e1";
const ORDER_LINE: &str = "04de1000-0000-0000-0000-0000000000e1";
const FULFILMENT: &str = "f01f0000-0000-0000-0000-0000000000e1";
const FULFILMENT_LINE: &str = "f01f1000-0000-0000-0000-0000000000e1";
const ALLOCATION: &str = "a110c000-0000-0000-0000-0000000000e1";
const CARTON: &str = "9ac00000-0000-0000-0000-0000000000e1";
const CONSIGNMENT: &str = "c05e0000-0000-0000-0000-0000000000e1";

/// Ten of the ten ordered, so every quantity in the walk is the same number and a
/// progress figure that stays at zero cannot be explained by arithmetic.
const ORDERED: i64 = 10;

fn step(tx: &mut Transaction, what: &str, sql: &str) {
    if let Err(e) = tx.batch_execute(sql) {
        let detail = e
            .as_db_error()
            .map(|d| d.message().to_string())
            .unwrap_or_else(|| e.to_string());
        panic!("the outbound walk stops at {what}: {detail}");
    }
}

#[test]
fn an_order_reaches_the_truck_without_leaving_the_application_role() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = spork_invariants::connect_exclusive(&url);
    let mut tx = c.transaction().expect("begin");
    tx.batch_execute("SET LOCAL ROLE spork_app").expect("become the app");
    tx.execute("SELECT set_config('spork.tenant_id', $1, true)", &[&TENANT])
        .expect("name the tenant");

    // ---------------------------------------------------------------------
    // The customer asks. An order is an intention: we plan it and may cancel it.
    // ---------------------------------------------------------------------
    step(&mut tx, "the order-entry event", &format!(
        "INSERT INTO client_event (tenant_id, client_event_id, submitted_at, recorded_by_id)
         VALUES ('{TENANT}', '{EVENT}', now(), '{PICKER}')"));

    step(&mut tx, "placing the order", &format!(
        "INSERT INTO \"order\" (id, tenant_id, site_id, customer_party_id,
             confirmation_number, source_channel_id, currency, state, placed_at)
         VALUES ('{ORDER}', '{TENANT}', '{SITE}', '{CUSTOMER}', 'SO-WALK',
             '{CHANNEL}', 'AUD', 'placed', now());
         INSERT INTO order_line (id, tenant_id, order_id, item_id, quantity_ordered,
             line_number)
         VALUES ('{ORDER_LINE}', '{TENANT}', '{ORDER}', '{GLOVE}', {ORDERED}, 1)"));

    // D15: a fulfilment is the commitment to ship part of an order, and it is what
    // the floor is given. Its states are planned, released and cancelled — there is
    // no completed, because D53 made progress three fractions on the lines.
    step(&mut tx, "committing to ship it", &format!(
        "INSERT INTO fulfilment (id, tenant_id, order_id, site_id, state)
         VALUES ('{FULFILMENT}', '{TENANT}', '{ORDER}', '{SITE}', 'released');
         INSERT INTO fulfilment_line (id, tenant_id, fulfilment_id, order_line_id, quantity)
         VALUES ('{FULFILMENT_LINE}', '{TENANT}', '{FULFILMENT}', '{ORDER_LINE}', {ORDERED})"));

    // The one durable reference to a stock cell the schema permits, per D12.
    step(&mut tx, "allocating stock against the line", &format!(
        "INSERT INTO stock_allocation (id, tenant_id, stock_id, quantity, state, firm,
             bound_at, fulfilment_line_id)
         SELECT '{ALLOCATION}', '{TENANT}', s.id, {ORDERED}, 'allocated', true, now(),
                '{FULFILMENT_LINE}'
           FROM stock s
          WHERE s.item_id = '{GLOVE}' AND s.holder_location_id IS NOT NULL
            AND s.quantity >= {ORDERED}
          ORDER BY s.id LIMIT 1"));

    // ---------------------------------------------------------------------
    // The floor picks it. From here it is all observation.
    // ---------------------------------------------------------------------
    // D97 found here: `created` asserts a placement, so it names a holder. Before
    // this migration the same event without one was accepted and then made the
    // containment fold raise, which aborted every projection for the tenant.
    step(&mut tx, "the shipping carton", &format!(
        "INSERT INTO package (id, tenant_id, fulfilment_id, sequence)
         VALUES ('{CARTON}', '{TENANT}', '{FULFILMENT}', 1);
         INSERT INTO package_event (id, tenant_id, package_id, kind, source, occurred_at,
             recorded_at, client_event_id, recorded_by_id, location_id)
         VALUES ('9ae00000-0000-0000-0000-0000000000e1', '{TENANT}', '{CARTON}',
             'created', 'operator_scan', now(), now(), '{EVENT}', '{PICKER}', '{BENCH}')"));

    // A pick is a movement from the bin into the carton — one ledger entry, both
    // sides of the cell key carried across.
    //
    // D99: it names the line it served. `from_location_id` is set, which is the
    // shape rule that makes this a pick rather than the consolidation or repack that
    // may follow it — holder-to-holder re-handling after a pick has no from-location
    // and is not counted a second time.
    step(&mut tx, "picking into the carton", &format!(
        "INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
             from_location_id, from_lot_id, from_status_id, from_owner_id,
             to_package_id, to_lot_id, to_status_id, to_owner_id,
             reason, occurred_at, recorded_at, recorded_by_id, fulfilment_line_id)
         SELECT '5b000000-0000-0000-0000-0000000000e1', '{TENANT}', '{EVENT}', '{GLOVE}',
                {ORDERED}, s.holder_location_id, s.lot_id, s.status_id, s.owner_id,
                '{CARTON}', s.lot_id, s.status_id, s.owner_id,
                'pick', now(), now(), '{PICKER}', '{FULFILMENT_LINE}'
           FROM stock s
          WHERE s.item_id = '{GLOVE}' AND s.holder_location_id IS NOT NULL
            AND s.quantity >= {ORDERED}
          ORDER BY s.id LIMIT 1"));

    step(&mut tx, "sealing it", &format!(
        "INSERT INTO package_event (id, tenant_id, package_id, kind, source, occurred_at,
             recorded_at, client_event_id, recorded_by_id)
         VALUES ('9ae00000-0000-0000-0000-0000000000e2', '{TENANT}', '{CARTON}',
             'sealed', 'operator_scan', now(), now(), '{EVENT}', '{PICKER}')"));

    step(&mut tx, "stamping the fulfilment picked and packed", &format!(
        "UPDATE fulfilment SET picked_at = now(), picked_by_id = '{PICKER}',
                packed_at = now(), packed_by_id = '{PICKER}'
          WHERE id = '{FULFILMENT}'"));

    // ---------------------------------------------------------------------
    // The carrier takes it. A consignment reaches its fulfilments through
    // packages rather than a direct FK, which is what lets one collection carry
    // parcels from several commitments (D15).
    // ---------------------------------------------------------------------
    step(&mut tx, "raising the consignment", &format!(
        "INSERT INTO consignment (id, tenant_id, carrier_consignment_number, despatch_at,
             currency)
         VALUES ('{CONSIGNMENT}', '{TENANT}', 'CON-WALK', now(), 'AUD');
         INSERT INTO consignment_package (tenant_id, consignment_id, package_id)
         VALUES ('{TENANT}', '{CONSIGNMENT}', '{CARTON}')"));

    step(&mut tx, "despatching", &format!(
        "INSERT INTO package_event (id, tenant_id, package_id, kind, source, occurred_at,
             recorded_at, client_event_id, recorded_by_id)
         VALUES ('9ae00000-0000-0000-0000-0000000000e3', '{TENANT}', '{CARTON}',
             'despatched', 'operator_scan', now(), now(), '{EVENT}', '{PICKER}')"));

    // The stock leaves. A movement with a from side and no to side, which the
    // whole-key CHECKs permit precisely so goods can exit the building.
    //
    // D99: no to side at all is the despatch shape, and the exact mirror of D45's
    // arrival test. It names the same line as the pick, and the two do not
    // double-count because they are separated by shape rather than summed together.
    step(&mut tx, "the stock leaving the building", &format!(
        "INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
             from_package_id, from_lot_id, from_status_id, from_owner_id,
             reason, occurred_at, recorded_at, recorded_by_id, fulfilment_line_id)
         SELECT '5b000000-0000-0000-0000-0000000000e2', '{TENANT}', '{EVENT}', '{GLOVE}',
                {ORDERED}, '{CARTON}', s.lot_id, s.status_id, s.owner_id,
                'despatch', now(), now(), '{PICKER}', '{FULFILMENT_LINE}'
           FROM stock s
          WHERE s.item_id = '{GLOVE}' AND s.holder_location_id IS NOT NULL
          ORDER BY s.id LIMIT 1"));

    // ---------------------------------------------------------------------
    // What the ledger now says, and what the commitment says about itself.
    // ---------------------------------------------------------------------
    //
    // The role changes here, and the change is itself a finding from the inbound
    // walk: the app may not fold its own writes, which is D25's separation and
    // question 163's subject. So the maintainer runs as the session role.
    tx.batch_execute("RESET ROLE").expect("stop being the app");
    tx.execute("SELECT projection_run_all(($1)::text::uuid)", &[&TENANT])
        .expect("fold");

    let (covered, picked, packed, despatched): (i64, i64, i64, i64) = {
        let r = tx
            .query_one(
                "SELECT covered_quantity, picked_quantity, packed_quantity,
                        despatched_quantity
                   FROM fulfilment_line WHERE id = ($1)::text::uuid",
                &[&FULFILMENT_LINE],
            )
            .expect("the line");
        (r.get(0), r.get(1), r.get(2), r.get(3))
    };

    let moved: i64 = tx
        .query_one(
            "SELECT coalesce(sum(quantity), 0)::bigint FROM stock_movement
              WHERE to_package_id = ($1)::text::uuid AND reason = 'pick'",
            &[&CARTON],
        )
        .expect("the ledger")
        .get(0);

    assert_eq!(covered, ORDERED, "the allocation covers the line");
    assert_eq!(moved, ORDERED, "the ledger records the pick that happened");

    // What the fold *would* say under D99's shape rule, computed here rather than by
    // a maintainer because the maintainer has not been changed yet. Picked is the
    // movements naming the line that left a storage location; despatched is those
    // with no to side at all, the mirror of D45's arrival test. The same two rows
    // answer both without double-counting, which is the property the rule exists for.
    let (ledger_picked, ledger_despatched): (i64, i64) = {
        let r = tx
            .query_one(
                "SELECT coalesce(sum(quantity) FILTER (
                            WHERE from_location_id IS NOT NULL), 0)::bigint,
                        coalesce(sum(quantity) FILTER (
                            WHERE to_location_id IS NULL AND to_package_id IS NULL), 0)::bigint
                   FROM stock_movement
                  WHERE fulfilment_line_id = ($1)::text::uuid",
                &[&FULFILMENT_LINE],
            )
            .expect("the ledger, grouped by the cause D99 added");
        (r.get(0), r.get(1))
    };

    assert_eq!(
        (ledger_picked, ledger_despatched),
        (ORDERED, ORDERED),
        "the ledger now says which line it served, and the shape rule separates the \
         pick from the despatch without counting either twice"
    );

    // **This was the finding, and D100 closed it.**
    //
    // Through migrations 51 to 53 these three read zero here while the ledger read
    // ten, because `projection_fulfilment_rebuild` folded `stock_allocation.state`
    // and the allocation is still `allocated` — advancing it is a separate act this
    // walk never performs and nothing requires. That gap was question 165, and this
    // assertion was written to hold it still until somebody closed it.
    //
    // It now asserts the agreement. Nothing in the walk changed to make that true:
    // the same pick, the same seal, the same despatch, and the fold reads the facts
    // they produced. `packed` is ten because the carton's winning status is
    // `despatched` and the pick went into it; a carton opened and never resealed
    // would read zero here with `picked` still ten, which is the case that makes
    // the three genuinely different numbers rather than one in three costumes.
    assert_eq!(
        (picked, packed, despatched),
        (ORDERED, ORDERED, ORDERED),
        "outbound progress folds the ledger since D100, so a pick, a seal and a \
         despatch that the ledger records must move all three"
    );

    // The allocation was never advanced past `allocated`, and coverage is still
    // ten. That is the split D99 and D100 exist for, visible in one row: coverage
    // is what we *mean* for this line and folds the intention; the other three are
    // what happened and fold the facts. Under migration 15 all four came from the
    // allocation, so this line would have read 10, 0, 0, 0 — covered by a plan
    // nobody carried out, against a carton already on a truck.
    assert_eq!(covered, ORDERED, "coverage still folds the intention, and it stands");

    tx.rollback().expect("rollback");
}

/// The ledger carries both cause arms, and the cause CHECK is no longer vacuous.
///
/// The inbound walk's guard was written by name and could not fail. This one asks by
/// shape, and it has now tripped once as designed: it asserted one cause arm until
/// D99 added the second, which is what a tripwire is for.
///
/// It keeps two things honest. The arm count is the D10-as-corrected cause set — two,
/// because migration 6 resolved the `discrepancy` cause in the other direction. And
/// the CHECK must name both arms: with one argument `num_nonnulls(x) <= 1` is always
/// true, which is how migration 21's version passed S3 for thirty-two migrations
/// while excluding nothing.
#[test]
fn the_ledger_names_both_causes_and_the_check_means_something() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = spork_invariants::connect_exclusive(&url);

    let causes: i64 = c
        .query_one(
            "SELECT count(*) FROM pg_constraint
              WHERE conrelid = 'stock_movement'::regclass AND contype = 'f'
                AND confrelid IN ('goods_receipt_line'::regclass,
                                  'fulfilment_line'::regclass)",
            &[],
        )
        .expect("catalogue")
        .get(0);

    assert_eq!(
        causes, 2,
        "D10 as corrected makes a movement name its cause with a typed FK, and the \
         outbound arm is what gives the progress fold a key to group by"
    );

    let def: String = c
        .query_one(
            "SELECT pg_get_constraintdef(oid) FROM pg_constraint
              WHERE conrelid = 'stock_movement'::regclass
                AND conname = 'stock_movement_cause_ck'",
            &[],
        )
        .expect("the cause check")
        .get(0);

    for arm in ["goods_receipt_line_id", "fulfilment_line_id"] {
        assert!(
            def.contains(arm),
            "stock_movement_cause_ck must name {arm}, or it excludes nothing and S3 \
             passes on a rule with nothing to say: {def}"
        );
    }
    assert!(
        def.contains("<= 1"),
        "S3's rule, and the third time in this schema: an internal move has no \
         demand-side cause and is still a movement: {def}"
    );
}
