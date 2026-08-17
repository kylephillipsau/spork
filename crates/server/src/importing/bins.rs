//! Loading reference data somebody exported from the system of record.
//!
//! [`crate::bins`] decides what a bin code means and stays pure, because the
//! judgement is the difficulty. This is the other half: reading the file and
//! writing the rows, which needs a database and therefore cannot live there.
//!
//! # One definition, two callers
//!
//! The `import_bins` example and `POST /api/import/bins` both call [`load`].
//! That is the same arrangement as [`crate::packing::stage`] and for the same
//! reason: an importer that exists twice is an importer whose two copies
//! disagree about a warehouse, and the disagreement shows up as bins nobody can
//! find rather than as a failing test.
//!
//! # The dry run is the real write, rolled back
//!
//! A dry run that *estimates* what would happen is a second implementation of
//! the write, and the interesting numbers — how many bins are new against how
//! many are corrections — cannot be known without asking the database anyway.
//! So [`load`] always performs the writes and rolls them back when it was not
//! told to apply. The report cannot disagree with what applying would do,
//! because it is what applying does.
//!
//! It rolls back to a savepoint rather than aborting the transaction, so the
//! caller keeps a usable handle either way. `SAVEPOINT` is issued as a statement
//! rather than through `Transaction::savepoint`, which needs `&mut` — and
//! [`crate::tenancy::TenantScope::run`] hands out `&Transaction`, deliberately,
//! so that a handler cannot commit out from under it.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::bins;

/// One row of the bin export, after trimming and before any judgement.
#[derive(Clone, Debug)]
pub struct Row {
    pub code: String,
    pub location: String,
    pub wms_kind: String,
    pub picking_order: Option<i32>,
    pub bin_sequence: Option<i32>,
}

/// The export's 0 means "no position", not "first".
///
/// `location_pick_sequence_ck` refuses 0 outright, so this is where the
/// sentinel becomes an absence. 150 rows use it, including `3PL` and
/// `ASSEMBLY-BIN`, which are not stops on a route; loaded literally, every one
/// of them sorts ahead of the first real bin.
pub fn position(raw: Option<i32>) -> Option<i32> {
    raw.filter(|v| *v >= 1)
}

/// Read the export from anything, so a path and a request body are the same.
///
/// Keyed by header name rather than by index, which is safe here because the
/// bin export has no duplicated column name — unlike the sales order export,
/// where `Name` appears twice and any reader keyed by name silently keeps one.
pub fn read<R: std::io::Read>(source: R) -> Result<Vec<Row>, String> {
    let mut rdr = csv::Reader::from_reader(source);
    let mut rows = vec![];
    for rec in rdr.deserialize::<HashMap<String, String>>() {
        let r = rec.map_err(|e| e.to_string())?;
        let get = |k: &str| r.get(k).map(|s| s.trim().to_string()).unwrap_or_default();
        let code = get("Bin Number");
        if code.is_empty() {
            continue;
        }
        rows.push(Row {
            code,
            location: get("Location"),
            wms_kind: get("WMS Bin Type"),
            picking_order: get("WMS Picking Order").parse().ok(),
            bin_sequence: get("WMS Bin Sequence").parse().ok(),
        });
    }
    Ok(rows)
}

/// One warehouse, as the file describes it and before anything is written.
#[derive(Debug, Serialize)]
pub struct SiteSurvey {
    pub warehouse: String,
    pub bins: usize,
    pub typed: usize,
    pub untyped: usize,
    /// The clock, or why this warehouse is being left alone.
    pub note: String,
    pub skipped: bool,
}

/// What the file says. Pure: no database is consulted to produce this.
#[derive(Debug, Serialize)]
pub struct Survey {
    pub bins: usize,
    pub sites: Vec<SiteSurvey>,
    /// Bins carrying a walking position.
    pub sequenced: usize,
    /// Bins saying 0, which the export means as unsequenced.
    pub zeroed: usize,
    /// Bins whose `WMS Bin Sequence` disagrees with their `WMS Picking Order`.
    ///
    /// Only one of the two can be the order somebody walks, so only one is
    /// stored and the disagreement is reported rather than averaged.
    pub disagree: usize,
    pub untyped: usize,
}

/// Look at the file, warehouse by warehouse.
///
/// A warehouse is left alone when its clock is unknown — `site.timezone` is NOT
/// NULL and defaulting a new warehouse to Melbourne's is wrong in Brisbane
/// twice a year — or when it looks like somebody else's building, because
/// creating a site asserts this business has premises there.
pub fn survey(rows: &[Row], include_external: bool) -> Survey {
    let mut per_site: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    for b in rows {
        let e = per_site.entry(b.location.clone()).or_default();
        e.0 += 1;
        if bins::kind_of(&b.wms_kind).is_some() {
            e.1 += 1;
        } else {
            e.2 += 1;
        }
    }

    let mut sites = vec![];
    for (warehouse, (bins_n, typed, untyped)) in &per_site {
        let external = bins::looks_external(warehouse);
        let (note, skipped) = if external && !include_external {
            ("not yours? skipped".to_string(), true)
        } else if let Some(tz) = bins::timezone_for(warehouse) {
            (tz.to_string(), false)
        } else {
            ("unknown clock, skipped".to_string(), true)
        };
        sites.push(SiteSurvey {
            warehouse: warehouse.clone(),
            bins: *bins_n,
            typed: *typed,
            untyped: *untyped,
            note,
            skipped,
        });
    }

    Survey {
        bins: rows.len(),
        sequenced: rows.iter().filter(|b| position(b.picking_order).is_some()).count(),
        zeroed: rows.iter().filter(|b| b.picking_order == Some(0)).count(),
        disagree: rows
            .iter()
            .filter(|b| {
                matches!((position(b.picking_order), position(b.bin_sequence)),
                         (Some(a), Some(c)) if a != c)
            })
            .count(),
        untyped: per_site.values().map(|v| v.2).sum(),
        sites,
    }
}

/// What the database did, or would have done.
#[derive(Debug, Default, Serialize)]
pub struct Loaded {
    pub sites_created: usize,
    pub sites_matched: usize,
    pub bins_created: usize,
    pub bins_corrected: usize,
    /// Bins in a skipped warehouse, or stating no type with nothing assumed.
    pub bins_left_out: usize,
    /// False when the writes were rolled back.
    pub applied: bool,
}

/// Write the bin list, or find out what writing it would do.
///
/// The transaction must already carry the tenant: this runs as `nylonite_app`
/// under row-level security, so `site` and `location` are reached on the same
/// terms as every other write in the server rather than on a privileged
/// connection. The examples arrange that themselves; handlers get it from
/// [`crate::tenancy::TenantScope`].
pub async fn load(
    tx: &Transaction<'_>,
    tenant: Uuid,
    rows: &[Row],
    survey: &Survey,
    assume_kind: Option<&str>,
    apply: bool,
) -> Result<Loaded, String> {
    tx.batch_execute("SAVEPOINT nylonite_import")
        .await
        .map_err(|e| e.to_string())?;

    let mut out = Loaded { applied: apply, ..Default::default() };

    // **Match before create.** A second `MEL` would split the warehouse in two
    // without saying so, and nothing downstream would report it.
    let mut site_ids: HashMap<String, Uuid> = HashMap::new();
    for s in &survey.sites {
        if s.skipped {
            continue;
        }
        let warehouse = &s.warehouse;
        let Some(tz) = bins::timezone_for(warehouse) else { continue };
        let (name, code) = (bins::site_name(warehouse), bins::site_code(warehouse));
        let existing: Option<Uuid> = tx
            .query_opt(
                "SELECT id FROM site WHERE tenant_id = $1 AND (code = $2 OR name = $3)",
                &[&tenant, &code, &name],
            )
            .await
            .map_err(|e| e.to_string())?
            .map(|r| r.get(0));
        let id = match existing {
            Some(id) => {
                out.sites_matched += 1;
                id
            }
            None => {
                out.sites_created += 1;
                tx.query_one(
                    "INSERT INTO site (tenant_id, code, name, timezone, active)
                     VALUES ($1, $2, $3, $4, true) RETURNING id",
                    &[&tenant, &code, &name, &tz],
                )
                .await
                .map_err(|e| format!("site {name}: {e}"))?
                .get(0)
            }
        };
        site_ids.insert(warehouse.clone(), id);
    }

    for b in rows {
        let Some(site_id) = site_ids.get(&b.location) else {
            out.bins_left_out += 1;
            continue;
        };
        let kind = match bins::kind_of(&b.wms_kind) {
            Some(k) => k.to_string(),
            None => match assume_kind {
                Some(k) => k.to_string(),
                None => {
                    out.bins_left_out += 1;
                    continue;
                }
            },
        };
        let p = bins::decompose(&b.code);
        // `xmax = 0` is true for a row this statement inserted and false for one
        // it updated, which is how the report tells a new shelf from a corrected
        // one.
        let written = tx
            .query(
                "INSERT INTO location
                     (tenant_id, site_id, code, kind, aisle, bay, level, position, active,
                      pick_sequence)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, true, $9)
                 -- Named rather than bare: a bare DO NOTHING silently catches
                 -- nothing if the constraint it assumed is not there, and the
                 -- second run then doubles the shelving.
                 --
                 -- **DO UPDATE, because a bin list is reference data.** The
                 -- first version did nothing on conflict, so when migration 74
                 -- added `pick_sequence` a re-import could not fill it: 8,047
                 -- bins, none with a position, and an importer whose own
                 -- documentation said re-importing was a correction. It is one
                 -- now. `WHERE ... IS DISTINCT FROM` so a run that changes
                 -- nothing reports nothing.
                 ON CONFLICT (tenant_id, site_id, code) DO UPDATE
                    SET pick_sequence = EXCLUDED.pick_sequence,
                        kind          = EXCLUDED.kind,
                        aisle         = EXCLUDED.aisle,
                        bay           = EXCLUDED.bay,
                        level         = EXCLUDED.level,
                        position      = EXCLUDED.position
                  WHERE (location.pick_sequence, location.kind, location.aisle,
                         location.bay, location.level, location.position)
                        IS DISTINCT FROM
                        (EXCLUDED.pick_sequence, EXCLUDED.kind, EXCLUDED.aisle,
                         EXCLUDED.bay, EXCLUDED.level, EXCLUDED.position)
                  RETURNING (xmax = 0)",
                &[
                    &tenant,
                    site_id,
                    &b.code,
                    &kind,
                    &p.aisle,
                    &p.bay,
                    &p.level,
                    &p.position,
                    &position(b.picking_order),
                ],
            )
            .await
            .map_err(|e| format!("bin {} at {}: {e}", b.code, b.location))?;
        match written.first().map(|r| r.get::<_, bool>(0)) {
            Some(true) => out.bins_created += 1,
            Some(false) => out.bins_corrected += 1,
            None => {}
        }
    }

    if apply {
        tx.batch_execute("RELEASE SAVEPOINT nylonite_import")
            .await
            .map_err(|e| e.to_string())?;
    } else {
        tx.batch_execute("ROLLBACK TO SAVEPOINT nylonite_import")
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(out)
}
