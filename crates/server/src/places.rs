//! Places, read and drafted. D173.
//!
//! [`crate::layout`] decides what a place's grid, pattern and position mean.
//! This reads the rows and answers the three questions a screen asks:
//!
//! - **Where is this bin?** `GET /bins/{id}`: the bin, its cell, and the place
//!   that holds it, drawn as the face a worker would stand in front of.
//! - **What is here?** `GET /places/{id}`: one place, the way up out of it, the
//!   places inside it, its bins by cell, and a plan to find it on.
//! - **What does this site have?** `GET /layout`: every place, and how many bins
//!   are not on the layout yet.
//!
//! And it writes two things so far: `POST /layout/draft`, a first layout from
//! the bin list, which is how a site gets one without anybody drawing (like
//! every import, a dry run unless told to apply, and the dry run is the real
//! write rolled back); and `POST /places/{id}/reach`, how many of a rack's
//! levels can be reached from the floor (D180).

use std::collections::{BTreeSet, HashMap, HashSet};

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::error::ApiError;
use crate::layout::{self, GridCell, Frame, Grid, Pattern};
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// How many codes a report lists before it only counts.
const SAMPLE: usize = 20;

/// Whether a bin can be reached from the floor, as SQL over a `location`
/// aliased `loc` (D180).
///
/// **One rule, wherever a bin is chosen to walk to.** A bin on the layout is
/// in reach when its level is among the rack's levels in reach, counted up
/// from the floor. A bin not on the layout yet has no level to count, so the
/// other system's bin type stands in: its "Pick" bins are the bottom level,
/// and nothing else of it says anything about reach.
pub fn within_reach(loc: &str) -> String {
    format!(
        "(CASE WHEN {loc}.place_id IS NOT NULL
               THEN {loc}.slot_level <= (SELECT rp.reach_levels FROM place rp WHERE rp.id = {loc}.place_id)
               ELSE {loc}.kind = 'pick_face' END)"
    )
}

/// The location kinds that are racking or shelving: solid, where a picker
/// reaches in from an aisle. Staging and docks are floor.
const SOLID_KINDS: &[&str] = &["pick_face", "bulk", "overflow"];

/// A place as its row says.
#[derive(Clone, Debug)]
struct Row {
    id: Uuid,
    parent_id: Option<Uuid>,
    name: String,
    solid: bool,
    x: f64,
    y: f64,
    z: f64,
    length: f64,
    depth: f64,
    height: f64,
    turn: f64,
    outline: Option<Vec<f64>>,
    grid: Grid,
    pattern: Option<String>,
    reach_levels: i32,
}

const PLACE_COLUMNS: &str = "id, parent_id, name, solid, x, y, z, length, depth, height, turn,
                             outline, bays, levels, rows, positions, first_bay, bay_step,
                             first_level, bin_pattern, sides, reach_levels";

fn row(r: &tokio_postgres::Row) -> Row {
    Row {
        id: r.get(0),
        parent_id: r.get(1),
        name: r.get(2),
        solid: r.get(3),
        x: r.get(4),
        y: r.get(5),
        z: r.get(6),
        length: r.get(7),
        depth: r.get(8),
        height: r.get(9),
        turn: r.get::<_, i16>(10) as f64,
        outline: r.get(11),
        grid: Grid {
            bays: r.get(12),
            levels: r.get(13),
            rows: r.get(14),
            positions: r.get::<_, Option<Vec<i32>>>(15).unwrap_or_default(),
            first_bay: r.get(16),
            bay_step: r.get(17),
            first_level: r.get(18),
            sides: r.get::<_, i16>(20) as i32,
        },
        pattern: r.get(19),
        reach_levels: r.get::<_, i16>(21) as i32,
    }
}

async fn site_places(tx: &Transaction<'_>, site: Uuid) -> Result<Vec<Row>, ApiError> {
    Ok(tx
        .query(
            &format!("SELECT {PLACE_COLUMNS} FROM place WHERE site_id = $1 ORDER BY name, id"),
            &[&site],
        )
        .await?
        .iter()
        .map(row)
        .collect())
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// A step on the way up out of a place.
#[derive(Serialize, Debug)]
pub struct PlaceCrumb {
    pub place_id: Uuid,
    pub name: String,
}

/// A place inside the one being shown.
#[derive(Serialize, Debug)]
pub struct ChildPlace {
    pub place_id: Uuid,
    pub name: String,
    pub solid: bool,
    /// Bins in it and in everything inside it.
    pub bins: i64,
}

/// A bin in a cell of the place being shown.
#[derive(Serialize, Debug)]
pub struct CellBin {
    pub location_id: Uuid,
    pub code: String,
    pub kind: String,
    pub cell: GridCell,
}

/// One place on the plan, as a footprint on the site.
#[derive(Serialize, Debug)]
pub struct PlanShape {
    pub place_id: Uuid,
    pub name: String,
    pub solid: bool,
    /// How many places it is inside, counted from the plan's outermost.
    pub nesting: usize,
    /// Corners on the site, in cells: `[x, y]` in order around the edge.
    pub corners: Vec<[f64; 2]>,
    /// How far above the site's floor it starts, and how tall it is, in cells.
    pub z: f64,
    pub height: f64,
}

/// One place, as a page.
#[derive(Serialize, Debug)]
pub struct PlaceView {
    pub place_id: Uuid,
    pub name: String,
    pub solid: bool,
    pub bays: i32,
    pub levels: i32,
    pub rows: i32,
    /// How many bins share a bay at each level, lowest first.
    pub positions: Vec<i32>,
    pub pattern: Option<String>,
    /// 1, or 2 for a rack with a face on each side, numbered round it.
    pub sides: i32,
    /// The numbers on the bays' labels, left to right as you face it.
    pub bay_labels: Vec<String>,
    /// The back's, left to right as you face the back: the columns from the
    /// far end round to the first. Empty with one side.
    pub back_labels: Vec<String>,
    /// The numbers on the levels' labels, lowest first.
    pub level_labels: Vec<String>,
    /// How many of its levels, from the floor up, can be reached without a
    /// forklift (D180).
    pub reach_levels: i32,
    /// From the outermost place down to this one's parent.
    pub trail: Vec<PlaceCrumb>,
    pub children: Vec<ChildPlace>,
    pub bins: Vec<CellBin>,
    /// The outermost place this is in, and everything inside it.
    pub plan: Vec<PlanShape>,
}

/// A bin, and where it is.
#[derive(Serialize, Debug)]
pub struct BinView {
    pub location_id: Uuid,
    pub code: String,
    pub kind: String,
    pub active: bool,
    /// Its cell, when it is on the layout.
    pub cell: Option<GridCell>,
    pub place: Option<PlaceView>,
}

/// Build a place's page from the site's places.
async fn place_view(
    tx: &Transaction<'_>,
    place_id: Uuid,
) -> Result<Option<PlaceView>, ApiError> {
    let Some(site): Option<Uuid> = tx
        .query_opt("SELECT site_id FROM place WHERE id = $1", &[&place_id])
        .await?
        .map(|r| r.get(0))
    else {
        return Ok(None);
    };
    let places = site_places(tx, site).await?;
    let by_id: HashMap<Uuid, &Row> = places.iter().map(|p| (p.id, p)).collect();
    let me = by_id[&place_id];

    // Up: the trail, guarded against a loop (J78) so a bad row cannot hang a
    // request.
    let mut trail = vec![];
    let mut seen = HashSet::from([place_id]);
    let mut at = me.parent_id;
    while let Some(p) = at.and_then(|id| by_id.get(&id)) {
        if !seen.insert(p.id) {
            break;
        }
        trail.push(PlaceCrumb { place_id: p.id, name: p.name.clone() });
        at = p.parent_id;
    }
    trail.reverse();
    let root = trail.first().map(|c| c.place_id).unwrap_or(place_id);

    // Bins per place, and everything under a place counted into it.
    let counts: HashMap<Uuid, i64> = tx
        .query(
            "SELECT place_id, count(*) FROM location
              WHERE site_id = $1 AND place_id IS NOT NULL GROUP BY place_id",
            &[&site],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    let mut kids: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for p in &places {
        if let Some(parent) = p.parent_id {
            kids.entry(parent).or_default().push(p.id);
        }
    }
    let under = |start: Uuid| -> Vec<(Uuid, usize)> {
        let mut out = vec![];
        let mut stack = vec![(start, 0usize)];
        let mut visited = HashSet::new();
        while let Some((id, depth)) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            out.push((id, depth));
            for k in kids.get(&id).into_iter().flatten().rev() {
                stack.push((*k, depth + 1));
            }
        }
        out
    };

    let children = kids
        .get(&place_id)
        .into_iter()
        .flatten()
        .map(|k| {
            let p = by_id[k];
            ChildPlace {
                place_id: p.id,
                name: p.name.clone(),
                solid: p.solid,
                bins: under(p.id).iter().map(|(id, _)| counts.get(id).copied().unwrap_or(0)).sum(),
            }
        })
        .collect();

    let bins = tx
        .query(
            "SELECT id, code, kind, slot_bay, slot_level, slot_row, slot_position, slot_side
               FROM location WHERE place_id = $1
              ORDER BY slot_side, slot_level, slot_bay, slot_row, slot_position",
            &[&place_id],
        )
        .await?
        .iter()
        .map(|r| CellBin {
            location_id: r.get(0),
            code: r.get(1),
            kind: r.get(2),
            cell: GridCell {
                bay: r.get(3),
                level: r.get(4),
                row: r.get(5),
                position: r.get(6),
                side: r.get::<_, i16>(7) as i32,
            },
        })
        .collect();

    let boxes: HashMap<Uuid, (Option<Uuid>, f64, f64, f64, f64)> = places
        .iter()
        .map(|p| (p.id, (p.parent_id, p.x, p.y, p.z, p.turn)))
        .collect();
    let frames = layout::frames(&boxes);
    let plan = under(root)
        .into_iter()
        .filter_map(|(id, nesting)| {
            let p = by_id[&id];
            let f: &Frame = frames.get(&id)?;
            Some(PlanShape {
                place_id: id,
                name: p.name.clone(),
                solid: p.solid,
                nesting,
                corners: layout::footprint(f, p.length, p.depth, p.outline.as_deref()),
                z: f.z,
                height: p.height,
            })
        })
        .collect();

    let pattern = me.pattern.as_deref().and_then(|s| Pattern::parse(s).ok());
    let (bay_labels, level_labels) = layout::labels(&me.grid, pattern.as_ref());
    let back_labels = layout::back_labels(&me.grid, pattern.as_ref());
    Ok(Some(PlaceView {
        place_id,
        name: me.name.clone(),
        solid: me.solid,
        bays: me.grid.bays,
        levels: me.grid.levels,
        rows: me.grid.rows,
        sides: me.grid.sides,
        positions: (1..=me.grid.levels).map(|l| me.grid.positions_at(l)).collect(),
        pattern: me.pattern.clone(),
        bay_labels,
        back_labels,
        level_labels,
        reach_levels: me.reach_levels,
        trail,
        children,
        bins,
        plan,
    }))
}

/// A bin, the cell it is in, and the place around it.
///
/// **A scan lands here.** D111's locator resolves a location code to its id;
/// this is the page that answers "where is it", which is the question a bin
/// code is scanned to ask.
#[get("/bins/{location_id}")]
pub async fn bin_page(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let id = path.into_inner();
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let view = scope
        .run(|tx| {
            Box::pin(async move {
                let Some(r) = tx
                    .query_opt(
                        "SELECT code, kind, active, place_id, slot_bay, slot_level, slot_row,
                                slot_position, slot_side
                           FROM location WHERE id = $1",
                        &[&id],
                    )
                    .await?
                else {
                    return Err(ApiError::NotFound);
                };
                let place_id: Option<Uuid> = r.get(3);
                let cell = place_id.map(|_| GridCell {
                    bay: r.get(4),
                    level: r.get(5),
                    row: r.get(6),
                    position: r.get(7),
                    side: r.get::<_, i16>(8) as i32,
                });
                let place = match place_id {
                    Some(p) => place_view(tx, p).await?,
                    None => None,
                };
                Ok(BinView {
                    location_id: id,
                    code: r.get(0),
                    kind: r.get(1),
                    active: r.get(2),
                    cell,
                    place,
                })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(view))
}

#[get("/places/{place_id}")]
pub async fn place_page(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let id = path.into_inner();
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let view = scope
        .run(|tx| Box::pin(async move { place_view(tx, id).await?.ok_or(ApiError::NotFound) }))
        .await?;
    Ok(HttpResponse::Ok().json(view))
}

/// One place in the site's list.
#[derive(Serialize, Debug)]
pub struct LayoutPlace {
    pub place_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub solid: bool,
    pub bays: i32,
    pub levels: i32,
    pub rows: i32,
    /// 1, or 2 for a rack with a face on each side.
    pub sides: i32,
    /// How many bins share a bay at each level, lowest first: where the map
    /// splits a bay (D208).
    pub positions: Vec<i32>,
    pub pattern: Option<String>,
    /// How many of its levels, from the floor up, can be reached without a
    /// forklift (D180).
    pub reach_levels: i32,
    /// Bins in its own cells, not counting places inside it.
    pub bins: i64,
}

/// A site's layout, as a list.
#[derive(Serialize, Debug)]
pub struct LayoutView {
    pub site_code: String,
    pub places: Vec<LayoutPlace>,
    /// Active bins at the site.
    pub bins: i64,
    /// Active bins with no cell: what J77 reports once the site has places.
    pub unplaced: i64,
    pub unplaced_sample: Vec<String>,
    /// Every place on the site, as footprints: what the warehouse's plan and
    /// its 3D view draw.
    pub plan: Vec<PlanShape>,
}

/// Every place on the site as it stands there, outermost first.
///
/// The same composition a place's page draws its plan from, over every place
/// standing on the site rather than the one around a single place: positions
/// are worked out from the chain of parents when read, never stored (D173).
fn site_plan(places: &[Row]) -> Vec<PlanShape> {
    let by_id: HashMap<Uuid, &Row> = places.iter().map(|p| (p.id, p)).collect();
    let mut kids: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    let mut roots = vec![];
    for p in places {
        match p.parent_id.filter(|id| by_id.contains_key(id)) {
            Some(parent) => kids.entry(parent).or_default().push(p.id),
            // A parent that is not on the site is treated as the site, so a
            // place is never lost from the plan.
            None => roots.push(p.id),
        }
    }
    let boxes: HashMap<Uuid, (Option<Uuid>, f64, f64, f64, f64)> = places
        .iter()
        .map(|p| (p.id, (p.parent_id, p.x, p.y, p.z, p.turn)))
        .collect();
    let frames = layout::frames(&boxes);
    let mut out = vec![];
    let mut visited = HashSet::new();
    for root in roots {
        let mut stack = vec![(root, 0usize)];
        while let Some((id, nesting)) = stack.pop() {
            // J78's guard: a loop in the parents cannot hang the read.
            if !visited.insert(id) {
                continue;
            }
            let (Some(p), Some(f)) = (by_id.get(&id), frames.get(&id)) else { continue };
            out.push(PlanShape {
                place_id: id,
                name: p.name.clone(),
                solid: p.solid,
                nesting,
                corners: layout::footprint(f, p.length, p.depth, p.outline.as_deref()),
                z: f.z,
                height: p.height,
            });
            for k in kids.get(&id).into_iter().flatten().rev() {
                stack.push((*k, nesting + 1));
            }
        }
    }
    out
}

/// The site the caller is working at, or a refusal that says what to do.
fn working_site(site: Option<Uuid>) -> Result<Uuid, ApiError> {
    site.ok_or_else(|| {
        ApiError::Rejected("choose the warehouse you are working at to see its layout".into())
    })
}

#[get("/layout")]
pub async fn site_layout(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let site = working_site(who.site_id)?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let view = scope
        .run(|tx| {
            Box::pin(async move {
                let site_code: String = tx
                    .query_opt("SELECT code FROM site WHERE id = $1", &[&site])
                    .await?
                    .ok_or(ApiError::NotFound)?
                    .get(0);
                let counts: HashMap<Uuid, i64> = tx
                    .query(
                        "SELECT place_id, count(*) FROM location
                          WHERE site_id = $1 AND place_id IS NOT NULL GROUP BY place_id",
                        &[&site],
                    )
                    .await?
                    .iter()
                    .map(|r| (r.get(0), r.get(1)))
                    .collect();
                let rows = site_places(tx, site).await?;
                let plan = site_plan(&rows);
                let places = rows
                    .into_iter()
                    .map(|p| LayoutPlace {
                        place_id: p.id,
                        parent_id: p.parent_id,
                        bins: counts.get(&p.id).copied().unwrap_or(0),
                        name: p.name,
                        solid: p.solid,
                        bays: p.grid.bays,
                        levels: p.grid.levels,
                        rows: p.grid.rows,
                        sides: p.grid.sides,
                        positions: (1..=p.grid.levels).map(|l| p.grid.positions_at(l)).collect(),
                        pattern: p.pattern,
                        reach_levels: p.reach_levels,
                    })
                    .collect();
                let r = tx
                    .query_one(
                        "SELECT count(*), count(*) FILTER (WHERE place_id IS NULL)
                           FROM location WHERE site_id = $1 AND active",
                        &[&site],
                    )
                    .await?;
                let unplaced_sample = tx
                    .query(
                        "SELECT code FROM location
                          WHERE site_id = $1 AND active AND place_id IS NULL
                          ORDER BY code LIMIT $2",
                        &[&site, &(SAMPLE as i64)],
                    )
                    .await?
                    .iter()
                    .map(|r| r.get(0))
                    .collect();
                Ok(LayoutView {
                    site_code,
                    places,
                    bins: r.get(0),
                    unplaced: r.get(1),
                    unplaced_sample,
                    plan,
                })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(view))
}

// ---------------------------------------------------------------------------
// The bins, as a list
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct BinsQuery {
    /// The bins in this place's own cells.
    pub place: Option<Uuid>,
    /// The bins in no cell at all: the tray the draft left.
    #[serde(default)]
    pub unplaced: bool,
    /// Part of a bin's code.
    pub q: Option<String>,
}

/// Something NetSuite last reported on a shelf.
#[derive(Serialize, Debug)]
pub struct BinContent {
    pub item_id: Uuid,
    pub item_code: String,
    /// Text, as the report carried it.
    pub on_hand: String,
}

/// One bin, where it is, and what each record says is in it.
#[derive(Serialize, Debug)]
pub struct BinRow {
    pub location_id: Uuid,
    pub code: String,
    pub kind: String,
    pub place_id: Option<Uuid>,
    pub place_name: Option<String>,
    pub cell: Option<GridCell>,
    /// Its cell in the words the rack's labels use: "bay 05, level 3".
    pub whereabouts: Option<String>,
    pub pick_sequence: Option<i32>,
    /// Whether it can be reached from the floor, without a forklift (D180).
    pub within_reach: bool,
    /// What NetSuite's last inventory balance put on this shelf, most first,
    /// three at most; and how many items in all.
    pub reported: Vec<BinContent>,
    pub reported_items: i64,
    /// What this system's own ledger holds here.
    pub held: i64,
}

#[derive(Serialize, Debug)]
pub struct BinsList {
    pub bins: Vec<BinRow>,
    /// How many match, when that is more than were sent.
    pub total: i64,
}

/// How many bins one read sends. A rack is a few hundred; the site is
/// thousands, and a screen asks for one place at a time.
const BINS_AT_ONCE: i64 = 500;

/// The bins at the caller's site, in code order, with what is in them.
#[get("/bins")]
pub async fn bin_list(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<BinsQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let site = working_site(who.site_id)?;
    let place = query.place;
    let unplaced = query.unplaced;
    let like = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(|q| format!("%{}%", q.replace(['%', '_'], "")));
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                // Each place's grid and pattern, so a cell reads as the rack's
                // labels do.
                let grids: HashMap<Uuid, (Grid, Option<Pattern>)> = site_places(tx, site)
                    .await?
                    .into_iter()
                    .map(|p| {
                        let pattern = p.pattern.as_deref().and_then(|s| Pattern::parse(s).ok());
                        (p.id, (p.grid, pattern))
                    })
                    .collect();
                let sql = format!(
                        "SELECT l.id, l.code, l.kind, l.place_id, p.name,
                                l.slot_bay, l.slot_level, l.slot_row, l.slot_position,
                                l.pick_sequence,
                                coalesce(rep.items, 0), rep.ids, rep.codes, rep.qtys,
                                coalesce(held.q, 0)::bigint,
                                count(*) OVER (),
                                l.slot_side,
                                {reach}
                           FROM location l
                           LEFT JOIN place p ON p.id = l.place_id
                           -- The report totalled once for the site: it has no
                           -- index by bin, and a lookup per bin read it whole
                           -- each time.
                           LEFT JOIN (
                               SELECT rs.location_id,
                                      count(*) AS items,
                                      (array_agg(i.id ORDER BY rs.on_hand DESC, i.code))[1:3] AS ids,
                                      (array_agg(i.code ORDER BY rs.on_hand DESC, i.code))[1:3] AS codes,
                                      (array_agg(rs.on_hand::text ORDER BY rs.on_hand DESC, i.code))[1:3]
                                          AS qtys
                                 FROM reported_stock rs
                                 JOIN item i ON i.id = rs.item_id
                                WHERE rs.site_id = $1 AND rs.location_id IS NOT NULL
                                GROUP BY rs.location_id
                           ) rep ON rep.location_id = l.id
                           LEFT JOIN LATERAL (
                               SELECT sum(s.quantity) AS q FROM stock s
                                WHERE s.holder_location_id = l.id AND s.quantity > 0
                           ) held ON true
                          WHERE l.site_id = $1 AND l.active
                            AND ($2::uuid IS NULL OR l.place_id = $2)
                            AND (NOT $3::bool OR l.place_id IS NULL)
                            AND ($4::text IS NULL OR l.code ILIKE $4)
                          ORDER BY l.code
                          LIMIT $5",
                    reach = within_reach("l")
                );
                let rows = tx
                    .query(&sql, &[&site, &place, &unplaced, &like, &BINS_AT_ONCE])
                    .await?;
                let total: i64 = rows.first().map(|r| r.get(15)).unwrap_or(0);
                let bins = rows
                    .iter()
                    .map(|r| {
                        let place_id: Option<Uuid> = r.get(3);
                        let cell = r.get::<_, Option<i32>>(5).map(|bay| GridCell {
                            bay,
                            level: r.get(6),
                            row: r.get(7),
                            position: r.get(8),
                            side: r.get::<_, Option<i16>>(16).unwrap_or(1) as i32,
                        });
                        let whereabouts = match (place_id.and_then(|p| grids.get(&p)), cell) {
                            (Some((grid, pattern)), Some(c)) => Some(layout::whereabouts(grid, pattern.as_ref(), c)),
                            _ => None,
                        };
                        let ids: Vec<Uuid> = r.get::<_, Option<Vec<Uuid>>>(11).unwrap_or_default();
                        let codes: Vec<String> = r.get::<_, Option<Vec<String>>>(12).unwrap_or_default();
                        let qtys: Vec<String> = r.get::<_, Option<Vec<String>>>(13).unwrap_or_default();
                        BinRow {
                            location_id: r.get(0),
                            code: r.get(1),
                            kind: r.get(2),
                            place_id,
                            place_name: r.get(4),
                            cell,
                            whereabouts,
                            pick_sequence: r.get(9),
                            within_reach: r.get(17),
                            reported: ids
                                .into_iter()
                                .zip(codes)
                                .zip(qtys)
                                .map(|((item_id, item_code), on_hand)| BinContent { item_id, item_code, on_hand })
                                .collect(),
                            reported_items: r.get(10),
                            held: r.get(14),
                        }
                    })
                    .collect();
                Ok(BinsList { bins, total })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

// ---------------------------------------------------------------------------
// The bin map
// ---------------------------------------------------------------------------

/// One bin as the map draws it: its cell, and enough to colour it (D208).
#[derive(Serialize, Debug)]
pub struct MapBin {
    pub location_id: Uuid,
    pub code: String,
    pub place_id: Uuid,
    pub side: i32,
    pub bay: i32,
    pub level: i32,
    pub row: i32,
    pub position: i32,
    /// Whether it can be reached from the floor, without a forklift (D180).
    pub within_reach: bool,
    /// How many items NetSuite's last inventory balance put on this shelf,
    /// and how many units of them in all.
    pub reported_items: i64,
    pub reported_on_hand: f64,
    /// What this system's own ledger holds here.
    pub held: i64,
}

#[derive(Serialize, Debug)]
pub struct MapBins {
    pub bins: Vec<MapBin>,
    /// Active bins in no cell, which the map cannot draw.
    pub unplaced: i64,
}

/// Every bin in a cell at the caller's site, at once (D208).
///
/// The map draws the whole site, so it reads every placed bin in one call,
/// a couple of thousand small rows, where the list pages them. What is on a
/// shelf is summed rather than listed: the map colours by it, and the bin
/// chosen on it is read on its own.
#[get("/layout/bins")]
pub async fn map_bins(req: HttpRequest, state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let site = working_site(who.site_id)?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let sql = format!(
                    "SELECT l.id, l.code, l.place_id,
                            coalesce(l.slot_side, 1)::int, l.slot_bay, l.slot_level, l.slot_row, l.slot_position,
                            {reach},
                            coalesce(rep.items, 0), coalesce(rep.units, 0)::float8,
                            coalesce(held.q, 0)::bigint
                       FROM location l
                       -- The report and the ledger, each totalled once for the
                       -- site rather than looked up bin by bin.
                       LEFT JOIN (
                           SELECT rs.location_id, count(*) AS items, sum(rs.on_hand) AS units
                             FROM reported_stock rs
                            WHERE rs.site_id = $1 AND rs.location_id IS NOT NULL
                            GROUP BY rs.location_id
                       ) rep ON rep.location_id = l.id
                       LEFT JOIN (
                           SELECT s.holder_location_id, sum(s.quantity) AS q
                             FROM stock s
                            WHERE s.holder_location_id IS NOT NULL AND s.quantity > 0
                            GROUP BY s.holder_location_id
                       ) held ON held.holder_location_id = l.id
                      WHERE l.site_id = $1 AND l.active
                        AND l.place_id IS NOT NULL AND l.slot_bay IS NOT NULL
                      ORDER BY l.code",
                    reach = within_reach("l")
                );
                let rows = tx.query(&sql, &[&site]).await?;
                let bins = rows
                    .iter()
                    .map(|r| MapBin {
                        location_id: r.get(0),
                        code: r.get(1),
                        place_id: r.get(2),
                        side: r.get(3),
                        bay: r.get(4),
                        level: r.get(5),
                        row: r.get(6),
                        position: r.get(7),
                        within_reach: r.get(8),
                        reported_items: r.get(9),
                        reported_on_hand: r.get(10),
                        held: r.get(11),
                    })
                    .collect();
                let unplaced: i64 = tx
                    .query_one(
                        "SELECT count(*) FROM location
                          WHERE site_id = $1 AND active AND (place_id IS NULL OR slot_bay IS NULL)",
                        &[&site],
                    )
                    .await?
                    .get(0);
                Ok(MapBins { bins, unplaced })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

// ---------------------------------------------------------------------------
// Reach
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct ReachRequest {
    /// How many of its levels, counting up from its floor, can be reached
    /// without a forklift. Zero: none of it.
    pub levels: i32,
}

/// What a place says of reach, after saying it.
#[derive(Serialize, Debug)]
pub struct ReachSaid {
    pub place_id: Uuid,
    pub reach_levels: i32,
    pub levels: i32,
}

/// Say how many of a rack's levels can be reached from the floor (D180).
///
/// Part of the drawing, like the rest of a place: set, not appended. Who drew
/// what is the layout's history, which is not built yet for any of it.
#[post("/places/{place_id}/reach")]
pub async fn set_reach(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<ReachRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let id = path.into_inner();
    let levels = body.levels;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let has: i32 = tx
                    .query_opt("SELECT levels FROM place WHERE id = $1", &[&id])
                    .await?
                    .ok_or(ApiError::NotFound)?
                    .get(0);
                if !(0..=has).contains(&levels) {
                    return Err(ApiError::Rejected(format!(
                        "it has {has} level{}; from 0 to {has} of them can be in reach, not {levels}",
                        if has == 1 { "" } else { "s" }
                    )));
                }
                tx.execute("UPDATE place SET reach_levels = $2 WHERE id = $1", &[&id, &(levels as i16)])
                    .await?;
                Ok(ReachSaid { place_id: id, reach_levels: levels, levels: has })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

// ---------------------------------------------------------------------------
// Drafting
// ---------------------------------------------------------------------------

/// A place the draft made, or would make.
#[derive(Serialize, Debug)]
pub struct DraftedPlace {
    pub name: String,
    pub solid: bool,
    pub pattern: String,
    pub bays: i32,
    pub levels: i32,
    pub bins: usize,
    /// 2 when it was made as a rack with a face on each side.
    pub sides: i32,
    /// How its bays would share out between two sides, as its labels read
    /// (`["01–18", "19–36"]`); none for a place that cannot have two.
    pub split: Option<[String; 2]>,
}

/// What drafting did, or would have done.
#[derive(Serialize, Debug, Default)]
pub struct DraftReport {
    /// The place the draft put new places in.
    pub inside: Option<String>,
    /// True when that place was made by the draft rather than already there.
    pub inside_created: bool,
    pub places: Vec<DraftedPlace>,
    /// Bins that dropped into places already on the layout, by their patterns.
    pub bins_filled: usize,
    /// Bins placed in the places the draft made.
    pub bins_placed: usize,
    /// Active bins still with no cell: they wait to be placed by hand.
    pub unplaced: usize,
    pub unplaced_sample: Vec<String>,
    /// The places a person said not to make, by name. Their bins are counted
    /// in `unplaced`.
    pub left_out: Vec<String>,
    pub applied: bool,
}

#[derive(Deserialize, Debug)]
pub struct DraftQuery {
    /// Absent means a dry run, as for every import.
    #[serde(default)]
    pub apply: bool,
}

/// What a person changed about the draft before applying it. Optional: no
/// body is the draft as proposed.
#[derive(Deserialize, Debug, Default)]
pub struct DraftRequest {
    /// Places not to make, by the names the preview gave them: families of
    /// codes that are no rack at all. Their bins wait in the tray.
    #[serde(default)]
    pub leave_out: Vec<String>,
    /// Racks with a face on each side, numbered round them
    /// (`layout::two_sides`), by the names the preview gave them.
    #[serde(default)]
    pub two_sided: Vec<String>,
}

/// How a proposed place's bays would share out between two sides, as its
/// labels read: the front's first to last, and the back's on to the family's
/// last bay.
fn split_of(d: &layout::Drafted) -> Option<[String; 2]> {
    if !d.solid {
        return None;
    }
    let two = layout::two_sides(&d.grid)?;
    let pattern = Pattern::parse(&d.pattern).ok();
    let (whole, _) = layout::labels(&d.grid, pattern.as_ref());
    let span = |from: usize, to: usize| match (whole.get(from), whole.get(to)) {
        (Some(a), Some(b)) if from != to => Some(format!("{a}–{b}")),
        (Some(a), _) => Some(a.clone()),
        _ => None,
    };
    let front = two.bays as usize;
    Some([span(0, front - 1)?, span(front, whole.len() - 1)?])
}

/// `base`, or `base (2)`, `base (3)`… whichever nothing in `used` is called,
/// and now taken.
fn unused_name(used: &mut HashSet<String>, base: &str) -> String {
    let mut name = base.to_string();
    let mut n = 1;
    while used.contains(&name) {
        n += 1;
        name = format!("{base} ({n})");
    }
    used.insert(name.clone());
    name
}

/// Put the bins that are not on the layout into it.
///
/// First into the places already there, where a place's pattern names a bin
/// and the cell is free. Then the rest are drafted into new places, one per
/// family of codes, laid out in a row of aisles inside the site's first
/// walk-through place (made, if there is none). **Nothing already in a cell
/// moves**: which bin is where is the exact half of the layout, and a person
/// may have put it there.
///
/// A place named in `leave_out` is not made and takes no row; its bins stay
/// in the tray. A rack named in `two_sided` is made as its two sides, back to
/// back. The names are worked out the same way whatever is asked, so the
/// names a preview showed are the names an apply matches. A name the draft
/// does not propose is refused rather than ignored, because a bin list that
/// changed since the preview could otherwise make what was left out under
/// another name.
pub async fn draft(
    tx: &Transaction<'_>,
    tenant: Uuid,
    site: Uuid,
    apply: bool,
    asked: &DraftRequest,
) -> Result<DraftReport, ApiError> {
    tx.batch_execute("SAVEPOINT spork_draft").await?;
    let mut out = DraftReport { applied: apply, ..Default::default() };

    let places = site_places(tx, site).await?;
    let mut waiting: Vec<(Uuid, String, String)> = tx
        .query(
            "SELECT id, code, kind FROM location
              WHERE site_id = $1 AND active AND place_id IS NULL ORDER BY code",
            &[&site],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get(2)))
        .collect();

    // Into the places already drawn.
    let mut taken: HashSet<(Uuid, GridCell)> = tx
        .query(
            "SELECT place_id, slot_bay, slot_level, slot_row, slot_position, slot_side FROM location
              WHERE site_id = $1 AND place_id IS NOT NULL",
            &[&site],
        )
        .await?
        .iter()
        .map(|r| {
            let cell = GridCell {
                bay: r.get(1),
                level: r.get(2),
                row: r.get(3),
                position: r.get(4),
                side: r.get::<_, i16>(5) as i32,
            };
            (r.get(0), cell)
        })
        .collect();
    let mut names: HashMap<String, (Uuid, GridCell)> = HashMap::new();
    for p in &places {
        let Some(pattern) = p.pattern.as_deref().and_then(|s| Pattern::parse(s).ok()) else {
            continue;
        };
        if let Ok(named) = pattern.names(&p.grid) {
            for (cell, name) in named {
                names.entry(name).or_insert((p.id, cell));
            }
        }
    }
    let mut fill: Vec<(Uuid, Uuid, GridCell)> = vec![];
    waiting.retain(|(id, code, _)| match names.get(code) {
        Some((place, cell)) if taken.insert((*place, *cell)) => {
            fill.push((*id, *place, *cell));
            false
        }
        _ => true,
    });
    put(tx, &fill).await?;
    out.bins_filled = fill.len();

    // The rest, drafted.
    let proposal = layout::draft(
        &waiting
            .iter()
            .map(|(_, code, kind)| (code.clone(), SOLID_KINDS.contains(&kind.as_str())))
            .collect::<Vec<_>>(),
    );
    // Inside the first walk-through place standing on the site, or a new one.
    let container = places.iter().find(|p| p.parent_id.is_none() && !p.solid);
    let existing_children: Vec<&Row> = match container {
        Some(c) => places.iter().filter(|p| p.parent_id == Some(c.id)).collect(),
        None => vec![],
    };

    // Names first, the same whatever is asked: `Rack C`, then `Rack C (2)`
    // beside a place already called that.
    let mut used: HashSet<String> = existing_children.iter().map(|p| p.name.clone()).collect();
    let mut named = vec![];
    for d in &proposal.places {
        let name = unused_name(&mut used, &d.name);
        named.push((d, name));
    }
    for wanted in asked.leave_out.iter().chain(&asked.two_sided) {
        if !named.iter().any(|(_, name)| name == wanted) {
            return Err(ApiError::Rejected(format!(
                "the draft no longer proposes {wanted}; preview it again"
            )));
        }
    }
    let mut unmatched = proposal.unmatched.clone();
    named.retain(|(d, name)| {
        if !asked.leave_out.contains(name) {
            return true;
        }
        out.left_out.push(name.clone());
        unmatched.extend(d.bins.iter().map(|(code, _)| code.clone()));
        false
    });
    unmatched.sort();

    // What will be made: each family's place, with two sides where asked. A
    // rack with two is the same codes read on a grid half as long, so its bins
    // are found again by name, each in the cell its pattern spells it in.
    let mut making: Vec<(&layout::Drafted, String, Grid, Vec<(&str, GridCell)>)> = vec![];
    for (d, name) in named {
        if !asked.two_sided.contains(&name) {
            let bins = d.bins.iter().map(|(code, cell)| (code.as_str(), *cell)).collect();
            making.push((d, name, d.grid.clone(), bins));
            continue;
        }
        let Some(grid) = layout::two_sides(&d.grid).filter(|_| d.solid) else {
            return Err(ApiError::Rejected(format!("{name} has no bays to put on two sides")));
        };
        let by_name: HashMap<String, GridCell> = Pattern::parse(&d.pattern)
            .and_then(|p| p.names(&grid))
            .map_err(|e| ApiError::Rejected(format!("{name} cannot be read on two sides: {e}")))?
            .into_iter()
            .map(|(cell, code)| (code, cell))
            .collect();
        let bins: Vec<(&str, GridCell)> = d
            .bins
            .iter()
            .filter_map(|(code, _)| by_name.get(code).map(|cell| (code.as_str(), *cell)))
            .collect();
        if bins.len() != d.bins.len() {
            return Err(ApiError::Rejected(format!("{name} has codes its two sides do not name")));
        }
        making.push((d, name, grid, bins));
    }

    // A row of aisles: each place along x, one aisle of two cells between.
    let mut y = existing_children
        .iter()
        .map(|p| p.y + p.depth)
        .fold(0.0_f64, f64::max)
        + if existing_children.is_empty() { 1.0 } else { 2.0 };

    if !making.is_empty() {
        // Each place's box: a bay a cell along, a level a cell up, a row a cell
        // in from each face.
        let boxed: Vec<_> = making
            .into_iter()
            .map(|(d, name, grid, bins)| {
                let at = y;
                let depth = (grid.rows * grid.sides) as f64;
                y += depth + 2.0;
                let height = if d.solid { grid.levels as f64 } else { 1.0 };
                (d, name, grid, bins, at, depth, height)
            })
            .collect();
        let wide = boxed.iter().map(|b| b.2.bays as f64 + 2.0).fold(10.0_f64, f64::max);
        let tall = boxed.iter().map(|b| b.6 + 2.0).fold(4.0_f64, f64::max);
        let deep = y - 1.0;

        let container_id = match container {
            Some(c) => {
                // Grown to hold what was put in it, never shrunk.
                tx.execute(
                    "UPDATE place SET length = greatest(length, $2), depth = greatest(depth, $3),
                                      height = greatest(height, $4)
                      WHERE id = $1",
                    &[&c.id, &wide, &deep, &tall],
                )
                .await?;
                out.inside = Some(c.name.clone());
                c.id
            }
            None => {
                let name = unused_top_name(&places, "Building");
                out.inside = Some(name.clone());
                out.inside_created = true;
                tx.query_one(
                    "INSERT INTO place (tenant_id, site_id, name, solid, length, depth, height)
                     VALUES ($1, $2, $3, false, $4, $5, $6) RETURNING id",
                    &[&tenant, &site, &name, &wide, &deep, &tall],
                )
                .await?
                .get(0)
            }
        };

        let by_code: HashMap<&str, Uuid> = waiting.iter().map(|(id, c, _)| (c.as_str(), *id)).collect();
        for (d, name, grid, bins, at, depth, height) in boxed {
            let positions: Option<Vec<i32>> =
                if grid.positions.is_empty() { None } else { Some(grid.positions.clone()) };
            let length = grid.bays as f64;
            let sides = grid.sides as i16;
            let id: Uuid = tx
                .query_one(
                    "INSERT INTO place (tenant_id, site_id, parent_id, name, solid, x, y, length,
                                        depth, height, bays, levels, rows, positions, bin_pattern,
                                        first_bay, bay_step, first_level, sides)
                     VALUES ($1, $2, $3, $4, $5, 1, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                             $15, $16, $17, $18)
                     RETURNING id",
                    &[
                        &tenant, &site, &container_id, &name, &d.solid, &at, &length, &depth,
                        &height, &grid.bays, &grid.levels, &grid.rows, &positions, &d.pattern,
                        &grid.first_bay, &grid.bay_step, &grid.first_level, &sides,
                    ],
                )
                .await?
                .get(0);
            let cells: Vec<(Uuid, Uuid, GridCell)> =
                bins.iter().map(|(code, cell)| (by_code[*code], id, *cell)).collect();
            put(tx, &cells).await?;
            out.bins_placed += cells.len();
            out.places.push(DraftedPlace {
                name,
                solid: d.solid,
                pattern: d.pattern.clone(),
                bays: grid.bays,
                levels: grid.levels,
                bins: bins.len(),
                sides: grid.sides,
                split: if grid.sides == 1 { split_of(d) } else { None },
            });
        }
    }

    out.unplaced = unmatched.len();
    out.unplaced_sample = unmatched.into_iter().take(SAMPLE).collect();

    let end = if apply { "RELEASE SAVEPOINT spork_draft" } else { "ROLLBACK TO SAVEPOINT spork_draft" };
    tx.batch_execute(end).await?;
    Ok(out)
}

/// A name for a new place on the site that no other there has.
fn unused_top_name(places: &[Row], base: &str) -> String {
    let used: BTreeSet<&str> =
        places.iter().filter(|p| p.parent_id.is_none()).map(|p| p.name.as_str()).collect();
    let mut name = base.to_string();
    let mut n = 1;
    while used.contains(name.as_str()) {
        n += 1;
        name = format!("{base} ({n})");
    }
    name
}

/// Put bins in cells: `(bin, place, cell)`. Only a bin with no cell moves.
async fn put(tx: &Transaction<'_>, cells: &[(Uuid, Uuid, GridCell)]) -> Result<(), ApiError> {
    if cells.is_empty() {
        return Ok(());
    }
    let ids: Vec<Uuid> = cells.iter().map(|c| c.0).collect();
    let places: Vec<Uuid> = cells.iter().map(|c| c.1).collect();
    let bays: Vec<i32> = cells.iter().map(|c| c.2.bay).collect();
    let levels: Vec<i32> = cells.iter().map(|c| c.2.level).collect();
    let rows: Vec<i32> = cells.iter().map(|c| c.2.row).collect();
    let positions: Vec<i32> = cells.iter().map(|c| c.2.position).collect();
    let sides: Vec<i16> = cells.iter().map(|c| c.2.side as i16).collect();
    tx.execute(
        "UPDATE location l
            SET place_id = t.place, slot_bay = t.bay, slot_level = t.level,
                slot_row = t.row, slot_position = t.position, slot_side = t.side
           FROM unnest($1::uuid[], $2::uuid[], $3::int4[], $4::int4[], $5::int4[], $6::int4[],
                       $7::int2[])
                AS t(id, place, bay, level, row, position, side)
          WHERE l.id = t.id AND l.place_id IS NULL",
        &[&ids, &places, &bays, &levels, &rows, &positions, &sides],
    )
    .await?;
    Ok(())
}

/// Draft the caller's site from its bin list. A dry run unless `?apply=true`.
///
/// **A person's act, on a session**, unlike the file imports: nothing arrives,
/// somebody at the desk asks for a first layout. The body, when there is one,
/// is a [`DraftRequest`]: the places they said not to make.
#[post("/layout/draft")]
pub async fn draft_layout(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<DraftQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let site = working_site(who.site_id)?;
    // No body is the draft as proposed. A body that does not read is refused,
    // never taken as none, or what was left out would be made.
    let asked: DraftRequest = if body.is_empty() {
        DraftRequest::default()
    } else {
        serde_json::from_slice(&body)
            .map_err(|e| ApiError::Rejected(format!("the draft request did not read: {e}")))?
    };
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let tenant = scope.tenant();
    let apply = query.apply;
    let report = scope
        .run(move |tx| Box::pin(async move { draft(tx, tenant, site, apply, &asked).await }))
        .await?;
    Ok(HttpResponse::Ok().json(report))
}
