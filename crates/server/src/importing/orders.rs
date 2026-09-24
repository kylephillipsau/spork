//! Loading sales order lines: customers, orders, lines, and the work to pick.
//!
//! Two callers, one loader. The `import_orders` example reads NetSuite's order
//! export; `POST /import/fulfilment` takes the lines of one item fulfilment,
//! sent by a userscript from the NetSuite page while it is open. They arrive in
//! different shapes and are the same writes, so each caller reads its own shape
//! into [`Line`] and hands the rows here. An importer that exists twice is an
//! importer whose copies disagree.
//!
//! # One fulfilment per order *and site*
//!
//! A NetSuite order can ship from more than one warehouse — the Fast
//! Fulfillment script grew a "Mark Melbourne" button because mixed orders are
//! ordinary. `fulfilment.site_id` is where the work happens, so a line is
//! committed to the fulfilment for its own site rather than to whichever site
//! the order's first line named. The order itself takes the first line's site.
//!
//! # Idempotent, and first write wins
//!
//! Orders are keyed by document number, lines by `(order, item, line number)`,
//! commitments by `(fulfilment, order line)`. Loading the same rows twice writes
//! nothing the second time. Loading *different* quantities for a line already
//! on file changes nothing either — they are reported in `differs`, because a
//! quantity that moved underneath work already started is for a person to look
//! at, not for an importer to overwrite.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::bins::{site_code, site_name, timezone_for};
use crate::orders::{customer_code, parse_date};

/// One order line, as a caller read it.
#[derive(Debug, Clone)]
pub struct Line {
    /// The order's document number, the key a person quotes.
    pub doc: String,
    pub line_no: i32,
    /// The sellable code, already through [`crate::orders::item_code`].
    pub item: String,
    /// Used only when the item is created here (see [`Options::create_items`]).
    pub description: Option<String>,
    /// What the order line records. When the caller cannot see what was
    /// ordered, the most it knows was outstanding.
    pub ordered: i64,
    /// What is still to pick. Zero means no commitment.
    pub outstanding: i64,
    pub customer: String,
    pub po_ref: String,
    /// The warehouse, as NetSuite names it: `Melbourne Warehouse`.
    pub location: String,
    /// Day-first, as the export writes it. Blank means now.
    pub date: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Options {
    /// Create an item the catalogue lacks, from the line's code and
    /// description. The export carries no description, so the example leaves
    /// this off and reports the item as missing; a fulfilment page shows both,
    /// and is as much evidence the item exists as an order is that a warehouse
    /// does.
    pub create_items: bool,
}

/// A line that was not loaded, and why.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub doc: String,
    pub line: i32,
    pub item: String,
    pub reason: SkipReason,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// Not in the catalogue, and this load does not create items.
    UnknownItem,
    /// No site on file and no clock known for the warehouse, so a site cannot
    /// be made for it.
    UnknownWarehouse,
    /// Nothing ordered and nothing to pick: `order_line_quantity_ck`.
    NothingOrdered,
}

/// A line already on file whose quantities disagree with what arrived.
#[derive(Debug, Serialize, Clone)]
pub struct Differs {
    pub doc: String,
    pub line: i32,
    pub item: String,
    pub on_file: i64,
    pub arrived: i64,
    pub field: &'static str,
}

/// What the database did, or would have done.
#[derive(Debug, Default, Serialize)]
pub struct OrdersLoaded {
    pub sites_created: usize,
    pub customers_created: usize,
    pub items_created: usize,
    pub orders_created: u64,
    pub lines_created: usize,
    pub fulfilments_created: u64,
    pub commitments_created: u64,
    /// Lines that loaded, or were already on file.
    pub lines_loaded: usize,
    /// Units still to pick across the lines that loaded.
    pub units_to_pick: i64,
    pub skipped: Vec<Skipped>,
    pub differs: Vec<Differs>,
    pub applied: bool,
}

/// Write the lines, or find out what writing them would do.
///
/// The writes happen and are rolled back to a savepoint when `apply` is false,
/// the same bargain as the other loaders: the dry run *is* the apply, undone.
pub async fn load(
    tx: &Transaction<'_>,
    tenant: Uuid,
    lines: &[Line],
    options: Options,
    apply: bool,
) -> Result<OrdersLoaded, String> {
    tx.batch_execute("SAVEPOINT spork_import")
        .await
        .map_err(|e| e.to_string())?;
    let out = write(tx, tenant, lines, options, apply).await;
    let end = if apply && out.is_ok() {
        "RELEASE SAVEPOINT spork_import"
    } else {
        "ROLLBACK TO SAVEPOINT spork_import"
    };
    tx.batch_execute(end).await.map_err(|e| e.to_string())?;
    out
}

async fn write(
    tx: &Transaction<'_>,
    tenant: Uuid,
    lines: &[Line],
    options: Options,
    apply: bool,
) -> Result<OrdersLoaded, String> {
    let mut out = OrdersLoaded { applied: apply, ..Default::default() };

    // ---- items ----
    let mut item_ids: HashMap<String, Uuid> = HashMap::new();
    for row in tx
        .query("SELECT code, id FROM item WHERE tenant_id = $1", &[&tenant])
        .await
        .map_err(|e| e.to_string())?
    {
        item_ids.insert(row.get(0), row.get(1));
    }
    if options.create_items {
        let mut made: BTreeSet<&str> = BTreeSet::new();
        for l in lines {
            if item_ids.contains_key(&l.item) || !made.insert(l.item.as_str()) {
                continue;
            }
            let description = l
                .description
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .unwrap_or(&l.item);
            let row = tx
                .query_one(
                    "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
                     SELECT $1, $2, $3, u.id, 'none' FROM unit u WHERE u.code = 'ea'
                     RETURNING id",
                    &[&tenant, &l.item, &description],
                )
                .await
                .map_err(|e| format!("item {}: {e}", l.item))?;
            item_ids.insert(l.item.clone(), row.get(0));
            out.items_created += 1;
        }
    }

    // ---- sites ----
    //
    // **A warehouse an order ships from is evidence the warehouse exists.**
    // Perth appeared on 18 lines and was absent from the bin export, so it has
    // no shelves — but it has a clock, and a site with no shelves is visible and
    // true where a dropped order line is neither.
    let mut site_ids: HashMap<String, Uuid> = HashMap::new();
    for row in tx
        .query("SELECT name, id FROM site WHERE tenant_id = $1", &[&tenant])
        .await
        .map_err(|e| e.to_string())?
    {
        site_ids.insert(row.get::<_, String>(0), row.get(1));
    }
    let warehouses: BTreeSet<&str> = lines.iter().map(|l| l.location.as_str()).collect();
    for name in warehouses {
        let short = site_name(name);
        if site_ids.contains_key(&short) {
            continue;
        }
        let Some(tz) = timezone_for(name) else {
            continue;
        };
        let id: Uuid = tx
            .query_one(
                "INSERT INTO site (tenant_id, code, name, timezone, active)
                 VALUES ($1, $2, $3, $4, true)
                 ON CONFLICT (tenant_id, code) DO UPDATE SET name = EXCLUDED.name
                 RETURNING id",
                &[&tenant, &site_code(name), &short, &tz],
            )
            .await
            .map_err(|e| format!("site {short}: {e}"))?
            .get(0);
        site_ids.insert(short, id);
        out.sites_created += 1;
    }

    // ---- what loads ----
    let mut loadable: Vec<(&Line, Uuid, Uuid)> = vec![]; // line, item, site
    for l in lines {
        let skip = |reason| Skipped {
            doc: l.doc.clone(),
            line: l.line_no,
            item: l.item.clone(),
            reason,
        };
        let Some(&item) = item_ids.get(&l.item) else {
            out.skipped.push(skip(SkipReason::UnknownItem));
            continue;
        };
        let Some(&site) = site_ids.get(&site_name(&l.location)) else {
            out.skipped.push(skip(SkipReason::UnknownWarehouse));
            continue;
        };
        if l.ordered <= 0 {
            out.skipped.push(skip(SkipReason::NothingOrdered));
            continue;
        }
        loadable.push((l, item, site));
    }
    if loadable.is_empty() {
        return Ok(out);
    }

    // The channel these arrived through. D39: an order carries which system is
    // its record of authority, and this one is ours.
    tx.execute(
        "INSERT INTO source_channel (tenant_id, code, name, authority)
         VALUES ($1, 'netsuite', 'NetSuite', 'local')
         ON CONFLICT (tenant_id, code) DO NOTHING",
        &[&tenant],
    )
    .await
    .map_err(|e| format!("source_channel: {e}"))?;
    let channel: Uuid = tx
        .query_one(
            "SELECT id FROM source_channel WHERE tenant_id = $1 AND code = 'netsuite'",
            &[&tenant],
        )
        .await
        .map_err(|e| e.to_string())?
        .get(0);

    // ---- customers ----
    let mut party_ids: HashMap<String, Uuid> = HashMap::new();
    let mut codes: HashSet<String> = tx
        .query("SELECT code FROM party WHERE tenant_id = $1", &[&tenant])
        .await
        .map_err(|e| e.to_string())?
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect();
    // Sorted, so the codes a run assigns do not depend on row order.
    let names: BTreeSet<&str> = loadable
        .iter()
        .map(|(l, _, _)| l.customer.as_str())
        .filter(|n| !n.is_empty())
        .collect();
    for name in names {
        if let Some(row) = tx
            .query_opt(
                "SELECT id FROM party WHERE tenant_id = $1 AND name = $2",
                &[&tenant, &name],
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
                &[&tenant, &name, &code],
            )
            .await
            .map_err(|e| format!("party {name}: {e}"))?
            .get(0);
        party_ids.insert(name.to_string(), id);
        out.customers_created += 1;
    }

    // ---- orders, lines, commitments ----
    let mut order_ids: HashMap<&str, Uuid> = HashMap::new();
    let mut fulfilment_ids: HashMap<(Uuid, Uuid), Uuid> = HashMap::new();

    for &(l, item, site) in &loadable {
        let order_id = match order_ids.get(l.doc.as_str()) {
            Some(id) => *id,
            None => {
                let placed = parse_date(&l.date)
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|dt| dt.and_utc())
                    .unwrap_or_else(chrono::Utc::now);
                // **The INSERT's row count, not the loop's.** Counted per order
                // *seen*, a second run reports every order as made and wrote
                // none of them.
                out.orders_created += tx
                    .execute(
                        "INSERT INTO \"order\" (tenant_id, site_id, customer_party_id,
                             confirmation_number, external_ref, source_channel_id,
                             placed_at, promised_to, state, currency)
                         SELECT $1, $2, $3, $4, NULLIF($5, ''), $6, $7, $7, 'placed', 'AUD'
                          WHERE NOT EXISTS (SELECT 1 FROM \"order\" o
                                             WHERE o.tenant_id = $1 AND o.confirmation_number = $4)",
                        &[
                            &tenant,
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
                let id: Uuid = tx
                    .query_one(
                        "SELECT id FROM \"order\"
                          WHERE tenant_id = $1 AND confirmation_number = $2
                          ORDER BY placed_at DESC LIMIT 1",
                        &[&tenant, &l.doc],
                    )
                    .await
                    .map_err(|e| e.to_string())?
                    .get(0);
                order_ids.insert(l.doc.as_str(), id);
                id
            }
        };

        // **One commitment per order and site, planned, with nothing picked.**
        // Nothing has moved in this system, so every progress quantity is zero
        // and that is the truth rather than a gap.
        let fulfilment_id = match fulfilment_ids.get(&(order_id, site)) {
            Some(id) => *id,
            None => {
                out.fulfilments_created += tx
                    .execute(
                        "INSERT INTO fulfilment (tenant_id, order_id, site_id, state)
                         SELECT $1, $2, $3, 'planned'
                          WHERE NOT EXISTS (SELECT 1 FROM fulfilment f
                                             WHERE f.order_id = $2 AND f.site_id = $3)",
                        &[&tenant, &order_id, &site],
                    )
                    .await
                    .map_err(|e| format!("fulfilment for {}: {e}", l.doc))?;
                let id: Uuid = tx
                    .query_one(
                        "SELECT id FROM fulfilment
                          WHERE order_id = $1 AND site_id = $2 ORDER BY id LIMIT 1",
                        &[&order_id, &site],
                    )
                    .await
                    .map_err(|e| e.to_string())?
                    .get(0);
                fulfilment_ids.insert((order_id, site), id);
                id
            }
        };

        let existing = tx
            .query_opt(
                "SELECT id, quantity_ordered FROM order_line
                  WHERE order_id = $1 AND item_id = $2 AND line_number = $3",
                &[&order_id, &item, &l.line_no],
            )
            .await
            .map_err(|e| e.to_string())?;
        let line_id: Uuid = match existing {
            Some(row) => {
                let on_file: i64 = row.get(1);
                if on_file != l.ordered {
                    out.differs.push(Differs {
                        doc: l.doc.clone(),
                        line: l.line_no,
                        item: l.item.clone(),
                        on_file,
                        arrived: l.ordered,
                        field: "ordered",
                    });
                }
                row.get(0)
            }
            None => {
                out.lines_created += 1;
                tx.query_one(
                    "INSERT INTO order_line (tenant_id, order_id, item_id,
                         quantity_ordered, line_number)
                     VALUES ($1, $2, $3, $4, $5) RETURNING id",
                    &[&tenant, &order_id, &item, &l.ordered, &l.line_no],
                )
                .await
                .map_err(|e| format!("order_line {} {}: {e}", l.doc, l.item))?
                .get(0)
            }
        };
        out.lines_loaded += 1;

        // What is still to pick. Nothing outstanding means no commitment:
        // `fulfilment_line_quantity_ck` insists on more than zero, rightly.
        if l.outstanding > 0 {
            let committed = tx
                .query_opt(
                    "SELECT quantity FROM fulfilment_line
                      WHERE fulfilment_id = $1 AND order_line_id = $2",
                    &[&fulfilment_id, &line_id],
                )
                .await
                .map_err(|e| e.to_string())?;
            match committed {
                Some(row) => {
                    let on_file: i64 = row.get(0);
                    if on_file != l.outstanding {
                        out.differs.push(Differs {
                            doc: l.doc.clone(),
                            line: l.line_no,
                            item: l.item.clone(),
                            on_file,
                            arrived: l.outstanding,
                            field: "to_pick",
                        });
                    }
                }
                None => {
                    out.commitments_created += tx
                        .execute(
                            "INSERT INTO fulfilment_line (tenant_id, fulfilment_id,
                                 order_line_id, quantity)
                             VALUES ($1, $2, $3, $4)",
                            &[&tenant, &fulfilment_id, &line_id, &l.outstanding],
                        )
                        .await
                        .map_err(|e| format!("fulfilment_line {} {}: {e}", l.doc, l.item))?;
                }
            }
            out.units_to_pick += l.outstanding;
        }
    }

    Ok(out)
}
