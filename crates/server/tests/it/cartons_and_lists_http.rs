//! A carton said at the item (D178), and a list of items to work through
//! (D179), over HTTP.
//!
//! An item is modelled as itself and its carton as a box of so many of it.
//! Its page offers the carton whatever is on file, and saying how many it
//! holds makes it a definite thing to measure. A list is a sheet's codes, kept
//! in the sheet's order, that the item list narrows to.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

fn nonce() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        % 1_000_000_000
}

#[actix_web::test]
async fn a_carton_is_said_at_the_item_and_then_measured() {
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
    let made = |code: String| {
        let db = &db;
        async move {
            db.query_one(
                "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
                 SELECT current_tenant(), $1, 'Roll towel, 16 to a carton', u.id, 'none'
                   FROM unit u WHERE u.code = 'ea'
                 RETURNING id",
                &[&code],
            )
            .await
            .expect("the item")
            .get::<_, Uuid>(0)
        }
    };
    let n = nonce();
    let item = made(format!("CTN-{n}")).await;

    let app = test::init_service(App::new().app_data(state.clone()).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);
    let page = |id: Uuid| {
        let auth = auth.clone();
        let app = &app;
        async move {
            common::ok_json(
                app,
                test::TestRequest::get().uri(&format!("/items/{id}")).insert_header(auth).to_request(),
                "the item's page",
            )
            .await
        }
    };
    let post = |uri: String, body: Value| {
        let auth = auth.clone();
        let app = &app;
        async move {
            let r = test::call_service(
                app,
                test::TestRequest::post().uri(&uri).insert_header(auth).set_json(body).to_request(),
            )
            .await;
            let status = r.status().as_u16();
            let text = String::from_utf8_lossy(&test::read_body(r).await).to_string();
            (status, serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text)))
        }
    };
    let say = |id: Uuid, holds: Value, event: Uuid| {
        post(
            format!("/items/{id}/carton"),
            json!({ "holds": holds, "client_event_id": event, "occurred_at": "2026-10-02T00:00:00Z" }),
        )
    };
    let weigh_carton = |id: Uuid| {
        post(
            "/observations".into(),
            json!({
                "item_id": id,
                "packaging_level": "carton",
                "measurements": [{ "metric": "gross_weight", "entered_value": "5.2", "unit": "kg" }],
                "method": "instrument",
                "ingestion_channel": "keyed",
                "client_event_id": Uuid::new_v4(),
                "occurred_at": "2026-10-02T00:00:00Z",
            }),
        )
    };

    // ── its page offers the carton before anything says it has one ──────
    let before = page(item).await;
    let levels: Vec<&str> = before["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["packaging_level"].as_str())
        .collect();
    assert_eq!(levels, ["carton", "each"], "the carton first, then the item itself: {before}");
    let carton = &before["subjects"][0];
    assert_eq!(carton["item_id"], item.to_string());
    assert_eq!(carton["wants"], json!([]), "a carton nobody has said asks for nothing: {carton}");
    assert!(before["packing"].is_null(), "and no case pack is on file: {before}");

    // The writer still refuses it until somebody says there is one (D23).
    let (status, refused) = weigh_carton(item).await;
    assert_eq!(status, 400, "{refused}");

    // ── saying what it holds makes it ────────────────────────────────────
    let first = Uuid::new_v4();
    let (status, said) = say(item, json!(16), first).await;
    assert_eq!(status, 200, "{said}");
    assert_eq!(said["changed"], true);
    assert_eq!(said["units_per_inner"], 1);
    assert_eq!(said["inners_per_carton"], 16, "a carton of 16 of the item: {said}");
    let config = said["item_packing_config_id"].clone();

    // A retried press is the same act, and saying it again is no act at all.
    let (_, again) = say(item, json!(16), first).await;
    assert_eq!(again["item_packing_config_id"], config);
    assert_eq!(again["changed"], false);
    let (_, same) = say(item, json!(16), Uuid::new_v4()).await;
    assert_eq!(same["item_packing_config_id"], config);
    assert_eq!(same["changed"], false, "{same}");

    let (status, weighed) = weigh_carton(item).await;
    assert_eq!(status, 200, "the carton is a definite thing to weigh now: {weighed}");
    let after = page(item).await;
    assert_eq!(after["packing"]["inners_per_carton"], 16, "{after}");

    // ── a different count is a different carton, from that day (D23) ─────
    let (status, twelve) = say(item, json!(12), Uuid::new_v4()).await;
    assert_eq!(status, 200, "{twelve}");
    assert_eq!(twelve["changed"], true);
    assert_ne!(twelve["item_packing_config_id"], config, "a new version, not a rewrite");
    assert_eq!(page(item).await["packing"]["inners_per_carton"], 12);

    // ── nonsense is refused ──────────────────────────────────────────────
    let (status, _) = say(item, json!(0), Uuid::new_v4()).await;
    assert_eq!(status, 400, "a carton holds at least one");
    let (status, _) = say(Uuid::nil(), json!(4), Uuid::new_v4()).await;
    assert_eq!(status, 404, "an item nobody holds");

    // ── a carton the loader knew of, count unsaid, is filled in ──────────
    let listed = made(format!("CTN-{n}-L")).await;
    let loaded: Uuid = db
        .query_one(
            "INSERT INTO item_packing_config (tenant_id, item_id, effective_from)
             VALUES (current_tenant(), $1, '2026-09-30') RETURNING id",
            &[&listed],
        )
        .await
        .expect("a loaded carton, count unsaid")
        .get(0);
    let (status, filled) = say(listed, json!(6), Uuid::new_v4()).await;
    assert_eq!(status, 200, "{filled}");
    assert_eq!(filled["item_packing_config_id"], loaded.to_string(), "the same carton, now described");
    assert_eq!(filled["inners_per_carton"], 6);
    let who: Option<Uuid> = db
        .query_one("SELECT recorded_by_id FROM item_packing_config WHERE id = $1", &[&loaded])
        .await
        .expect("the row")
        .get(0);
    assert!(who.is_some(), "and who said it travels on the row");

    // ── an empty count makes a carton and says nothing of what is in it ──
    let unsaid = made(format!("CTN-{n}-U")).await;
    let (status, bare) = say(unsaid, Value::Null, Uuid::new_v4()).await;
    assert_eq!(status, 200, "{bare}");
    assert_eq!(bare["changed"], true);
    assert!(bare["inners_per_carton"].is_null(), "{bare}");
    let (status, weighed) = weigh_carton(unsaid).await;
    assert_eq!(status, 200, "it can be weighed all the same: {weighed}");
}

#[actix_web::test]
async fn a_list_is_kept_in_its_order_and_narrows_the_item_list() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);
    let get = |uri: String| {
        let auth = auth.clone();
        let app = &app;
        async move {
            let r = test::call_service(app, test::TestRequest::get().uri(&uri).insert_header(auth).to_request()).await;
            let status = r.status().as_u16();
            let body: Value = serde_json::from_slice(&test::read_body(r).await).unwrap_or(Value::Null);
            (status, body)
        }
    };
    let make = |body: Value| {
        let auth = auth.clone();
        let app = &app;
        async move {
            let r = test::call_service(
                app,
                test::TestRequest::post().uri("/item-lists").insert_header(auth).set_json(body).to_request(),
            )
            .await;
            let status = r.status().as_u16();
            let text = String::from_utf8_lossy(&test::read_body(r).await).to_string();
            (status, serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text)))
        }
    };
    let name = format!("Sheet {}", nonce());
    let event = Uuid::new_v4();

    // ── a code nobody knows refuses the list, and says which ─────────────
    let (status, refused) = make(json!({
        "name": name, "codes": ["STY-7720-08", "NO-SUCH-CODE"],
        "client_event_id": Uuid::new_v4(), "occurred_at": "2026-10-02T00:00:00Z",
    }))
    .await;
    assert_eq!(status, 400, "{refused}");
    assert!(refused["detail"].as_str().unwrap_or_default().contains("NO-SUCH-CODE"), "{refused}");

    // ── in the order given, each once, a code's case forgiven ────────────
    let (status, made) = make(json!({
        "name": name, "codes": ["STY-7720-08", " glove-m ", "STY-7720-08", ""],
        "client_event_id": event, "occurred_at": "2026-10-02T00:00:00Z",
    }))
    .await;
    assert_eq!(status, 200, "{made}");
    assert_eq!(made["items"], 2, "{made}");
    let id = made["item_list_id"].as_str().expect("its id").to_string();
    let (_, replayed) = make(json!({
        "name": name, "codes": ["STY-7720-08", "GLOVE-M"],
        "client_event_id": event, "occurred_at": "2026-10-02T00:00:00Z",
    }))
    .await;
    assert_eq!(replayed["item_list_id"], id.as_str(), "a retried press is the same list");

    let (status, lists) = get("/item-lists".into()).await;
    assert_eq!(status, 200, "{lists}");
    let ours = lists.as_array().unwrap().iter().find(|l| l["item_list_id"] == id.as_str()).expect("listed");
    assert_eq!(ours["name"], name.as_str());
    assert_eq!(ours["items"], 2);
    assert!(ours["recorded_by_name"].is_string(), "{ours}");

    // ── the item list, narrowed to it, in its order ──────────────────────
    let (status, page) = get(format!("/items?list={id}&order=list")).await;
    assert_eq!(status, 200, "{page}");
    let rows: Vec<(String, i64)> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| (i["code"].as_str().unwrap().to_string(), i["list_position"].as_i64().unwrap()))
        .collect();
    assert_eq!(rows, [("STY-7720-08".to_string(), 1), ("GLOVE-M".to_string(), 2)], "{page}");
    assert_eq!(page["total"], 2);
    let (_, coded) = get(format!("/items?list={id}")).await;
    assert_eq!(coded["total"], 2, "any order narrows the same");

    let (status, _) = get("/items?order=list".into()).await;
    assert_eq!(status, 400, "the order of a list needs a list");
    let (_, plain) = get("/items?q=GLOVE-M".into()).await;
    assert!(plain["items"][0]["list_position"].is_null(), "no list, no place on one: {plain}");
}

/// A run of an item that looks different is a subject of its own (D182).
#[actix_web::test]
async fn a_run_that_looks_different_is_measured_on_its_own() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let db = state.pool.get().await.expect("a connection");
    db.execute("SELECT set_config('spork.tenant_id', $1, false)", &[&TENANT]).await.expect("the tenant scope");
    let item: Uuid = db
        .query_one(
            "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
             SELECT current_tenant(), $1, 'Bin liners, two printings', u.id, 'none'
               FROM unit u WHERE u.code = 'ea' RETURNING id",
            &[&format!("RUN-{}", nonce())],
        )
        .await
        .expect("the item")
        .get(0);
    let app = test::init_service(App::new().app_data(state.clone()).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);
    let post = |uri: String, body: Value| {
        let auth = auth.clone();
        let app = &app;
        async move {
            let r = test::call_service(app, test::TestRequest::post().uri(&uri).insert_header(auth).set_json(body).to_request()).await;
            let status = r.status().as_u16();
            (status, serde_json::from_slice::<Value>(&test::read_body(r).await).unwrap_or(Value::Null))
        }
    };

    let (status, run) = post(format!("/items/{item}/lots"), json!({ "code": "O/N 66081, made in India" })).await;
    assert_eq!(status, 200, "{run}");
    assert_eq!(run["added"], true);
    let (_, again) = post(format!("/items/{item}/lots"), json!({ "code": " O/N 66081, made in India " })).await;
    assert_eq!(again["lot_id"], run["lot_id"], "the same name is the same run");
    assert_eq!(again["added"], false);
    let (status, _) = post(format!("/items/{item}/lots"), json!({ "code": "  " })).await;
    assert_eq!(status, 400, "a run needs a name");

    let (status, weighed) = post(
        "/observations".into(),
        json!({
            "lot_id": run["lot_id"],
            "measurements": [{ "metric": "gross_weight", "entered_value": "4.1", "unit": "kg" }],
            "method": "instrument", "ingestion_channel": "keyed",
            "client_event_id": Uuid::new_v4(), "occurred_at": "2026-10-02T00:00:00Z",
        }),
    )
    .await;
    assert_eq!(status, 200, "a run is weighed on its own: {weighed}");

    let page: Value = common::ok_json(
        &app,
        test::TestRequest::get().uri(&format!("/items/{item}")).insert_header(auth.clone()).to_request(),
        "the item's page",
    )
    .await;
    let subject = page["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["lot_id"] == run["lot_id"])
        .unwrap_or_else(|| panic!("the run is a subject of the item: {page}"));
    assert_eq!(subject["lot_code"], "O/N 66081, made in India");
    assert!(subject["packaging_level"].is_null(), "a run has no level: {subject}");
    assert!(subject["wants"].as_array().unwrap().iter().any(|w| w == "photographs"), "{subject}");

    // ── the variant that is its carton (D184) ───────────────────────────
    let r = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/items/{item}/default-lot"))
            .insert_header(auth.clone())
            .set_json(json!({ "lot_id": run["lot_id"] }))
            .to_request(),
    )
    .await;
    assert_eq!(r.status().as_u16(), 204);
    let (status, _) = post(format!("/items/{item}/default-lot"), json!({ "lot_id": Uuid::new_v4() })).await;
    assert_eq!(status, 400, "only a variant of this item");
    // The rebuild that fills observation_current runs in the scheduler; the
    // carton reads whatever the variant shows, so ask for the page again.
    let page: Value = common::ok_json(
        &app,
        test::TestRequest::get().uri(&format!("/items/{item}")).insert_header(auth.clone()).to_request(),
        "the item's page",
    )
    .await;
    let carton = page["subjects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["packaging_level"] == "carton")
        .unwrap_or_else(|| panic!("its carton: {page}"));
    assert_eq!(carton["variant_lot_id"], run["lot_id"], "{carton}");
    assert_eq!(carton["variant_code"], "O/N 66081, made in India");
}
