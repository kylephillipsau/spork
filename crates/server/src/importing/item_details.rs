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
//!
//! **And every field as NetSuite said it** (D238): each column but the item's,
//! under the column's name, its text as it came, into `reported_item_field`.
//! A value NetSuite changes or stops saying is closed and the new one opened,
//! so what NetSuite said, and when, is kept. What a field means (a weight, a
//! barcode, a colour shown beside the code) is read at the other end, through
//! `reported_item_said`, and none of it is Spork's record of the product.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

const ITEM: &[&str] = &["Item", "Item Code", "Code", "Name"];
const UNIT: &[&str] = &["Unit", "Pack Unit", "Sale Unit", "Selling Unit", "Units"];
const SUPPLIER_PART: &[&str] = &[
    "Supplier Part No.",
    "Supplier Part",
    "Alternative Code",
    "Article No.",
    "Vendor Code",
    "Vendor Name",
    "Vendor Part",
];

fn pick(header: &[String], names: &[&str]) -> Option<usize> {
    names
        .iter()
        .find_map(|n| header.iter().position(|h| h.trim().eq_ignore_ascii_case(n)))
}

/// One item's details, as the export states them. Blank is not said.
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub item_code: String,
    pub selling_unit: Option<String>,
    pub supplier_part: Option<String>,
    /// Every column but the item's, by its name, as it came; blank ones left
    /// out, being not said (D238).
    pub fields: Vec<(String, String)>,
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
    if header.len() < 2 {
        return Err(format!("nothing but the item: the file has {}", header.join(", ")));
    }
    let i_unit = pick(&header, UNIT);
    let i_part = pick(&header, SUPPLIER_PART);
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
        let fields = header
            .iter()
            .enumerate()
            .filter(|(i, name)| *i != i_item && !name.is_empty())
            .filter_map(|(i, name)| at(i).map(|v| (name.clone(), v)))
            .collect();
        rows.push(Row {
            item_code,
            selling_unit: i_unit.and_then(at),
            supplier_part: i_part.and_then(at),
            fields,
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
    /// How many rows say each field, by its name (D238).
    pub fields: BTreeMap<String, usize>,
}

pub fn survey(rows: &[Row]) -> DetailsSurvey {
    let mut fields = BTreeMap::new();
    for r in rows {
        for (name, _) in &r.fields {
            *fields.entry(name.clone()).or_insert(0) += 1;
        }
    }
    DetailsSurvey {
        rows: rows.len(),
        with_unit: rows.iter().filter(|r| r.selling_unit.is_some()).count(),
        with_supplier_part: rows.iter().filter(|r| r.supplier_part.is_some()).count(),
        fields,
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
    /// Fields NetSuite says now that it didn't, or said otherwise (D238).
    pub fields_opened: usize,
    /// Fields it said before and now says otherwise, or nothing of.
    pub fields_closed: usize,
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

    // Every field as NetSuite said it (D238): what it says otherwise or no
    // longer says is closed first, then what it says now and didn't is
    // opened, so one value is open a field, an item, a feed.
    let (mut f_codes, mut f_names, mut f_values): (Vec<&str>, Vec<&str>, Vec<&str>) = (vec![], vec![], vec![]);
    for (code, row) in &said {
        for (name, value) in &row.fields {
            f_codes.push(code);
            f_names.push(name);
            f_values.push(value);
        }
    }
    const NOW: &str = "WITH f AS (SELECT i.id AS item_id, f.field, f.value
                         FROM unnest($1::text[], $2::text[], $3::text[]) AS f(code, field, value)
                         JOIN item i ON i.code = f.code)";
    let closed = tx
        .execute(
            &format!(
                "{NOW}
                 UPDATE reported_item_field r SET said_to = $5
                  WHERE r.source = $4 AND r.said_to IS NULL
                    AND NOT EXISTS (SELECT 1 FROM f
                                     WHERE f.item_id = r.item_id AND f.field = r.field AND f.value = r.value)"
            ),
            &[&f_codes, &f_names, &f_values, &source, &as_at],
        )
        .await
        .map_err(fail)?;
    let opened = tx
        .execute(
            &format!(
                "{NOW}
                 INSERT INTO reported_item_field (tenant_id, item_id, source, field, value, said_from)
                 SELECT $6, f.item_id, $4, f.field, f.value, $5 FROM f
                  WHERE NOT EXISTS (SELECT 1 FROM reported_item_field r
                                     WHERE r.item_id = f.item_id AND r.source = $4 AND r.field = f.field
                                       AND r.said_to IS NULL)"
            ),
            &[&f_codes, &f_names, &f_values, &source, &as_at, &tenant],
        )
        .await
        .map_err(fail)?;

    let out = DetailsLoaded {
        applied: apply,
        rows_written: written as usize,
        rows_replaced: gone as usize,
        items_unknown: (named - known) as usize,
        fields_opened: opened as usize,
        fields_closed: closed as usize,
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
    fn netsuites_own_names_are_read_and_a_file_of_only_items_is_refused() {
        let rows =
            read("Name,Pack Unit,Alternative Code\nPBL-9558B,Roll,PBL-9558B\n".as_bytes()).unwrap();
        assert_eq!(rows[0].selling_unit.as_deref(), Some("Roll"));
        assert_eq!(rows[0].supplier_part.as_deref(), Some("PBL-9558B"));
        let rows = read("Name,Sale Unit,Vendor Name\nU.KTS,Roll,KTS\n".as_bytes()).unwrap();
        assert_eq!(rows[0].selling_unit.as_deref(), Some("Roll"));
        assert_eq!(rows[0].supplier_part.as_deref(), Some("KTS"));
        assert!(read("Item\nA\n".as_bytes()).is_err());
    }

    #[test]
    fn every_field_is_kept_as_netsuite_said_it() {
        let csv = "Item,Colour,Item Weight,Weight Unit,Length (cm),Alert\n\
                   ABC-1,Blue,1.5,lb,12, Charge bulky freight \n\
                   ABC-2,,0.25,kg,,\n";
        let rows = read(csv.as_bytes()).unwrap();
        assert_eq!(
            rows[0].fields,
            [
                ("Colour".into(), "Blue".into()),
                ("Item Weight".into(), "1.5".into()),
                ("Weight Unit".into(), "lb".into()),
                ("Length (cm)".into(), "12".into()),
                ("Alert".into(), "Charge bulky freight".into()),
            ],
            "its text as it came, trimmed, under its own name"
        );
        assert_eq!(rows[1].fields.len(), 2, "a blank is not said");
        let s = survey(&rows);
        assert_eq!((s.fields["Item Weight"], s.fields["Colour"]), (2, 1));
    }
}
