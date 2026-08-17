//! The mediated write, run as the role it exists to constrain.
//!
//! **This is the test whose absence is question 161.** D89 and D90 both named the
//! same pattern — take UPDATE away from the application, hand it a function that
//! refuses when the rule says refuse — and both were verified as `postgres`,
//! which has every privilege and bypasses row-level security. Under that role a
//! function with no privileges of its own works, a function the caller could have
//! skipped looks mediated, and a definer that escapes tenancy looks safe.
//!
//! Every assertion here therefore runs after `SET ROLE nylonite_app`. S53 and S54
//! assert the structure; this asserts the behaviour, because the structure was
//! right in the register and wrong in the database for four migrations.
//!
//! Skips rather than fails without `DATABASE_URL`.

use postgres::Client;

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const OTHER_TENANT: &str = "22222222-2222-2222-2222-222222222222";
const ACTOR: &str = "77770000-0000-0000-0000-000000000001";
const GLOVE: &str = "17e10000-0000-0000-0000-000000000001";
/// Frozen by a receipt line and by no check, which is D91's fixture shape.
const FROZEN_CLAIM: &str = "a5c00000-0000-0000-0000-000000000002";

fn client() -> Option<Client> {
    let url = std::env::var("DATABASE_URL").ok()?;
    Some(nylonite_invariants::connect_exclusive(&url))
}

/// The server's own message rather than `db error`, which is all Display gives.
fn db_message(e: &postgres::Error) -> String {
    e.as_db_error().map(|d| d.message().to_string()).unwrap_or_else(|| e.to_string())
}

/// Everything here writes, so every test runs inside a transaction it rolls back.
fn as_app(c: &mut Client, tenant: &str, body: impl FnOnce(&mut postgres::Transaction)) {
    let mut tx = c.transaction().expect("begin");
    tx.batch_execute("SET LOCAL ROLE nylonite_app").expect("become the app");
    tx.execute("SELECT set_config('nylonite.tenant_id', $1, true)", &[&tenant])
        .expect("name the tenant");
    body(&mut tx);
    tx.rollback().expect("rollback");
}

#[test]
fn the_app_can_perform_the_write_the_function_mediates() {
    let Some(mut c) = client() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };

    // A claim nothing has compared against yet, so the freeze does not apply and
    // D21's re-resolution must succeed.
    let mut tx = c.transaction().expect("begin");
    tx.execute(
        "INSERT INTO asserted_unit_content
             (id, tenant_id, asserted_unit_id, raw_gtin, quantity, entered_quantity,
              raw_unit_code, resolved_unit_id)
         SELECT 'a5c00000-0000-0000-0000-0000000000fe', ($1)::text::uuid,
                'a5010000-0000-0000-0000-000000000002', '09312345000012', 50, 50,
                'PCE', u.id FROM unit u WHERE u.code = 'ea'",
        &[&TENANT],
    )
    .expect("an unfrozen claim");

    tx.batch_execute("SET LOCAL ROLE nylonite_app").expect("become the app");
    tx.execute("SELECT set_config('nylonite.tenant_id', $1, true)", &[&TENANT])
        .expect("name the tenant");

    // Before D94 this raised `permission denied for table asserted_unit_content`
    // on the function's own first statement.
    tx.execute(
        "SELECT asserted_unit_content_resolve('a5c00000-0000-0000-0000-0000000000fe',
                    ($1)::text::uuid, NULL, ($2)::text::uuid, 'manual')",
        &[&GLOVE, &ACTOR],
    )
    .expect("the success path D21 insists on");

    let resolved: bool = tx
        .query_one(
            "SELECT resolved_item_id IS NOT NULL FROM asserted_unit_content
              WHERE id = 'a5c00000-0000-0000-0000-0000000000fe'",
            &[],
        )
        .expect("the claim")
        .get(0);
    assert!(resolved, "a claim nothing has used re-resolves, which is half of D21");
    tx.rollback().expect("rollback");
}

#[test]
fn the_freeze_still_refuses_under_the_same_role() {
    let Some(mut c) = client() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    as_app(&mut c, TENANT, |tx| {
        let refused = tx.execute(
            "SELECT asserted_unit_content_resolve(($1)::text::uuid, ($2)::text::uuid,
                        NULL, ($3)::text::uuid, 'manual')",
            &[&FROZEN_CLAIM, &GLOVE, &ACTOR],
        );
        let e = refused.expect_err("a claim a receipt line has used is frozen");
        let msg = db_message(&e);
        assert!(
            msg.contains("frozen"),
            "the refusal must be the freeze rather than a privilege error: {msg}"
        );
    });
}

#[test]
fn the_app_cannot_make_the_write_without_the_function() {
    let Some(mut c) = client() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    as_app(&mut c, TENANT, |tx| {
        // D89's guarantee, which was a courtesy until D94: the app held UPDATE on
        // the disposition columns and could write them itself, so "a change of
        // mind is a correction, not a second disposition" bound only the callers
        // that chose to be bound.
        let direct = tx.execute(
            "UPDATE goods_receipt_line SET accepted_at = now()
              WHERE id = '92c10000-0000-0000-0000-000000000001'",
            &[],
        );
        assert!(
            direct.is_err(),
            "the app rewrote a disposition without going through the function that refuses"
        );
    });

    as_app(&mut c, TENANT, |tx| {
        let direct = tx.execute(
            "UPDATE asserted_unit_content SET resolved_item_id = ($1)::text::uuid
              WHERE id = ($2)::text::uuid",
            &[&GLOVE, &FROZEN_CLAIM],
        );
        assert!(direct.is_err(), "the app rewrote a resolution D90 froze");
    });
}

#[test]
fn a_definer_does_not_carry_the_caller_across_tenants() {
    let Some(mut c) = client() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    // The property that makes SECURITY DEFINER safe here rather than a hole. The
    // owner has neither SUPERUSER nor BYPASSRLS and the table forces RLS, so the
    // function sees what the caller's tenant sees and nothing else.
    as_app(&mut c, OTHER_TENANT, |tx| {
        let reached = tx.execute(
            "SELECT asserted_unit_content_resolve(($1)::text::uuid, ($2)::text::uuid,
                        NULL, ($3)::text::uuid, 'manual')",
            &[&FROZEN_CLAIM, &GLOVE, &ACTOR],
        );
        let e = reached.expect_err("another tenant's claim is not reachable");
        let msg = db_message(&e);
        assert!(
            msg.contains("no asserted unit content"),
            "it must be invisible rather than merely refused: {msg}"
        );
    });
}
