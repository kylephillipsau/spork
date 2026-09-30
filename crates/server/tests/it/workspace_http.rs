//! A site saying where it packs and whose goods it holds, over HTTP.
//!
//! On a site of this file's own, because the fixture's Melbourne is where every
//! bench test packs, and moving its pack location under them would be a test
//! breaking its neighbours.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

async fn call<S>(app: &S, req: test::TestRequest) -> (u16, Value)
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let r = test::call_service(app, req.to_request()).await;
    let status = r.status().as_u16();
    let bytes = test::read_body(r).await;
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

#[actix_web::test]
async fn a_site_says_where_it_packs_and_whose_goods_it_holds() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let db = common::connect(&u, false).await;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        % 1_000_000;
    let code = format!("W{nonce}");
    let site: Uuid = db
        .query_one(
            "INSERT INTO site (tenant_id, code, name, timezone)
             VALUES ($1::text::uuid, $2, $3, 'Australia/Melbourne') RETURNING id",
            &[&TENANT, &code, &format!("Workspace test {nonce}")],
        )
        .await
        .expect("a site of this test's own")
        .get(0);

    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);

    let find = |ws: &Value| -> Value {
        ws["sites"]
            .as_array()
            .expect("sites")
            .iter()
            .find(|s| s["code"] == code.as_str())
            .cloned()
            .expect("this test's site is listed")
    };

    // ── nothing said yet ────────────────────────────────────────────────
    let (status, ws) = call(&app, test::TestRequest::get().uri("/workspace").insert_header(auth.clone())).await;
    assert_eq!(status, 200, "{ws}");
    assert!(find(&ws)["pack_location"].is_null());
    assert!(find(&ws)["owner"].is_null());

    // ── a code the site does not have is made, then found ───────────────
    let pack = |c: &str| {
        test::TestRequest::post()
            .uri(&format!("/workspace/sites/{site}/pack-location"))
            .insert_header(auth.clone())
            .set_json(json!({ "code": c }))
    };
    let (status, set) = call(&app, pack(" PACK ")).await;
    assert_eq!(status, 200, "{set}");
    assert_eq!(set["code"], "PACK", "trimmed");
    assert_eq!(set["created"], true, "the bench is not in a bin list, so it is made");
    let (_, again) = call(&app, pack("PACK")).await;
    assert_eq!(again["created"], false, "the second time it is found, not made twice");
    let kind: String = db
        .query_one("SELECT kind FROM location WHERE site_id = $1 AND code = 'PACK'", &[&site])
        .await
        .expect("the location")
        .get(0);
    assert_eq!(kind, "staging", "where goods wait on their way out");

    let (status, empty) = call(&app, pack("  ")).await;
    assert_eq!(status, 400, "a blank name names nothing: {empty}");

    // ── the organisation owns what it holds ─────────────────────────────
    let owner = |o: &str| {
        test::TestRequest::post()
            .uri(&format!("/workspace/sites/{site}/owner"))
            .insert_header(auth.clone())
            .set_json(json!({ "owner": o }))
    };
    let (status, set) = call(&app, owner("business")).await;
    assert_eq!(status, 200, "{set}");
    assert_eq!(set["owner"], "Alpha Foods", "the organisation's own party, by its slug");
    assert_eq!(set["created"], false, "the fixture already has it");
    let (status, refused) = call(&app, owner("a customer")).await;
    assert_eq!(status, 400, "not an answer taken yet: {refused}");

    let (_, ws) = call(&app, test::TestRequest::get().uri("/workspace").insert_header(auth.clone())).await;
    assert_eq!(find(&ws)["pack_location"], "PACK");
    assert_eq!(find(&ws)["owner"], "Alpha Foods");

    // ── a site nobody holds ─────────────────────────────────────────────
    let (status, _) = call(
        &app,
        test::TestRequest::post()
            .uri(&format!("/workspace/sites/{}/pack-location", Uuid::nil()))
            .insert_header(auth.clone())
            .set_json(json!({ "code": "PACK" })),
    )
    .await;
    assert_eq!(status, 404);

    // Tidy, so the site count other suites read stays what they expect.
    db.batch_execute(&format!(
        "UPDATE site SET pack_location_id = NULL WHERE id = '{site}';
         DELETE FROM location WHERE site_id = '{site}';
         DELETE FROM site WHERE id = '{site}';"
    ))
    .await
    .expect("tidy");
}
