//! What a maintainer that raises costs, demonstrated on a real one.
//!
//! D97 found the first way in: a `created` package event with no holder made the
//! containment fold raise, `projection_run_all` aborted at ordinal 60 and rolled
//! back, and every projection for that tenant was left unwritten with nothing
//! recording why. D97 closed that case. **D98 is about the blast radius**, which
//! belonged to the orchestrator rather than to the constraint.
//!
//! So this registers a step that raises on purpose, runs the real orchestrator
//! against it, and asserts the three things D98 changed: the steps before it are
//! kept, the failure is recorded where a human looks, and the steps after it are
//! not run.
//!
//! Skips rather than fails without `DATABASE_URL`.

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
/// Between the taxonomy closures (10, 20) and the stock fold (30), so there is
/// something both before and after it.
const ORDINAL: i32 = 25;

fn setup(c: &mut postgres::Client) {
    c.batch_execute(
        "CREATE OR REPLACE FUNCTION projection_breaks_on_purpose(p_tenant uuid)
             RETURNS bigint LANGUAGE plpgsql AS $$
         BEGIN
             RAISE EXCEPTION 'this maintainer cannot complete'
                 USING ERRCODE = 'raise_exception';
         END $$;",
    )
    .expect("a step that raises");
    // D95's `freshness_bound` is NOT NULL with no default precisely so a step added
    // later has to state one, and it caught this test on its first run.
    c.execute(
        "INSERT INTO projection_step (function_name, ordinal, note, freshness_bound)
         VALUES ('projection_breaks_on_purpose', $1,
                 'D98 test fixture; removed by the test that adds it',
                 interval '1 hour')",
        &[&ORDINAL],
    )
    .expect("declare it");
}

fn teardown(c: &mut postgres::Client) {
    let _ = c.execute(
        "DELETE FROM projection_freshness WHERE function_name = 'projection_breaks_on_purpose'",
        &[],
    );
    let _ = c.execute(
        "DELETE FROM projection_step WHERE function_name = 'projection_breaks_on_purpose'",
        &[],
    );
    let _ = c.batch_execute("DROP FUNCTION IF EXISTS projection_breaks_on_purpose(uuid)");
    let _ = c.execute(
        "DELETE FROM discrepancy WHERE kind = 'projection_failed'
          AND detail LIKE 'projection_breaks_on_purpose%'",
        &[],
    );
}

#[test]
fn one_maintainer_that_raises_does_not_cost_the_others() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = nylonite_invariants::connect_exclusive(&url);

    // A clean baseline, so "was not run" means what it says.
    c.execute("SELECT projection_run_all(($1)::text::uuid)", &[&TENANT])
        .expect("settle");
    c.execute(
        "UPDATE projection_freshness SET last_run_at = now() - interval '1 day'
          WHERE tenant_id = ($1)::text::uuid",
        &[&TENANT],
    )
    .expect("age every stamp so a fresh one is visible");

    setup(&mut c);

    // The orchestrator itself must not raise. Before D98 this call propagated the
    // exception and the caller's transaction was the thing that rolled back.
    let ran = c.execute("SELECT projection_run_all(($1)::text::uuid)", &[&TENANT]);

    let checks = (|| -> Result<(bool, bool, bool, bool), postgres::Error> {
        let before: bool = c
            .query_one(
                "SELECT last_run_at > now() - interval '1 minute'
                   FROM projection_freshness
                  WHERE tenant_id = ($1)::text::uuid
                    AND function_name = 'projection_item_class_closure_rebuild'",
                &[&TENANT],
            )?
            .get(0);
        let after: bool = c
            .query_one(
                "SELECT last_run_at > now() - interval '1 minute'
                   FROM projection_freshness
                  WHERE tenant_id = ($1)::text::uuid
                    AND function_name = 'projection_stock_rebuild'",
                &[&TENANT],
            )?
            .get(0);
        let recorded: bool = c
            .query_one(
                "SELECT last_error IS NOT NULL AND last_run_at IS NULL
                   FROM projection_freshness
                  WHERE tenant_id = ($1)::text::uuid
                    AND function_name = 'projection_breaks_on_purpose'",
                &[&TENANT],
            )?
            .get(0);
        let raised: bool = c
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM discrepancy
                                 WHERE kind = 'projection_failed'
                                   AND detail LIKE 'projection_breaks_on_purpose%')",
                &[],
            )?
            .get(0);
        Ok((before, after, recorded, raised))
    })();

    teardown(&mut c);
    // Leave the tenant folded, so nothing downstream sees the aged stamps.
    c.execute("SELECT projection_run_all(($1)::text::uuid)", &[&TENANT])
        .expect("re-settle");

    assert!(ran.is_ok(), "the orchestrator propagated the exception: {ran:?}");
    let (before, after, recorded, raised) = checks.expect("read back");

    assert!(
        before,
        "a step that ran before the failure was rolled back with it, which is the \
         whole defect D98 exists to remove"
    );
    assert!(
        !after,
        "a step after the failure ran anyway, folding over inputs the failed step \
         never wrote — the ordinal is a dependency order"
    );
    assert!(
        recorded,
        "the failure left no trace on projection_freshness, so J66 would report the \
         projection as merely stale and nothing would say why"
    );
    assert!(
        raised,
        "the failure raised no discrepancy, so it never reaches the queue a human reads"
    );
}
