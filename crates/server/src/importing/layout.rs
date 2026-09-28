//! Loading a site's layout: its racks, and the floor between them. D173.
//!
//! [`crate::layout`] decides what a rack row means and stays pure. This is the
//! other half: reading the file, writing the racks, and putting every bin a
//! rack names where the rack says it is.
//!
//! # The site is reconciled whole
//!
//! Writing the racks in the file and placing only their bins would be simpler
//! and wrong twice over. A rack that is not in this file can name the same bin
//! as one that is, and the file alone cannot see that. A rack re-imported with
//! fewer bays leaves bins placed by bays that no longer exist, and nothing in
//! the file names them. So once the racks are written, every active rack at each
//! site the file touches is expanded together, and every bin at that site is
//! brought into line with the result: placed, moved, or unplaced.
//!
//! **A measured box is never overwritten.** A bin whose `geometry_source` is
//! `survey` or `manual` is still marked as on its rack, and keeps its box.
//!
//! # A layout names sites; it never creates one
//!
//! For the bin importer's reason: creating a site asserts premises. A site code
//! that is not on file refuses the whole file.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::layout::{self, Box3, Rack};

/// How many codes a report lists before it only counts.
const SAMPLE: usize = 20;

/// One row of the rack file.
#[derive(Clone, Debug)]
pub struct RackRow {
    /// The site's code, as `site.code` has it.
    pub site: String,
    pub rack: Rack,
}

/// One row of the floor file.
#[derive(Clone, Debug)]
pub struct AreaRow {
    pub site: String,
    pub code: String,
    pub kind: String,
    pub outline: Vec<(i32, i32)>,
    pub height_mm: Option<i32>,
    /// The code of the location this area is, at the same site.
    pub location: Option<String>,
}

/// What `floor_area_kind_ck` allows, said here first so a wrong word is a
/// sentence rather than a constraint name.
pub const AREA_KINDS: &[&str] =
    &["dock", "staging", "packing", "walkway", "wall", "floor_stack", "office", "restricted"];

fn records<R: std::io::Read>(
    source: R,
    required: &[&str],
) -> Result<Vec<(usize, HashMap<String, String>)>, String> {
    let mut rdr = csv::Reader::from_reader(source);
    let headers: HashSet<String> = rdr
        .headers()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    let missing: Vec<&str> = required.iter().copied().filter(|h| !headers.contains(*h)).collect();
    if !missing.is_empty() {
        return Err(format!("the file has no {} column", missing.join(", no ")));
    }
    let mut out = vec![];
    for (i, rec) in rdr.deserialize::<HashMap<String, String>>().enumerate() {
        let r = rec.map_err(|e| e.to_string())?;
        let r: HashMap<String, String> = r
            .into_iter()
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
            .collect();
        if r.values().all(|v| v.is_empty()) {
            continue;
        }
        // Row 1 is the header, so the first record is row 2, which is the
        // number a spreadsheet shows beside it.
        out.push((i + 2, r));
    }
    Ok(out)
}

/// Read the rack file.
///
/// Every row is read before anything is refused, so a file with five mistakes
/// says all five at once rather than one per attempt.
pub fn read_racks<R: std::io::Read>(source: R) -> Result<Vec<RackRow>, String> {
    let rows = records(
        source,
        &["site", "rack", "aisle", "x_mm", "y_mm", "rotation", "depth_mm", "height_mm",
          "bay_widths_mm", "level_z_mm", "template"],
    )?;
    let mut out = vec![];
    let mut errors = vec![];
    for (n, r) in rows {
        let get = |k: &str| r.get(k).cloned().unwrap_or_default();
        let int = |k: &str, default: Option<i32>| -> Result<i32, String> {
            let v = get(k);
            if v.is_empty() {
                return default.ok_or(format!("row {n}: {k} is empty"));
            }
            v.parse().map_err(|_| format!("row {n}: {k} `{v}` is not a whole number"))
        };
        let list = |k: &str| layout::parse_list(&get(k)).map_err(|e| format!("row {n}: {k}: {e}"));
        let parsed = (|| -> Result<RackRow, String> {
            let level_z_mm = list("level_z_mm")?;
            // One count means every level; a survey rarely varies it.
            let mut positions = if get("positions").is_empty() { vec![1] } else { list("positions")? };
            if positions.len() == 1 {
                positions = vec![positions[0]; level_z_mm.len()];
            }
            let rotation = int("rotation", None)?;
            Ok(RackRow {
                site: get("site"),
                rack: Rack {
                    code: get("rack"),
                    aisle: get("aisle"),
                    x_mm: int("x_mm", None)?,
                    y_mm: int("y_mm", None)?,
                    rotation: i16::try_from(rotation)
                        .map_err(|_| format!("row {n}: rotation `{rotation}` is not a quarter turn"))?,
                    depth_mm: int("depth_mm", None)?,
                    height_mm: int("height_mm", None)?,
                    upright_mm: int("upright_mm", Some(0))?,
                    first_bay: int("first_bay", Some(1))?,
                    bay_step: int("bay_step", Some(1))?,
                    bay_widths_mm: list("bay_widths_mm")?,
                    first_level: int("first_level", Some(1))?,
                    level_z_mm,
                    positions,
                    code_template: get("template"),
                },
            })
        })();
        match parsed {
            Ok(row) => {
                if row.site.is_empty() {
                    errors.push(format!("row {n}: no site"));
                }
                errors.extend(row.rack.problems().into_iter().map(|p| format!("row {n}: {p}")));
                out.push(row);
            }
            Err(e) => errors.push(e),
        }
    }
    let mut seen = HashSet::new();
    for r in &out {
        if !seen.insert((r.site.clone(), r.rack.code.clone())) {
            errors.push(format!("rack {} at {} is in the file twice", r.rack.code, r.site));
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors.join("\n"))
    }
}

/// Read the floor file.
pub fn read_floor<R: std::io::Read>(source: R) -> Result<Vec<AreaRow>, String> {
    let rows = records(source, &["site", "area", "kind", "outline"])?;
    let mut out = vec![];
    let mut errors = vec![];
    let mut seen = HashSet::new();
    for (n, r) in rows {
        let get = |k: &str| r.get(k).cloned().unwrap_or_default();
        let (site, code, kind) = (get("site"), get("area"), get("kind"));
        if site.is_empty() || code.is_empty() {
            errors.push(format!("row {n}: an area needs a site and a code"));
        }
        if !AREA_KINDS.contains(&kind.as_str()) {
            errors.push(format!(
                "row {n}: `{kind}` is not a kind of area; say one of {}",
                AREA_KINDS.join(", ")
            ));
        }
        let mut outline = match layout::parse_outline(&get("outline")) {
            Ok(o) => o,
            Err(e) => {
                errors.push(format!("row {n}: {e}"));
                continue;
            }
        };
        if let Some(p) = layout::outline_problem(&outline) {
            errors.push(format!("row {n}: area {code}: {p}"));
        }
        // Stored open: the first corner is not repeated.
        if outline.len() > 1 && outline.first() == outline.last() {
            outline.pop();
        }
        let height_mm = match get("height_mm") {
            v if v.is_empty() => None,
            v => match v.parse::<i32>() {
                Ok(h) if h > 0 => Some(h),
                _ => {
                    errors.push(format!("row {n}: height_mm `{v}` is not a height"));
                    None
                }
            },
        };
        if !seen.insert((site.clone(), code.clone())) {
            errors.push(format!("area {code} at {site} is in the file twice"));
        }
        let location = Some(get("location")).filter(|l| !l.is_empty());
        out.push(AreaRow { site, code, kind, outline, height_mm, location });
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors.join("\n"))
    }
}

/// One site's bins, after its racks were expanded together.
#[derive(Debug, Default, Serialize)]
pub struct SitePlaced {
    pub site: String,
    pub racks: usize,
    pub slots: usize,
    /// Bins that had no box and now have one from their rack.
    pub bins_placed: usize,
    /// Bins whose template box changed, because their rack did.
    pub bins_moved: usize,
    /// Bins on a rack whose measured box was kept.
    pub bins_kept_measured: usize,
    /// Bins no rack names any more, which lose their template box.
    pub bins_unplaced: usize,
    /// Slots whose code is no bin on file. Reported, never created (D173).
    pub slots_without_bin: usize,
    pub slots_without_bin_sample: Vec<String>,
    /// Active bins in an aisle these racks run along that none of them names
    /// and that have no box of their own: J77, before it is a finding.
    pub bins_uncovered: usize,
    pub bins_uncovered_sample: Vec<String>,
}

/// What the rack import did, or would have done.
#[derive(Debug, Default, Serialize)]
pub struct RacksLoaded {
    pub racks_created: usize,
    pub racks_changed: usize,
    pub racks_unchanged: usize,
    pub sites: Vec<SitePlaced>,
    /// False when the writes were rolled back.
    pub applied: bool,
}

/// What the floor import did, or would have done.
#[derive(Debug, Default, Serialize)]
pub struct FloorLoaded {
    pub areas_created: usize,
    pub areas_changed: usize,
    pub areas_unchanged: usize,
    pub applied: bool,
}

async fn savepoint(tx: &Transaction<'_>) -> Result<(), String> {
    tx.batch_execute("SAVEPOINT spork_layout").await.map_err(|e| e.to_string())
}

async fn finish(tx: &Transaction<'_>, apply: bool) -> Result<(), String> {
    let sql = if apply {
        "RELEASE SAVEPOINT spork_layout"
    } else {
        "ROLLBACK TO SAVEPOINT spork_layout"
    };
    tx.batch_execute(sql).await.map_err(|e| e.to_string())
}

/// Site codes to ids, refusing any that are not on file.
async fn sites(
    tx: &Transaction<'_>,
    tenant: Uuid,
    codes: impl Iterator<Item = &String>,
) -> Result<BTreeMap<String, Uuid>, String> {
    let mut out = BTreeMap::new();
    let mut unknown = BTreeSet::new();
    for code in codes {
        if out.contains_key(code) || unknown.contains(code) {
            continue;
        }
        let row = tx
            .query_opt(
                "SELECT id FROM site WHERE tenant_id = $1 AND code = $2",
                &[&tenant, code],
            )
            .await
            .map_err(|e| e.to_string())?;
        match row {
            Some(r) => {
                out.insert(code.clone(), r.get(0));
            }
            None => {
                unknown.insert(code.clone());
            }
        }
    }
    if !unknown.is_empty() {
        return Err(format!(
            "no site is called {}; a layout never creates a warehouse, so load its bins first",
            unknown.into_iter().collect::<Vec<_>>().join(" or ")
        ));
    }
    Ok(out)
}

/// Write the racks, then reconcile every site they touch.
///
/// Runs as `spork_app` on a transaction that already carries the tenant, as
/// [`crate::importing::bins::load`] does, and like it performs the writes and
/// rolls them back when not told to apply.
pub async fn load_racks(
    tx: &Transaction<'_>,
    tenant: Uuid,
    rows: &[RackRow],
    apply: bool,
) -> Result<RacksLoaded, String> {
    savepoint(tx).await?;
    let mut out = RacksLoaded { applied: apply, ..Default::default() };
    let site_ids = sites(tx, tenant, rows.iter().map(|r| &r.site)).await?;

    for row in rows {
        let r = &row.rack;
        let site_id = site_ids[&row.site];
        let written = tx
            .query(
                "INSERT INTO rack
                     (tenant_id, site_id, code, aisle, x_mm, y_mm, rotation, depth_mm,
                      height_mm, upright_mm, first_bay, bay_step, bay_widths_mm,
                      first_level, level_z_mm, positions, code_template, active)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                         $15, $16, $17, true)
                 ON CONFLICT (tenant_id, site_id, code) DO UPDATE
                    SET aisle = EXCLUDED.aisle, x_mm = EXCLUDED.x_mm, y_mm = EXCLUDED.y_mm,
                        rotation = EXCLUDED.rotation, depth_mm = EXCLUDED.depth_mm,
                        height_mm = EXCLUDED.height_mm, upright_mm = EXCLUDED.upright_mm,
                        first_bay = EXCLUDED.first_bay, bay_step = EXCLUDED.bay_step,
                        bay_widths_mm = EXCLUDED.bay_widths_mm,
                        first_level = EXCLUDED.first_level,
                        level_z_mm = EXCLUDED.level_z_mm, positions = EXCLUDED.positions,
                        code_template = EXCLUDED.code_template, active = true
                  WHERE (rack.aisle, rack.x_mm, rack.y_mm, rack.rotation, rack.depth_mm,
                         rack.height_mm, rack.upright_mm, rack.first_bay, rack.bay_step,
                         rack.bay_widths_mm, rack.first_level, rack.level_z_mm,
                         rack.positions, rack.code_template, rack.active)
                        IS DISTINCT FROM
                        (EXCLUDED.aisle, EXCLUDED.x_mm, EXCLUDED.y_mm, EXCLUDED.rotation,
                         EXCLUDED.depth_mm, EXCLUDED.height_mm, EXCLUDED.upright_mm,
                         EXCLUDED.first_bay, EXCLUDED.bay_step, EXCLUDED.bay_widths_mm,
                         EXCLUDED.first_level, EXCLUDED.level_z_mm, EXCLUDED.positions,
                         EXCLUDED.code_template, true)
                  RETURNING (xmax = 0)",
                &[
                    &tenant, &site_id, &r.code, &r.aisle, &r.x_mm, &r.y_mm, &r.rotation,
                    &r.depth_mm, &r.height_mm, &r.upright_mm, &r.first_bay, &r.bay_step,
                    &r.bay_widths_mm, &r.first_level, &r.level_z_mm, &r.positions,
                    &r.code_template,
                ],
            )
            .await
            .map_err(|e| format!("rack {} at {}: {e}", r.code, row.site))?;
        match written.first().map(|w| w.get::<_, bool>(0)) {
            Some(true) => out.racks_created += 1,
            Some(false) => out.racks_changed += 1,
            None => out.racks_unchanged += 1,
        }
    }

    for (code, site_id) in &site_ids {
        out.sites.push(place(tx, tenant, *site_id, code).await?);
    }

    finish(tx, apply).await?;
    Ok(out)
}

/// A bin as it stands, for [`place`].
struct Bin {
    id: Uuid,
    code: String,
    aisle: Option<String>,
    active: bool,
    rack_id: Option<Uuid>,
    source: Option<String>,
    r#box: Option<Box3>,
}

/// Bring every bin at one site into line with its active racks.
///
/// Public so that anything else that changes a rack (a plan editor, one day)
/// reconciles the same way rather than a second way.
pub async fn place(
    tx: &Transaction<'_>,
    tenant: Uuid,
    site_id: Uuid,
    site_code: &str,
) -> Result<SitePlaced, String> {
    let db = |e: tokio_postgres::Error| e.to_string();
    let racks: Vec<(Uuid, Rack)> = tx
        .query(
            "SELECT id, code, aisle, x_mm, y_mm, rotation, depth_mm, height_mm, upright_mm,
                    first_bay, bay_step, bay_widths_mm, first_level, level_z_mm, positions,
                    code_template
               FROM rack
              WHERE tenant_id = $1 AND site_id = $2 AND active
              ORDER BY code",
            &[&tenant, &site_id],
        )
        .await
        .map_err(db)?
        .iter()
        .map(|r| {
            (
                r.get(0),
                Rack {
                    code: r.get(1),
                    aisle: r.get(2),
                    x_mm: r.get(3),
                    y_mm: r.get(4),
                    rotation: r.get(5),
                    depth_mm: r.get(6),
                    height_mm: r.get(7),
                    upright_mm: r.get(8),
                    first_bay: r.get(9),
                    bay_step: r.get(10),
                    bay_widths_mm: r.get(11),
                    first_level: r.get(12),
                    level_z_mm: r.get(13),
                    positions: r.get(14),
                    code_template: r.get(15),
                },
            )
        })
        .collect();

    let only: Vec<Rack> = racks.iter().map(|(_, r)| r.clone()).collect();
    let slots = layout::expand_site(&only).map_err(|e| format!("at {site_code}: {e}"))?;
    let by_code: HashMap<&str, (Uuid, Box3)> = slots
        .iter()
        .map(|(i, s)| (s.code.as_str(), (racks[*i].0, s.r#box)))
        .collect();
    let aisles: HashSet<&str> = only.iter().map(|r| r.aisle.as_str()).collect();

    let bins: Vec<Bin> = tx
        .query(
            "SELECT id, code, aisle, active, rack_id, geometry_source,
                    x_mm, y_mm, z_mm, length_mm, width_mm, height_mm
               FROM location
              WHERE tenant_id = $1 AND site_id = $2
              ORDER BY code",
            &[&tenant, &site_id],
        )
        .await
        .map_err(db)?
        .iter()
        .map(|r| Bin {
            id: r.get(0),
            code: r.get(1),
            aisle: r.get(2),
            active: r.get(3),
            rack_id: r.get(4),
            source: r.get(5),
            r#box: r.get::<_, Option<i32>>(6).map(|x_mm| Box3 {
                x_mm,
                y_mm: r.get(7),
                z_mm: r.get(8),
                length_mm: r.get(9),
                width_mm: r.get(10),
                height_mm: r.get(11),
            }),
        })
        .collect();

    let mut out = SitePlaced {
        site: site_code.to_string(),
        racks: racks.len(),
        slots: slots.len(),
        ..Default::default()
    };

    // Each changed bin as one row of the arrays the UPDATE unnests.
    let mut ids = vec![];
    let mut rack_ids: Vec<Option<Uuid>> = vec![];
    let mut sources: Vec<Option<String>> = vec![];
    let mut boxes: [Vec<Option<i32>>; 6] = Default::default();
    let mut on_file = HashSet::new();

    for b in &bins {
        on_file.insert(b.code.as_str());
        let measured = matches!(b.source.as_deref(), Some("survey") | Some("manual"));
        let (rack_id, source, bx) = match by_code.get(b.code.as_str()) {
            Some((rid, _)) if measured => {
                out.bins_kept_measured += 1;
                (Some(*rid), b.source.clone(), b.r#box)
            }
            Some((rid, slot)) => {
                match b.r#box {
                    None => out.bins_placed += 1,
                    Some(old) if old != *slot => out.bins_moved += 1,
                    _ => {}
                }
                (Some(*rid), Some("template".to_string()), Some(*slot))
            }
            None if measured => (None, b.source.clone(), b.r#box),
            None => {
                if b.rack_id.is_some() || b.r#box.is_some() {
                    out.bins_unplaced += 1;
                }
                (None, None, None)
            }
        };
        if b.active && bx.is_none() && b.aisle.as_deref().is_some_and(|a| aisles.contains(a)) {
            out.bins_uncovered += 1;
            if out.bins_uncovered_sample.len() < SAMPLE {
                out.bins_uncovered_sample.push(b.code.clone());
            }
        }
        if (rack_id, &source, bx) == (b.rack_id, &b.source, b.r#box) {
            continue;
        }
        ids.push(b.id);
        rack_ids.push(rack_id);
        sources.push(source);
        let v = bx.map(|x| [x.x_mm, x.y_mm, x.z_mm, x.length_mm, x.width_mm, x.height_mm]);
        for (k, col) in boxes.iter_mut().enumerate() {
            col.push(v.map(|v| v[k]));
        }
    }

    for (_, s) in &slots {
        if !on_file.contains(s.code.as_str()) {
            out.slots_without_bin += 1;
            if out.slots_without_bin_sample.len() < SAMPLE {
                out.slots_without_bin_sample.push(s.code.clone());
            }
        }
    }

    if !ids.is_empty() {
        tx.execute(
            "UPDATE location l
                SET rack_id = t.rack_id, geometry_source = t.source,
                    x_mm = t.x, y_mm = t.y, z_mm = t.z,
                    length_mm = t.l, width_mm = t.w, height_mm = t.h
               FROM unnest($1::uuid[], $2::uuid[], $3::text[], $4::int4[], $5::int4[],
                           $6::int4[], $7::int4[], $8::int4[], $9::int4[])
                    AS t(id, rack_id, source, x, y, z, l, w, h)
              WHERE l.id = t.id",
            &[
                &ids, &rack_ids, &sources, &boxes[0], &boxes[1], &boxes[2], &boxes[3],
                &boxes[4], &boxes[5],
            ],
        )
        .await
        .map_err(|e| format!("placing bins at {site_code}: {e}"))?;
    }
    Ok(out)
}

/// Write the floor areas.
pub async fn load_floor(
    tx: &Transaction<'_>,
    tenant: Uuid,
    rows: &[AreaRow],
    apply: bool,
) -> Result<FloorLoaded, String> {
    savepoint(tx).await?;
    let mut out = FloorLoaded { applied: apply, ..Default::default() };
    let site_ids = sites(tx, tenant, rows.iter().map(|r| &r.site)).await?;

    let mut unknown = vec![];
    for a in rows {
        let site_id = site_ids[&a.site];
        let location_id: Option<Uuid> = match &a.location {
            None => None,
            Some(code) => {
                let found = tx
                    .query_opt(
                        "SELECT id FROM location WHERE tenant_id = $1 AND site_id = $2 AND code = $3",
                        &[&tenant, &site_id, code],
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                match found {
                    Some(r) => Some(r.get(0)),
                    None => {
                        unknown.push(format!("area {} names location {code}, which {} does not have", a.code, a.site));
                        continue;
                    }
                }
            }
        };
        let outline: Vec<i32> = a.outline.iter().flat_map(|(x, y)| [*x, *y]).collect();
        let written = tx
            .query(
                "INSERT INTO floor_area
                     (tenant_id, site_id, code, kind, outline_mm, height_mm, location_id)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (tenant_id, site_id, code) DO UPDATE
                    SET kind = EXCLUDED.kind, outline_mm = EXCLUDED.outline_mm,
                        height_mm = EXCLUDED.height_mm, location_id = EXCLUDED.location_id
                  WHERE (floor_area.kind, floor_area.outline_mm, floor_area.height_mm,
                         floor_area.location_id)
                        IS DISTINCT FROM
                        (EXCLUDED.kind, EXCLUDED.outline_mm, EXCLUDED.height_mm,
                         EXCLUDED.location_id)
                  RETURNING (xmax = 0)",
                &[&tenant, &site_id, &a.code, &a.kind, &outline, &a.height_mm, &location_id],
            )
            .await
            .map_err(|e| format!("area {} at {}: {e}", a.code, a.site))?;
        match written.first().map(|w| w.get::<_, bool>(0)) {
            Some(true) => out.areas_created += 1,
            Some(false) => out.areas_changed += 1,
            None => out.areas_unchanged += 1,
        }
    }
    if !unknown.is_empty() {
        // Nothing half-loaded: the areas written so far go with the refusal.
        tx.batch_execute("ROLLBACK TO SAVEPOINT spork_layout")
            .await
            .map_err(|e| e.to_string())?;
        return Err(unknown.join("\n"));
    }

    finish(tx, apply).await?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "site,rack,aisle,x_mm,y_mm,rotation,depth_mm,height_mm,upright_mm,\
                        first_bay,bay_step,bay_widths_mm,level_z_mm,positions,template\n";

    #[test]
    fn a_rack_file_reads_as_a_survey_writes_it() {
        let f = format!("{HEAD}MAIN,A-L,A,0,0,0,1100,6000,100,1,2,3x2700,0 1500 3000,2,{{aisle}}-{{bay:02}}-{{level}}-{{position}}\n");
        let rows = read_racks(f.as_bytes()).unwrap();
        assert_eq!(rows[0].rack.bay_widths_mm, vec![2700; 3]);
        assert_eq!(rows[0].rack.positions, vec![2, 2, 2], "one count is every level's");
        assert_eq!(rows[0].rack.expand().unwrap().len(), 18);
    }

    /// The worked example in docs/layout.md, so the document cannot drift from
    /// what the generator does with it.
    #[test]
    fn the_documented_aisle_is_two_racks_facing_across_it() {
        let doc = include_str!("../../../../docs/layout.md").replace("\r\n", "\n");
        let start = doc.find("```csv\nsite,rack").expect("the example") + "```csv\n".len();
        let csv = &doc[start..start + doc[start..].find("```").unwrap()];
        let rows = read_racks(csv.as_bytes()).unwrap();
        let slots = layout::expand_site(&rows.iter().map(|r| r.rack.clone()).collect::<Vec<_>>())
            .unwrap();
        let at = |code: &str| slots.iter().find(|(_, s)| s.code == code).unwrap().1.r#box;
        // Bay 1 is the south rack's west end, facing bay 2 across the aisle.
        // Positions run left to right as each is faced, so they meet at the
        // west upright from opposite ends of the count: 3 across from 1.
        let (b1, b2) = (at("C-01-1-3"), at("C-02-1-1"));
        assert_eq!((b1.x_mm, b1.y_mm + b1.width_mm), (4090, 7100), "{b1:?}");
        assert_eq!((b2.x_mm, b2.y_mm), (4090, 9100), "{b2:?}");
        let b39 = at("C-39-1-1");
        assert_eq!(b39.x_mm + b39.length_mm, 59800, "39 is the east end, and its first position the eastmost");
        assert_eq!(slots.len(), 2 * 20 * (3 + 1 + 1 + 1));
    }

    #[test]
    fn every_mistake_in_a_rack_file_is_said_at_once() {
        let f = format!(
            "{HEAD}MAIN,A-L,A,0,0,45,1100,6000,100,1,2,3x2700,0 1500 3000,1,{{aisle}}-{{bay}}-{{level}}\n\
             MAIN,A-R,A,zero,0,0,1100,6000,100,1,2,3x2700,0 1500 3000,1,{{aisle}}-{{bay}}-{{level}}\n\
             MAIN,A-L,A,0,0,0,1100,6000,100,2,2,3x2700,0 1500 3000,1,{{aisle}}-{{bay}}-{{level}}\n"
        );
        let e = read_racks(f.as_bytes()).unwrap_err();
        assert!(e.contains("row 2") && e.contains("quarter turns"), "{e}");
        assert!(e.contains("row 3") && e.contains("x_mm `zero`"), "{e}");
        assert!(e.contains("in the file twice"), "{e}");
        let e = read_racks("site,rack\nMAIN,A\n".as_bytes()).unwrap_err();
        assert!(e.contains("no aisle"), "{e}");
    }

    #[test]
    fn a_floor_file_refuses_a_shape_with_no_inside() {
        let ok = "site,area,kind,outline,height_mm,location\n\
                  MAIN,STAGE-1,staging,0 0 ; 1000 0,,\n";
        assert!(read_floor(ok.as_bytes()).is_err(), "`0 0` is not a point");
        let ok = "site,area,kind,outline,height_mm,location\n\
                  MAIN,STAGE-1,staging,\"0,0 4000,0 4000,2000 0,2000 0,0\",,PACK-1\n\
                  MAIN,WALL-N,wall,\"0,9000 20000,9000 20000,9200 0,9200\",4000,\n";
        let rows = read_floor(ok.as_bytes()).unwrap();
        assert_eq!(rows[0].outline.len(), 4, "stored open");
        assert_eq!(rows[0].location.as_deref(), Some("PACK-1"));
        assert_eq!(rows[1].height_mm, Some(4000));
        let bad = "site,area,kind,outline\nMAIN,X,lobby,\"0,0 10,0 10,10 0,10\"\n\
                   MAIN,Y,wall,\"0,0 10,10 10,0 0,10\"\n";
        let e = read_floor(bad.as_bytes()).unwrap_err();
        assert!(e.contains("`lobby` is not a kind"), "{e}");
        assert!(e.contains("area Y") && e.contains("meets"), "{e}");
    }
}
