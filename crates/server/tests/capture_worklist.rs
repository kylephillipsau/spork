//! The capture worklist, over HTTP, and the one property it owes.
//!
//! `crate::capture` decides three things a unit test cannot check: that the
//! query runs at all, that the route is wired, and that the classification it
//! makes over real rows puts **a subject on one list or none**. The classifier
//! is a total function and `capture::tests` proves that much over every
//! combination of its inputs; what it cannot prove is that the query feeding it
//! yields one row per subject. A subject enumerated twice — by the style arm and
//! by its own — arrives as two rows, is classified twice, and lands on two
//! lists, and no amount of testing the function finds it.
//!
//! That failure has a name in this repository. The despatch bench asserts a
//! carton is waiting, booked or gone and never two of them, because a carton on
//! two lists is a screen telling somebody to send something twice. This is the
//! same assertion about the thing being measured: a subject on two lists is a
//! screen sending somebody to weigh one carton twice, which is exactly the trip
//! D108 exists to prevent.
//!
//! Nothing here writes. The read is read-only and the assertions are properties
//! of whatever the database happens to hold, not counts of fixture rows — the
//! suite runs against a database other tests have already written to.

use actix_web::{test, web, App};
use spork_server::{routes, AppState};
use serde_json::Value;
use std::collections::BTreeSet;

mod common;
use common::{pool, url};


/// The identity of a subject, which is what `POST /observations` takes.
fn key(s: &Value) -> String {
    format!(
        "{}/{}/{}/{}",
        s["item_id"].as_str().unwrap_or("-"),
        s["item_style_id"].as_str().unwrap_or("-"),
        s["item_part_id"].as_str().unwrap_or("-"),
        s["packaging_level"].as_str().unwrap_or("part")
    )
}

#[actix_web::test]
async fn the_capture_worklist_puts_each_subject_on_one_list() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let bearer = common::bearer(&app).await;

    let anon = test::call_service(
        &app,
        test::TestRequest::get().uri("/capture").to_request(),
    )
    .await;
    assert_eq!(anon.status(), 401, "a machine is told plainly");

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/capture")
            .insert_header(("authorization", bearer.clone()))
            .to_request(),
    )
    .await;
    let status = resp.status();
    let bytes = test::read_body(resp).await;
    let text = String::from_utf8_lossy(&bytes).to_string();
    assert!(
        status.is_success(),
        "GET /capture returned {status}: {}",
        if text.is_empty() {
            "(empty body — is the route registered?)"
        } else {
            &text
        }
    );
    let screen: Value = serde_json::from_str(&text).expect("the worklist is JSON");

    assert!(screen["site"].is_string(), "the worklist says where it is");

    // ── the property ────────────────────────────────────────────────────
    //
    // One list now, in walking order. It used to be three, and the property
    // this asserted — that nothing is on two of them — is the same property
    // said of one: a subject appears once.
    let walk = screen["walk"].as_array().expect("the worklist draws the walk");

    // **The walk has rows in it, and this is not a formality.** Everything
    // below is a property of each row, so an empty list satisfies all of it —
    // and the walk is now filtered to what `stock` says is on a shelf here,
    // which is exactly the kind of filter that can go from "correct" to
    // "returns nothing" without a single assertion noticing. The fixture puts
    // stock at Melbourne, so a walk with nothing on it means the filter, the
    // projection or the seed has moved.
    assert!(
        !walk.is_empty(),
        "the walk is empty: every assertion below would pass over nothing"
    );

    let seen: Vec<String> = walk.iter().map(key).collect();
    let unique: BTreeSet<&String> = seen.iter().collect();
    assert_eq!(
        unique.len(),
        seen.len(),
        "a subject is on the walk once; these are on it twice: {seen:?}"
    );

    // ── the walk is a walk ──────────────────────────────────────────────
    //
    // **Bin order, and it is the whole point of the list.** An operator holding
    // this is doing a lap of the building; an order that is anything else is a
    // list of homework. A row with no bin sorts last rather than first.
    let bins: Vec<Option<&str>> = walk
        .iter()
        .map(|s| s["location_code"].as_str())
        .collect();
    let ordered: Vec<Option<&str>> = {
        let mut b = bins.clone();
        b.sort_by_key(|x| (x.is_none(), *x));
        b
    };
    assert_eq!(bins, ordered, "the walk is not in bin order: {bins:?}");

    // **Only what is on a shelf here.** A catalogue row with no stock at this
    // site is not a thing anybody can walk to, and the printed sheet this
    // replaces carries a stock figure on every row for the same reason.
    for subject in walk {
        assert!(
            subject["soh"].as_i64().expect("soh") > 0,
            "{} is on the walk with nothing on the shelf",
            subject["code"]
        );
    }

    // ── and the rows say what they are ──────────────────────────────────
    {
        for subject in walk {
            let code = subject["code"].as_str().expect("a subject names itself");
            assert!(!code.is_empty(), "a subject with no code cannot be walked to");

            // Exactly one subject arm, which is what the write path requires:
            // "name exactly one subject".
            let arms = [
                subject["item_id"].is_string(),
                subject["item_style_id"].is_string(),
                subject["item_part_id"].is_string(),
            ];
            assert_eq!(
                arms.iter().filter(|a| **a).count(),
                1,
                "{code} names one subject arm, as POST /observations demands"
            );

            // **A part carries no level, and that is the assertion.** D139: the
            // read sends null precisely so that a screen cannot post one, and a
            // part that arrived carrying `each` would be a request the writer
            // refuses built from a row the read produced.
            match subject["packaging_level"].as_str() {
                Some(level) => assert!(
                    level == "each" || level == "carton",
                    "{code} is offered at a level the floor is asked to capture, got {level}"
                ),
                None => assert!(
                    subject["item_part_id"].is_string(),
                    "{code} has no packaging level and is not a part, which is no subject at all"
                ),
            }

            let wants: Vec<&str> = subject["wants"]
                .as_array()
                .expect("wants")
                .iter()
                .filter_map(Value::as_str)
                .collect();

            // **`wants` agrees with the figures beside it.** The list and the
            // numbers are two renderings of one state, and a screen drawing
            // "wants weight" against a weight is the drift this asserts away.
            //
            // **A declared absence answers the question too.** D138: an apron
            // with no bounding box is answered rather than missing, so `wants`
            // agrees with *figure or declared absence*, not with figure alone.
            let has_weight =
                subject["gross_weight_g"].is_number() || subject["weight_absent"] == true;
            assert_eq!(
                !wants.contains(&"weight"),
                has_weight,
                "{code}: wants {wants:?} against gross_weight_g {} and weight_absent {}",
                subject["gross_weight_g"],
                subject["weight_absent"]
            );
            let has_dimensions = (subject["length_mm"].is_number()
                && subject["width_mm"].is_number()
                && subject["height_mm"].is_number())
                || subject["dimensions_absent"] == true;
            assert_eq!(
                !wants.contains(&"dimensions"),
                has_dimensions,
                "{code}: wants {wants:?} against l/w/h and dimensions_absent {}",
                subject["dimensions_absent"]
            );
            let has_faces = !subject["faces"].as_array().expect("faces").is_empty();
            assert_eq!(
                !wants.contains(&"photographs"),
                has_faces,
                "{code}: wants {wants:?} against faces {}",
                subject["faces"]
            );

            // **Why a row is here and what it wants cannot disagree.** This
            // used to be checked against the list a subject was on; the list
            // is gone and `because` carries what it said, one word finer —
            // `overdue` and `never-measured` shared a list and are not one
            // reason.
            let why = subject["because"].as_str().expect("a row says why it is here");
            match why {
                "nothing" => assert_eq!(
                    wants.len(),
                    3,
                    "{code} says nothing is recorded and something is"
                ),
                "incomplete" => assert!(
                    !wants.is_empty() && wants.len() < 3,
                    "{code} says incomplete and wants {wants:?}"
                ),
                "never-measured" | "overdue" => assert!(
                    wants.is_empty(),
                    "{code} says {why} and is not complete: {wants:?}"
                ),
                other => panic!("{code} is on the walk saying {other}"),
            }

            // **A figure that is not the subject's own says whose it is.**
            // D108: a screen that cannot tell them apart reports a number
            // nobody took against this code as though somebody had.
            if subject["source"].as_str() == Some("style") {
                assert!(
                    subject["style_code"].is_string(),
                    "{code} inherits its figures and does not say from what"
                );
            }
        }
    }

    // A photograph is never claimed for a subject with no figures *and* no
    // pictures: `nothing` means nothing at all.
    for subject in walk
        .iter()
        .filter(|s| s["because"].as_str() == Some("nothing"))
    {
        assert!(
            subject["faces"].as_array().unwrap().is_empty(),
            "{} says nothing is recorded and has photographs",
            subject["code"]
        );
        assert!(
            subject["method"].is_null(),
            "{} says nothing is recorded and names a method",
            subject["code"]
        );
    }

    // ── the limit is a limit ────────────────────────────────────────────
    let capped: Value = {
        let r = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/capture?limit=1")
                .insert_header(("authorization", bearer))
                .to_request(),
        )
        .await;
        assert!(r.status().is_success(), "the worklist takes a limit");
        test::read_body_json(r).await
    };
    // **One limit over one list, and it used to be one limit per list.** Three
    // lists capped at one each is three rows for `limit=1`, which is the sort
    // of arithmetic nobody notices until a handheld asks for ten and gets
    // thirty.
    assert!(
        capped["walk"].as_array().unwrap().len() <= 1,
        "the walk respects the limit"
    );
}
