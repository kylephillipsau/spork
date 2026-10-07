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
//! **And what NetSuite says the item looks like, weighs and measures** (D237):
//! the file of its picture, its weight and size in whatever units it holds
//! them, and its UPC. Each is NetSuite's word beside Spork's own, never in its
//! place: the comparison is `item_netsuite_differs`.

use std::collections::HashMap;

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
    "Vendor Code",
    "Vendor Name",
    "Vendor Part",
];

const PICTURE: &[&str] = &["Picture", "Image", "Picture File", "Image File"];
const WEIGHT: &[&str] = &["Weight"];
const WEIGHT_UNIT: &[&str] = &["Weight Unit", "Weight Units"];
const LENGTH: &[&str] = &["Length"];
const WIDTH: &[&str] = &["Width"];
const HEIGHT: &[&str] = &["Height"];
const SIZE_UNIT: &[&str] = &["Dimension Unit", "Size Unit", "Dimensions Unit"];
const UPC: &[&str] = &["UPC", "UPC Code", "Barcode"];

/// Grams in one of a weight unit, as NetSuite names it.
fn grams(unit: &str) -> Option<f64> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "g" | "gram" | "grams" => Some(1.0),
        "kg" | "kgs" | "kilogram" | "kilograms" => Some(1000.0),
        "lb" | "lbs" | "pound" | "pounds" => Some(453.592_37),
        "oz" | "ounce" | "ounces" => Some(28.349_523_125),
        _ => None,
    }
}

/// Millimetres in one of a length unit, as NetSuite names it.
fn millimetres(unit: &str) -> Option<f64> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "mm" | "millimetre" | "millimetres" | "millimeter" | "millimeters" => Some(1.0),
        "cm" | "centimetre" | "centimetres" | "centimeter" | "centimeters" => Some(10.0),
        "m" | "metre" | "metres" | "meter" | "meters" => Some(1000.0),
        "in" | "inch" | "inches" => Some(25.4),
        "ft" | "foot" | "feet" => Some(304.8),
        _ => None,
    }
}

/// A figure in its unit as a whole number of the base one: none where either
/// is blank, unreadable, or not more than nothing.
fn figure(value: Option<&str>, unit: Option<f64>) -> Option<i32> {
    let v: f64 = value?.trim().replace(',', "").parse().ok()?;
    let n = (v * unit?).round();
    (n >= 1.0 && n <= f64::from(i32::MAX)).then_some(n as i32)
}

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
    /// NetSuite's reference to its picture's file (D237).
    pub picture_file: Option<String>,
    /// NetSuite's weight and size, in grams and millimetres. Absent where it
    /// is blank, or its unit is one this doesn't know, which `survey` counts.
    pub weight_g: Option<i32>,
    pub length_mm: Option<i32>,
    pub width_mm: Option<i32>,
    pub height_mm: Option<i32>,
    pub upc: Option<String>,
    /// A weight or size given in a unit this doesn't know.
    pub unreadable: bool,
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
    let i_picture = pick(&header, PICTURE);
    let i_weight = pick(&header, WEIGHT);
    let i_weight_unit = pick(&header, WEIGHT_UNIT);
    let (i_length, i_width, i_height) = (pick(&header, LENGTH), pick(&header, WIDTH), pick(&header, HEIGHT));
    let i_size_unit = pick(&header, SIZE_UNIT);
    let i_upc = pick(&header, UPC);
    if [i_unit, i_part, i_picture, i_weight, i_length, i_upc].iter().all(Option::is_none) {
        return Err(format!(
            "no unit, supplier part, picture, weight, size or UPC column: looked for {}, {} and the rest, \
             and the file has {}",
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
        let text = |i: Option<usize>| i.and_then(at);
        let weighed = text(i_weight);
        let in_g = text(i_weight_unit).and_then(|u| grams(&u));
        let sides = [text(i_length), text(i_width), text(i_height)];
        let in_mm = text(i_size_unit).and_then(|u| millimetres(&u));
        let said = |v: &Option<String>, unit: Option<f64>| figure(v.as_deref(), unit);
        rows.push(Row {
            item_code,
            selling_unit: text(i_unit),
            supplier_part: text(i_part),
            picture_file: text(i_picture),
            weight_g: said(&weighed, in_g),
            length_mm: said(&sides[0], in_mm),
            width_mm: said(&sides[1], in_mm),
            height_mm: said(&sides[2], in_mm),
            upc: text(i_upc),
            unreadable: (weighed.is_some() && in_g.is_none()) || (sides.iter().any(Option::is_some) && in_mm.is_none()),
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
    pub with_picture: usize,
    pub with_weight: usize,
    pub with_size: usize,
    pub with_upc: usize,
    /// Rows whose weight or size came in a unit this doesn't know, so wasn't read.
    pub unreadable: usize,
}

pub fn survey(rows: &[Row]) -> DetailsSurvey {
    let count = |f: fn(&Row) -> bool| rows.iter().filter(|r| f(r)).count();
    DetailsSurvey {
        rows: rows.len(),
        with_unit: count(|r| r.selling_unit.is_some()),
        with_supplier_part: count(|r| r.supplier_part.is_some()),
        with_picture: count(|r| r.picture_file.is_some()),
        with_weight: count(|r| r.weight_g.is_some()),
        with_size: count(|r| r.length_mm.is_some() && r.width_mm.is_some() && r.height_mm.is_some()),
        with_upc: count(|r| r.upc.is_some()),
        unreadable: count(|r| r.unreadable),
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
    let col = |f: fn(&Row) -> Option<&str>| codes.iter().map(|c| f(said[c])).collect::<Vec<_>>();
    let num = |f: fn(&Row) -> Option<i32>| codes.iter().map(|c| f(said[c])).collect::<Vec<_>>();
    let (pictures, upcs) = (col(|r| r.picture_file.as_deref()), col(|r| r.upc.as_deref()));
    let (weights, lengths, widths, heights) =
        (num(|r| r.weight_g), num(|r| r.length_mm), num(|r| r.width_mm), num(|r| r.height_mm));
    let r = tx
        .query_one(
            "WITH f AS (SELECT * FROM unnest($1::text[], $2::text[], $3::text[], $7::text[], $8::int[],
                                             $9::int[], $10::int[], $11::int[], $12::text[])
                                   AS f(code, unit, part, picture, weight, length, width, height, upc)),
                  m AS (SELECT i.id, f.* FROM f JOIN item i ON i.code = f.code),
                  gone AS (DELETE FROM reported_item r
                            WHERE r.source = $5 AND NOT EXISTS (SELECT 1 FROM m WHERE m.id = r.item_id)
                            RETURNING 1),
                  up AS (INSERT INTO reported_item
                             (tenant_id, item_id, selling_unit, supplier_part, picture_file, weight_g,
                              length_mm, width_mm, height_mm, upc, as_at, source)
                         SELECT $4, id, unit, part, picture, weight, length, width, height, upc, $6, $5 FROM m
                         ON CONFLICT (tenant_id, item_id, source) DO UPDATE
                            SET selling_unit = excluded.selling_unit, supplier_part = excluded.supplier_part,
                                picture_file = excluded.picture_file, weight_g = excluded.weight_g,
                                length_mm = excluded.length_mm, width_mm = excluded.width_mm,
                                height_mm = excluded.height_mm, upc = excluded.upc,
                                as_at = excluded.as_at, loaded_at = now()
                         RETURNING 1)
             SELECT (SELECT count(*) FROM f), (SELECT count(*) FROM m), (SELECT count(*) FROM up),
                    (SELECT count(*) FROM gone)",
            &[&codes, &units, &parts, &tenant, &source, &as_at, &pictures, &weights, &lengths, &widths, &heights, &upcs],
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
        let rows =
            read("Name,Pack Unit,Alternative Code\nPBL-9558B,Roll,PBL-9558B\n".as_bytes()).unwrap();
        assert_eq!(rows[0].selling_unit.as_deref(), Some("Roll"));
        assert_eq!(rows[0].supplier_part.as_deref(), Some("PBL-9558B"));
        let rows = read("Name,Sale Unit,Vendor Name\nU.KTS,Roll,KTS\n".as_bytes()).unwrap();
        assert_eq!(rows[0].selling_unit.as_deref(), Some("Roll"));
        assert_eq!(rows[0].supplier_part.as_deref(), Some("KTS"));
        assert!(read("Item,Description\nA,B\n".as_bytes()).is_err());
    }

    #[test]
    fn netsuites_picture_weight_size_and_upc_are_read_in_its_units() {
        let csv = "Item,Picture,Weight,Weight Unit,Length,Width,Height,Dimension Unit,UPC\n\
                   ABC-1,4471,1.5,lb,12,8,4,in,9312345678903\n\
                   ABC-2,,250,g,,,,,\n\
                   ABC-3,,2,stone,30,20,10,cm,\n";
        let rows = read(csv.as_bytes()).unwrap();
        assert_eq!(rows[0].picture_file.as_deref(), Some("4471"));
        assert_eq!(rows[0].weight_g, Some(680), "a pound and a half, in grams");
        assert_eq!((rows[0].length_mm, rows[0].width_mm, rows[0].height_mm), (Some(305), Some(203), Some(102)));
        assert_eq!(rows[0].upc.as_deref(), Some("9312345678903"));
        assert_eq!((rows[1].weight_g, rows[1].length_mm), (Some(250), None));
        assert_eq!((rows[2].weight_g, rows[2].length_mm, rows[2].unreadable), (None, Some(300), true), "a unit not known is not read");
        let s = survey(&rows);
        assert_eq!((s.with_picture, s.with_weight, s.with_size, s.with_upc, s.unreadable), (1, 2, 2, 1, 1));
    }
}
