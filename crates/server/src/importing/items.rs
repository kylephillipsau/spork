//! Loading the item master: a code, a description, and nothing to decide.
//!
//! The counterpart to [`super::bins`] and much the smaller of the two. A bin
//! list carries judgement — which warehouse is ours, what a code decomposes
//! into, which of two orderings is the walk — and this carries none. Every row
//! is a code and the words beside it.
//!
//! # What it does not do
//!
//! **It does not touch styles, packing configurations or measurements.** The
//! prepack list does those, it is 186 rows against seven thousand, and it makes
//! judgements this file cannot: which rows are styles, what a stated weight
//! means, which of two disagreeing records wins. Loading the master is what
//! gives capture something to measure; the prepack list is a separate import
//! and a separate decision.
//!
//! **It does not update a description.** `ON CONFLICT DO NOTHING`, because a
//! description already on file may have been corrected by somebody here and an
//! import is not evidence that it was wrong. A second run therefore reports
//! everything as already present, which is the honest answer.
//!
//! Every item is created `tracking = 'none'` in the base unit `ea`. Neither is
//! a guess: the export states no tracking regime, and D23's unit vocabulary
//! holds `ea` and nothing above it — a carton is a packaging level rather than
//! a unit, which is question 136.

use std::collections::HashMap;

use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

/// One row of the item export, trimmed.
#[derive(Clone, Debug)]
pub struct Row {
    pub code: String,
    pub description: String,
}

/// Read the export from anything, so a path and a request body are the same.
pub fn read<R: std::io::Read>(source: R) -> Result<Vec<Row>, String> {
    let mut rdr = csv::Reader::from_reader(source);
    let mut rows = vec![];
    for rec in rdr.deserialize::<HashMap<String, String>>() {
        let r = rec.map_err(|e| e.to_string())?;
        let code = r.get("Code").map(|s| s.trim()).unwrap_or("").to_string();
        if code.is_empty() {
            continue;
        }
        // **The code stands in for a missing description** rather than an empty
        // string: `item.description` is NOT NULL, and a blank one makes a
        // worklist row that names nothing.
        let description = r
            .get("Description")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or(&code)
            .to_string();
        rows.push(Row { code, description });
    }
    Ok(rows)
}

/// What the file says. Pure: no database is consulted.
#[derive(Debug, Serialize)]
pub struct ItemSurvey {
    pub items: usize,
    /// Rows whose description was blank and fell back to the code.
    pub unnamed: usize,
    /// Codes appearing more than once. The first wins; the rest are no-ops.
    pub duplicated: usize,
}

pub fn survey(rows: &[Row]) -> ItemSurvey {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for r in rows {
        *seen.entry(r.code.as_str()).or_default() += 1;
    }
    ItemSurvey {
        items: rows.len(),
        unnamed: rows.iter().filter(|r| r.description == r.code).count(),
        duplicated: seen.values().filter(|n| **n > 1).count(),
    }
}

/// What the database did, or would have done.
#[derive(Debug, Default, Serialize)]
pub struct ItemsLoaded {
    pub items_created: usize,
    /// Already on file, left exactly as they were.
    pub items_present: usize,
    pub applied: bool,
}

/// Write the item master, or find out what writing it would do.
///
/// Same bargain as the bin loader: the writes happen and are rolled back to a
/// savepoint when `apply` is false, so the report is what applying does rather
/// than a second implementation guessing at it.
pub async fn load(
    tx: &Transaction<'_>,
    tenant: Uuid,
    rows: &[Row],
    apply: bool,
) -> Result<ItemsLoaded, String> {
    tx.batch_execute("SAVEPOINT spork_import")
        .await
        .map_err(|e| e.to_string())?;

    let mut out = ItemsLoaded { applied: apply, ..Default::default() };

    for r in rows {
        // `SELECT … FROM unit` rather than a second round trip for the unit id:
        // it is the same value for all nine thousand rows and the database can
        // look it up faster than the network can carry it.
        let n = tx
            .execute(
                "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
                 SELECT $1, $2, $3, u.id, 'none' FROM unit u WHERE u.code = 'ea'
                 ON CONFLICT (tenant_id, code) DO NOTHING",
                &[&tenant, &r.code, &r.description],
            )
            .await
            .map_err(|e| format!("item {}: {e}", r.code))?;
        if n == 1 {
            out.items_created += 1;
        } else {
            out.items_present += 1;
        }
    }

    if apply {
        tx.batch_execute("RELEASE SAVEPOINT spork_import")
            .await
            .map_err(|e| e.to_string())?;
    } else {
        tx.batch_execute("ROLLBACK TO SAVEPOINT spork_import")
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(out)
}
