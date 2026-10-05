//! What NetSuite says each item is sold in, and its supplier's part number
//! (D217), into `reported_item`.
//!
//! The capture sheet prints both beside each item, and both are NetSuite's to
//! change, so they arrive as the balance does (migration 86): a report with its
//! age and its feed, the whole of it each time. A load replaces what the same
//! feed said before, and an item it no longer names is no longer said.
//!
//! Read by header name, and every column but the item's is optional, so a
//! file with only units is a file with only units.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

const ITEM: &[&str] = &["Item", "Item Code", "Code", "Name"];
const UNIT: &[&str] = &["Unit", "Sale Unit", "Selling Unit", "Units"];
const SUPPLIER_PART: &[&str] = &[
    "Supplier Part No.",
    "Supplier Part",
    "Vendor Name",
    "Vendor Code",
    "Vendor Part",
];

fn pick(header: &[String], names: &[&str]) -> Option<usize> {
    names
        .iter()
        .find_map(|n| header.iter().position(|h| h.trim().eq_ignore_ascii_case(n)))
}

/// One item's details, as the export states them. Blank is not said.
#[derive(Clone, Debug)]
pub struct Row {
    pub item_code: String,
    pub selling_unit: Option<String>,
    pub supplier_part: Option<String>,
}

/// Read the export from anything, so a path and a request body are the same.
pub fn read<R: std::io::Read>(source: R) -> Result<Vec<Row>, String> {
    let mut rdr = csv::Reader::from_reader(source);
    let header: Vec<String> = rdr
        .headers()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|s| s.trim().to_string())
        .collect();
    let i_item = pick(&header, ITEM).ok_or_else(|| {
        format!(
            "no item column: looked for {}, and the file has {}",
            ITEM.join(", "),
            header.join(", ")
        )
    })?;
    let i_unit = pick(&header, UNIT);
    let i_part = pick(&header, SUPPLIER_PART);
    if i_unit.is_none() && i_part.is_none() {
        return Err(format!(
            "no unit or supplier part column: looked for {} and {}, and the file has {}",
            UNIT.join(", "),
            SUPPLIER_PART.join(", "),
            header.join(", ")
        ));
    }
    let mut rows = vec![];
    for rec in rdr.records() {
        let r = rec.map_err(|e| e.to_string())?;
        let at = |i: usize| {
            r.get(i)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        let Some(item_code) = at(i_item) else {
            continue;
        };
        rows.push(Row {
            item_code,
            selling_unit: i_unit.and_then(at),
            supplier_part: i_part.and_then(at),
        });
    }
    Ok(rows)
}

/// What the file holds, before any database is asked.
#[derive(Serialize, Debug, Default)]
pub struct DetailsSurvey {
    pub rows: usize,
    pub with_unit: usize,
    pub with_supplier_part: usize,
}

pub fn survey(rows: &[Row]) -> DetailsSurvey {
    DetailsSurvey {
        rows: rows.len(),
        with_unit: rows.iter().filter(|r| r.selling_unit.is_some()).count(),
        with_supplier_part: rows.iter().filter(|r| r.supplier_part.is_some()).count(),
    }
}

#[derive(Serialize, Debug, Default)]
pub struct DetailsLoaded {
    pub applied: bool,
    /// Items said, one per code however many rows named it.
    pub rows_written: usize,
    /// Items the same feed said before and this load doesn't, cleared.
    pub rows_replaced: usize,
    /// Codes the item master does not have.
    pub items_unknown: usize,
}

/// Write the feed's whole word, replacing what it said before. Rolled back
/// unless `apply`, so the dry run is the load, undone.
pub async fn load(
    tx: &Transaction<'_>,
    tenant: Uuid,
    rows: &[Row],
    as_at: DateTime<Utc>,
    source: &str,
    apply: bool,
) -> Result<DetailsLoaded, String> {
    let fail = |e: tokio_postgres::Error| e.to_string();
    tx.batch_execute("SAVEPOINT spork_item_details")
        .await
        .map_err(fail)?;

    // The last row naming an item is its word.
    let mut said: HashMap<&str, &Row> = HashMap::new();
    for r in rows {
        said.insert(r.item_code.as_str(), r);
    }
    let codes: Vec<&str> = said.keys().copied().collect();
    let units: Vec<Option<&str>> = codes
        .iter()
        .map(|c| said[c].selling_unit.as_deref())
        .collect();
    let parts: Vec<Option<&str>> = codes
        .iter()
        .map(|c| said[c].supplier_part.as_deref())
        .collect();
    let r = tx
        .query_one(
            "WITH f AS (SELECT * FROM unnest($1::text[], $2::text[], $3::text[]) AS f(code, unit, part)),
                  m AS (SELECT i.id, f.unit, f.part FROM f JOIN item i ON i.code = f.code),
                  gone AS (DELETE FROM reported_item r
                            WHERE r.source = $5 AND NOT EXISTS (SELECT 1 FROM m WHERE m.id = r.item_id)
                            RETURNING 1),
                  up AS (INSERT INTO reported_item (tenant_id, item_id, selling_unit, supplier_part, as_at, source)
                         SELECT $4, id, unit, part, $6, $5 FROM m
                         ON CONFLICT (tenant_id, item_id, source) DO UPDATE
                            SET selling_unit = excluded.selling_unit, supplier_part = excluded.supplier_part,
                                as_at = excluded.as_at, loaded_at = now()
                         RETURNING 1)
             SELECT (SELECT count(*) FROM f), (SELECT count(*) FROM m), (SELECT count(*) FROM up),
                    (SELECT count(*) FROM gone)",
            &[&codes, &units, &parts, &tenant, &source, &as_at],
        )
        .await
        .map_err(fail)?;
    let (named, known, written, gone): (i64, i64, i64, i64) =
        (r.get(0), r.get(1), r.get(2), r.get(3));
    let out = DetailsLoaded {
        applied: apply,
        rows_written: written as usize,
        rows_replaced: gone as usize,
        items_unknown: (named - known) as usize,
    };
    let end = if apply {
        "RELEASE SAVEPOINT spork_item_details"
    } else {
        "ROLLBACK TO SAVEPOINT spork_item_details"
    };
    tx.batch_execute(end).await.map_err(fail)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_columns_are_found_by_name_and_blank_is_not_said() {
        let csv =
            "Item,Unit,Supplier Part No.\nJAR-8018,CTN,AC-8018\nGiveaways-Umbrella,,\n,Each,X\n";
        let rows = read(csv.as_bytes()).unwrap();
        assert_eq!(rows.len(), 2, "a row with no item is skipped");
        assert_eq!(rows[0].selling_unit.as_deref(), Some("CTN"));
        assert_eq!(rows[0].supplier_part.as_deref(), Some("AC-8018"));
        assert_eq!(
            (
                rows[1].selling_unit.as_deref(),
                rows[1].supplier_part.as_deref()
            ),
            (None, None)
        );
        let s = survey(&rows);
        assert_eq!((s.rows, s.with_unit, s.with_supplier_part), (2, 1, 1));
    }

    #[test]
    fn netsuites_own_names_are_read_and_a_file_with_neither_is_refused() {
        let rows = read("Name,Sale Unit,Vendor Name\nU.KTS,Roll,KTS\n".as_bytes()).unwrap();
        assert_eq!(rows[0].selling_unit.as_deref(), Some("Roll"));
        assert_eq!(rows[0].supplier_part.as_deref(), Some("KTS"));
        assert!(read("Item,Description\nA,B\n".as_bytes()).is_err());
    }
}
