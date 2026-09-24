//! Load sales order lines: customers, orders, lines, and the work to pick.
//!
//! ```sh
//! cargo run -p spork-server --example import_orders -- --orders orders.csv
//! cargo run -p spork-server --example import_orders -- --orders … --apply
//! ```
//!
//! Dry run by default, like the other two.
//!
//! # Read positionally, because two columns are called `Name`
//!
//! The export has `Name` at index 3 (the item) and again at index 8 (the
//! customer). Any reader keyed by header name keeps one and silently discards
//! the other, and the one it keeps depends on the library. So this reads by
//! position and names the columns itself.
//!
//! # What is left is not what was ordered
//!
//! 362 of 430 lines on `Pending Fulfillment` orders are already partly shipped.
//! The order line records what was ordered, because that is what was ordered;
//! the *fulfilment* line records `ordered − fulfilled`, because that is what is
//! still to pick. Loading the ordered figure as work would put units on the
//! bench that left the building last week.
//!
//! A line with nothing outstanding gets no fulfilment line at all —
//! `fulfilment_line_quantity_ck` insists a commitment is for more than zero, and
//! it is right to.
//!
//! # Correct by construction
//!
//! Nothing here has been picked in this system, so every progress quantity is
//! zero and *that is true* rather than a gap. This is the difference between
//! this file and the shipped fulfilment history: no movements have to be
//! invented, because no movements happened here yet.
//!
//! # The writes are shared
//!
//! Everything from here to the database is [`spork_server::importing::orders`],
//! which `POST /import/fulfilment` also uses. This file reads the export and
//! prints what the loader says.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use spork_server::importing::orders::{self, Options, SkipReason};
use spork_server::orders::{customer_name, item_code, outstanding};
use tokio_postgres::NoTls;
use uuid::Uuid;

// The columns, by position. See the note above.
const LINE: usize = 1;
const DOC: usize = 2;
const ITEM: usize = 3;
const QTY: usize = 5;
const COMMITTED: usize = 6;
const FULFILLED: usize = 7;
const CUSTOMER: usize = 8;
const PO_REF: usize = 9;
const LOCATION: usize = 10;
const DATE: usize = 12;

struct Args {
    orders: PathBuf,
    tenant: Uuid,
    apply: bool,
    url: String,
}

fn args() -> Result<Args, String> {
    let mut orders = None;
    let mut apply = false;
    let mut tenant = "11111111-1111-1111-1111-111111111111".to_string();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--orders" => orders = it.next().map(PathBuf::from),
            "--tenant" => tenant = it.next().unwrap_or_default(),
            "--apply" => apply = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        orders: orders.ok_or("--orders is required")?,
        tenant: tenant.parse().map_err(|_| "bad --tenant")?,
        apply,
        url: std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set")?,
    })
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\nusage: --orders <csv> [--apply]");
            std::process::exit(2);
        }
    };

    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(&args.orders)
        .map_err(|e| e.to_string())?;
    let mut lines = vec![];
    let mut committed_seen = 0;
    for rec in rdr.records() {
        let r = rec.map_err(|e| e.to_string())?;
        let g = |i: usize| r.get(i).unwrap_or("").trim().to_string();
        if g(DOC).is_empty() || g(ITEM).is_empty() {
            continue;
        }
        let ordered = g(QTY).parse::<f64>().unwrap_or(0.0).round() as i64;
        let fulfilled = g(FULFILLED).parse::<f64>().unwrap_or(0.0).round() as i64;
        if g(COMMITTED).parse::<f64>().is_ok() {
            committed_seen += 1;
        }
        // Internal ID is not read; the natural key is the document number,
        // which is what a person quotes.
        lines.push(orders::Line {
            doc: g(DOC),
            line_no: g(LINE).parse().unwrap_or(0),
            item: item_code(&g(ITEM)),
            // The export has no description column, so items are not created.
            description: None,
            ordered,
            outstanding: outstanding(ordered, fulfilled),
            customer: customer_name(&g(CUSTOMER)),
            po_ref: g(PO_REF),
            location: g(LOCATION),
            date: g(DATE),
        });
    }

    let docs: HashSet<&str> = lines.iter().map(|l| l.doc.as_str()).collect();
    let customers: HashSet<&str> = lines.iter().map(|l| l.customer.as_str()).collect();
    println!(
        "\n  {} lines, {} orders, {} customers\n",
        lines.len(),
        docs.len(),
        customers.len()
    );

    let (mut client, connection) = tokio_postgres::connect(&args.url, NoTls)
        .await
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
        .batch_execute("SET ROLE spork_app")
        .await
        .map_err(|e| e.to_string())?;
    client
        .execute(
            "SELECT set_config('spork.tenant_id', $1::text, false)",
            &[&args.tenant.to_string()],
        )
        .await
        .map_err(|e| e.to_string())?;

    let tx = client.transaction().await.map_err(|e| e.to_string())?;
    let loaded = orders::load(&tx, args.tenant, &lines, Options::default(), args.apply).await?;
    tx.commit().await.map_err(|e| e.to_string())?;

    // **Counted separately, because they are different problems.** The first
    // version said "the catalogue does not have this item" about lines whose
    // item was fine and whose warehouse was missing, which is the kind of report
    // that sends somebody looking in the wrong file.
    let count = |why: SkipReason| loaded.skipped.iter().filter(|s| s.reason == why).count();
    let with_work = lines.iter().filter(|l| l.outstanding > 0).count();
    println!("  {:>5} lines load", loaded.lines_loaded);
    println!(
        "  {with_work:>5} lines still have something to pick ({} units load)",
        loaded.units_to_pick
    );
    println!("  {:>5} name an item the catalogue does not have", count(SkipReason::UnknownItem));
    println!("  {:>5} name a warehouse with no site on file", count(SkipReason::UnknownWarehouse));
    let unknown_items: BTreeMap<&str, usize> = loaded
        .skipped
        .iter()
        .filter(|s| s.reason == SkipReason::UnknownItem)
        .fold(BTreeMap::new(), |mut m, s| {
            *m.entry(s.item.as_str()).or_insert(0) += 1;
            m
        });
    if !unknown_items.is_empty() {
        let mut v: Vec<_> = unknown_items.iter().collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        println!(
            "        {}",
            v.iter().take(8).map(|(c, n)| format!("{c} ×{n}")).collect::<Vec<_>>().join("  ")
        );
    }
    if !loaded.differs.is_empty() {
        println!("  {:>5} lines on file disagree with the export, and were left alone", loaded.differs.len());
    }

    println!();
    println!("    {:>5} sites created", loaded.sites_created);
    println!("    {:>5} customers created", loaded.customers_created);
    println!("    {:>5} orders", loaded.orders_created);
    println!("    {:>5} order lines", loaded.lines_created);
    println!(
        "    {:>5} fulfilments, {} lines to pick",
        loaded.fulfilments_created, loaded.commitments_created
    );
    if !args.apply {
        println!("\n  Dry run. Nothing was written. Add --apply once the above reads right.\n");
        return Ok(());
    }
    println!("\n  Applied.");
    if committed_seen > 0 {
        println!(
            "\n  `Quantity Committed` was read and not stored: an allocation names the\n  \
             stock cell it claims (D12), and the export says how many but not which.\n  \
             Recording it without a cell would be a claim against nothing.\n"
        );
    }
    Ok(())
}
