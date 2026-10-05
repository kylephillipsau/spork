//! An item not where NetSuite lists it, said on the floor (D215), over HTTP.
//!
//! NetSuite's balance lists the item in one bin. Said not to be there, and
//! found in another, each is a finding with both sides on it; said again it
//! is the same finding; and the item's page shows what is open. It removes
//! what it wrote, because other files count the fixture's findings.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url, SITE};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

#[actix_web::test]
async fn not_where_netsuite_lists_it_is_a_finding_said_once() {
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
    let site = Uuid::parse_str(SITE).unwrap();
    let n = Uuid::new_v4().simple().to_string()[..8].to_uppercase();
    let item: Uuid = db
        .query_one(
            "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
             SELECT current_tenant(), $1, 'Mop bucket wringer', u.id, 'none' FROM unit u WHERE u.code = 'ea'
             RETURNING id",
            &[&format!("WRING-{n}")],
        )
        .await
        .expect("the item")
        .get(0);
    let bin = |code: String| {
        let db = &db;
        async move {
            db.query_one(
                "INSERT INTO location (tenant_id, site_id, code, kind) VALUES (current_tenant(), $1, $2, 'pick_face')
                 RETURNING id",
                &[&site, &code],
            )
            .await
            .expect("a bin")
            .get::<_, Uuid>(0)
        }
    };
    let (listed_code, other_code) = (format!("L{n}-01"), format!("L{n}-02"));
    let listed = bin(listed_code.clone()).await;
    let other = bin(other_code.clone()).await;
    // Emptied: an older feed's row still says 7, NetSuite's newest says none.
    let emptied = bin(format!("L{n}-03")).await;
    db.execute(
        "INSERT INTO reported_stock (tenant_id, site_id, item_id, location_id, on_hand, as_at, source)
         VALUES (current_tenant(), $1, $2, $3, 7, now() - interval '2 days', 'netsuite-inventory-balance-manual'),
                (current_tenant(), $1, $2, $3, 0, now() - interval '3 minutes', 'netsuite-inventory-balance')",
        &[&site, &item, &emptied],
    )
    .await
    .expect("an emptied bin's balance");
    db.execute(
        "INSERT INTO reported_stock (tenant_id, site_id, item_id, location_id, on_hand, available, as_at, source)
         VALUES (current_tenant(), $1, $2, $3, 12, 12, now() - interval '3 minutes', 'netsuite-inventory-balance')",
        &[&site, &item, &listed],
    )
    .await
    .expect("NetSuite's balance");

    let app = test::init_service(App::new().app_data(state.clone()).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);
    let call = |req: test::TestRequest| {
        let auth = auth.clone();
        let app = &app;
        async move {
            let r = test::call_service(app, req.insert_header(auth).to_request()).await;
            let status = r.status().as_u16();
            let body = test::read_body(r).await;
            (status, serde_json::from_slice::<Value>(&body).unwrap_or_else(|_| json!(String::from_utf8_lossy(&body))))
        }
    };
    let flag = |body: Value| call(test::TestRequest::post().uri(&format!("/items/{item}/bin-flags")).set_json(body));
    let now = "2026-10-05T01:00:00Z";
    let finding = |id: &str| {
        let db = &db;
        let id = Uuid::parse_str(id).unwrap();
        async move {
            db.query_one(
                "SELECT kind::text, holder_location_id, expected_quantity::text, observed_quantity::text, detail,
                        detected_by_id IS NOT NULL, state::text
                   FROM discrepancy WHERE id = $1",
                &[&id],
            )
            .await
            .expect("the finding")
        }
    };

    // ── not in the bin NetSuite lists ───────────────────────────────────
    let act = Uuid::new_v4();
    let not_here = json!({ "said": "not_here", "location_id": listed, "client_event_id": act, "occurred_at": now });
    let (status, said) = flag(not_here.clone()).await;
    assert_eq!(status, 200, "{said}");
    assert_eq!(said["already"], false);
    assert_eq!(said["bin_code"], listed_code.as_str());
    let missing = said["discrepancy_id"].as_str().unwrap().to_string();
    let f = finding(&missing).await;
    assert_eq!(f.get::<_, String>(0), "not_in_listed_bin");
    assert_eq!(f.get::<_, Uuid>(1), listed);
    assert_eq!((f.get::<_, String>(2), f.get::<_, Option<String>>(3)), ("12".into(), Some("0".into())), "NetSuite's 12 against none");
    let detail: String = f.get(4);
    assert!(detail.contains("lists 12") && detail.contains("Put it right in NetSuite"), "{detail}");
    assert!(f.get::<_, bool>(5), "whoever said it is on it");

    let (_, again) = flag(not_here).await;
    assert_eq!((again["discrepancy_id"].as_str(), again["already"].as_bool()), (Some(missing.as_str()), Some(true)), "a retry is the same act");
    let (_, twice) = flag(json!({ "said": "not_here", "location_id": listed, "client_event_id": Uuid::new_v4(), "occurred_at": now })).await;
    assert_eq!((twice["discrepancy_id"].as_str(), twice["already"].as_bool()), (Some(missing.as_str()), Some(true)), "said again while open, it is the finding there");

    // ── found in a bin NetSuite doesn't list, by its code as typed ──────
    let (status, said) = flag(json!({ "said": "found_here", "bin_code": other_code.to_lowercase(), "quantity": 5,
                                      "note": "on the top shelf", "client_event_id": Uuid::new_v4(), "occurred_at": now }))
    .await;
    assert_eq!(status, 200, "{said}");
    let found = said["discrepancy_id"].as_str().unwrap().to_string();
    let f = finding(&found).await;
    assert_eq!(f.get::<_, String>(0), "found_in_unlisted_bin");
    assert_eq!(f.get::<_, Uuid>(1), other);
    assert_eq!((f.get::<_, String>(2), f.get::<_, Option<String>>(3)), ("0".into(), Some("5".into())));
    let detail: String = f.get(4);
    assert!(detail.contains(&format!("also lists it in {listed_code} (12)")), "the fixer is told where NetSuite has it: {detail}");
    assert!(detail.contains("on the top shelf"), "{detail}");

    // ── refused, in words ───────────────────────────────────────────────
    let refused = |body: Value, why: &'static str| {
        let flag = &flag;
        async move {
            let (status, said) = flag(body).await;
            assert_eq!(status, 400, "{why}: {said}");
        }
    };
    refused(json!({ "said": "not_here", "location_id": other, "client_event_id": Uuid::new_v4(), "occurred_at": now }),
            "NetSuite doesn't list it there").await;
    refused(json!({ "said": "not_here", "location_id": emptied, "client_event_id": Uuid::new_v4(), "occurred_at": now }),
            "NetSuite's newest balance lists none there, whatever an older feed said").await;
    refused(json!({ "said": "found_here", "location_id": listed, "client_event_id": Uuid::new_v4(), "occurred_at": now }),
            "NetSuite already lists it there").await;
    refused(json!({ "said": "found_here", "bin_code": "NO-SUCH-BIN", "client_event_id": Uuid::new_v4(), "occurred_at": now }),
            "no such bin").await;
    refused(json!({ "said": "found_here", "location_id": other, "quantity": 0, "client_event_id": Uuid::new_v4(), "occurred_at": now }),
            "a count of nothing").await;
    refused(json!({ "said": "moved", "location_id": other, "client_event_id": Uuid::new_v4(), "occurred_at": now }),
            "not a thing that can be said").await;

    // ── the item's page shows what is open ──────────────────────────────
    let page = || call(test::TestRequest::get().uri(&format!("/items/{item}")));
    let (status, view) = page().await;
    assert_eq!(status, 200, "{view}");
    let flags = view["flags"].as_array().unwrap();
    assert_eq!(flags.len(), 2, "{view}");
    let by_kind = |k: &str| flags.iter().find(|f| f["kind"] == k).unwrap_or_else(|| panic!("{k}: {view}")).clone();
    assert_eq!(by_kind("not_in_listed_bin")["bin_code"], listed_code.as_str());
    assert_eq!(by_kind("found_in_unlisted_bin")["found"], "5");
    assert!(by_kind("found_in_unlisted_bin")["detected_by"].is_string());

    // Put right in NetSuite and accepted: no longer open, no longer shown.
    let (status, accepted) = call(
        test::TestRequest::post()
            .uri(&format!("/discrepancies/{missing}/accept"))
            .set_json(json!({ "reason": "Moved the balance to the right bin in NetSuite" })),
    )
    .await;
    assert_eq!(status, 200, "{accepted}");
    let (_, view) = page().await;
    let flags = view["flags"].as_array().unwrap();
    assert_eq!(flags.len(), 1, "{view}");
    assert_eq!(flags[0]["kind"], "found_in_unlisted_bin");

    // ── what it wrote, removed ──────────────────────────────────────────
    // Other files read the fixture's findings: one counts its closed ones.
    for sql in [
        "DELETE FROM discrepancy WHERE item_id = $1",
        "DELETE FROM reported_stock WHERE item_id = $1",
    ] {
        db.execute(sql, &[&item]).await.expect(sql);
    }
    db.execute("DELETE FROM location WHERE id = ANY($1)", &[&vec![listed, other, emptied]]).await.expect("the bins");
    db.execute("DELETE FROM item WHERE id = $1", &[&item]).await.expect("the item");
}
