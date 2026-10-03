//! A workspace's backup (D193), taken over HTTP and restored, and who may
//! take one (D192).
//!
//! The restore is proved on the fixture workspace itself, inside a
//! transaction that is rolled back: its rows are deleted, the backup is put
//! back, and every table must read exactly as it did before.
//!
//! **Into a Spork migrated apart** (D194). Restoring into the database the
//! backup came from could never notice that a migration draws the platform's
//! metric ids as it runs: both sides had the same ones. So before the backup
//! goes back, this database is given other ids for those rows, as a server
//! migrated on its own would have, and the restored measurements must name
//! this server's metrics.

use actix_web::{test, web, App};
use serde_json::Value;
use spork_server::{backup, images, routes, AppState};

use super::common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const KYLE: &str = "77770000-0000-0000-0000-000000000001";
const PASSWORD: &str = "correct horse battery";

/// Whether `table` has a `tenant_id`: one without is the catalogue, kept whole.
async fn has_tenant(c: &tokio_postgres::Transaction<'_>, table: &str) -> bool {
    c.query_one(
        "SELECT EXISTS (SELECT 1 FROM pg_attribute
                         WHERE attrelid = $1::text::regclass AND attname = 'tenant_id' AND NOT attisdropped)",
        &[&format!("public.\"{table}\"")],
    )
    .await
    .unwrap()
    .get(0)
}

/// Each table's rows of the workspace, as one digest per table.
async fn fingerprint(c: &tokio_postgres::Transaction<'_>, manifest: &backup::Manifest) -> Vec<(String, String)> {
    let tenant = uuid::Uuid::parse_str(TENANT).unwrap();
    let mut out = vec![];
    for t in &manifest.tables {
        let filter = match t.table.as_str() {
            "tenant" => "id = $1",
            "person" => "id IN (SELECT person_id FROM person_tenant WHERE tenant_id = $1)",
            "person_credential" | "person_passkey" => "person_id IN (SELECT person_id FROM person_tenant WHERE tenant_id = $1)",
            _ if !has_tenant(c, &t.table).await => "true",
            _ if t.shared => "tenant_id = $1 OR tenant_id IS NULL",
            _ => "tenant_id = $1",
        };
        let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = if filter.contains("$1") { vec![&tenant] } else { vec![] };
        let digest: String = c
            .query_one(
                &format!(
                    "SELECT md5(coalesce(string_agg(j, E'\\n' ORDER BY j), ''))
                       FROM (SELECT row_to_json(t)::text AS j FROM \"{}\" t WHERE {filter}) x",
                    t.table
                ),
                &params,
            )
            .await
            .unwrap_or_else(|e| panic!("{}: {e}", t.table))
            .get(0);
        out.push((t.table.clone(), digest));
    }
    out
}

/// The platform's rows a migration seeds with ids it draws as it runs: two
/// servers migrated apart have the same rows under different ids.
const DRAWN: &[&str] = &["metric", "presentation", "adjustment_reason", "source_channel"];

/// Give each platform row of `DRAWN` a new id and point every row that names
/// it at the new one, making this database one migrated apart. Called again,
/// it puts the ids back.
async fn redraw(c: &tokio_postgres::Transaction<'_>) {
    c.batch_execute("CREATE TEMP TABLE IF NOT EXISTS redrawn (tab regclass, old uuid, new uuid) ON COMMIT DROP")
        .await
        .unwrap();
    let first: bool = c.query_one("SELECT count(*) = 0 FROM redrawn", &[]).await.unwrap().get(0);
    if first {
        for t in DRAWN {
            let platform = if has_tenant(c, t).await { "WHERE tenant_id IS NULL" } else { "" };
            c.execute(&format!("INSERT INTO redrawn SELECT '{t}'::regclass, id, gen_random_uuid() FROM {t} {platform}"), &[])
                .await
                .unwrap();
        }
    } else {
        c.batch_execute("UPDATE redrawn SET old = new, new = old").await.unwrap();
    }
    for t in DRAWN {
        c.execute(&format!("UPDATE {t} x SET id = r.new FROM redrawn r WHERE r.tab = '{t}'::regclass AND x.id = r.old"), &[])
            .await
            .unwrap();
        let naming = c
            .query(
                "SELECT DISTINCT k.conrelid::regclass::text, quote_ident(a.attname)::text
                   FROM pg_constraint k
                   CROSS JOIN LATERAL unnest(k.conkey, k.confkey) AS u(c, p)
                   JOIN pg_attribute a ON a.attrelid = k.conrelid AND a.attnum = u.c
                   JOIN pg_attribute pa ON pa.attrelid = k.confrelid AND pa.attnum = u.p
                  WHERE k.contype = 'f' AND k.confrelid = $1::text::regclass AND pa.attname = 'id'",
                &[&t],
            )
            .await
            .unwrap();
        for n in &naming {
            let (child, column): (String, String) = (n.get(0), n.get(1));
            c.execute(
                &format!("UPDATE {child} x SET {column} = r.new FROM redrawn r WHERE r.tab = '{t}'::regclass AND x.{column} = r.old"),
                &[],
            )
            .await
            .unwrap_or_else(|e| panic!("{child}.{column}: {e}"));
        }
    }
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
    // The newest migration this database has, whichever that is: the backup
    // is only restorable at it.
    let newest: String = state
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT max(name) FROM schema_migration", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(summary["schema"], newest.as_str(), "{summary}");
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
    assert!(rows("presentation").unwrap() > 0, "the catalogue its rows refer to is kept: {:?}", manifest.tables);

    // ── put back exactly, then rolled back ──────────────────────────────
    let mut conn = state.pool.get().await.unwrap();
    conn.batch_execute("RESET ROLE").await.unwrap();
    let mut tx = conn.transaction().await.unwrap();
    let before = fingerprint(&tx, &manifest).await;
    let refused = backup::restore(&tx, &file, PASSWORD, &images::directory()).await;
    assert!(refused.is_err(), "a database that has the workspace is not restored into");

    tx.batch_execute("SET LOCAL session_replication_role = replica").await.unwrap();
    for t in manifest.tables.iter().filter(|t| t.table != "tenant" && t.table != "person" && !t.table.starts_with("person_")) {
        if !has_tenant(&tx, &t.table).await {
            continue; // the catalogue is the server's, not the workspace's
        }
        tx.execute(&format!("DELETE FROM \"{}\" WHERE tenant_id = $1", t.table), &[&manifest.tenant_id])
            .await
            .unwrap_or_else(|e| panic!("{}: {e}", t.table));
    }
    tx.execute("DELETE FROM person_tenant WHERE tenant_id = $1", &[&manifest.tenant_id]).await.unwrap();
    tx.execute("DELETE FROM tenant WHERE id = $1", &[&manifest.tenant_id]).await.unwrap();

    // ── a row here that points at nothing refuses it, and leaves nothing ─
    {
        let sp = tx.transaction().await.unwrap();
        sp.execute("UPDATE site SET tenant_id = $1 WHERE tenant_id <> $2", &[&uuid::Uuid::now_v7(), &manifest.tenant_id])
            .await
            .unwrap();
        let refused = backup::restore(&sp, &file, PASSWORD, &images::directory()).await;
        let message = format!("{:?}", refused.as_ref().err());
        assert!(message.contains("site (tenant_id) name a tenant"), "{message}");
        sp.rollback().await.unwrap();
    }
    let none: bool = tx.query_one("SELECT NOT EXISTS (SELECT 1 FROM tenant WHERE id = $1)", &[&manifest.tenant_id]).await.unwrap().get(0);
    assert!(none, "a refused restore leaves nothing behind");

    // ── into a Spork migrated apart, matched by key (D194) ──────────────
    redraw(&tx).await;
    let restored = backup::restore(&tx, &file, PASSWORD, &images::directory()).await.expect("the restore");
    assert!(restored.rows > 0);
    let measured: i64 = tx
        .query_one("SELECT count(*) FROM observation WHERE tenant_id = $1", &[&manifest.tenant_id])
        .await
        .unwrap()
        .get(0);
    assert!(measured > 0, "the fixture has measurements to point");
    let unnamed: i64 = tx
        .query_one(
            "SELECT (SELECT count(*) FROM observation o WHERE o.tenant_id = $1
                       AND NOT EXISTS (SELECT 1 FROM metric m WHERE m.id = o.metric_id))
                  + (SELECT count(*) FROM observation_current o WHERE o.tenant_id = $1
                       AND NOT EXISTS (SELECT 1 FROM metric m WHERE m.id = o.metric_id))
                  + (SELECT count(*) FROM stock_movement s WHERE s.tenant_id = $1 AND s.adjustment_reason_id IS NOT NULL
                       AND NOT EXISTS (SELECT 1 FROM adjustment_reason r WHERE r.id = s.adjustment_reason_id))",
            &[&manifest.tenant_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(unnamed, 0, "every restored measurement and movement names this server's metric and reason");
    redraw(&tx).await;
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
