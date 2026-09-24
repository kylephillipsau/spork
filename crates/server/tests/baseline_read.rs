//! What a thing has weighed before, read out of the database it is stored in.
//!
//! **The unit tests prove the arithmetic and cannot prove the query.** Every
//! figure in `baseline`'s own tests is a slice of integers, so a cast to a type
//! that does not exist, a column on the wrong arm, or an enum comparison the
//! planner refuses would all pass them and fail on the floor. That is not a
//! hypothetical: `pack_walk_http.rs` exists because thirty-one endpoints were
//! written and never executed, and the three defects it caught were *a query
//! against a table that does not exist, a cast to a type that does not exist,
//! and an INSERT on a column the application holds no grant on*. This query has
//! two casts and joins four tables.
//!
//! # What it is really checking
//!
//! Migration 73 — *a style is what gets measured* — means the observations that
//! answer "what does one carton weigh" are mostly **not** against the SKU. A
//! read of the item arm alone returns nothing for the catalogue this was built
//! for, and it returns it silently, as an item nobody has ever weighed. So the
//! two assertions that matter are that a style's weighings are found at all,
//! and that they lose to the SKU's own the moment somebody weighs one.
//!
//! # What it writes
//!
//! Observations, and then removes them — by `client_event_id`, never by a
//! predicate, which is the rule `weighing_http.rs` states after that shape bit
//! three times. It also makes the two `observable` rows the fixture has no
//! carton arm for, and removes those too.

use spork_server::auth::Caller;
use spork_server::baseline::{self, Source};
use spork_server::AppState;
use uuid::Uuid;

use actix_web::web;

mod common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const PERSON: &str = "77770000-0000-0000-0000-000000000001";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";

/// Nitrile glove, medium. Belongs to no style — migration 73's ordinary case.
const GLOVE: &str = "17e10000-0000-0000-0000-000000000001";
const GLOVE_CARTON_CONFIG: &str = "9ac40000-0000-0000-0000-000000000001";
const SIZED_CARTON_CONFIG: &str = "9ac40000-0000-0000-0000-000000000002";

/// One size of a styled code. The fixture's carton observable is against the
/// **style**, which is the whole point.
const SIZED: &str = "17e10000-0000-0000-0000-000000000002";
const STYLE_CARTON_OBSERVABLE: &str = "0b5e0000-0000-0000-0000-000000000001";

/// Made by this test, so it can be removed by this test.
const GLOVE_CARTON_OBSERVABLE: &str = "0b5e0000-0000-0000-0000-0000000000f1";
const SIZED_CARTON_OBSERVABLE: &str = "0b5e0000-0000-0000-0000-0000000000f2";
/// The glove's `each` arm, which the fixture already carries — an each needs no
/// case pack, by `observable_item_config_ck`.
const GLOVE_EACH_OBSERVABLE: &str = "0b500000-0000-0000-0000-000000000001";

/// A fulfilment with a carton that has twenty gloves in it.
const PACKED_FULFILMENT: &str = "f01f0000-0000-0000-0000-000000000002";
const PACKED_CARTON: &str = "9ac00000-0000-0000-0000-00000000000d";

const SECOND_CONFIG: &str = "9ac40000-0000-0000-0000-0000000000f1";
const SECOND_OBSERVABLE: &str = "0b5e0000-0000-0000-0000-0000000000f3";
const ACT: &str = "c1e70000-0000-0000-0000-0000000000f1";

/// Seed the weighings, run the read, and take them out again.
///
/// One test rather than four, because the setup is a dozen rows and the
/// assertions are all about the same read of them. Splitting it would either
/// seed four times or share state between test functions, and this binary's
/// neighbours have paid for the second of those.
#[actix_web::test]
async fn what_a_thing_has_weighed_resolves_by_specificity_and_reaches_both_screens() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let (c, conn) = tokio_postgres::connect(&u, tokio_postgres::NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = conn.await;
    });

    // **Runs twice.** The handover's first warning is that this suite is not
    // re-runnable against the same database, and a test that fails partway
    // through seeding leaves its rows behind and then fails on the primary key
    // for every run after — reporting a collision rather than whatever went
    // wrong the first time. Everything this test creates is a fixed identifier,
    // so it can take its own leavings out before it starts.
    clear(&c).await;

    // A carton arm for the glove. `observable_item_config_ck`: anything above an
    // `each` needs a case pack to be a definite thing.
    c.batch_execute(&format!(
        "INSERT INTO client_event (tenant_id, client_event_id, site_id,
                                   recorded_by_id, submitted_at)
              VALUES ('{TENANT}', '{ACT}', '{SITE}', '{PERSON}', now());

         INSERT INTO observable (id, tenant_id, item_id, packaging_level,
                                 item_packing_config_id)
              VALUES ('{GLOVE_CARTON_OBSERVABLE}', '{TENANT}', '{GLOVE}',
                      'carton', '{GLOVE_CARTON_CONFIG}');"
    ))
    .await
    .expect("a carton arm for the glove");

    // Four weighings against the style and four against the glove, of which
    // three of each are eligible. The ineligible ones are the point: each is a
    // rule in `projection_observation_current_rebuild` that this read mirrors.
    //
    // One statement per row rather than one set-returning INSERT: pairing an
    // event with its value inside a CTE needs a key the values do not have, and
    // eight statements in a test is not the place to invent one.
    let rows: [(&str, &str, Option<&str>, i64); 8] = [
        (STYLE_CARTON_OBSERVABLE, "instrument", None, 5000),
        (STYLE_CARTON_OBSERVABLE, "instrument", None, 5010),
        (STYLE_CARTON_OBSERVABLE, "instrument", None, 4990),
        // Typed off a supplier's sheet. Not a witness to anything we did.
        (STYLE_CARTON_OBSERVABLE, "transcribed", None, 99999),
        (GLOVE_CARTON_OBSERVABLE, "instrument", None, 11400),
        (GLOVE_CARTON_OBSERVABLE, "instrument", None, 11402),
        (GLOVE_CARTON_OBSERVABLE, "instrument", None, 11398),
        // Theirs. D23: never trust their weight over our scale.
        (
            GLOVE_CARTON_OBSERVABLE,
            "asserted",
            Some("9a247000-0000-0000-0000-000000000002"),
            88888,
        ),
    ];
    for (observable, method, party, grams) in rows {
        let party = party.map(|p| format!("'{p}'")).unwrap_or("NULL".into());
        c.batch_execute(&format!(
            "WITH m AS (SELECT id, dimension_id FROM metric WHERE code = 'gross_weight'),
             ev AS (
               INSERT INTO observation_event
                 (tenant_id, client_event_id, observable_id, observed_at,
                  recorded_by_id, method, ingestion_channel, asserted_by_party_id)
               VALUES ('{TENANT}', '{ACT}', '{observable}', now(), '{PERSON}',
                       '{method}', 'scale', {party})
               RETURNING id)
             INSERT INTO observation
               (tenant_id, observation_event_id, observable_id, observed_at,
                client_event_id, metric_id, result_kind, dimension_id, value_numeric)
             SELECT '{TENANT}', ev.id, '{observable}', now(), '{ACT}',
                    m.id, 'quantity', m.dimension_id, {grams}
               FROM ev CROSS JOIN m;"
        ))
        .await
        .expect("a weighing");
    }

    let state = web::Data::new(AppState { pool: pool(&u) });
    let who = Caller {
        session_id: Uuid::nil(),
        person_id: PERSON.parse().unwrap(),
        tenant_id: TENANT.parse().unwrap(),
        site_id: Some(SITE.parse().unwrap()),
    };
    let glove: Uuid = GLOVE.parse().unwrap();
    let sized: Uuid = SIZED.parse().unwrap();
    // The case pack each is being counted against. A carton is only a definite
    // object relative to one, so it is part of the subject rather than context.
    let glove_config: Uuid = GLOVE_CARTON_CONFIG.parse().unwrap();
    let sized_config: Uuid = SIZED_CARTON_CONFIG.parse().unwrap();

    let got = baseline::for_items(&state, &who, &[(glove, Some(glove_config)), (sized, Some(sized_config))], "carton")
        .await
        .expect("the read runs");

    // The glove was weighed as a carton, and belongs to no style.
    let g = got.get(&glove).expect("the glove has a baseline");
    assert_eq!(g.source, Source::Own);
    assert_eq!(g.grams, 11400, "the median of three, not the mean of four");
    assert_eq!(
        g.n, 3,
        "a supplier's assertion is not one of our weighings"
    );
    assert!(g.is_established());

    // Nobody has weighed this size. Its style's cartons are what it ships in.
    let s = got.get(&sized).expect("the sized code inherits");
    assert_eq!(s.source, Source::Style, "and it says the figure is borrowed");
    assert_eq!(s.grams, 5000);
    assert_eq!(
        s.n, 3,
        "the fixture's transcribed carton weight is not a weighing either"
    );

    // Now somebody weighs this size once. Most specific wins — and the count
    // drops to one, because that is how many times this code has been weighed.
    c.batch_execute(&format!(
        "INSERT INTO observable (id, tenant_id, item_id, packaging_level,
                                 item_packing_config_id)
              VALUES ('{SIZED_CARTON_OBSERVABLE}', '{TENANT}',
                      '{SIZED}', 'carton', '{SIZED_CARTON_CONFIG}');
         WITH m AS (SELECT id, dimension_id FROM metric WHERE code = 'gross_weight'),
         ev AS (
           INSERT INTO observation_event
             (tenant_id, client_event_id, observable_id, observed_at,
              recorded_by_id, method, ingestion_channel)
           VALUES ('{TENANT}', '{ACT}', '{SIZED_CARTON_OBSERVABLE}',
                   now(), '{PERSON}', 'instrument', 'scale')
           RETURNING id)
         INSERT INTO observation
           (tenant_id, observation_event_id, observable_id, observed_at,
            client_event_id, metric_id, result_kind, dimension_id, value_numeric)
         SELECT '{TENANT}', ev.id, '{SIZED_CARTON_OBSERVABLE}', now(),
                '{ACT}', m.id, 'quantity', m.dimension_id, 5300
           FROM ev CROSS JOIN m;"
    ))
    .await
    .expect("one weighing of this very size");

    let got = baseline::for_items(&state, &who, &[(sized, Some(sized_config))], "carton")
        .await
        .expect("the read runs again");
    let s = got.get(&sized).expect("still has a baseline");
    assert_eq!(s.source, Source::Own, "somebody measured this one");
    assert_eq!(s.grams, 5300);
    assert_eq!(
        s.n, 1,
        "one weighing of this code, not four pooled with its style's"
    );
    assert!(
        !s.is_established(),
        "and one weighing is not yet a baseline"
    );

    for grams in [500, 502, 498] {
        c.batch_execute(&format!(
            "WITH m AS (SELECT id, dimension_id FROM metric WHERE code = 'gross_weight'),
             ev AS (
               INSERT INTO observation_event
                 (tenant_id, client_event_id, observable_id, observed_at,
                  recorded_by_id, method, ingestion_channel)
               VALUES ('{TENANT}', '{ACT}', '{GLOVE_EACH_OBSERVABLE}', now(),
                       '{PERSON}', 'instrument', 'scale')
               RETURNING id)
             INSERT INTO observation
               (tenant_id, observation_event_id, observable_id, observed_at,
                client_event_id, metric_id, result_kind, dimension_id, value_numeric)
             SELECT '{TENANT}', ev.id, '{GLOVE_EACH_OBSERVABLE}', now(), '{ACT}',
                    m.id, 'quantity', m.dimension_id, {grams}
               FROM ev CROSS JOIN m;"
        ))
        .await
        .expect("a weighing of one glove");
    }


    // **And the dock serves it, per level.** The receive screen offers the
    // levels a receiver can count in, and each one carries what a single one of
    // them has weighed — `each` and `carton` are two different figures about
    // one code, which is the whole reason the baseline hangs off the level
    // rather than the line.
    //
    // Asserted here rather than after the next block: a second case pack for
    // the glove exists from that point on, and `levels_of` takes the newest
    // config, so the carton figure would legitimately be the other box's.
    let dock = spork_server::receiving_list::screen(&state, &who, SITE.parse().unwrap(), 50)
        .await
        .expect("the receiving read runs");
    let line = dock
        .lines
        .iter()
        .find(|l| l.item_id == glove)
        .expect("the glove is expected at this site");
    let at = |name: &str| {
        line.levels
            .iter()
            .find(|l| l.level == name)
            .unwrap_or_else(|| panic!("the screen offers {name}"))
    };
    // The carton figure is asserted exactly and the each figure is not. The
    // carton observable belongs to this test; the glove's `each` arm is the
    // fixture's, and `capture_walk.rs` and `weighing_http.rs` both weigh it —
    // a stray row below 498 would move the lower median off 500 and fail this
    // for a reason that has nothing to do with what it checks.
    let each = at("each").baseline.expect("one glove has been weighed");
    assert!(each.n >= 3, "the three this test recorded, at least");
    assert!(!each.borrowed);
    let carton = at("carton").baseline.expect("and so has a carton of them");
    assert_eq!(carton.grams, 11400, "the carton figure, not the each one");
    assert!(carton.established);
    assert_ne!(
        each.grams, carton.grams,
        "a level is part of the subject: one glove and a carton of them are \
         different questions with different answers"
    );

    // A corrected case pack is a different box, and its weighings are not this
    // box's. Migration 7 versioned `item_packing_config` for exactly this, and
    // `observable_item_idx` carries it in the key — so a carton of twelve and a
    // carton of twenty-four must not meet in one median.
    c.batch_execute(&format!(
        "INSERT INTO item_packing_config (id, tenant_id, item_id, inners_per_carton)
              VALUES ('{SECOND_CONFIG}', '{TENANT}', '{GLOVE}', 2);
         INSERT INTO observable (id, tenant_id, item_id, packaging_level,
                                 item_packing_config_id)
              VALUES ('{SECOND_OBSERVABLE}', '{TENANT}', '{GLOVE}', 'carton',
                      '{SECOND_CONFIG}');"
    ))
    .await
    .expect("a second case pack for the glove");
    for grams in [22000, 22010, 21990] {
        c.batch_execute(&format!(
            "WITH m AS (SELECT id, dimension_id FROM metric WHERE code = 'gross_weight'),
             ev AS (
               INSERT INTO observation_event
                 (tenant_id, client_event_id, observable_id, observed_at,
                  recorded_by_id, method, ingestion_channel)
               VALUES ('{TENANT}', '{ACT}', '{SECOND_OBSERVABLE}', now(),
                       '{PERSON}', 'instrument', 'scale')
               RETURNING id)
             INSERT INTO observation
               (tenant_id, observation_event_id, observable_id, observed_at,
                client_event_id, metric_id, result_kind, dimension_id, value_numeric)
             SELECT '{TENANT}', ev.id, '{SECOND_OBSERVABLE}', now(), '{ACT}',
                    m.id, 'quantity', m.dimension_id, {grams}
               FROM ev CROSS JOIN m;"
        ))
        .await
        .expect("a weighing of the bigger box");
    }

    let second: Uuid = SECOND_CONFIG.parse().unwrap();
    let got = baseline::for_items(&state, &who, &[(glove, Some(glove_config))], "carton")
        .await
        .expect("the read runs");
    let g = got.get(&glove).expect("the original box still has a baseline");
    assert_eq!(
        g.grams, 11400,
        "the bigger box's weighings are not this box's"
    );
    assert_eq!(g.n, 3, "and they are not counted as evidence about it either");

    let got = baseline::for_items(&state, &who, &[(glove, Some(second))], "carton")
        .await
        .expect("the read runs");
    let g = got.get(&glove).expect("the bigger box has its own baseline");
    assert_eq!(g.grams, 22000);
    assert_eq!(g.n, 3);

    // **And the bench actually serves it.** Everything above proves the read;
    // this proves the wiring, which is the half `pack_walk_http.rs` exists
    // because nobody checked. The fixture's second fulfilment has a carton
    // holding twenty gloves, so once the glove has an `each` baseline the
    // carton has an expected weight — and `stock_movement.quantity` is base
    // units by migration 46, which is what makes `each` the right level.
    let cartons = spork_server::bench::cartons_on(&state, &who, PACKED_FULFILMENT.parse().unwrap())
        .await
        .expect("the bench read runs");
    let carton = cartons
        .iter()
        .find(|c| c.id.to_string() == PACKED_CARTON)
        .expect("the fixture's packed carton is on the bench");
    let e = carton
        .expected
        .as_ref()
        .expect("and it now says what it should weigh");
    // Twenty gloves at the median of three weighings, on a preset with no
    // stated tare.
    assert_eq!(e.baseline.grams, 20 * 500);
    // **At least three, rather than exactly three.** `capture_walk.rs` and
    // `weighing_http.rs` both weigh this glove at `each` and both delete by
    // act — but the handover's whole first section is about runs that fail
    // partway and leave rows behind, and an exact count here would then fail
    // with a number instead of a reason. The median is unmoved by a stray
    // weighing either way, which is the property being relied on.
    assert!(e.baseline.n >= 3, "the three this test recorded, at least");
    assert!(!e.baseline.borrowed, "the glove belongs to no style");
    assert!(e.baseline.established);

    // Out again, by act.
    clear(&c).await;
}

/// Everything this test creates, removed by identifier.
///
/// **By act, never by a predicate.** `weighing_http.rs` states the rule after
/// the alternative bit three times: its first version deleted every recent
/// finding whose detail looked like a weighing, and reached into one another
/// test in the same binary was still asserting on. A test may remove what it
/// created and nothing else.
async fn clear(c: &tokio_postgres::Client) {
    c.batch_execute(&format!(
        "DELETE FROM observation WHERE client_event_id = '{ACT}';
         DELETE FROM observation_event WHERE client_event_id = '{ACT}';
         DELETE FROM client_event WHERE client_event_id = '{ACT}';
         DELETE FROM observable WHERE id IN ('{GLOVE_CARTON_OBSERVABLE}',
                                             '{SIZED_CARTON_OBSERVABLE}',
                                             '{SECOND_OBSERVABLE}');
         DELETE FROM item_packing_config WHERE id = '{SECOND_CONFIG}';"
    ))
    .await
    .expect("the test removes what it recorded");
}
