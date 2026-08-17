//! Load sales order lines: customers, orders, lines, and the work to pick.
//!
//! ```sh
//! cargo run -p nylonite-server --example import_orders -- --orders orders.csv
//! cargo run -p nylonite-server --example import_orders -- --orders … --apply
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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use nylonite_server::orders::{customer_code, customer_name, item_code, outstanding, parse_date};
use tokio_postgres::NoTls;
use uuid::Uuid;

// The columns, by position. See the note above.
const I_ID: usize = 0;
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

struct Line {
    doc: String,
    line_no: i32,
    item: String,
    quantity: i64,
    committed: Option<i64>,
    fulfilled: i64,
    customer: String,
    po_ref: String,
    location: String,
    date: String,
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
    for rec in rdr.records() {
        let r = rec.map_err(|e| e.to_string())?;
        let g = |i: usize| r.get(i).unwrap_or("").trim().to_string();
        if g(DOC).is_empty() || g(ITEM).is_empty() {
            continue;
        }
        lines.push(Line {
            doc: g(DOC),
            line_no: g(LINE).parse().unwrap_or(0),
            item: item_code(&g(ITEM)),
            quantity: g(QTY).parse::<f64>().unwrap_or(0.0).round() as i64,
            committed: g(COMMITTED).parse::<f64>().ok().map(|v| v.round() as i64),
            fulfilled: g(FULFILLED).parse::<f64>().unwrap_or(0.0).round() as i64,
            customer: customer_name(&g(CUSTOMER)),
            po_ref: g(PO_REF),
            location: g(LOCATION),
            date: g(DATE),
            // Internal ID is read for the report only; the natural key here is
            // the document number, which is what a person quotes.
        });
        let _ = I_ID;
    }

    let docs: HashSet<&str> = lines.iter().map(|l| l.doc.as_str()).collect();
    let customers: HashSet<&str> = lines.iter().map(|l| l.customer.as_str()).collect();
    println!(
        "\n  {} lines, {} orders, {} customers\n",
        lines.len(),
        docs.len(),
        customers.len()
    );

    let (client, connection) = tokio_postgres::connect(&args.url, NoTls)
        .await
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
        .batch_execute("SET ROLE nylonite_app")
        .await
        .map_err(|e| e.to_string())?;
    client
        .execute(
            "SELECT set_config('nylonite.tenant_id', $1::text, false)",
            &[&args.tenant.to_string()],
        )
        .await
        .map_err(|e| e.to_string())?;

    // ---- what resolves, and what does not ----
    let mut item_ids: HashMap<String, Uuid> = HashMap::new();
    for row in client
        .query(
            "SELECT code, id FROM item WHERE tenant_id = $1",
            &[&args.tenant],
        )
        .await
        .map_err(|e| e.to_string())?
    {
        item_ids.insert(row.get(0), row.get(1));
    }
    let mut site_ids: HashMap<String, Uuid> = HashMap::new();
    for row in client
        .query("SELECT name, id FROM site WHERE tenant_id = $1", &[&args.tenant])
        .await
        .map_err(|e| e.to_string())?
    {
        site_ids.insert(row.get::<_, String>(0), row.get(1));
    }

    let site_of = |csv_name: &str| -> Option<Uuid> {
        site_ids
            .get(&nylonite_server::bins::site_name(csv_name))
            .copied()
    };

    let unknown_items: BTreeMap<&str, usize> =
        lines.iter().filter(|l| !item_ids.contains_key(&l.item)).fold(
            BTreeMap::new(),
            |mut m, l| {
                *m.entry(l.item.as_str()).or_insert(0) += 1;
                m
            },
        );
    let unknown_sites: BTreeMap<&str, usize> =
        lines.iter().filter(|l| site_of(&l.location).is_none()).fold(
            BTreeMap::new(),
            |mut m, l| {
                *m.entry(l.location.as_str()).or_insert(0) += 1;
                m
            },
        );

    // **What will load, counting the sites this run is about to create.** The
    // first version filtered against the sites that existed *before* the write,
    // so 18 Perth lines were dropped even though the report said Perth would be
    // made — and with them two entire orders. The report and the write have to
    // agree about what is loadable, so they compute it the same way.
    let will_have_site = |l: &Line| {
        site_of(&l.location).is_some() || nylonite_server::bins::timezone_for(&l.location).is_some()
    };
    let loadable: Vec<&Line> = lines
        .iter()
        .filter(|l| item_ids.contains_key(&l.item) && will_have_site(l))
        .collect();
    let to_pick: i64 = loadable
        .iter()
        .map(|l| outstanding(l.quantity, l.fulfilled))
        .sum();
    let with_work = loadable
        .iter()
        .filter(|l| outstanding(l.quantity, l.fulfilled) > 0)
        .count();

    // **Counted separately, because they are different problems.** The first
    // version said "the catalogue does not have this item" about lines whose
    // item was fine and whose warehouse was missing, which is the kind of report
    // that sends somebody looking in the wrong file.
    let missing_item = lines.iter().filter(|l| !item_ids.contains_key(&l.item)).count();
    let missing_site = lines
        .iter()
        .filter(|l| item_ids.contains_key(&l.item) && !will_have_site(l))
        .count();

    println!("  {:>5} lines load", loadable.len());
    println!("  {with_work:>5} of them still have something to pick ({to_pick} units)");
    println!("  {missing_item:>5} name an item the catalogue does not have");
    println!("  {missing_site:>5} name a warehouse with no site on file");
    if !unknown_items.is_empty() {
        let mut v: Vec<_> = unknown_items.iter().collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        println!("        {}", v.iter().take(8)
            .map(|(c, n)| format!("{c} ×{n}")).collect::<Vec<_>>().join("  "));
    }
    if !unknown_sites.is_empty() {
        println!("\n  warehouses with no site yet:");
        for (s, n) in &unknown_sites {
            let known = nylonite_server::bins::timezone_for(s).is_some();
            println!(
                "      {s} ×{n}  {}",
                if known { "clock known, will be created" } else { "clock unknown, left out" }
            );
        }
    }

    if !args.apply {
        println!("\n  Dry run. Nothing was written. Add --apply once the above reads right.\n");
        return Ok(());
    }

    // ---- write ----
    let mut client = client;
    let tx = client.transaction().await.map_err(|e| e.to_string())?;

    // The channel these arrived through. D39: an order carries which system is
    // its record of authority, and this one is ours.
    let channel: Uuid = {
        tx.execute(
            "INSERT INTO source_channel (tenant_id, code, name, authority)
             VALUES ($1, 'netsuite', 'NetSuite', 'local')
             ON CONFLICT (tenant_id, code) DO NOTHING",
            &[&args.tenant],
        )
        .await
        .map_err(|e| format!("source_channel: {e}"))?;
        tx.query_one(
            "SELECT id FROM source_channel WHERE tenant_id = $1 AND code = 'netsuite'",
            &[&args.tenant],
        )
        .await
        .map_err(|e| e.to_string())?
        .get(0)
    };

    let mut party_ids: HashMap<String, Uuid> = HashMap::new();
    let mut codes: HashSet<String> = tx
        .query("SELECT code FROM party WHERE tenant_id = $1", &[&args.tenant])
        .await
        .map_err(|e| e.to_string())?
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect();
    let mut parties_made = 0;
    // Sorted, so the codes a run assigns do not depend on row order.
    let mut names: Vec<&str> = customers.into_iter().collect();
    names.sort_unstable();
    for name in names {
        if let Some(row) = tx
            .query_opt(
                "SELECT id FROM party WHERE tenant_id = $1 AND name = $2",
                &[&args.tenant, &name],
            )
            .await
            .map_err(|e| e.to_string())?
        {
            party_ids.insert(name.to_string(), row.get(0));
            continue;
        }
        let code = customer_code(name, &|c| codes.contains(c));
        codes.insert(code.clone());
        let id: Uuid = tx
            .query_one(
                "INSERT INTO party (tenant_id, name, code) VALUES ($1, $2, $3) RETURNING id",
                &[&args.tenant, &name, &code],
            )
            .await
            .map_err(|e| format!("party {name}: {e}"))?
            .get(0);
        party_ids.insert(name.to_string(), id);
        parties_made += 1;
    }

    // **A warehouse an order ships from is evidence the warehouse exists.**
    // Perth appears on 18 lines and was absent from the bin export, so it has no
    // shelves — but it has a clock, and a site with no shelves is visible and
    // true where a dropped order line is neither.
    let mut sites_made = 0;
    for name in lines
        .iter()
        .map(|l| l.location.as_str())
        .collect::<std::collections::BTreeSet<_>>()
    {
        let short = nylonite_server::bins::site_name(name);
        if site_ids.contains_key(&short) {
            continue;
        }
        let Some(tz) = nylonite_server::bins::timezone_for(name) else {
            continue;
        };
        let code = nylonite_server::bins::site_code(name);
        let id: Uuid = tx
            .query_one(
                "INSERT INTO site (tenant_id, code, name, timezone, active)
                 VALUES ($1, $2, $3, $4, true)
                 ON CONFLICT (tenant_id, code) DO UPDATE SET name = EXCLUDED.name
                 RETURNING id",
                &[&args.tenant, &code, &short, &tz],
            )
            .await
            .map_err(|e| format!("site {short}: {e}"))?
            .get(0);
        site_ids.insert(short, id);
        sites_made += 1;
    }
    let mut orders_made: u64 = 0;
    let mut lines_made = 0;
    let mut fulfilments_made: u64 = 0;
    let mut commitments_made = 0;
    let mut order_ids: HashMap<&str, (Uuid, Uuid)> = HashMap::new(); // doc -> (order, fulfilment)

    for l in &loadable {
        // Re-resolved: the map has grown since `loadable` was computed.
        let Some(site) = site_ids
            .get(&nylonite_server::bins::site_name(&l.location))
            .copied()
        else {
            continue;
        };
        let placed = parse_date(&l.date)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc())
            .unwrap_or_else(chrono::Utc::now);

        let (order_id, fulfilment_id) = match order_ids.get(l.doc.as_str()) {
            Some(v) => *v,
            None => {
                // **The INSERT's row count, not the loop's.** These are counted
                // per order *seen*, and on a second run every order is seen and
                // none is created — so the first version reported "113 orders"
                // for a run that wrote nothing.
                let created = tx.execute(
                    "INSERT INTO \"order\" (tenant_id, site_id, customer_party_id,
                         confirmation_number, external_ref, source_channel_id,
                         placed_at, promised_to, state, currency)
                     SELECT $1, $2, $3, $4, NULLIF($5, ''), $6, $7, $7, 'placed', 'AUD'
                      WHERE NOT EXISTS (SELECT 1 FROM \"order\" o
                                         WHERE o.tenant_id = $1 AND o.confirmation_number = $4)",
                    &[
                        &args.tenant,
                        &site,
                        &party_ids.get(&l.customer),
                        &l.doc,
                        &l.po_ref,
                        &channel,
                        &placed,
                    ],
                )
                .await
                .map_err(|e| format!("order {}: {e}", l.doc))?;
                orders_made += created;
                let oid: Uuid = tx
                    .query_one(
                        "SELECT id FROM \"order\"
                          WHERE tenant_id = $1 AND confirmation_number = $2
                          ORDER BY placed_at DESC LIMIT 1",
                        &[&args.tenant, &l.doc],
                    )
                    .await
                    .map_err(|e| e.to_string())?
                    .get(0);

                // **One commitment per order, planned, with nothing picked.**
                // Nothing has moved in this system, so every progress quantity
                // is zero and that is the truth rather than a gap.
                fulfilments_made += tx
                    .execute(
                        "INSERT INTO fulfilment (tenant_id, order_id, site_id, state)
                         SELECT $1, $2, $3, 'planned'
                          WHERE NOT EXISTS (SELECT 1 FROM fulfilment f WHERE f.order_id = $2)",
                        &[&args.tenant, &oid, &site],
                    )
                    .await
                    .map_err(|e| format!("fulfilment for {}: {e}", l.doc))?;
                let fid: Uuid = tx
                    .query_one(
                        "SELECT id FROM fulfilment WHERE order_id = $1 ORDER BY id LIMIT 1",
                        &[&oid],
                    )
                    .await
                    .map_err(|e| e.to_string())?
                    .get(0);
                order_ids.insert(l.doc.as_str(), (oid, fid));
                (oid, fid)
            }
        };

        let item = item_ids[&l.item];
        let existing: Option<Uuid> = tx
            .query_opt(
                "SELECT id FROM order_line
                  WHERE order_id = $1 AND item_id = $2 AND line_number = $3",
                &[&order_id, &item, &l.line_no],
            )
            .await
            .map_err(|e| e.to_string())?
            .map(|r| r.get(0));
        let line_id = match existing {
            Some(id) => id,
            None => {
                lines_made += 1;
                tx.query_one(
                    "INSERT INTO order_line (tenant_id, order_id, item_id,
                         quantity_ordered, line_number)
                     VALUES ($1, $2, $3, $4, $5) RETURNING id",
                    &[&args.tenant, &order_id, &item, &l.quantity, &l.line_no],
                )
                .await
                .map_err(|e| format!("order_line {} {}: {e}", l.doc, l.item))?
                .get(0)
            }
        };

        // What is still to pick. Nothing outstanding means no commitment:
        // `fulfilment_line_quantity_ck` insists on more than zero, rightly.
        let left = outstanding(l.quantity, l.fulfilled);
        if left > 0 {
            commitments_made += tx
                .execute(
                    "INSERT INTO fulfilment_line (tenant_id, fulfilment_id, order_line_id,
                         quantity)
                     SELECT $1, $2, $3, $4
                      WHERE NOT EXISTS (SELECT 1 FROM fulfilment_line fl
                                         WHERE fl.fulfilment_id = $2 AND fl.order_line_id = $3)",
                    &[&args.tenant, &fulfilment_id, &line_id, &left],
                )
                .await
                .map_err(|e| format!("fulfilment_line {} {}: {e}", l.doc, l.item))?;
        }
        let _ = l.committed; // see the note in the report below
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    println!("\n  Applied.");
    println!("    {sites_made:>5} sites created");
    println!("    {parties_made:>5} customers created");
    println!("    {orders_made:>5} orders");
    println!("    {lines_made:>5} order lines");
    println!("    {fulfilments_made:>5} fulfilments, {commitments_made} lines to pick");
    println!(
        "\n  `Quantity Committed` was read and not stored: an allocation names the\n  \
         stock cell it claims (D12), and the export says how many but not which.\n  \
         Recording it without a cell would be a claim against nothing.\n"
    );
    Ok(())
}
