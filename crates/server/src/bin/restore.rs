//! Restore a workspace's backup into an empty Spork. D193.
//!
//!     spork-restore <backup.zip>
//!
//! `DATABASE_URL` names the database, as the database owner: a restore sets
//! constraints and triggers aside while it loads. `SPORK_IMAGE_DIR` is where
//! the photographs go. The password is `SPORK_BACKUP_PASSWORD`, or read from
//! the terminal. Everything is one transaction: a restore that fails leaves
//! the database as it was (photographs already written stay, by address).

use std::io::BufRead;
use std::path::PathBuf;

use spork_server::{backup, images};
use tokio_postgres::NoTls;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("spork-restore: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::args().nth(1).map(PathBuf::from) else {
        return Err("usage: spork-restore <backup.zip>".into());
    };
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://postgres@localhost:55432/spork".into());
    let password = match std::env::var("SPORK_BACKUP_PASSWORD") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            eprint!("Backup password: ");
            let mut line = String::new();
            std::io::stdin().lock().read_line(&mut line)?;
            line.trim_end_matches(['\r', '\n']).to_string()
        }
    };

    let manifest = backup::manifest_of(&path, &password)?;
    println!(
        "Restoring {} ({} rows in {} tables, {} photos), taken {} at migration {}",
        manifest.tenant_name,
        manifest.tables.iter().map(|t| t.rows).sum::<u64>(),
        manifest.tables.len(),
        manifest.photos,
        manifest.exported_at.format("%Y-%m-%d %H:%M UTC"),
        manifest.schema
    );

    let (mut client, connection) = tokio_postgres::connect(&url, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("spork-restore: the database connection failed: {e}");
        }
    });
    let tx = client.transaction().await?;
    let restored = backup::restore(&tx, &path, &password, &images::directory()).await?;
    tx.commit().await?;
    println!("Restored {} rows and {} photos. Sign in as before.", restored.rows, restored.photos);
    Ok(())
}
