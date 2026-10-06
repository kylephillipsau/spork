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
//! # One fulfilment per item fulfilment, when the sender says which (D172)
//!
//! A line can say which document it came from: the sales order's internal id,
//! the item fulfilment's internal id and number, and its own line key. When it
//! does, the fulfilment is found by the item fulfilment's id rather than by
//! order and site, so two item fulfilments at one site are two fulfilments
//! here, and the same one sent twice is the same row. A fulfilment loaded
//! before senders said (by order and site, with no id) is adopted by the first
//! send that does, rather than duplicated.
//!
//! # A kit is ordered, and its parts are packed (D223)
//!
//! NetSuite sells some things as kits, and an item fulfilment carries the
//! kit's line and then a line for each part. A line says which it is
//! ([`Role`]). The kit's own line is loaded as what was ordered and commits
//! nothing, so nothing downstream counts it as goods; a part is an ordinary
//! line that says which kit it is part of.
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
    /// Which document this line came from, when the sender knows (D172). The
    /// export does not, and loads by order and site as it always has.
    pub source: Option<Source>,
    /// Goods, a kit's own line, or one of a kit's parts (D223).
    pub role: Role,
}

/// What a line is to the work (D223).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Role {
    /// Goods to pick and pack: every line, unless the sender says otherwise.
    #[default]
    Goods,
    /// A kit's own line: what the customer ordered, never stocked, and no
    /// work. Its parts are the goods.
    Kit,
    /// One of a kit's parts: goods, and which kit, by the kit line's item and
    /// line number on the same order.
    PartOf { item: String, line_no: i32 },
}

/// One line of an item fulfilment, in the sender's words, as far as kits go.
pub struct Sent<'a> {
    /// The line's own key in the item fulfilment.
    pub key: Option<&'a str>,
    /// The key of the kit line it is part of, when it is one of a kit's parts.
    pub kit_line: Option<&'a str>,
    /// NetSuite's type for its item: `Kit`, `InvtPart`, …
    pub item_type: Option<&'a str>,
    /// The item's code, through [`crate::orders::item_code`], and the order line.
    pub item: String,
    pub line_no: i32,
}

/// What each line of one item fulfilment is (D223), from what the sender says.
///
/// A line is a kit's when its item's type says so, or when a line says it is
/// part of it; a line naming another line in the same send is one of its
/// parts. A part naming a line the send doesn't have is taken as goods, and a
/// line naming itself is the kit.
pub fn roles(lines: &[Sent]) -> Vec<Role> {
    let named: HashSet<&str> = lines.iter().filter_map(|l| l.kit_line).collect();
    lines
        .iter()
        .map(|l| {
            let kit = l.kit_line.filter(|k| l.key != Some(*k)).and_then(|k| lines.iter().find(|x| x.key == Some(k)));
            if let Some(kit) = kit {
                return Role::PartOf { item: kit.item.clone(), line_no: kit.line_no };
            }
            let kit_type = l.item_type.is_some_and(|t| t.trim().eq_ignore_ascii_case("kit"));
            if kit_type || l.key.is_some_and(|k| named.contains(k)) {
                Role::Kit
            } else {
                Role::Goods
            }
        })
        .collect()
}

/// Where a line came from, in the channel's own keys.
#[derive(Debug, Clone, Default)]
pub struct Source {
    /// The sales order's internal id.
    pub order_id: Option<String>,
    /// The item fulfilment's internal id: what a fulfilment is found by.
    pub fulfilment_id: Option<String>,
    /// Its number, the one a person quotes: `IF270947`.
    pub fulfilment_number: Option<String>,
    /// The line's key within the item fulfilment.
    pub line: Option<String>,
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
    /// `ordered`, `to_pick`, or `kit`: a kit's own line committed before the
    /// sender said it was one, which stays committed (D223).
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
    /// Fulfilments loaded before their document was known, now matched to it.
    pub fulfilments_adopted: u64,
    pub commitments_created: u64,
    /// Lines that learnt which kit they are part of (D223).
    pub parts_of_kits: u64,
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
    // Each part's line, its order, and its kit's item and line (D223).
    let mut parts: Vec<(Uuid, Uuid, Uuid, i32)> = vec![];
    // Keyed by order, site and, when the line names one, the item fulfilment.
    let mut fulfilment_ids: HashMap<(Uuid, Uuid, Option<String>), Uuid> = HashMap::new();

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
                             placed_at, promised_to, state, currency, external_id)
                         SELECT $1, $2, $3, $4, NULLIF($5, ''), $6, $7, $7, 'placed', 'AUD', $8
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
                            &order_external_id(l),
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
                // An order loaded before its id was known learns it now. Only
                // into an empty column: an id already on file is not overwritten
                // by a different one, which would be two orders sharing a number.
                if let Some(external) = order_external_id(l) {
                    tx.execute(
                        "UPDATE \"order\" SET external_id = $2
                          WHERE id = $1 AND external_id IS NULL",
                        &[&id, &external],
                    )
                    .await
                    .map_err(|e| format!("order {}: {e}", l.doc))?;
                }
                order_ids.insert(l.doc.as_str(), id);
                id
            }
        };

        // **One commitment per item fulfilment when the line says which, and
        // per order and site when it does not** — planned, with nothing picked.
        // Nothing has moved in this system, so every progress quantity is zero
        // and that is the truth rather than a gap. A pick made elsewhere is
        // reported separately (D172), not written here.
        let document = document_of(l);
        let key = (order_id, site, document.map(|d| d.0.to_string()));
        let fulfilment_id = match fulfilment_ids.get(&key) {
            Some(id) => *id,
            None => {
                let id = match document {
                    Some((external, number)) => {
                        let (id, made, adopted) = fulfilment_for_document(
                            tx, tenant, channel, order_id, site, external, number,
                        )
                        .await
                        .map_err(|e| format!("fulfilment {external} for {}: {e}", l.doc))?;
                        out.fulfilments_created += u64::from(made);
                        out.fulfilments_adopted += u64::from(adopted);
                        id
                    }
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
                        tx.query_one(
                            "SELECT id FROM fulfilment
                              WHERE order_id = $1 AND site_id = $2 ORDER BY id LIMIT 1",
                            &[&order_id, &site],
                        )
                        .await
                        .map_err(|e| e.to_string())?
                        .get(0)
                    }
                };
                fulfilment_ids.insert(key, id);
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
        if let Role::PartOf { item: kit, line_no } = &l.role {
            if let Some(&kit_item) = item_ids.get(kit) {
                parts.push((line_id, order_id, kit_item, *line_no));
            }
        }

        // What is still to pick. Nothing outstanding means no commitment:
        // `fulfilment_line_quantity_ck` insists on more than zero, rightly. A
        // kit's own line is no work, whatever the document says is to pick.
        let outstanding = if l.role == Role::Kit { 0 } else { l.outstanding };
        let external_line = l.source.as_ref().and_then(|s| s.line.as_deref());
        let committed = tx
            .query_opt(
                "SELECT quantity, id FROM fulfilment_line
                  WHERE fulfilment_id = $1 AND order_line_id = $2",
                &[&fulfilment_id, &line_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        match committed {
            Some(row) => {
                let on_file: i64 = row.get(0);
                // A commitment loaded before its line key was known learns it.
                if let Some(key) = external_line {
                    let id: Uuid = row.get(1);
                    tx.execute(
                        "UPDATE fulfilment_line SET external_line = $2
                          WHERE id = $1 AND external_line IS NULL",
                        &[&id, &key],
                    )
                    .await
                    .map_err(|e| format!("fulfilment_line {} {}: {e}", l.doc, l.item))?;
                }
                // A kit's line committed before the sender said it was one
                // stays committed, as every commitment made does, and is said.
                if l.role == Role::Kit {
                    out.differs.push(Differs {
                        doc: l.doc.clone(),
                        line: l.line_no,
                        item: l.item.clone(),
                        on_file,
                        arrived: 0,
                        field: "kit",
                    });
                } else if on_file != outstanding && outstanding > 0 {
                    out.differs.push(Differs {
                        doc: l.doc.clone(),
                        line: l.line_no,
                        item: l.item.clone(),
                        on_file,
                        arrived: outstanding,
                        field: "to_pick",
                    });
                }
            }
            None if outstanding > 0 => {
                out.commitments_created += tx
                    .execute(
                        "INSERT INTO fulfilment_line (tenant_id, fulfilment_id,
                             order_line_id, quantity, external_line)
                         VALUES ($1, $2, $3, $4, $5)",
                        &[&tenant, &fulfilment_id, &line_id, &outstanding, &external_line],
                    )
                    .await
                    .map_err(|e| format!("fulfilment_line {} {}: {e}", l.doc, l.item))?;
            }
            None => {}
        }
        out.units_to_pick += outstanding;
    }

    // Each part learns its kit: the kit's line on the same order. Only into an
    // empty column, as an order learns its id.
    for (part, order_id, kit_item, kit_line) in parts {
        out.parts_of_kits += tx
            .execute(
                "UPDATE order_line p SET kit_line_id = k.id
                   FROM order_line k
                  WHERE p.id = $1 AND p.kit_line_id IS NULL
                    AND k.order_id = $2 AND k.item_id = $3 AND k.line_number = $4 AND k.id <> p.id",
                &[&part, &order_id, &kit_item, &kit_line],
            )
            .await
            .map_err(|e| format!("order_line kit: {e}"))?;
    }

    Ok(out)
}

/// The sales order's id in its channel, when the line carries one.
fn order_external_id(l: &Line) -> Option<&str> {
    l.source.as_ref()?.order_id.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// The item fulfilment's id and number, when the line carries the id.
fn document_of(l: &Line) -> Option<(&str, Option<&str>)> {
    let s = l.source.as_ref()?;
    let id = s.fulfilment_id.as_deref().map(str::trim).filter(|s| !s.is_empty())?;
    let number = s.fulfilment_number.as_deref().map(str::trim).filter(|s| !s.is_empty());
    Some((id, number))
}

/// The fulfilment that is this item fulfilment: found by its id, else adopted
/// from a row loaded by order and site before ids were sent, else made.
///
/// Returns the id, whether one was made, and whether one was adopted. The
/// number is recorded where the row has none, and left alone where it has one:
/// a number that changed under a known id is for a person, not an importer.
async fn fulfilment_for_document(
    tx: &Transaction<'_>,
    tenant: Uuid,
    channel: Uuid,
    order_id: Uuid,
    site: Uuid,
    external: &str,
    number: Option<&str>,
) -> Result<(Uuid, bool, bool), tokio_postgres::Error> {
    if let Some(row) = tx
        .query_opt(
            "SELECT id FROM fulfilment
              WHERE tenant_id = $1 AND source_channel_id = $2 AND external_id = $3",
            &[&tenant, &channel, &external],
        )
        .await?
    {
        let id: Uuid = row.get(0);
        tx.execute(
            "UPDATE fulfilment SET reference = $2 WHERE id = $1 AND reference IS NULL",
            &[&id, &number],
        )
        .await?;
        return Ok((id, false, false));
    }

    // **Adopt, do not duplicate.** Before senders named the item fulfilment,
    // the userscript loaded one fulfilment per order and site. The first send
    // that names it claims that row, so the work already recorded against it
    // stays with the document it was for. Only a row nobody has claimed, and
    // not a cancelled one.
    if let Some(row) = tx
        .query_opt(
            "SELECT id FROM fulfilment
              WHERE order_id = $1 AND site_id = $2 AND external_id IS NULL
                AND state <> 'cancelled'
              ORDER BY id LIMIT 1",
            &[&order_id, &site],
        )
        .await?
    {
        let id: Uuid = row.get(0);
        tx.execute(
            "UPDATE fulfilment
                SET source_channel_id = $2, external_id = $3,
                    reference = coalesce(reference, $4)
              WHERE id = $1",
            &[&id, &channel, &external, &number],
        )
        .await?;
        return Ok((id, false, true));
    }

    let id: Uuid = tx
        .query_one(
            "INSERT INTO fulfilment (tenant_id, order_id, site_id, state,
                 source_channel_id, external_id, reference)
             VALUES ($1, $2, $3, 'planned', $4, $5, $6)
             RETURNING id",
            &[&tenant, &order_id, &site, &channel, &external, &number],
        )
        .await?
        .get(0);
    Ok((id, true, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sent<'a>(key: &'a str, kit_line: Option<&'a str>, item_type: Option<&'a str>, item: &str, line_no: i32) -> Sent<'a> {
        Sent { key: Some(key), kit_line, item_type, item: item.into(), line_no }
    }

    #[test]
    fn a_kit_is_its_own_line_and_its_parts_say_so() {
        // A kit and its two parts, then the same kit again in red, then goods.
        let lines = [
            sent("0", None, Some("Kit"), "SPR-B", 1),
            sent("1", Some("0"), Some("InvtPart"), "HEAD-B", 2),
            sent("4", Some("0"), Some("InvtPart"), "BOTTLE", 3),
            sent("7", None, None, "SPR-R", 4),
            sent("8", Some("7"), None, "HEAD-R", 5),
            sent("9", None, Some("InvtPart"), "GLOVE", 6),
        ];
        let part = |item: &str, line_no| Role::PartOf { item: item.into(), line_no };
        assert_eq!(
            roles(&lines),
            vec![Role::Kit, part("SPR-B", 1), part("SPR-B", 1), Role::Kit, part("SPR-R", 4), Role::Goods],
            "a kit by its type, or by a part naming it"
        );
        // A part naming a line the send doesn't have is goods, not a guess.
        assert_eq!(roles(&[sent("1", Some("99"), None, "HEAD-B", 2)]), vec![Role::Goods]);
        assert_eq!(roles(&[sent("0", None, Some(" kit "), "SPR-B", 1)]), vec![Role::Kit], "however it is written");
        // A kit line NetSuite names as its own kit is the kit, not a part of itself.
        assert_eq!(
            roles(&[sent("0", Some("0"), None, "SPR-B", 1), sent("1", Some("0"), None, "HEAD-B", 2)]),
            vec![Role::Kit, part("SPR-B", 1)]
        );
    }
}
