//! The picking walk, routed (D211).
//!
//! The walk's lines come from `picking_list` in the order somebody typed
//! (`location.pick_sequence`). Here the bins those lines are taken from are
//! put on the floor, each with the spot in its aisle a picker stands on, and
//! the walk is put in the order that walks least, from the packing bench and
//! back when the bench is on the layout.
//!
//! **The floor is kept per site**, rebuilt when the layout or its scale
//! changes. Where a bin's picker stands is worked out per walk, from the bins
//! the walk has: a draft that drops bins into existing cells changes no place,
//! so standing spots kept with the floor could go stale.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::error::ApiError;
use crate::layout::{self, Frame, GridCell};
use crate::routing::{self, Floor, Pt};

/// Where a line's bin is on the layout.
#[derive(Clone, Copy, Debug)]
pub struct Whereabouts {
    pub place_id: Uuid,
    pub cell: GridCell,
}

/// The walk's route, beside the order it was typed in.
#[derive(Serialize, Debug)]
pub struct WalkRoute {
    /// Where it starts: `pack`, from the packing location and back to it;
    /// `first`, from the typed order's first stop, ending at the last.
    pub from: String,
    /// How long a cell is, once the site says; the lengths below are in cells.
    pub cell_mm: Option<i32>,
    /// The walk in route order, and in the typed order over the same stops,
    /// from the same start under the same rule.
    pub walked: f64,
    pub typed: f64,
    /// Bins visited.
    pub stops: i64,
    /// Lines whose bin isn't on the layout or can't be reached from an aisle.
    /// They are walked last, in the typed order.
    pub off_route: i64,
    /// Where the walk goes, on the site, in cells.
    pub path: Vec<[f64; 2]>,
}

/// A place as the floor needs it.
struct Shape {
    frame: Frame,
    length: f64,
    depth: f64,
    bays: i32,
    rows: i32,
    solid: bool,
}

/// A site's floor, and its places' shapes.
struct SiteFloor {
    floor: Floor,
    places: HashMap<Uuid, Shape>,
    /// How far out from a face to look for somewhere to stand.
    reach: f64,
}

/// Each site's floor, with the fingerprint of the layout and scale it was built from.
static FLOORS: LazyLock<Mutex<HashMap<Uuid, (String, Arc<SiteFloor>)>>> = LazyLock::new(Default::default);

async fn floor_for(tx: &Transaction<'_>, site: Uuid) -> Result<Option<(Arc<SiteFloor>, Option<i32>)>, ApiError> {
    let cell_mm: Option<i32> = tx
        .query_opt("SELECT cell_mm FROM site WHERE id = $1", &[&site])
        .await?
        .ok_or(ApiError::NotFound)?
        .get(0);
    let version = format!("{}:{:?}", crate::places::layout_version(tx, site).await?, cell_mm);
    if let Some((v, f)) = FLOORS.lock().unwrap_or_else(|p| p.into_inner()).get(&site) {
        if *v == version {
            return Ok(Some((f.clone(), cell_mm)));
        }
    }
    let rows = tx
        .query(
            "SELECT id, parent_id, solid, x, y, z, length, depth, turn, outline, bays, rows
               FROM place WHERE site_id = $1",
            &[&site],
        )
        .await?;
    let boxes: HashMap<Uuid, (Option<Uuid>, f64, f64, f64, f64)> = rows
        .iter()
        .map(|r| (r.get(0), (r.get(1), r.get(3), r.get(4), r.get(5), r.get::<_, i16>(8) as f64)))
        .collect();
    let frames = layout::frames(&boxes);
    let (mut walkways, mut solids) = (vec![], vec![]);
    let mut places = HashMap::new();
    for r in &rows {
        let id: Uuid = r.get(0);
        let Some(frame) = frames.get(&id).copied() else { continue };
        let (solid, length, depth): (bool, f64, f64) = (r.get(2), r.get(6), r.get(7));
        let outline: Option<Vec<f64>> = r.get(9);
        let ring: Vec<Pt> = layout::footprint(&frame, length, depth, outline.as_deref());
        if solid {
            solids.push(ring);
        } else {
            walkways.push(ring);
        }
        places.insert(id, Shape { frame, length, depth, bays: r.get(10), rows: r.get(11), solid });
    }
    // A quarter of a metre once the site is measured, half a cell before.
    let step = cell_mm.map(|mm| 250.0 / mm as f64).unwrap_or(0.5);
    let Some(floor) = Floor::build(&walkways, &solids, step) else {
        return Ok(None);
    };
    let reach = cell_mm.map(|mm| 1500.0 / mm as f64).unwrap_or(1.5).max(floor.step() * 2.0);
    let built = Arc::new(SiteFloor { floor, places, reach });
    FLOORS.lock().unwrap_or_else(|p| p.into_inner()).insert(site, (version, built.clone()));
    Ok(Some((built, cell_mm)))
}

/// Where a picker stands for a bin: in the aisle its face opens onto, or, for
/// a spot on a walk-through floor, on the spot.
fn stand(site: &SiteFloor, at: &Whereabouts) -> Option<usize> {
    let shape = site.places.get(&at.place_id)?;
    if shape.solid {
        let (p, out) = routing::face(&shape.frame, shape.length, shape.depth, shape.bays, at.cell);
        site.floor.standing(p, out, site.reach)
    } else {
        let u = (at.cell.bay as f64 - 0.5) * shape.length / shape.bays.max(1) as f64;
        let v = (at.cell.row as f64 - 0.5) * shape.depth / shape.rows.max(1) as f64;
        site.floor.node_of(shape.frame.point(u, v)).filter(|&n| site.floor.is_open(n))
    }
}

/// The walk's lines in route order, by their index in the typed order, and
/// the route. Nothing when none of the walk's bins is on the layout.
pub async fn route(
    tx: &Transaction<'_>,
    site: Uuid,
    lines: &[Option<Whereabouts>],
) -> Result<Option<(Vec<usize>, WalkRoute)>, ApiError> {
    if lines.iter().all(Option::is_none) {
        return Ok(None);
    }
    let Some((floor, cell_mm)) = floor_for(tx, site).await? else {
        return Ok(None);
    };

    // One stop per spot to stand on, in the order the typed walk reaches them.
    let mut stops: Vec<usize> = vec![];
    let mut index: HashMap<usize, usize> = HashMap::new();
    let stop_of: Vec<Option<usize>> = lines
        .iter()
        .map(|line| {
            let node = stand(&floor, line.as_ref()?)?;
            Some(*index.entry(node).or_insert_with(|| {
                stops.push(node);
                stops.len() - 1
            }))
        })
        .collect();
    if stops.is_empty() {
        return Ok(None);
    }

    // The packing bench, when the site's packing location is on the layout.
    let base = tx
        .query_opt(
            "SELECT l.place_id, l.slot_bay, l.slot_level, l.slot_row, l.slot_position, coalesce(l.slot_side, 1)::int
               FROM site s JOIN location l ON l.id = s.pack_location_id
              WHERE s.id = $1 AND l.place_id IS NOT NULL AND l.slot_bay IS NOT NULL",
            &[&site],
        )
        .await?
        .and_then(|r| {
            let at = Whereabouts {
                place_id: r.get(0),
                cell: GridCell { bay: r.get(1), level: r.get(2), row: r.get(3), position: r.get(4), side: r.get(5) },
            };
            stand(&floor, &at)
        });

    let planned = routing::plan(&floor.floor, base, &stops);
    let mut placed = vec![false; lines.len()];
    let mut order = Vec::with_capacity(lines.len());
    for stop in &planned.order {
        for (i, s) in stop_of.iter().enumerate() {
            if *s == Some(*stop) {
                order.push(i);
                placed[i] = true;
            }
        }
    }
    let off_route = placed.iter().filter(|p| !**p).count() as i64;
    order.extend((0..lines.len()).filter(|&i| !placed[i]));

    Ok(Some((
        order,
        WalkRoute {
            from: if base.is_some() { "pack" } else { "first" }.into(),
            cell_mm,
            walked: planned.walked,
            typed: planned.typed,
            stops: planned.order.len() as i64,
            off_route,
            path: planned.path,
        },
    )))
}
