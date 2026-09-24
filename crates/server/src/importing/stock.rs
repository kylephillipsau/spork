//! Where the system of record says a thing is.
//!
//! The Inventory Balance export, landing in `reported_stock` — the table
//! migration 86 built for it and nothing has written to since.
//!
//! # This does not touch `stock`, and that is the whole design
//!
//! `stock` is a fold of `stock_movement`, and the application role holds no
//! privilege on it at all. Writing these rows as movements was the obvious
//! alternative and migration 86 refuses it in as many words: it would be this
//! system asserting it holds the goods, which starts the integrity machinery
//! comparing a days-old snapshot against a ledger that has recorded nothing,
//! and every finding it raised would be false.
//!
//! So there is no ledger write here, no `client_event` per row, and no
//! attribution floor. [`crate::auth::Machine`] says why: a loader is not a
//! person, and putting somebody's name on a report they did not write is worse
//! than leaving it off.
//!
//! # A snapshot replaces, it does not accumulate
//!
//! The unique index is `(tenant, site, item, location, source)` and DELETE is
//! granted for exactly this: loading a fresh export clears what the last one
//! wrote **for that source**. Two copies of a snapshot is not twice the stock.
//! `source` is therefore the identity of a feed rather than of a file, and
//! naming two different exports the same thing makes one silently replace the
//! other.
//!
//! # What it refuses to invent
//!
//! **`as_at`.** Required, and never defaulted to now. The printed sheet that
//! prompted migration 86 was six days old and three items had moved in the
//! meantime; a number from a report that cannot say how old it is gets read as
//! current forever.
//!
//! **An item or a bin it has not seen.** A stock report naming an unknown code
//! is a gap in an earlier import, not a licence to create one here: inventing
//! an item from a quantity is inventing an attribute from a number. They are
//! counted and skipped, which is [`super::bins`]'s position on a bin with no
//! stated type.
//!
//! **A row count that disagrees with what was expected.** See [`shortfall`].

use std::collections::HashMap;

use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

/// One row of the inventory balance export, trimmed.
#[derive(Clone, Debug)]
pub struct Row {
    pub item_code: String,
    /// The warehouse. NetSuite calls this "Location"; here it is a `site`.
    pub warehouse: String,
    /// The shelf, empty when the report names a quantity without a position.
    pub bin_code: String,
    /// Carried as text and cast in the statement, because `on_hand` is
    /// `numeric` and this crate has no decimal dependency. The same convention
    /// `discrepancy` quantities already use.
    pub on_hand: String,
    pub available: Option<String>,
    pub status: Option<String>,
}

/// The column names this reader will answer to.
///
/// Several spellings each, because the export's headings depend on how the
/// saved search was built and the alternative is a reader that works on one
/// person's file. An unrecognised header is refused with the header echoed
/// back rather than guessed at — see [`read`].
const ITEM: &[&str] = &["Item", "Item Code", "Code"];
const WAREHOUSE: &[&str] = &["Location", "Warehouse", "Inventory Location"];
const BIN: &[&str] = &["Bin Number", "Bin"];
const ON_HAND: &[&str] = &["On Hand", "Quantity On Hand", "Quantity Available", "Quantity"];
const AVAILABLE: &[&str] = &["Available", "Quantity Available"];
const STATUS: &[&str] = &["Status", "Inventory Status"];

fn pick(header: &[String], names: &[&str]) -> Option<usize> {
    names.iter().find_map(|n| {
        header
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(n))
    })
}

/// Read the export from anything, so a path and a request body are the same.
///
/// **By header name, and it refuses a header it does not know.** The bin export
/// is read the same way and can be, because its columns are unambiguous. What
/// is different here is that this file has never been seen: rather than map a
/// guess onto `on_hand` and load a warehouse's worth of wrong numbers, an
/// unrecognised heading is an error naming every column it did find, so the
/// next run is a one-line change to the list above.
pub fn read<R: std::io::Read>(source: R) -> Result<Vec<Row>, String> {
    let mut rdr = csv::Reader::from_reader(source);
    let header: Vec<String> = rdr
        .headers()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|s| s.trim().to_string())
        .collect();

    let missing = |what: &str, names: &[&str]| {
        format!(
            "no {what} column: looked for {}, and the file has {}",
            names.join(", "),
            header.join(", ")
        )
    };
    let i_item = pick(&header, ITEM).ok_or_else(|| missing("item", ITEM))?;
    let i_house = pick(&header, WAREHOUSE).ok_or_else(|| missing("warehouse", WAREHOUSE))?;
    let i_hand = pick(&header, ON_HAND).ok_or_else(|| missing("on hand", ON_HAND))?;
    let i_bin = pick(&header, BIN);
    let i_avail = pick(&header, AVAILABLE).filter(|i| *i != i_hand);
    let i_status = pick(&header, STATUS);

    let mut rows = vec![];
    for rec in rdr.records() {
        let r = rec.map_err(|e| e.to_string())?;
        let at = |i: usize| r.get(i).map(str::trim).unwrap_or("").to_string();
        let item_code = at(i_item);
        let on_hand = at(i_hand);
        // A row naming no item, or no quantity, is a subtotal or a spacer. The
        // export has them and they are not stock.
        if item_code.is_empty() || on_hand.is_empty() {
            continue;
        }
        rows.push(Row {
            item_code,
            warehouse: at(i_house),
            bin_code: i_bin.map(at).unwrap_or_default(),
            on_hand,
            available: i_avail.map(at).filter(|s| !s.is_empty()),
            status: i_status.map(at).filter(|s| !s.is_empty()),
        });
    }
    Ok(rows)
}

/// What the file says, per warehouse, before any database is consulted.
#[derive(Debug, Serialize)]
pub struct WarehouseRows {
    pub warehouse: String,
    pub rows: usize,
    pub positioned: usize,
}

/// What the export contains, which is what a person reads before deciding.
#[derive(Debug, Serialize)]
pub struct StockSurvey {
    pub rows: usize,
    /// Rows naming a shelf. The rest name a quantity at a warehouse and no
    /// more, which `reported_stock.location_id` is nullable to hold.
    pub positioned: usize,
    /// Rows reporting nothing on hand. Kept rather than dropped: "the report
    /// says this shelf is empty" is a different statement from silence.
    pub empty: usize,
    pub warehouses: Vec<WarehouseRows>,
}

pub fn survey(rows: &[Row]) -> StockSurvey {
    let mut per: HashMap<&str, (usize, usize)> = HashMap::new();
    for r in rows {
        let e = per.entry(r.warehouse.as_str()).or_default();
        e.0 += 1;
        if !r.bin_code.is_empty() {
            e.1 += 1;
        }
    }
    let mut warehouses: Vec<WarehouseRows> = per
        .into_iter()
        .map(|(warehouse, (rows, positioned))| WarehouseRows {
            warehouse: warehouse.to_string(),
            rows,
            positioned,
        })
        .collect();
    warehouses.sort_by(|a, b| a.warehouse.cmp(&b.warehouse));

    StockSurvey {
        rows: rows.len(),
        positioned: rows.iter().filter(|r| !r.bin_code.is_empty()).count(),
        empty: rows
            .iter()
            .filter(|r| r.on_hand.parse::<f64>().map(|v| v == 0.0).unwrap_or(false))
            .count(),
        warehouses,
    }
}

/// Whether the file is the size it was said to be.
///
/// **The one gate that costs nothing and catches the failure that reads as
/// success.** A scheduled saved-search email truncates at ten thousand rows,
/// and an on-hand export for a site of eight thousand bins sits close enough to
/// that to cross it without anybody noticing. A bin missing from a truncated
/// export does not read as missing, it reads as *empty*, and an empty bin is an
/// instruction to put something there.
///
/// The export cannot state its own length, so the count comes from whoever ran
/// the search and read it off the screen. Absent means unchecked, and the
/// survey says so rather than implying a check happened.
pub fn shortfall(rows: usize, expect: Option<usize>) -> Option<String> {
    match expect {
        Some(n) if n != rows => Some(format!(
            "the file has {rows} rows and was said to have {n}: \
             {}. Nothing was loaded — a partial inventory reads as empty shelves.",
            if rows < n {
                "the export was truncated, or the search moved"
            } else {
                "this is not the export that was counted"
            }
        )),
        _ => None,
    }
}

/// What the database did, or would have done.
#[derive(Debug, Default, Serialize)]
pub struct StockLoaded {
    pub rows_written: usize,
    /// Rows the previous load of this source left behind, now cleared.
    pub rows_replaced: usize,
    /// Codes the item master does not have. A gap in that import, not this one.
    pub items_unknown: usize,
    /// Bins the bin list does not have, including the ones it left out for
    /// stating no type.
    pub bins_unknown: usize,
    pub warehouses_unknown: usize,
    /// False when the writes were rolled back.
    pub applied: bool,
}

/// Write the report, or find out what writing it would do.
///
/// `source` names the feed, not the file: a second export under the same source
/// replaces the first, because a snapshot is not cumulative. `as_at` is when the
/// report was taken and has no default.
pub async fn load(
    tx: &Transaction<'_>,
    tenant: Uuid,
    rows: &[Row],
    as_at: chrono::DateTime<chrono::Utc>,
    source: &str,
    apply: bool,
) -> Result<StockLoaded, String> {
    tx.batch_execute("SAVEPOINT spork_stock")
        .await
        .map_err(|e| e.to_string())?;

    let mut out = StockLoaded { applied: apply, ..Default::default() };

    // **Cleared before anything is written, and inside the savepoint**, so a
    // dry run reports the replacement without performing it.
    out.rows_replaced = tx
        .execute(
            "DELETE FROM reported_stock WHERE tenant_id = $1 AND source = $2",
            &[&tenant, &source],
        )
        .await
        .map_err(|e| format!("clearing the previous {source}: {e}"))? as usize;

    let mut sites: HashMap<String, Option<Uuid>> = HashMap::new();
    let mut items: HashMap<String, Option<Uuid>> = HashMap::new();
    let mut bins: HashMap<(Uuid, String), Option<Uuid>> = HashMap::new();

    for r in rows {
        // The same matching `bins::load` does, so a warehouse named one way in
        // one export and another way in the next is still one site.
        let site = match sites.get(&r.warehouse) {
            Some(s) => *s,
            None => {
                let name = crate::bins::site_name(&r.warehouse);
                let code = crate::bins::site_code(&r.warehouse);
                let found: Option<Uuid> = tx
                    .query_opt(
                        "SELECT id FROM site WHERE tenant_id = $1 AND (code = $2 OR name = $3)",
                        &[&tenant, &code, &name],
                    )
                    .await
                    .map_err(|e| e.to_string())?
                    .map(|row| row.get(0));
                sites.insert(r.warehouse.clone(), found);
                found
            }
        };
        let Some(site_id) = site else {
            out.warehouses_unknown += 1;
            continue;
        };

        let item = match items.get(&r.item_code) {
            Some(i) => *i,
            None => {
                let found: Option<Uuid> = tx
                    .query_opt(
                        "SELECT id FROM item WHERE tenant_id = $1 AND code = $2",
                        &[&tenant, &r.item_code],
                    )
                    .await
                    .map_err(|e| e.to_string())?
                    .map(|row| row.get(0));
                items.insert(r.item_code.clone(), found);
                found
            }
        };
        let Some(item_id) = item else {
            out.items_unknown += 1;
            continue;
        };

        let mut location_id: Option<Uuid> = None;
        if !r.bin_code.is_empty() {
            let key = (site_id, r.bin_code.clone());
            let found = match bins.get(&key) {
                Some(b) => *b,
                None => {
                    let f: Option<Uuid> = tx
                        .query_opt(
                            "SELECT id FROM location
                              WHERE tenant_id = $1 AND site_id = $2 AND code = $3",
                            &[&tenant, &site_id, &r.bin_code],
                        )
                        .await
                        .map_err(|e| e.to_string())?
                        .map(|row| row.get(0));
                    bins.insert(key, f);
                    f
                }
            };
            match found {
                Some(id) => location_id = Some(id),
                None => {
                    out.bins_unknown += 1;
                    continue;
                }
            }
        }

        // `$n::text::numeric` because `on_hand` is numeric and the quantity
        // arrives as text.
        //
        // **Two conflict targets, because migration 86 built two partial unique
        // indexes**: one keyed on the shelf and one for rows that name none.
        // A single `ON CONFLICT` naming the first infers nothing for a row whose
        // `location_id` is NULL, so a file listing one item twice at a warehouse
        // with no bin would raise a duplicate key rather than replace. The
        // statements are otherwise identical and differ only in what they match.
        const SET: &str = "DO UPDATE SET on_hand = EXCLUDED.on_hand,
                                         available = EXCLUDED.available,
                                         status = EXCLUDED.status,
                                         as_at = EXCLUDED.as_at,
                                         loaded_at = now()";
        let sql = format!(
            "INSERT INTO reported_stock
                 (tenant_id, site_id, item_id, location_id,
                  on_hand, available, status, as_at, source)
             VALUES ($1, $2, $3, $4, $5::text::numeric, $6::text::numeric, $7, $8, $9)
             ON CONFLICT {target} {SET}",
            target = if location_id.is_some() {
                "(tenant_id, site_id, item_id, location_id, source) WHERE location_id IS NOT NULL"
            } else {
                "(tenant_id, site_id, item_id, source) WHERE location_id IS NULL"
            },
        );
        tx.execute(
            &sql,
            &[
                &tenant,
                &site_id,
                &item_id,
                &location_id,
                &r.on_hand,
                &r.available,
                &r.status,
                &as_at,
                &source,
            ],
        )
        .await
        .map_err(|e| format!("{} at {}: {e}", r.item_code, r.bin_code))?;
        out.rows_written += 1;
    }

    if apply {
        tx.batch_execute("RELEASE SAVEPOINT spork_stock")
            .await
            .map_err(|e| e.to_string())?;
    } else {
        tx.batch_execute("ROLLBACK TO SAVEPOINT spork_stock")
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "Item,Location,Bin Number,On Hand,Available,Status\n";

    #[test]
    fn a_header_it_does_not_know_is_refused_with_the_header_in_the_message() {
        // The export has never been seen here. Mapping a guess onto `on_hand`
        // loads a warehouse's worth of wrong numbers, so the refusal names both
        // what was looked for and what the file actually has.
        let e = read("Widget,Qty\nA,1\n".as_bytes()).unwrap_err();
        assert!(e.contains("no item column"), "{e}");
        assert!(e.contains("Widget, Qty"), "it echoes the header it was given: {e}");
    }

    #[test]
    fn a_row_with_no_item_or_no_quantity_is_a_subtotal_and_not_stock() {
        let csv = format!("{HEADER}GLOVE-M,Melbourne,A-01-1,4,4,Good\n,,,,,\nGLOVE-M,Melbourne,A-01-1,,,\n");
        let rows = read(csv.as_bytes()).unwrap();
        assert_eq!(rows.len(), 1, "the blank and the quantityless row are not stock");
    }

    #[test]
    fn a_quantity_is_carried_as_text_all_the_way_to_the_statement() {
        // `on_hand` is numeric and this crate has no decimal dependency, so a
        // fractional quantity must survive as the characters it arrived as
        // rather than through a float.
        let csv = format!("{HEADER}GLOVE-M,Melbourne,A-01-1,1.005,,Good\n");
        let rows = read(csv.as_bytes()).unwrap();
        assert_eq!(rows[0].on_hand, "1.005", "no parse, no rounding");
    }

    #[test]
    fn the_available_column_is_not_taken_from_the_on_hand_column() {
        // "Quantity Available" appears in both alias lists, deliberately: some
        // exports have only that column and it is the on-hand figure. When it
        // has been claimed as on hand it must not also be read as available,
        // or a file with one quantity column reports it twice.
        let rows = read("Item,Location,Quantity Available\nGLOVE-M,Melbourne,6\n".as_bytes())
            .unwrap();
        assert_eq!(rows[0].on_hand, "6");
        assert_eq!(rows[0].available, None, "one column cannot be two facts");
    }

    #[test]
    fn an_unchecked_count_is_not_a_passed_check() {
        assert_eq!(shortfall(10, None), None, "absent means nobody counted");
        assert_eq!(shortfall(10, Some(10)), None);
    }

    #[test]
    fn a_count_that_disagrees_says_which_way_it_disagrees() {
        let short = shortfall(9_998, Some(10_000)).expect("short is refused");
        assert!(short.contains("truncated"), "{short}");
        let over = shortfall(12, Some(10)).expect("a longer file is refused too");
        assert!(
            over.contains("not the export that was counted"),
            "more rows than expected is a different file, not a truncation: {over}"
        );
    }
}
