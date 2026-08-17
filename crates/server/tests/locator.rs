//! Resolving a scan, over HTTP, against the three surfaces.
//!
//! `crate::barcodes` proves the parsing and `locator::outcome_of` proves the
//! four outcome words, both without a database. What neither can prove is the
//! part D34 actually decides: that a GTIN read off a carton and the same trade
//! item's EAN-13 land on **one row**, that a closed `effective` range stops
//! resolving, and that a tenant's own binding wins over the shared catalogue.
//!
//! Those are properties of a query, and the seed carries exactly the three
//! bindings that exercise them.

use actix_web::{test, web, App};
use nylonite_server::{routes, AppState};
use serde_json::Value;

mod common;
use common::{pool, url};


#[actix_web::test]
async fn a_scan_resolves_to_what_it_names_and_to_nothing_else() {
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;

    let bearer = common::bearer(&app).await;

    let anon = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/resolve?scan=GLV-M-100")
            .to_request(),
    )
    .await;
    assert_eq!(anon.status(), 401, "a machine is told plainly");

    let get = |uri: String| {
        let bearer = bearer.clone();
        let app = &app;
        async move {
            let r = test::call_service(
                app,
                test::TestRequest::get()
                    .uri(&uri)
                    .insert_header(("authorization", bearer))
                    .to_request(),
            )
            .await;
            let status = r.status();
            let bytes = test::read_body(r).await;
            let text = String::from_utf8_lossy(&bytes).to_string();
            assert!(status.is_success(), "GET {uri} returned {status}: {text}");
            serde_json::from_str::<Value>(&text).expect("the locator answers JSON")
        }
    };

    // ── the defect D34 exists to prevent ────────────────────────────────
    //
    // The gumboot's binding is stored as fourteen characters. Its printed
    // EAN-13 is thirteen and the GS1-128 on the carton carries the fourteen
    // under AI 01. **Both must land on the same row**, which is exactly what
    // storing the unnormalised form breaks — silently, and only for the carton.
    let retail = get("/resolve?scan=9312345678907".into()).await;
    let carton = get("/resolve?scan=%1D0109312345678907".into()).await;

    for (what, r) in [("retail EAN-13", &retail), ("carton GS1-128", &carton)] {
        assert_eq!(r["outcome"], "resolved", "{what}: {r}");
        assert_eq!(r["gtin"], "09312345678907", "{what} normalises to fourteen");
        assert_eq!(r["subjects"].as_array().unwrap().len(), 1, "{what}");
        assert_eq!(r["subjects"][0]["kind"], "item", "{what}");
        assert_eq!(r["subjects"][0]["code"], "STY-7720-08", "{what}");
        assert_eq!(r["subjects"][0]["via"], "item_barcode", "{what}");
    }
    assert_eq!(
        retail["subjects"][0]["id"], carton["subjects"][0]["id"],
        "one trade item, however it was printed — this is the whole of D34"
    );

    // ── the scan lands on the worklist's own subjects ───────────────────
    //
    // **The trap this guards.** A scan resolves to an item, and opening a
    // carton session against a styled variant would walk back into the trip
    // D108 exists to prevent: the worklist offers the *style's* carton, not the
    // variant's. The resolver returns `capture::subjects_for_item`, so there is
    // one enumeration rather than two that can disagree.
    let capture = retail["subjects"][0]["capture"].as_array().expect("capture subjects");
    assert!(!capture.is_empty(), "a scanned item offers somewhere to capture");
    for s in capture {
        let arms = [
            s["item_id"].is_string(),
            s["item_style_id"].is_string(),
            s["item_part_id"].is_string(),
        ];
        assert_eq!(
            arms.iter().filter(|a| **a).count(),
            1,
            "each subject names one arm, as POST /observations demands: {s}"
        );
        // A part has no packaging level and the read sends null so that nothing
        // can post one. D139 — asserted here rather than left to the day a
        // scanned item first has parts.
        match s["packaging_level"].as_str() {
            Some(level) => assert!(level == "each" || level == "carton", "got {level}"),
            None => assert!(
                s["item_part_id"].is_string(),
                "no level and no part is no subject at all: {s}"
            ),
        }
    }
    // STY-7720-08 is in style STY-7720, and the seed gives the style the carton
    // figures. So the carton subject offered here is the style's.
    let styled_carton = capture.iter().find(|s| {
        s["packaging_level"] == "carton" && s["item_style_id"].is_string()
    });
    assert!(
        styled_carton.is_some(),
        "the style's carton is offered rather than the variant's: {capture:?}"
    );

    // ── an internal code, and an exact one ──────────────────────────────
    let internal = get("/resolve?scan=GLV-M-100".into()).await;
    assert_eq!(internal["outcome"], "resolved");
    assert_eq!(internal["subjects"][0]["code"], "GLOVE-M");
    assert_eq!(internal["subjects"][0]["via"], "item_barcode");
    // A prefix is not a match. This is a locator, not a search, and resolving
    // `GLV-M` to whichever binding sorted first is the confident wrong answer.
    let prefix = get("/resolve?scan=GLV-M".into()).await;
    assert_eq!(prefix["outcome"], "identifier_unknown", "{prefix}");

    // ── the SKU arm ─────────────────────────────────────────────────────
    let by_code = get("/resolve?scan=GLOVE-M".into()).await;
    assert_eq!(by_code["outcome"], "resolved");
    assert_eq!(by_code["subjects"][0]["via"], "item_code");

    // ── the range is not decoration ─────────────────────────────────────
    //
    // `LEGACY-9` meant the glove until the end of 2025. Without
    // `effective @> CURRENT_DATE` in the query it would still resolve, and the
    // whole argument for a daterange over an `active` boolean would be
    // ornamental.
    let closed = get("/resolve?scan=LEGACY-9".into()).await;
    assert_eq!(
        closed["outcome"], "identifier_unknown",
        "a closed binding stops resolving and the row stays for the audit: {closed}"
    );

    // ── the four words are four states, not two ─────────────────────────
    //
    // A well-formed GTIN nobody stocks is a different report from a smudge, and
    // an operator can act on the first.
    let unheld = get("/resolve?scan=12345670".into()).await;
    assert_eq!(unheld["outcome"], "identifier_unknown");
    assert_eq!(unheld["gtin"], "00000012345670", "well-formed, and not ours");

    // ── narrowing ───────────────────────────────────────────────────────
    //
    // A screen that can only act on items says so, and a carton scanned there
    // does not navigate somewhere nobody asked to go.
    let narrowed = get("/resolve?scan=GLOVE-M&expect=location".into()).await;
    assert_eq!(
        narrowed["outcome"], "identifier_unknown",
        "an item is not offered to a screen expecting a location: {narrowed}"
    );

    // ── one scan, three answers ─────────────────────────────────────────
    //
    // A GS1-128 carton label carries the lot and the expiry beside the GTIN.
    // Receiving stops keying batch numbers, which is most of the operational
    // value of doing this properly.
    let full = get("/resolve?scan=010931234567890717260101101ABC42".into()).await;
    assert_eq!(full["outcome"], "resolved");
    assert_eq!(full["gtin"], "09312345678907");
    assert_eq!(full["expiry"], "260101");
    assert_eq!(full["lot"], "1ABC42");

    // An empty scan is refused rather than resolved to everything.
    let empty = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/resolve?scan=%20")
            .insert_header(("authorization", bearer))
            .to_request(),
    )
    .await;
    assert_eq!(empty.status(), 400, "a scan of nothing is not a scan");
}
