//! A workspace's backup: one encrypted zip of its rows and photographs. D193.
//!
//! **What is in it.** Every row of the workspace (each table with a
//! `tenant_id`, and the shared tables' rows it can see), the workspace itself,
//! the people who belong to it with their sign-ins, and every photograph its
//! rows name. Sessions are left out: a restore is signed into afresh.
//!
//! **How.** `manifest.json` says what it is: the format, the database's newest
//! migration, the workspace, and each table's row count. `data/<table>.jsonl`
//! holds a table's rows as Postgres writes them with `row_to_json`, one per
//! line, and `images/<digest>` each photograph's bytes. Every entry is
//! encrypted with AES-256 from a password the person sets when downloading.
//!
//! **A restore is into an empty Spork at the same migration**, by the
//! `spork-restore` command, as the database owner. Rows are only meaningful
//! at the schema they were written under, so a backup taken at migration 111
//! is restored at 111 and the database is migrated afterwards. Nothing on the
//! web writes a backup back: a restore replaces who can sign in.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use actix_files::NamedFile;
use actix_web::http::header::{ContentDisposition, DispositionParam, DispositionType};
use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_postgres::Transaction;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{AesMode, CompressionMethod, ZipArchive, ZipWriter};

use crate::error::ApiError;
use crate::images;
use crate::routes::{administrator, caller};
use crate::tenancy::{ensure_app_role, ensure_login_role, TenantScope};
use crate::AppState;

pub const FORMAT: &str = "spork-backup";
pub const FORMAT_VERSION: u32 = 1;

/// The shortest password a backup is encrypted with. The file holds every
/// row of the business and its sign-ins, and it travels on USB sticks.
pub const LEAST_PASSWORD: usize = 12;

/// Rows a restore inserts per statement.
const BATCH: usize = 500;

/// Tables with a `tenant_id` that are not kept: a session is a sign-in in
/// progress, and a restore is signed into afresh.
const LEFT_OUT: &[&str] = &["session"];

/// The workspace's rows of the tables without a `tenant_id`, as a `WHERE`
/// clause on `$1`, the workspace. Inserted on a restore unless already there:
/// a person can belong to two workspaces on one server.
const GLOBAL: &[(&str, &str)] = &[
    ("tenant", "id = $1"),
    ("person", "id IN (SELECT person_id FROM person_tenant WHERE tenant_id = $1)"),
    ("person_credential", "person_id IN (SELECT person_id FROM person_tenant WHERE tenant_id = $1)"),
    ("person_passkey", "person_id IN (SELECT person_id FROM person_tenant WHERE tenant_id = $1)"),
];

/// What a backup says about itself.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Manifest {
    pub format: String,
    pub format_version: u32,
    /// The newest migration of the database it was taken from.
    pub schema: String,
    pub tenant_id: Uuid,
    pub tenant_name: String,
    pub exported_at: DateTime<Utc>,
    pub tables: Vec<TableRows>,
    pub photos: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TableRows {
    pub table: String,
    pub rows: u64,
    /// Rows that may already be there on a restore: the shared catalogue's,
    /// and people, who can belong to more than one workspace.
    pub shared: bool,
}

enum Entry {
    Text(String, String),
    Photo(String),
}

/// The newest migration this database has.
async fn schema_of(c: &Transaction<'_>) -> Result<String, ApiError> {
    Ok(c.query_one("SELECT max(name) FROM schema_migration", &[]).await?.get(0))
}

/// Write the workspace's backup to `out`, read in one snapshot.
///
/// On a connection whose login role reads every table, because sign-ins are
/// read by no application role; every read names the workspace.
pub async fn export(
    conn: &mut tokio_postgres::Client,
    tenant: Uuid,
    password: &str,
    image_dir: &Path,
    out: &Path,
) -> Result<Manifest, ApiError> {
    let tx = conn
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await?;
    let schema = schema_of(&tx).await?;
    let tenant_name: String = tx
        .query_opt("SELECT name FROM tenant WHERE id = $1", &[&tenant])
        .await?
        .ok_or(ApiError::NotFound)?
        .get(0);

    let (send, receive) = mpsc::sync_channel::<Entry>(4);
    let writer = {
        let out = out.to_path_buf();
        let image_dir = image_dir.to_path_buf();
        let password = password.to_string();
        tokio::task::spawn_blocking(move || write_zip(&out, &image_dir, &password, receive))
    };
    let sent = |entry: Entry| send.send(entry).map_err(|_| ApiError::Rejected("the backup file could not be written".into()));

    let mut tables = vec![];
    for (table, filter) in GLOBAL {
        let rows = rows_of(&tx, table, filter, tenant).await?;
        tables.push(TableRows { table: table.to_string(), rows: rows.1, shared: *table != "tenant" });
        sent(Entry::Text(format!("data/{table}.jsonl"), rows.0))?;
    }
    let scoped = tx
        .query(
            "SELECT c.relname, NOT a.attnotnull
               FROM pg_class c
               JOIN pg_attribute a ON a.attrelid = c.oid AND a.attname = 'tenant_id' AND NOT a.attisdropped
              WHERE c.relnamespace = 'public'::regnamespace AND c.relkind IN ('r', 'p')
              ORDER BY c.relname",
            &[],
        )
        .await?;
    for r in &scoped {
        let table: String = r.get(0);
        if LEFT_OUT.contains(&table.as_str()) {
            continue;
        }
        let shared: bool = r.get(1);
        let filter = if shared { "tenant_id = $1 OR tenant_id IS NULL" } else { "tenant_id = $1" };
        let rows = rows_of(&tx, &table, filter, tenant).await?;
        sent(Entry::Text(format!("data/{table}.jsonl"), rows.0))?;
        tables.push(TableRows { table, rows: rows.1, shared });
    }

    let photos = tx
        .query(
            "SELECT digest FROM observation_image WHERE tenant_id = $1
             UNION SELECT digest FROM observation_image_cut WHERE tenant_id = $1
             UNION SELECT digest FROM box_picture WHERE tenant_id = $1",
            &[&tenant],
        )
        .await?;
    for p in &photos {
        sent(Entry::Photo(p.get(0)))?;
    }
    tx.commit().await?;

    let manifest = Manifest {
        format: FORMAT.into(),
        format_version: FORMAT_VERSION,
        schema,
        tenant_id: tenant,
        tenant_name,
        exported_at: Utc::now(),
        tables,
        photos: photos.len() as u64,
    };
    sent(Entry::Text("manifest.json".into(), serde_json::to_string_pretty(&manifest).expect("a manifest serialises")))?;
    drop(send);
    writer
        .await
        .map_err(|e| ApiError::Rejected(format!("the backup writer stopped: {e}")))??;
    Ok(manifest)
}

/// A table's rows as JSON lines, and how many.
async fn rows_of(tx: &Transaction<'_>, table: &str, filter: &str, tenant: Uuid) -> Result<(String, u64), ApiError> {
    let rows = tx
        .query(
            &format!("SELECT row_to_json(t)::text FROM {} t WHERE {filter}", quote(table)),
            &[&tenant],
        )
        .await?;
    let mut out = String::new();
    for r in &rows {
        out.push_str(r.get::<_, &str>(0));
        out.push('\n');
    }
    Ok((out, rows.len() as u64))
}

/// A table's name as an identifier: `order` is a keyword.
fn quote(table: &str) -> String {
    format!("\"{}\"", table.replace('"', "\"\""))
}

fn write_zip(out: &Path, image_dir: &Path, password: &str, entries: mpsc::Receiver<Entry>) -> Result<(), ApiError> {
    let failed = |e: &dyn std::fmt::Display| ApiError::Rejected(format!("the backup file could not be written: {e}"));
    let file = std::fs::File::create(out).map_err(|e| failed(&e))?;
    let mut zip = ZipWriter::new(std::io::BufWriter::new(file));
    let text = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .large_file(true)
        .with_aes_encryption(AesMode::Aes256, password);
    // A photograph is already compressed; deflating it again is time for nothing.
    let photo = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .with_aes_encryption(AesMode::Aes256, password);
    for entry in entries {
        match entry {
            Entry::Text(name, body) => {
                zip.start_file(name, text).map_err(|e| failed(&e))?;
                zip.write_all(body.as_bytes()).map_err(|e| failed(&e))?;
            }
            Entry::Photo(digest) => {
                if !images::is_digest(&digest) {
                    continue;
                }
                // A row whose file is gone is reported by the manifest's count
                // against the files, not by failing the whole backup.
                let Ok(bytes) = std::fs::read(images::path_for(image_dir, &digest)) else {
                    tracing::warn!(%digest, "a photograph named by a row is not in the image store");
                    continue;
                };
                zip.start_file(format!("images/{digest}"), photo).map_err(|e| failed(&e))?;
                zip.write_all(&bytes).map_err(|e| failed(&e))?;
            }
        }
    }
    zip.finish().map_err(|e| failed(&e))?.flush().map_err(|e| failed(&e))?;
    Ok(())
}

/// What a restore put back.
#[derive(Debug)]
pub struct Restored {
    pub manifest: Manifest,
    pub rows: u64,
    pub photos: u64,
}

fn opened(path: &Path) -> Result<ZipArchive<std::fs::File>, ApiError> {
    let file = std::fs::File::open(path).map_err(|e| ApiError::Rejected(format!("could not open the backup: {e}")))?;
    ZipArchive::new(file).map_err(|_| ApiError::Rejected("that is not a Spork backup".into()))
}

fn read_entry(zip: &mut ZipArchive<std::fs::File>, name: &str, password: &str) -> Result<Vec<u8>, ApiError> {
    let mut entry = zip.by_name_decrypt(name, password.as_bytes()).map_err(|e| match e {
        zip::result::ZipError::InvalidPassword => ApiError::Rejected("the password is wrong".into()),
        zip::result::ZipError::FileNotFound => ApiError::Rejected(format!("the backup has no {name}")),
        other => ApiError::Rejected(format!("the backup is damaged: {other}")),
    })?;
    let mut bytes = vec![];
    entry
        .read_to_end(&mut bytes)
        .map_err(|_| ApiError::Rejected("the password is wrong, or the backup is damaged".into()))?;
    Ok(bytes)
}

/// Read and check a backup's manifest.
pub fn manifest_of(path: &Path, password: &str) -> Result<Manifest, ApiError> {
    let mut zip = opened(path)?;
    let manifest: Manifest = serde_json::from_slice(&read_entry(&mut zip, "manifest.json", password)?)
        .map_err(|_| ApiError::Rejected("that is not a Spork backup".into()))?;
    if manifest.format != FORMAT || manifest.format_version != FORMAT_VERSION {
        return Err(ApiError::Rejected(format!(
            "this is a {} backup, version {}; this Spork restores {FORMAT} version {FORMAT_VERSION}",
            manifest.format, manifest.format_version
        )));
    }
    Ok(manifest)
}

/// Put a backup back, inside `tx`, which the caller commits.
///
/// Refused unless the database is at the backup's migration and has no
/// workspace of its id. Constraints and triggers are set aside for the
/// transaction (`session_replication_role`), because the rows were consistent
/// when read and their order is the database's, not a dependency order; that
/// takes the database owner.
pub async fn restore(tx: &Transaction<'_>, path: &Path, password: &str, image_dir: &Path) -> Result<Restored, ApiError> {
    let manifest = manifest_of(path, password)?;
    let here = schema_of(tx).await?;
    if here != manifest.schema {
        return Err(ApiError::Rejected(format!(
            "the backup was taken at migration {}; this database is at {here}. Restore it with the Spork it was taken with, then update",
            manifest.schema
        )));
    }
    if tx.query_opt("SELECT 1 FROM tenant WHERE id = $1", &[&manifest.tenant_id]).await?.is_some() {
        return Err(ApiError::Rejected(format!(
            "this database already has the workspace {}; a backup is restored into an empty Spork",
            manifest.tenant_name
        )));
    }
    tx.batch_execute("SET LOCAL session_replication_role = replica")
        .await
        .map_err(|_| ApiError::Rejected("a restore runs as the database owner".into()))?;

    let mut zip = opened(path)?;
    let mut rows = 0;
    for t in &manifest.tables {
        let exists: bool = tx
            .query_one("SELECT to_regclass($1) IS NOT NULL", &[&format!("public.{}", quote(&t.table))])
            .await?
            .get(0);
        if !exists {
            return Err(ApiError::Rejected(format!("this database has no table {}", t.table)));
        }
        let body = String::from_utf8(read_entry(&mut zip, &format!("data/{}.jsonl", t.table), password)?)
            .map_err(|_| ApiError::Rejected(format!("the rows of {} are damaged", t.table)))?;
        let lines: Vec<&str> = body.lines().filter(|l| !l.is_empty()).collect();
        if lines.len() as u64 != t.rows {
            return Err(ApiError::Rejected(format!("the backup's {} has {} rows, not {}", t.table, lines.len(), t.rows)));
        }
        if t.table == "person" {
            clashes(tx, &lines).await?;
        }
        let columns: String = tx
            .query_one(
                "SELECT string_agg(quote_ident(attname), ', ' ORDER BY attnum)
                   FROM pg_attribute
                  WHERE attrelid = $1::text::regclass AND attnum > 0 AND NOT attisdropped AND attgenerated = ''",
                &[&format!("public.{}", quote(&t.table))],
            )
            .await?
            .get(0);
        let insert = format!(
            "INSERT INTO {table} ({columns}) OVERRIDING SYSTEM VALUE
             SELECT {columns} FROM json_populate_recordset(NULL::{table}, $1::text::json) {conflict}",
            table = quote(&t.table),
            conflict = if t.shared { "ON CONFLICT DO NOTHING" } else { "" },
        );
        for batch in lines.chunks(BATCH) {
            let json = format!("[{}]", batch.join(","));
            rows += tx.execute(&insert, &[&json]).await?;
        }
    }

    let mut photos = 0;
    let names: Vec<String> = zip.file_names().filter(|n| n.starts_with("images/")).map(str::to_string).collect();
    for name in names {
        let digest = &name["images/".len()..];
        let bytes = read_entry(&mut zip, &name, password)?;
        if images::put(image_dir, &bytes).await? != digest {
            return Err(ApiError::Rejected(format!("the photograph {digest} is damaged")));
        }
        photos += 1;
    }
    Ok(Restored { manifest, rows, photos })
}

/// A person in the backup whose email belongs to somebody else here.
async fn clashes(tx: &Transaction<'_>, people: &[&str]) -> Result<(), ApiError> {
    let json = format!("[{}]", people.join(","));
    let taken: Option<String> = tx
        .query_opt(
            "SELECT b.email FROM json_populate_recordset(NULL::person, $1::text::json) b
               JOIN person p ON lower(p.email) = lower(b.email) AND p.id <> b.id
              LIMIT 1",
            &[&json],
        )
        .await?
        .map(|r| r.get(0));
    match taken {
        Some(email) => Err(ApiError::Rejected(format!("{email} already signs in to another person on this server"))),
        None => Ok(()),
    }
}

/// What a backup of this workspace would hold.
#[derive(Serialize, Debug)]
pub struct BackupSummary {
    pub items: i64,
    pub photos: i64,
    /// The photographs' size, in bytes, as recorded when each was kept.
    pub photo_bytes: i64,
    pub schema: String,
}

#[get("/backup")]
pub async fn backup_summary(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    administrator(&state, &who).await?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let r = tx
                    .query_one(
                        "WITH photos AS (
                             SELECT digest, byte_count FROM observation_image
                             UNION SELECT digest, byte_count FROM observation_image_cut
                             UNION SELECT digest, byte_count FROM box_picture)
                         SELECT (SELECT count(*) FROM item WHERE tenant_id = current_tenant()),
                                (SELECT count(DISTINCT digest) FROM photos),
                                (SELECT coalesce(sum(byte_count), 0)::bigint FROM (SELECT DISTINCT digest, byte_count FROM photos) d),
                                (SELECT max(name) FROM schema_migration)",
                        &[],
                    )
                    .await?;
                Ok(BackupSummary { items: r.get(0), photos: r.get(1), photo_bytes: r.get(2), schema: r.get(3) })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

#[derive(Deserialize)]
pub struct BackupRequest {
    pub password: String,
}

/// Where backups are written before they are sent; each is removed an hour on.
fn spool() -> PathBuf {
    std::env::temp_dir().join("spork-backups")
}

/// Download the workspace's backup, encrypted with the password given. A form
/// post, so the browser saves it to disk as it arrives rather than holding
/// gigabytes of photographs in memory.
#[post("/backup")]
pub async fn download_backup(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Form<BackupRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    administrator(&state, &who).await?;
    let password = body.into_inner().password;
    if password.chars().count() < LEAST_PASSWORD {
        return Err(ApiError::Rejected(format!("a backup's password is at least {LEAST_PASSWORD} characters")));
    }
    let dir = spool();
    std::fs::create_dir_all(&dir).map_err(|e| ApiError::Rejected(format!("could not write the backup: {e}")))?;
    sweep(&dir);
    let out = dir.join(format!("{}.zip", Uuid::now_v7()));

    let mut conn = state.pool.get().await?;
    ensure_login_role(&conn).await?;
    let made = export(&mut conn, who.tenant_id, &password, &images::directory(), &out).await;
    ensure_app_role(&conn).await?;
    let manifest = made?;

    let name = format!("spork-backup-{}.zip", manifest.exported_at.format("%Y-%m-%d-%H%M"));
    let file = NamedFile::open_async(&out)
        .await
        .map_err(|e| ApiError::Rejected(format!("could not read the backup back: {e}")))?
        .set_content_type("application/zip".parse().expect("a media type"))
        .set_content_disposition(ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![DispositionParam::Filename(name)],
        });
    Ok(file.into_response(&req))
}

/// Remove backups written more than an hour ago: long since sent.
fn sweep(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .map(|t| t.elapsed().map(|e| e.as_secs() > 3600).unwrap_or(false))
            .unwrap_or(false);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}
