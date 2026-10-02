//! A workspace's backup (D193), taken over HTTP and restored, and who may
//! take one (D192).
//!
//! The restore is proved on the fixture workspace itself, inside a
//! transaction that is rolled back: its rows are deleted, the backup is put
//! back, and every table must read exactly as it did before.

use actix_web::{test, web, App};
use serde_json::Value;
use spork_server::{backup, images, routes, AppState};

use super::common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const KYLE: &str = "77770000-0000-0000-0000-000000000001";
const PASSWORD: &str = "correct horse battery";

/// Each table's rows of the workspace, as one digest per table.
async fn fingerprint(c: &tokio_postgres::Transaction<'_>, manifest: &backup::Manifest) -> Vec<(String, String)> {
    let mut out = vec![];
    for t in &manifest.tables {
        let filter = match t.table.as_str() {
            "tenant" => "id = $1",
            "person" => "id IN (SELECT person_id FROM person_tenant WHERE tenant_id = $1)",
            "person_credential" | "person_passkey" => "person_id IN (SELECT person_id FROM person_tenant WHERE tenant_id = $1)",
            _ if t.shared => "tenant_id = $1 OR tenant_id IS NULL",
            _ => "tenant_id = $1",
        };
        let digest: String = c
            .query_one(
                &format!(
                    "SELECT md5(coalesce(string_agg(j, E'\\n' ORDER BY j), ''))
                       FROM (SELECT row_to_json(t)::text AS j FROM \"{}\" t WHERE {filter}) x",
                    t.table
                ),
                &[&uuid::Uuid::parse_str(TENANT).unwrap()],
            )
            .await
            .unwrap_or_else(|e| panic!("{}: {e}", t.table))
            .get(0);
        out.push((t.table.clone(), digest));
    }
    out
}

#[actix_web::test]
async fn a_backup_is_an_administrators_and_restores_exactly() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state.clone()).configure(routes::configure)).await;
    let auth = ("authorization", common::bearer(&app).await);

    // ── what it would hold ──────────────────────────────────────────────
    let summary: Value = common::ok_json(
        &app,
        test::TestRequest::get().uri("/backup").insert_header(auth.clone()).to_request(),
        "the backup's summary",
    )
    .await;
    assert!(summary["schema"].as_str().unwrap().contains("administers"), "{summary}");
    let session: Value = common::ok_json(
        &app,
        test::TestRequest::get().uri("/sessions/current").insert_header(auth.clone()).to_request(),
        "who am I",
    )
    .await;
    assert_eq!(session["administrator"], true, "{session}");

    // ── a short password is refused ─────────────────────────────────────
    let short = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/backup")
            .insert_header(auth.clone())
            .set_form([("password", "too short")])
            .to_request(),
    )
    .await;
    assert_eq!(short.status().as_u16(), 400);

    // ── downloaded, encrypted ───────────────────────────────────────────
    let r = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/backup")
            .insert_header(auth.clone())
            .set_form([("password", PASSWORD)])
            .to_request(),
    )
    .await;
    assert_eq!(r.status().as_u16(), 200);
    let disposition = r.headers().get("content-disposition").unwrap().to_str().unwrap().to_string();
    assert!(disposition.contains("attachment") && disposition.contains("spork-backup-"), "{disposition}");
    let bytes = test::read_body(r).await;
    let file = std::env::temp_dir().join(format!("spork-backup-test-{}.zip", uuid::Uuid::now_v7()));
    std::fs::write(&file, &bytes).unwrap();

    assert!(backup::manifest_of(&file, "not the password at all").is_err(), "the wrong password opens nothing");
    let manifest = backup::manifest_of(&file, PASSWORD).expect("the manifest");
    assert_eq!(manifest.tenant_id.to_string(), TENANT);
    let rows = |t: &str| manifest.tables.iter().find(|x| x.table == t).map(|x| x.rows);
    assert_eq!(rows("tenant"), Some(1));
    assert!(rows("person_credential").unwrap() >= 1, "sign-ins are kept");
    assert!(rows("item").unwrap() > 0 && rows("site").unwrap() > 0, "{:?}", manifest.tables);
    assert_eq!(rows("session"), None, "sessions are not");

    // ── put back exactly, then rolled back ──────────────────────────────
    let mut conn = state.pool.get().await.unwrap();
    conn.batch_execute("RESET ROLE").await.unwrap();
    let tx = conn.transaction().await.unwrap();
    let before = fingerprint(&tx, &manifest).await;
    let refused = backup::restore(&tx, &file, PASSWORD, &images::directory()).await;
    assert!(refused.is_err(), "a database that has the workspace is not restored into");

    tx.batch_execute("SET LOCAL session_replication_role = replica").await.unwrap();
    for t in manifest.tables.iter().filter(|t| t.table != "tenant" && t.table != "person" && !t.table.starts_with("person_")) {
        tx.execute(&format!("DELETE FROM \"{}\" WHERE tenant_id = $1", t.table), &[&manifest.tenant_id])
            .await
            .unwrap_or_else(|e| panic!("{}: {e}", t.table));
    }
    tx.execute("DELETE FROM person_tenant WHERE tenant_id = $1", &[&manifest.tenant_id]).await.unwrap();
    tx.execute("DELETE FROM tenant WHERE id = $1", &[&manifest.tenant_id]).await.unwrap();

    let restored = backup::restore(&tx, &file, PASSWORD, &images::directory()).await.expect("the restore");
    assert!(restored.rows > 0);
    let after = fingerprint(&tx, &manifest).await;
    for ((table, was), (_, now)) in before.iter().zip(&after) {
        assert_eq!(was, now, "{table} reads as it did before the backup");
    }
    tx.rollback().await.unwrap();
    std::fs::remove_file(&file).ok();

    // ── a member who is not an administrator ────────────────────────────
    let db = state.pool.get().await.unwrap();
    db.batch_execute("RESET ROLE").await.unwrap();
    db.execute("UPDATE person_tenant SET role = 'operator' WHERE person_id = $1::text::uuid AND tenant_id = $2::text::uuid", &[&KYLE, &TENANT])
        .await
        .unwrap();
    let denied = test::call_service(&app, test::TestRequest::get().uri("/backup").insert_header(auth.clone()).to_request()).await;
    db.execute("UPDATE person_tenant SET role = 'administrator' WHERE person_id = $1::text::uuid AND tenant_id = $2::text::uuid", &[&KYLE, &TENANT])
        .await
        .unwrap();
    assert_eq!(denied.status().as_u16(), 403, "only an administrator takes a backup");
}
