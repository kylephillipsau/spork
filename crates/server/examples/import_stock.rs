//! Load the inventory balance: where NetSuite says each thing is.
//!
//! ```sh
//! cargo run -p spork-server --example import_stock -- --stock balance.csv \
//!     --as-at 2026-09-30T09:10:00+10:00 --source netsuite-inventory-balance
//! # then, once the report reads right:
//! cargo run -p spork-server --example import_stock -- ... --apply --recorded-by <person>
//! ```
//!
//! Dry run by default, like [the bin importer](import_bins), and a front end
//! in the same way: reading, surveying and writing are
//! [`spork_server::importing::stock`], which `POST /api/import/stock` also
//! calls. What is left here is arguments, a connection, and printing.
//!
//! # What it asks for
//!
//! **`--as-at`, when the report was taken.** Required and never defaulted, for
//! migration 86's reason: a number that cannot say how old it is gets read as
//! current for ever. For a search exported by hand, the moment the file was
//! downloaded is the moment it describes.
//!
//! **`--source`, which feed this is.** A second load under the same source
//! replaces the first, because two copies of a snapshot is not twice the stock.
//!
//! **`--expect`, optionally, the row count the search showed.** A file of a
//! different length is refused before anything is written, because a
//! truncated export reads as empty shelves. Absent means unchecked, and the
//! report says so.
//!
//! # What it writes
//!
//! `reported_stock`, and nothing else: not `stock`, not a movement. Rows at a
//! warehouse, item or bin this database does not have are counted and skipped,
//! which is how a bin list loaded for one warehouse keeps the others out.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use spork_server::importing;
use tokio_postgres::NoTls;
use uuid::Uuid;

struct Args {
    stock: PathBuf,
    tenant: Uuid,
    recorded_by: Option<Uuid>,
    as_at: DateTime<Utc>,
    source: String,
    expect: Option<usize>,
    apply: bool,
    url: String,
}

fn args() -> Result<Args, String> {
    let mut stock = None;
    let mut apply = false;
    let mut tenant = "11111111-1111-1111-1111-111111111111".to_string();
    let mut recorded_by: Option<String> = None;
    let mut as_at: Option<String> = None;
    let mut source: Option<String> = None;
    let mut expect: Option<String> = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--stock" => stock = it.next().map(PathBuf::from),
            "--tenant" => tenant = it.next().unwrap_or_default(),
            "--recorded-by" => recorded_by = it.next(),
            "--as-at" => as_at = it.next(),
            "--source" => source = it.next(),
            "--expect" => expect = it.next(),
            "--apply" => apply = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let as_at = as_at
        .ok_or("--as-at is required: when the report was taken, e.g. 2026-09-30T09:10:00+10:00")?;
    let as_at = DateTime::parse_from_rfc3339(&as_at)
        .map_err(|e| format!("--as-at {as_at}: {e}"))?
        .with_timezone(&Utc);
    let source = source
        .filter(|s| !s.trim().is_empty())
        .ok_or("--source is required: the feed this export is, e.g. netsuite-inventory-balance")?;
    let expect = match expect {
        Some(n) => Some(n.parse::<usize>().map_err(|_| format!("bad --expect {n}"))?),
        None => None,
    };
    // Required to apply and only to apply, for `import_bins`' reason: the
    // arrival is filed as an act and an act names who performed it.
    let recorded_by = match recorded_by {
        Some(p) => Some(p.parse::<Uuid>().map_err(|_| "bad --recorded-by")?),
        None if apply => {
            return Err(
                "--recorded-by <person uuid> is required with --apply: the arrival is \
                 recorded as an act and an act names who performed it"
                    .into(),
            )
        }
        None => None,
    };
    Ok(Args {
        stock: stock.ok_or("--stock is required")?,
        tenant: tenant.parse().map_err(|_| "bad --tenant")?,
        recorded_by,
        as_at,
        source,
        expect,
        apply,
        url: std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set")?,
    })
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!(
                "{e}\n\nusage: --stock <csv> --as-at <rfc3339> --source <feed> [--expect <rows>] \
                 [--apply --recorded-by <person>]"
            );
            std::process::exit(2);
        }
    };

    let payload = std::fs::read(&args.stock)
        .map_err(|e| format!("{}: {e}", args.stock.display()))?;
    let rows = importing::stock::read(payload.as_slice())?;
    if rows.is_empty() {
        return Err("no rows with an item and a quantity: is this the inventory balance export?".into());
    }
    let survey = importing::stock::survey(&rows);

    println!("\n  {} rows, as at {}\n", survey.rows, args.as_at);
    println!("  {:<24}{:>7}{:>12}", "warehouse", "rows", "on a shelf");
    for w in &survey.warehouses {
        println!("  {:<24}{:>7}{:>12}", w.warehouse, w.rows, w.positioned);
    }
    println!("\n    {:>5} rows report nothing on hand, kept: an empty shelf is a statement", survey.empty);

    match importing::stock::shortfall(rows.len(), args.expect) {
        Some(refused) => {
            println!("\n  Refused. {refused}\n");
            std::process::exit(1);
        }
        None if args.expect.is_none() => println!(
            "\n  The row count was not checked: pass --expect with the count the search showed."
        ),
        None => println!("\n  The row count matches what the search showed."),
    }

    // Under row-level security as the application role, the footing
    // `POST /api/import/stock` runs on.
    let (client, connection) = tokio_postgres::connect(&args.url, NoTls)
        .await
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client.batch_execute("SET ROLE spork_app").await.map_err(|e| e.to_string())?;
    client
        .execute(
            "SELECT set_config('spork.tenant_id', $1::text, false)",
            &[&args.tenant.to_string()],
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut client = client;
    let tx = client.transaction().await.map_err(|e| e.to_string())?;
    // A dry run keeps nothing, so it files nothing either.
    let arrival = match (args.apply, args.recorded_by) {
        (true, Some(person)) => Some(
            importing::received::record(
                &tx,
                args.tenant,
                importing::received::Actor::Person(person),
                args.stock.file_name().and_then(|n| n.to_str()),
                &payload,
            )
            .await?,
        ),
        _ => None,
    };
    let loaded =
        importing::stock::load(&tx, args.tenant, &rows, args.as_at, &args.source, args.apply)
            .await?;
    if let Some(a) = &arrival {
        importing::received::parsed(&tx, a.party_message_id).await?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;

    println!("\n  {}", if loaded.applied { "Applied." } else { "Dry run — nothing was kept." });
    println!("    {:>5} balances written, one per item per shelf", loaded.rows_written);
    println!(
        "    {:>5} rows summed into another for the same item and shelf (lots, statuses)",
        loaded.rows_summed
    );
    println!("    {:>5} rows left out: the balance was below zero", loaded.rows_negative);
    println!("    {:>5} rows at a warehouse this database does not have", loaded.warehouses_unknown);
    println!("    {:>5} rows naming an item the item master lacks", loaded.items_unknown);
    println!("    {:>5} rows naming a bin the bin list lacks", loaded.bins_unknown);
    println!(
        "    {:>5} balances from the previous {} load, replaced",
        loaded.rows_replaced, args.source
    );
    if let Some(a) = &arrival {
        println!(
            "\n  Filed as party_message {}{}",
            a.party_message_id,
            if a.is_replay() { " — these exact bytes had already arrived" } else { "" }
        );
    }
    if !loaded.applied {
        println!("\n  Add --apply --recorded-by <person> once the above reads right.");
    }
    println!();
    Ok(())
}
