//! Runs the job-asserted suite against a live database.
//!
//! The contract differs from the structural suite and the difference is the
//! point. A J failure is a `discrepancy`, never an error, so these checks return
//! findings and the harness decides. Here the harness is CI, so findings fail
//! the run. In production the same findings become rows in a queue and the floor
//! keeps moving.

use spork_invariants::jobs::{run, spec, Check, Verdict, ALL};
use spork_invariants::reason;
use postgres::Client;

fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

#[test]
fn every_id_has_a_spec_and_they_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for &id in ALL {
        assert!(seen.insert(id), "{id} appears twice in ALL");
        let inv = spec(id);
        assert_eq!(inv.id, id, "spec({id}) returned metadata for {}", inv.id);
        assert!(!inv.statement.is_empty(), "{id} has no statement");
    }
    assert_eq!(seen.len(), 73, "the register holds 73 job-asserted invariants");
}

#[test]
fn job_asserted_invariants_hold() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut client = spork_invariants::connect_exclusive(&url);

    let (mut passed, mut vacuous, mut pending, mut flagged) = (0, 0, 0, 0);
    let mut findings = vec![];

    println!("\n  job-asserted invariants\n");
    for &id in ALL {
        let inv = spec(id);
        let (verdict, examined, fs) = run(&mut client, id).expect("run check");
        match verdict {
            Verdict::Pass => {
                passed += 1;
                println!("  PASS     {id:<4} examined {examined:<5} {}", inv.statement);
            }
            Verdict::Vacuous => {
                vacuous += 1;
                println!("  VACUOUS  {id:<4} examined 0     {}", inv.statement);
            }
            Verdict::Pending => {
                pending += 1;
                if let Check::Pending(blockers) = inv.check {
                    println!("  PENDING  {id:<4}                {}", reason(blockers));
                }
            }
            Verdict::Findings => {
                flagged += 1;
                println!("  FINDING  {id:<4} {}", inv.statement);
                for f in fs {
                    println!("           [{}] {}", f.kind, f.detail);
                    findings.push(format!("{id} [{}]: {}", f.kind, f.detail));
                }
            }
        }
    }

    println!(
        "\n  {passed} passed, {vacuous} vacuous, {pending} pending, {flagged} raising findings of {}\n",
        ALL.len()
    );

    assert!(
        findings.is_empty(),
        "job-asserted invariants raised findings:\n{}",
        findings.join("\n")
    );
}

/// J6's second half, which cannot be a read-only check because demonstrating it
/// requires perturbing arrival order.
///
/// > replay in **any** arrival order is identical
///
/// The register calls this a property test and it is: the projection is a
/// compare-and-set over `(occurred_at, recorded_at, id)`, so shuffling the order
/// events were *received* in must not change what they fold to. If it does, the
/// projection is order-dependent and D5's "entries can arrive in any order
/// without changing the result" is false.
#[test]
fn package_fold_is_independent_of_arrival_order() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut client = spork_invariants::connect_exclusive(&url);

    let tenants: Vec<uuid_str::Uuid> = client
        .query("SELECT DISTINCT tenant_id::text FROM package_event", &[])
        .expect("tenants")
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect();

    if tenants.is_empty() {
        eprintln!("no package events: this property has nothing to test against");
        return;
    }

    let snapshot = |c: &mut Client| -> Vec<String> {
        c.query(
            "SELECT id::text || '|' || coalesce(parent_package_id::text,'-')
                    || '|' || coalesce(location_id::text,'-')
                    || '|' || coalesce(resolved_location_id::text,'-')
                    || '|' || coalesce(depth::text,'-')
               FROM package ORDER BY id",
            &[],
        )
        .expect("snapshot")
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect()
    };

    let before = snapshot(&mut client);

    for _ in 0..5 {
        // Perturb the order the server received them in, leaving the device
        // clock alone. recorded_at is only a tiebreak, so a different receive
        // order must fold to the same answer.
        client
            .execute(
                "UPDATE package_event SET recorded_at = now() - (random() * interval '2 days')",
                &[],
            )
            .expect("shuffle arrival order");
        for t in &tenants {
            client
                .execute("SELECT projection_package_rebuild(($1)::text::uuid)", &[t])
                .expect("rebuild");
        }
        let after = snapshot(&mut client);
        assert_eq!(
            before, after,
            "the package fold changed when arrival order changed, so the projection is \
             order-dependent and D5's arrival-order guarantee does not hold"
        );
    }
}

/// J46's second half, which D42 asked for in the same words as J6's.
///
/// > replay in **any** arrival order is identical
///
/// The order fold is not a compare-and-set like the package fold. It is a
/// last-writer-per-column aggregate over `(occurred_at, recorded_at, id)`, and
/// `recorded_at` is a real term in that ordering rather than a pure tiebreak. So
/// the property is narrower and worth stating precisely: shuffling the order the
/// server *received* amendments in must not change the fold, as long as it does
/// not reorder them against each other on the key itself.
///
/// This shuffles within the gaps rather than at random. Each amendment keeps its
/// position in the `recorded_at` sequence and moves inside it, which is what
/// varying network delay actually does to a queue of scans. A random restamp
/// would test something else: that the fold ignores a term the register says it
/// reads.
#[test]
fn order_fold_is_independent_of_arrival_order() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut client = spork_invariants::connect_exclusive(&url);

    let tenants: Vec<String> = client
        .query("SELECT DISTINCT tenant_id::text FROM intention_amendment", &[])
        .expect("tenants")
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect();

    if tenants.is_empty() {
        eprintln!("no amendments: this property has nothing to test against");
        return;
    }

    let snapshot = |c: &mut Client| -> Vec<String> {
        let mut rows: Vec<String> = c
            .query(
                "SELECT id::text || '|' || coalesce(promised_from::text,'-')
                        || '|' || coalesce(promised_to::text,'-')
                        || '|' || coalesce(required_by::text,'-')
                        || '|' || state::text
                   FROM \"order\"
                  UNION ALL
                 SELECT id::text || '|' || quantity_ordered::text
                        || '|' || coalesce(unit_price_minor::text,'-')
                        || '|' || coalesce(price_basis_quantity::text,'-')
                        || '|' || line_state::text
                   FROM order_line",
                &[],
            )
            .expect("snapshot")
            .iter()
            .map(|r| r.get::<_, String>(0))
            .collect();
        rows.sort();
        rows
    };

    let before = snapshot(&mut client);

    for _ in 0..5 {
        // Re-space the arrival times, keeping their sequence. The device clock
        // and the tiebreaking id are untouched.
        client
            .execute(
                "WITH ordered AS (
                     SELECT id, row_number() OVER (ORDER BY recorded_at, id) AS n
                       FROM intention_amendment)
                 UPDATE intention_amendment a
                    SET recorded_at = (SELECT max(occurred_at) FROM intention_amendment)
                                    + (o.n * interval '1 second')
                                    + (random() * interval '900 milliseconds')
                   FROM ordered o WHERE o.id = a.id",
                &[],
            )
            .expect("re-space arrival order");
        for t in &tenants {
            client
                .execute("SELECT projection_order_rebuild(($1)::text::uuid)", &[t])
                .expect("rebuild");
        }
        assert_eq!(
            before,
            snapshot(&mut client),
            "the amendment fold changed when arrival times moved without reordering, so the \
             projection depends on when the server happened to receive a change rather than on \
             when it happened"
        );
    }
}

/// J30's second half, which cannot be a read-only check because demonstrating it
/// requires running the rebuild.
///
/// > Rebuilding `expected_supply` preserves row identity. Truncate-and-regenerate
/// > is forbidden while any allocation holds an `expected_supply_id`.
///
/// The foreign key is `ON DELETE RESTRICT`, so a regenerating rebuild would
/// either fail outright or orphan a commitment. The maintainer upserts on the
/// arm's partial unique index instead, and this is what proves it: rebuild twice
/// and every id has to be the one it was.
#[test]
fn expected_supply_rebuild_preserves_identity() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut client = spork_invariants::connect_exclusive(&url);

    let tenants: Vec<String> = client
        .query("SELECT DISTINCT tenant_id::text FROM expected_supply", &[])
        .expect("tenants")
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect();

    if tenants.is_empty() {
        eprintln!("no expected supply: this property has nothing to test against");
        return;
    }

    let ids = |c: &mut Client| -> Vec<String> {
        let mut v: Vec<String> = c
            .query(
                "SELECT id::text || '|' || purchase_order_line_id::text FROM expected_supply",
                &[],
            )
            .expect("ids")
            .iter()
            .map(|r| r.get::<_, String>(0))
            .collect();
        v.sort();
        v
    };

    let before = ids(&mut client);

    for _ in 0..3 {
        for t in &tenants {
            client
                .execute("SELECT projection_expected_supply_rebuild(($1)::text::uuid)", &[t])
                .expect("rebuild");
        }
        assert_eq!(
            before,
            ids(&mut client),
            "rebuilding expected_supply changed a row identity, so any allocation holding \
             one is now pointing at a promise that no longer exists under that id"
        );
    }
}

/// D68. A rebuild that changes nothing writes nothing.
///
/// Every maintainer here is idempotent in the sense that mattered until now:
/// running it twice leaves the same values. **That is not the same as writing
/// nothing**, and the difference is invisible until there is a year of data to
/// see it against.
///
/// Measured before this was fixed: one `projection_run_all` over a year rewrote
/// **39,565 rows with nothing changed**, of which 35,040 were `expected_supply`.
/// Under MVCC every one of those is a new row version, so a projection rebuilt
/// hourly churns its whole table hourly — `expected_supply` carried 35,040 dead
/// tuples against 35,041 live ones, and only 8,427 of 243,214 updates were HOT,
/// so most of them wrote index entries too.
///
/// The cause is a class rather than a case: an `ON CONFLICT DO UPDATE` with no
/// `WHERE`, and an `UPDATE ... FROM` with no change predicate, both rewrite a row
/// to the value it already holds.
///
/// So this runs the whole set twice and asserts the second run writes nothing.
/// It is a property test rather than a check because it has to run the
/// maintainers to observe them, which is J6's and J46's shape.
#[test]
fn a_rebuild_that_changes_nothing_writes_nothing() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut client = spork_invariants::connect_exclusive(&url);

    let tenants: Vec<String> = client
        .query("SELECT id::text FROM tenant ORDER BY slug", &[])
        .expect("tenants")
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect();
    if tenants.is_empty() {
        eprintln!("no tenants: this property has nothing to test against");
        return;
    }

    // **`projection_freshness` is excluded by name, and the reason is written
    // down here so the exclusion cannot quietly widen.** D95 records when each
    // maintainer last ran *whether or not it found work*, because a fold that ran
    // and found nothing is fresh while a fold that has not run is stale, and the
    // two are indistinguishable if the stamp only moves on work. So this table is
    // written on every run by design, and counting it here would assert the
    // opposite of what D95 decided.
    //
    // What D68 is actually about survives untouched: it measured 39,565 rows
    // rewritten with nothing changed, 35,040 of them `expected_supply` — a table
    // churning in proportion to its own size. This one writes one row per step per
    // tenant, which is bounded by the register rather than by the data, and it is
    // the orchestrator recording itself rather than a maintainer rewriting a fold.
    // S17 carries the same shape of scope for the same reason.
    let written = |c: &mut Client| -> i64 {
        // pg_stat is updated asynchronously, so settle it before reading.
        c.execute("SELECT pg_stat_force_next_flush()", &[]).ok();
        c.query_one(
            "SELECT coalesce(sum(n_tup_upd + n_tup_ins), 0)::bigint
               FROM pg_stat_user_tables
              WHERE relname <> 'projection_freshness'",
            &[],
        )
        .expect("stat")
        .get(0)
    };

    // First run settles whatever was outstanding.
    for t in &tenants {
        client
            .execute("SELECT projection_run_all(($1)::text::uuid)", &[t])
            .expect("rebuild");
    }

    let before = written(&mut client);
    for t in &tenants {
        client
            .execute("SELECT projection_run_all(($1)::text::uuid)", &[t])
            .expect("rebuild");
    }
    let after = written(&mut client);

    assert_eq!(
        after - before,
        0,
        "a second rebuild with nothing changed rewrote {} rows. Under MVCC each is a dead \
         tuple, so a projection rebuilt hourly churns its whole table hourly",
        after - before
    );
}

/// J47 can fail, demonstrated rather than asserted.
///
/// **This session found two guards that could not fail**: one that named a column
/// the schema did not have, and one whose blocker named the larger absence and hid
/// a smaller one inside it. A check with a clean population proves it does not fire
/// on good data; it proves nothing at all about bad data.
///
/// So this builds the violation D43 says cannot happen — one pallet under two
/// purchase orders — runs the real check against it, and asserts it is found. The
/// rows are removed before anything is asserted, so a failure here leaves the
/// fixture as it was rather than poisoning every check after it.
#[test]
fn j47_finds_a_pallet_that_spans_two_purchase_orders() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut client = spork_invariants::connect_exclusive(&url);

    const TENANT: &str = "11111111-1111-1111-1111-111111111111";
    const PALLET: &str = "a5010000-0000-0000-0000-000000000002";
    const SECOND_PO: &str = "90000000-0000-0000-0000-0000000000ff";
    const SECOND_LINE: &str = "901e0000-0000-0000-0000-0000000000ff";
    const SECOND_CONTENT: &str = "a5c00000-0000-0000-0000-0000000000ff";

    client
        .batch_execute(&format!(
            "INSERT INTO purchase_order (id, tenant_id, site_id, supplier_party_id,
                 order_number, source_channel_id, currency, state, issued_at)
             VALUES ('{SECOND_PO}', '{TENANT}', 'a5170000-0000-0000-0000-000000000001',
                 '9a247000-0000-0000-0000-000000000002', 'PO-2026-XXXX',
                 '5c000000-0000-0000-0000-000000000003', 'AUD', 'issued', now());
             INSERT INTO purchase_order_line (id, tenant_id, purchase_order_id, item_id,
                 quantity_ordered)
             VALUES ('{SECOND_LINE}', '{TENANT}', '{SECOND_PO}',
                 '17e10000-0000-0000-0000-000000000001', 50);
             INSERT INTO asserted_unit_content (id, tenant_id, asserted_unit_id, raw_gtin,
                 quantity, entered_quantity, raw_unit_code, resolved_unit_id,
                 resolved_purchase_order_line_id)
             SELECT '{SECOND_CONTENT}', '{TENANT}', '{PALLET}', '09312345000012',
                 50, 50, 'PCE', u.id, '{SECOND_LINE}' FROM unit u WHERE u.code = 'ea'"
        ))
        .expect("a pallet carrying goods for a second purchase order");

    let outcome = run(&mut client, spork_invariants::jobs::Id::J47);

    // Clean up before asserting, so a failure here does not leave the fixture
    // carrying a violation every later check would trip over.
    client
        .batch_execute(&format!(
            "DELETE FROM asserted_unit_content WHERE id = '{SECOND_CONTENT}';
             DELETE FROM purchase_order_line WHERE id = '{SECOND_LINE}';
             DELETE FROM purchase_order WHERE id = '{SECOND_PO}'"
        ))
        .expect("clean");

    let (verdict, examined, findings) = outcome.expect("run J47");
    assert_eq!(verdict, Verdict::Findings, "J47 examined {examined} and found nothing");
    let kinds: Vec<&str> = findings.iter().map(|f| f.kind).collect();
    assert!(
        kinds.contains(&"subtree_spans_purchase_orders"),
        "the pallet spans two purchase orders and J47 did not say so: {kinds:?}"
    );
    assert!(
        kinds.contains(&"package_spans_purchase_orders"),
        "the pallet was collapsed onto a package, so the package arm should fire too: {kinds:?}"
    );
}

/// J68 can fail, demonstrated rather than asserted, and both arms fire separately.
///
/// The same reasoning as the J47 test above: a clean population proves a check does
/// not fire on good data and proves nothing about bad data. J68 is vacuous against
/// the fixture because nothing there writes `stock_movement.fulfilment_line_id`, and
/// a vacuous check is the weakest possible evidence that a check works.
///
/// **Since D100 this builds real projection drift rather than the architectural
/// gap it was written for.** Twenty units are picked and despatched against a line
/// whose progress columns still read zero, because the movements are written and
/// the maintainer has not run. That is exactly what J68 is for now: the projection
/// is behind the ledger that produces it, which is the condition every fold in this
/// system is allowed to be in briefly and none is allowed to stay in.
///
/// The two movements are deliberately different shapes. The pick has a `to` side, so
/// it answers the picked arm only; the despatch leaves a carton with no `to` side at
/// all, so it answers the despatched arm only. If the shape rules were confused with
/// each other -- or replaced by `reason`, which is a label not a fold discriminator
/// (D105) -- one of these two assertions would fail.
#[test]
fn j68_finds_progress_that_disagrees_with_the_ledger() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut client = spork_invariants::connect_exclusive(&url);

    const TENANT: &str = "11111111-1111-1111-1111-111111111111";
    const LINE: &str = "f11e0000-0000-0000-0000-000000000002";
    const ITEM: &str = "17e10000-0000-0000-0000-000000000001";
    const EVENT: &str = "ce000000-0000-0000-0000-000000000009";
    const PICKER: &str = "77770000-0000-0000-0000-000000000001";
    const CARTON: &str = "9ac00000-0000-0000-0000-00000000000a";
    const PICK: &str = "5b000000-0000-0000-0000-0000000000f1";
    const DESPATCH: &str = "5b000000-0000-0000-0000-0000000000f2";

    client
        .batch_execute(&format!(
            // A pick: out of a bin and into another holder. `from_location_id` is
            // set, which is D99's picked shape.
            "INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
                 from_location_id, from_lot_id, from_status_id, from_owner_id,
                 to_location_id, to_lot_id, to_status_id, to_owner_id,
                 reason, occurred_at, recorded_at, recorded_by_id, fulfilment_line_id)
             VALUES ('{PICK}', '{TENANT}', '{EVENT}', '{ITEM}', 20,
                 '10c00000-0000-0000-0000-000000000001',
                 '10700000-0000-0000-0000-000000000001',
                 '57a70000-0000-0000-0000-000000000001',
                 '9a247000-0000-0000-0000-000000000001',
                 '10c00000-0000-0000-0000-000000000003',
                 '10700000-0000-0000-0000-000000000001',
                 '57a70000-0000-0000-0000-000000000001',
                 '9a247000-0000-0000-0000-000000000001',
                 'pick', now(), now(), '{PICKER}', '{LINE}');
             -- A despatch: out of a carton and out of the building. No `to` side at
             -- all, which is D99's despatched shape and D45's arrival test mirrored.
             INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
                 from_package_id, from_lot_id, from_status_id, from_owner_id,
                 reason, occurred_at, recorded_at, recorded_by_id, fulfilment_line_id)
             VALUES ('{DESPATCH}', '{TENANT}', '{EVENT}', '{ITEM}', 20,
                 '{CARTON}',
                 '10700000-0000-0000-0000-000000000001',
                 '57a70000-0000-0000-0000-000000000001',
                 '9a247000-0000-0000-0000-000000000001',
                 'despatch', now(), now(), '{PICKER}', '{LINE}')"
        ))
        .expect("a pick and a despatch that name the line they served");

    let outcome = run(&mut client, spork_invariants::jobs::Id::J68);

    // Cleaned up before asserting, so a failure here does not leave the fixture
    // carrying movements every later check would fold.
    client
        .batch_execute(&format!(
            "DELETE FROM stock_movement WHERE id IN ('{PICK}', '{DESPATCH}')"
        ))
        .expect("clean");

    let (verdict, examined, findings) = outcome.expect("run J68");
    assert_eq!(verdict, Verdict::Findings, "J68 examined {examined} and found nothing");

    let messages: Vec<&str> = findings.iter().map(|f| f.detail.as_str()).collect();
    assert!(
        messages.iter().any(|m| m.contains("picked_quantity")),
        "twenty units left a bin against this line and J68 did not say so: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("despatched_quantity")),
        "twenty units left the building against this line and J68 did not say so: {messages:?}"
    );
}

/// D103's recursion, on the shape the fixture now holds and one level deeper.
///
/// The fixture receives a hundred, corrects four away, and corrects that correction
/// by one, so `quantity_received` reads 97 = 100 − (4 − 1). **96 is what D102 would
/// give** (one-level netting), and living in seed means every rebuild, J26 and J51
/// examine the chain rather than only a rolled-back test.
///
/// This then adds a third level briefly: the second correction was itself wrong by
/// one, putting received back to 96, and asserts the fold follows. Cleaned up so the
/// fixture stays at 97 for every later check.
#[test]
fn a_correction_to_a_correction_returns_the_original() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut client = spork_invariants::connect_exclusive(&url);

    const TENANT: &str = "11111111-1111-1111-1111-111111111111";
    const SUPPLY_LINE: &str = "901e0000-0000-0000-0000-000000000001";
    // The fixture's second-level correction: one of the four put back.
    const SECOND_CORRECTION: &str = "5b000000-0000-0000-0000-0000000000c2";
    const THIRD: &str = "5b000000-0000-0000-0000-0000000000fa";

    let received = |c: &mut Client| -> i64 {
        c.execute("SELECT projection_run_all(($1)::text::uuid)", &[&TENANT])
            .expect("fold");
        c.query_one(
            "SELECT e.quantity_received FROM expected_supply e
              WHERE e.purchase_order_line_id = ($1)::text::uuid",
            &[&SUPPLY_LINE],
        )
        .expect("the promise")
        .get(0)
    };

    let before = received(&mut client);

    // Depth three: reverse the second correction by one. Mirrors its target
    // (nothing -> bin becomes bin -> nothing). Same occurred_at as D47 / J50.
    client
        .batch_execute(&format!(
            "INSERT INTO stock_movement (id, tenant_id, client_event_id, item_id, quantity,
                 from_location_id, from_lot_id, from_status_id, from_owner_id,
                 reason, occurred_at, recorded_at, recorded_by_id,
                 reverses_movement_id, adjustment_reason_id)
             SELECT '{THIRD}', '{TENANT}', 'ce000000-0000-0000-0000-000000000004',
                    t.item_id, 1,
                    t.to_location_id, t.to_lot_id, t.to_status_id, t.to_owner_id,
                    'adjustment', t.occurred_at, '2026-08-04T01:00:00Z',
                    '77770000-0000-0000-0000-000000000001',
                    t.id,
                    (SELECT id FROM adjustment_reason
                      WHERE code = 'miscount' AND tenant_id IS NULL)
               FROM stock_movement t WHERE t.id = '{SECOND_CORRECTION}'"
        ))
        .expect("a third level of correction");

    let with_depth_three = received(&mut client);
    let verdicts: Vec<(spork_invariants::jobs::Id, Verdict)> =
        [
            spork_invariants::jobs::Id::J26,
            spork_invariants::jobs::Id::J50,
            spork_invariants::jobs::Id::J51,
            spork_invariants::jobs::Id::J52,
            spork_invariants::jobs::Id::J68,
        ]
        .into_iter()
        .map(|id| (id, run(&mut client, id).expect("run").0))
        .collect();

    client
        .batch_execute(&format!("DELETE FROM stock_movement WHERE id = '{THIRD}'"))
        .expect("clean");
    let after = received(&mut client);

    assert_eq!(
        before, 97,
        "the fixture receives a hundred, corrects four, corrects that by one"
    );
    assert_eq!(
        with_depth_three, 96,
        "a third level still nets: 100 − (4 − (1 − 1)) = 96"
    );
    assert_eq!(after, 97, "and the fixture is left as it was found");

    for (id, verdict) in verdicts {
        assert_eq!(
            verdict,
            Verdict::Pass,
            "{id} did not pass with a legal correction chain in place: recording one is \
             allowed, and D103's job was to make the folds able to read it"
        );
    }
}

use uuid as uuid_lib;

mod uuid_str {
    pub type Uuid = String;
}

/// J71 can fail, demonstrated rather than asserted.
///
/// The register's own warning applies to this one exactly: it asserts an
/// absence, so it passes on an empty population, and until the fixture carried
/// `pick_sequence` at all it examined nothing while reporting success.
///
/// So this builds the thing the real bin list contains — two bins in one site at
/// one walking position — runs the shipped check against it, and asserts it is
/// found. The row is removed before anything is asserted, so a failure here
/// leaves the fixture as it was.
#[test]
fn j71_finds_two_bins_at_one_walking_position() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut client = spork_invariants::connect_exclusive(&url);

    const TENANT: &str = "11111111-1111-1111-1111-111111111111";
    const SITE: &str = "a5170000-0000-0000-0000-000000000001";
    const RIVAL: &str = "10c00000-0000-0000-0000-0000000000ff";

    // A-01-1 sits at position 1 in the fixture; this claims the same one.
    client
        .batch_execute(&format!(
            "INSERT INTO location (id, tenant_id, site_id, code, kind, pick_sequence, active)
             VALUES ('{RIVAL}', '{TENANT}', '{SITE}', 'A-01-1-RIVAL', 'pick_face', 1, true);"
        ))
        .expect("the rival bin");

    let outcome = spork_invariants::jobs::run(&mut client, spork_invariants::jobs::Id::J71);

    client
        .batch_execute(&format!("DELETE FROM location WHERE id = '{RIVAL}';"))
        .expect("the test removes the bin it added");

    let (_, examined, findings) = outcome.expect("J71 runs");
    assert!(examined > 0, "J71 examined nothing, so it proved nothing");
    let said: Vec<&str> = findings.iter().map(|f| f.detail.as_str()).collect();
    assert!(
        said.iter().any(|d| d.contains("A-01-1-RIVAL")),
        "J71 did not report two bins at one position: {said:?}"
    );
}

/// J72 can fail, demonstrated rather than asserted.
///
/// The register's warning applies here as it does to J71: this asserts an
/// absence, and the writer refuses the thing it looks for, so the only rows it
/// will ever examine are ones that came in some other way. **A check whose
/// population is only ever produced by the defect it hunts is a check that
/// reports success for as long as the defect stays away — and cannot be shown
/// to be looking in the right place.**
///
/// That is exactly D138's risk. `POST /observations` rejects a length at `each`
/// with no presentation, so if the invariant's SQL named a wrong column or
/// scoped itself to the wrong level, nothing would ever notice. So this writes
/// what a loader would write — straight into the table, no writer involved,
/// which is the case J72 exists for — and asserts the shipped check finds it.
#[test]
fn j72_finds_a_size_with_no_arrangement_behind_it() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut client = spork_invariants::connect_exclusive(&url);

    const TENANT: &str = "11111111-1111-1111-1111-111111111111";
    const GLOVE: &str = "17e10000-0000-0000-0000-000000000001";
    const ACT: &str = "e5e10000-0000-0000-0000-0000000000f2";
    const FACT: &str = "0b5f0000-0000-0000-0000-0000000000f2";

    // **The subject is found rather than minted.** `observable` is one row per
    // thing under a partial unique index, so a fixed id here collides with
    // whatever else has measured a glove and `ON CONFLICT DO NOTHING` then
    // leaves the id pointing at nothing — which is the get-or-create the writer
    // does and the reason the registry has that index at all.
    let subject: uuid_lib::Uuid = client
        .query_one(
            "INSERT INTO observable (tenant_id, item_id, packaging_level)
             VALUES ($1, $2, 'each')
             ON CONFLICT (tenant_id, item_id, packaging_level, item_packing_config_id)
                  WHERE item_id IS NOT NULL
             DO UPDATE SET item_id = excluded.item_id
             RETURNING id",
            &[
                &uuid_lib::Uuid::parse_str(TENANT).unwrap(),
                &uuid_lib::Uuid::parse_str(GLOVE).unwrap(),
            ],
        )
        .expect("the each subject")
        .get(0);

    // An act with no presentation on it, and a length. The act still claims a
    // `client_event`, because D25 makes that the identity of the act rather
    // than a courtesy of the HTTP writer.
    client
        .batch_execute(&format!(
            "INSERT INTO client_event
                 (tenant_id, client_event_id, recorded_by_id, submitted_at, received_at)
             VALUES ('{TENANT}', '{ACT}', '77770000-0000-0000-0000-000000000001', now(), now());
             INSERT INTO observation_event
                 (id, tenant_id, client_event_id, observable_id, observed_at, recorded_at,
                  method, ingestion_channel, recorded_by_id)
             VALUES ('{ACT}', '{TENANT}', '{ACT}', '{subject}', now(), now(),
                     'transcribed', 'csv', '77770000-0000-0000-0000-000000000001');
             INSERT INTO observation
                 (id, tenant_id, observation_event_id, observable_id, observed_at,
                  client_event_id, metric_id, result_kind, dimension_id, value_numeric)
             SELECT '{FACT}', '{TENANT}', '{ACT}', '{subject}', now(), '{ACT}',
                    m.id, 'quantity', m.dimension_id, 240
               FROM metric m WHERE m.code = 'length' AND m.tenant_id IS NULL;"
        ))
        .expect("the unqualified length");

    let outcome = spork_invariants::jobs::run(&mut client, spork_invariants::jobs::Id::J72);

    // The registry row stays. It is one row per thing and something else has
    // almost certainly measured a glove already; removing it would be removing
    // a subject this test did not create.
    client
        .batch_execute(&format!(
            "DELETE FROM observation WHERE id = '{FACT}';
             DELETE FROM observation_event WHERE id = '{ACT}';
             DELETE FROM client_event WHERE client_event_id = '{ACT}';"
        ))
        .expect("the test removes the rows it added");

    let (_, examined, findings) = outcome.expect("J72 runs");
    assert!(examined > 0, "J72 examined nothing, so it proved nothing");
    let said: Vec<&str> = findings.iter().map(|f| f.detail.as_str()).collect();
    assert!(
        said.iter().any(|d| d.contains(FACT)),
        "J72 did not report a length of a single thing with no arrangement: {said:?}"
    );
}
