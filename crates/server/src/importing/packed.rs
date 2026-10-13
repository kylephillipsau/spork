//! Who packed each order, as NetSuite says (D254), landing in `reported_packing`.
//!
//! The item fulfilments NetSuite has packed at a warehouse, one row each, with
//! its Packed By: the name the fast fulfilment script set as the packer marked
//! it Packed. The Bridge sends the last five weeks every few minutes.
//!
//! # Kept, not replaced
//!
//! The balance and the open orders are replaced by each load, because each is
//! the whole of what is true now. This is a record of what happened, so a row
//! is kept once seen and only its Packed By follows NetSuite. Spork cannot
//! learn from NetSuite when a fulfilment was packed, so it keeps when it first
//! knew: see migration 137 for the three ways a time is had.

use std::collections::HashMap;

use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

/// One packed item fulfilment, as sent.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// As a person quotes it: IF272977.
    pub fulfilment: String,
    /// NetSuite's id for it.
    pub fulfilment_id: String,
    /// Day first, as NetSuite writes it: `7/10/2026`.
    pub date: Option<String>,
    pub packed_by: Option<String>,
    /// The warehouse, as NetSuite names it.
    pub location: String,
}

/// The columns, by the names the Bridge writes.
const FULFILMENT: &str = "Fulfilment";
const FULFILMENT_ID: &str = "Fulfilment ID";
const DATE: &str = "Date";
const PACKED_BY: &str = "Packed By";
const LOCATION: &str = "Location";

/// Read the report from anything. By header name; a column it needs and can't
/// find is an error naming the columns it did find.
pub fn read<R: std::io::Read>(source: R) -> Result<Vec<Row>, String> {
    let mut rdr = csv::Reader::from_reader(source);
    let header: Vec<String> = rdr.headers().map_err(|e| e.to_string())?.iter().map(|s| s.trim().to_string()).collect();
    let find = |name: &str| header.iter().position(|h| h.eq_ignore_ascii_case(name));
    let need = |name: &str| find(name).ok_or_else(|| format!("no {name} column: the file has {}", header.join(", ")));
    let (i_fulfilment, i_id, i_packed_by, i_location) = (need(FULFILMENT)?, need(FULFILMENT_ID)?, need(PACKED_BY)?, need(LOCATION)?);
    let i_date = find(DATE);

    let mut rows = vec![];
    for rec in rdr.records() {
        let r = rec.map_err(|e| e.to_string())?;
        let at = |i: usize| r.get(i).map(str::trim).unwrap_or("").to_string();
        let opt = |i: usize| Some(at(i)).filter(|s| !s.is_empty());
        let row = Row {
            fulfilment: at(i_fulfilment),
            fulfilment_id: at(i_id),
            date: i_date.and_then(opt),
            packed_by: opt(i_packed_by),
            location: at(i_location),
        };
        // A row with no fulfilment is a spacer.
        if row.fulfilment.is_empty() || row.fulfilment_id.is_empty() {
            continue;
        }
        rows.push(row);
    }
    Ok(rows)
}

/// What the database did, or would have done.
#[derive(Debug, Default, Serialize)]
pub struct PackedLoaded {
    /// Fulfilments not seen before.
    pub fulfilments_new: usize,
    /// Fulfilments already kept, their Packed By read again.
    pub fulfilments_known: usize,
    /// Left out: their warehouse is no site here.
    pub warehouses_unknown: usize,
    /// Left out: the same fulfilment sent twice in one load.
    pub fulfilments_repeated: usize,
    pub applied: bool,
}

/// Write the report, or find out what writing it would do. `source` names the
/// feed: its first load is the one that dates rows by the fulfilment's date.
pub async fn load(
    tx: &Transaction<'_>,
    tenant: Uuid,
    rows: &[Row],
    as_at: chrono::DateTime<chrono::Utc>,
    source: &str,
    apply: bool,
) -> Result<PackedLoaded, String> {
    let mut out = PackedLoaded { applied: apply, ..Default::default() };

    let mut sites: HashMap<String, Option<Uuid>> = HashMap::new();
    let (mut numbers, mut ids, mut dates, mut packers, mut site_ids) = (vec![], vec![], vec![], vec![], vec![]);
    let mut seen = std::collections::HashSet::new();
    for r in rows {
        let site = match sites.get(&r.location) {
            Some(s) => *s,
            None => {
                // The matching every NetSuite feed uses, so one warehouse is one site.
                let found: Option<Uuid> = tx
                    .query_opt(
                        "SELECT id FROM site WHERE tenant_id = $1 AND (code = $2 OR name = $3)",
                        &[&tenant, &crate::bins::site_code(&r.location), &crate::bins::site_name(&r.location)],
                    )
                    .await
                    .map_err(|e| e.to_string())?
                    .map(|row| row.get(0));
                sites.insert(r.location.clone(), found);
                found
            }
        };
        let Some(site_id) = site else {
            out.warehouses_unknown += 1;
            continue;
        };
        if !seen.insert(r.fulfilment_id.clone()) {
            out.fulfilments_repeated += 1;
            continue;
        }
        numbers.push(r.fulfilment.clone());
        ids.push(r.fulfilment_id.clone());
        dates.push(r.date.as_deref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%d/%m/%Y").ok()));
        packers.push(r.packed_by.clone());
        site_ids.push(site_id);
    }

    tx.batch_execute("SAVEPOINT spork_packed").await.map_err(|e| e.to_string())?;
    // When it was packed is kept from the first time it was known: when Spork
    // saw NetSuite close it, or this load, or (the feed's first load, which
    // reports weeks gone) the start of the fulfilment's day at its site.
    let written = tx
        .query(
            "WITH sent AS (
                 SELECT * FROM unnest($2::text[], $3::text[], $4::date[], $5::text[], $6::uuid[])
                     AS s(number, external_id, dated, packed_by, site_id)
             ), first AS (
                 SELECT NOT EXISTS (SELECT 1 FROM reported_packing WHERE tenant_id = $1 AND source = $7) AS is_first
             )
             INSERT INTO reported_packing
                 (tenant_id, site_id, fulfilment_number, fulfilment_external_id, packed_by, packed_at, as_at, source)
             SELECT $1, s.site_id, s.number, s.external_id, s.packed_by,
                    coalesce(
                        (SELECT min(f.closed_elsewhere_at) FROM fulfilment f
                          WHERE f.tenant_id = $1 AND f.external_id = s.external_id),
                        CASE WHEN first.is_first AND s.dated IS NOT NULL
                             THEN least($8, s.dated::timestamp AT TIME ZONE st.timezone)
                             ELSE $8 END),
                    $8, $7
               FROM sent s
               CROSS JOIN first
               JOIN site st ON st.id = s.site_id
             ON CONFLICT ON CONSTRAINT reported_packing_once DO UPDATE
                SET packed_by = EXCLUDED.packed_by,
                    fulfilment_number = EXCLUDED.fulfilment_number,
                    site_id = EXCLUDED.site_id,
                    as_at = EXCLUDED.as_at
             RETURNING (xmax = 0)",
            &[&tenant, &numbers, &ids, &dates, &packers, &site_ids, &source, &as_at],
        )
        .await
        .map_err(|e| format!("writing what was packed: {e}"))?;
    out.fulfilments_new = written.iter().filter(|r| r.get::<_, bool>(0)).count();
    out.fulfilments_known = written.len() - out.fulfilments_new;

    let end = if apply { "RELEASE SAVEPOINT spork_packed" } else { "ROLLBACK TO SAVEPOINT spork_packed" };
    tx.batch_execute(end).await.map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reads_the_bridge_s_columns() {
        let rows = read(
            "Fulfilment,Fulfilment ID,Date,Packed By,Location\n\
             IF100001,501,7/10/2026,Sam Rivera,Melbourne Warehouse\n\
             IF100002,502,7/10/2026,,Melbourne Warehouse\n\
             ,,,,\n"
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(rows.len(), 2, "the spacer is no fulfilment");
        assert_eq!((rows[0].fulfilment.as_str(), rows[0].packed_by.as_deref()), ("IF100001", Some("Sam Rivera")));
        assert_eq!(rows[1].packed_by, None, "nobody named is nobody, not an empty name");
    }

    #[test]
    fn a_missing_column_is_named() {
        let err = read("Fulfilment,Fulfilment ID,Location\nIF1,1,X\n".as_bytes()).unwrap_err();
        assert!(err.contains("no Packed By column"), "{err}");
    }
}
