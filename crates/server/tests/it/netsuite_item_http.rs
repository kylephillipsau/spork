//! What NetSuite says of an item, beside what Spork recorded (D237), over HTTP.
//!
//! The item details carry its article number, its picture's file, its weight
//! and its UPC. Spork asks for the picture it doesn't hold, keeps it as
//! NetSuite's, shows it until anything of Spork's pictures the item, finds the
//! item by its article number, and says where NetSuite's weight disagrees with
//! what was weighed here. Nothing of either is written over the other.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

#[actix_web::test]
async fn netsuites_word_on_an_item_is_kept_beside_sporks() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let db = state.pool.get().await.expect("a connection");
    db.execute("SELECT set_config('spork.tenant_id', $1, false)", &[&TENANT])
        .await
        .expect("the tenant scope");
    let n = Uuid::new_v4().simple().to_string()[..8].to_uppercase();
    let code = format!("ABC-{n}");
    let item: Uuid = db
        .query_one(
            "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
             SELECT current_tenant(), $1, 'Hi-vis vest', u.id, 'none' FROM unit u WHERE u.code = 'ea'
             RETURNING id",
            &[&code],
        )
        .await
        .expect("the item")
        .get(0);

    let app = test::init_service(App::new().app_data(state.clone()).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);
    let call = |req: test::TestRequest| {
        let auth = auth.clone();
        let app = &app;
        async move {
            let r = test::call_service(app, req.insert_header(auth).to_request()).await;
            let status = r.status().as_u16();
            (status, serde_json::from_slice::<Value>(&test::read_body(r).await).unwrap_or(Value::Null))
        }
    };
    let session = common::bearer_without_site(&app).await;
    let minted = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/tokens")
            .insert_header(("authorization", session))
            .set_json(json!({ "label": "NetSuite's item details, from a test" }))
            .to_request(),
    )
    .await;
    let minted: Value = serde_json::from_slice(&test::read_body(minted).await).unwrap();
    let machine = ("authorization", format!("Bearer {}", minted["token"].as_str().unwrap()));
    let bridge = |req: test::TestRequest| {
        let machine = machine.clone();
        let app = &app;
        async move {
            let r = test::call_service(app, req.insert_header(machine).to_request()).await;
            let status = r.status().as_u16();
            (status, serde_json::from_slice::<Value>(&test::read_body(r).await).unwrap_or(Value::Null))
        }
    };
    // A feed of its own: a load replaces what its feed said before.
    let source = format!("test-netsuite-{n}");
    let page = || async {
        let (status, page) = call(test::TestRequest::get().uri(&format!("/items/{item}"))).await;
        assert_eq!(status, 200, "{page}");
        page
    };

    // ── weighed here at 500 g ────────────────────────────────────────────
    let (status, said) = call(test::TestRequest::post().uri("/observations").set_json(json!({
        "item_id": item, "packaging_level": "each",
        "measurements": [{ "metric": "gross_weight", "entered_value": "0.5", "unit": "kg" }],
        "method": "instrument", "client_event_id": Uuid::new_v4(), "occurred_at": "2026-10-08T01:00:00Z",
    })))
    .await;
    assert_eq!(status, 200, "{said}");
    db.execute("SELECT projection_observation_current_rebuild(current_tenant())", &[]).await.expect("the fold");

    // ── NetSuite's details: its article number, picture, 1.2 kg, its UPC ─
    let details = format!(
        "Item,Supplier Part No.,Picture,Weight,Weight Unit,UPC\n{code},ART-{n},F{n},1.2,kg,9312345678903\n"
    );
    let (status, loaded) = bridge(
        test::TestRequest::post()
            .uri(&format!("/import/item-details?apply=true&source={source}&as_at=2026-10-08T02:00:00Z"))
            .set_payload(details),
    )
    .await;
    assert_eq!(status, 200, "{loaded}");
    assert_eq!(loaded["survey"]["with_picture"], 1, "{loaded}");

    // ── the picture it names, asked for, brought, and not asked for again ─
    let wanted = || bridge(test::TestRequest::get().uri(&format!("/import/item-pictures/wanted?source={source}")));
    let (status, asked) = wanted().await;
    assert_eq!(status, 200, "{asked}");
    assert_eq!(asked, json!([{ "item": code, "file": format!("F{n}") }]));
    let picture = common::png(640, 480);
    let bring = |file: String| {
        bridge(
            test::TestRequest::post()
                .uri(&format!("/import/item-pictures?source={source}&item={code}&file={file}"))
                .insert_header(("content-type", "image/png"))
                .set_payload(picture.clone()),
        )
    };
    let (status, kept) = bring(format!("F{n}")).await;
    assert_eq!((status, &kept["changed"]), (200, &json!(true)), "{kept}");
    let digest = kept["digest"].as_str().unwrap().to_string();
    let (_, again) = bring(format!("F{n}")).await;
    assert_eq!(again["changed"], false, "the same picture twice is kept once");
    let (status, _) = bring("OTHER".into()).await;
    assert_eq!(status, 400, "a file the details don't name is not taken");
    assert_eq!(wanted().await.1, json!([]), "held, so not asked for");

    // ── on the item: NetSuite's word, beside Spork's, and where they differ ─
    let p = page().await;
    let ns = &p["netsuite"];
    assert_eq!((&ns["art_no"], &ns["picture"], &ns["weight_g"]), (&json!(format!("ART-{n}")), &json!(digest), &json!(1200)), "{p}");
    assert_eq!(ns["weight_differs"], true, "1.2 kg against 500 g weighed here: {ns}");
    assert_eq!((&p["picture"]["digest"], &p["picture"]["source"]), (&json!(digest), &json!("netsuite")), "pictured by NetSuite until Spork has one");
    let (status, _) = call(test::TestRequest::get().uri(&format!("/images/{digest}"))).await;
    assert_eq!(status, 200, "and its bytes are served");

    // ── not this product, then its main picture after all ────────────────
    let say = |said: &str| {
        call(test::TestRequest::post().uri(&format!("/items/{item}/pictures")).set_json(json!({
            "said": said, "digest": digest, "client_event_id": Uuid::new_v4(), "occurred_at": "2026-10-08T03:00:00Z",
        })))
    };
    assert_eq!(say("not_it").await.0, 204);
    let p = page().await;
    assert!(p["picture"].is_null() && p["netsuite"]["picture_not_it"] == true, "shown to nobody: {p}");
    assert_eq!(say("main").await.0, 204);
    let p = page().await;
    assert_eq!((&p["main_picture"], &p["picture"]["digest"]), (&json!(digest), &json!(digest)), "{p}");

    // ── found by its article number, and listed where NetSuite differs ───
    let (_, found) = call(test::TestRequest::get().uri(&format!("/items?q=ART-{n}"))).await;
    assert_eq!(found["items"][0]["code"], code.as_str(), "{found}");
    assert_eq!(found["items"][0]["art_no"], format!("ART-{n}"));
    let (_, differs) = call(test::TestRequest::get().uri(&format!("/items?needs=netsuite&q={code}"))).await;
    assert_eq!(differs["total"], 1, "{differs}");
    let (_, scanned) = call(test::TestRequest::get().uri(&format!("/resolve?scan=ART-{n}"))).await;
    let hit = scanned["subjects"].as_array().or(scanned.as_array()).cloned().unwrap_or_default();
    assert!(
        hit.iter().any(|s| s["code"] == code.as_str() && s["via"] == "item_art_no"),
        "a box's article number scanned finds the item: {scanned}"
    );
}
