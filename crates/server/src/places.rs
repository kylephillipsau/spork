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
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::error::ApiError;
use crate::layout::{self, GridCell, Frame, Grid, Pattern};
use crate::pictures::{self, Picture};
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::client_events::{claim_act, NewClientEvent};
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
pub const SOLID_KINDS: &[&str] = &["pick_face", "bulk", "overflow"];

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
                             first_level, bin_pattern, sides, reach_levels, from_right";

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
            from_right: r.get(22),
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
    /// Its own corner and turn on the site: what a place inside it is
    /// positioned against (D209).
    pub frame: Frame,
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
    /// What is on it by either record, the most NetSuite counts first; at most
    /// [`ITEMS_ON_A_BIN`] of them (D221).
    pub contents: Vec<BinItem>,
    /// How many items are on it in all.
    pub contents_total: i64,
}

/// One item on a bin, as a screen shows it to find it by: what it looks like,
/// and how much of it each record says is here (D221).
#[derive(Serialize, Debug)]
pub struct BinItem {
    pub item_id: Uuid,
    pub item_code: String,
    pub description: String,
    pub picture: Option<Picture>,
    /// NetSuite's newest count of it here, as text; none when only this
    /// system's own ledger has it here.
    pub on_hand: Option<String>,
    /// What this system's own ledger holds of it here.
    pub held: i64,
}

/// How many items a bin's read lists. Most bins hold a handful; a catch-all
/// bin can hold hundreds, which are counted instead.
const ITEMS_ON_A_BIN: i64 = 50;

/// What is on a bin by either record: NetSuite's newest count of each item
/// (D215), and what this system's ledger holds; and how many items in all.
async fn contents_of(tx: &Transaction<'_>, bin: Uuid) -> Result<(Vec<BinItem>, i64), ApiError> {
    let rows = tx
        .query(
            "WITH said AS (
                 SELECT DISTINCT ON (rs.item_id) rs.item_id, rs.on_hand
                   FROM reported_stock rs
                  WHERE rs.location_id = $1
                  ORDER BY rs.item_id, rs.as_at DESC
             ), held AS (
                 SELECT s.item_id, sum(s.quantity)::bigint AS q
                   FROM stock s
                  WHERE s.holder_location_id = $1 AND s.quantity > 0
                  GROUP BY s.item_id
             )
             SELECT i.id, i.code, i.description, said.on_hand::text, coalesce(held.q, 0)::bigint,
                    count(*) OVER ()
               FROM said
               FULL JOIN held ON held.item_id = said.item_id
               JOIN item i ON i.id = coalesce(said.item_id, held.item_id)
              ORDER BY said.on_hand DESC NULLS LAST, held.q DESC NULLS LAST, i.code
              LIMIT $2",
            &[&bin, &ITEMS_ON_A_BIN],
        )
        .await?;
    let total = rows.first().map(|r| r.get(5)).unwrap_or(0);
    let ids: Vec<Uuid> = rows.iter().map(|r| r.get(0)).collect();
    let mut pictured = pictures::of(tx, &ids).await?;
    let items = rows
        .iter()
        .map(|r| {
            let item_id: Uuid = r.get(0);
            BinItem {
                item_id,
                item_code: r.get(1),
                description: r.get(2),
                picture: pictured.remove(&item_id),
                on_hand: r.get(3),
                held: r.get(4),
            }
        })
        .collect();
    Ok((items, total))
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
                frame: *f,
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

/// A bin, the cell it is in, the place around it, and what is on it.
///
/// **A scan lands here.** D111's locator resolves a location code to its id;
/// this is the page that answers "where is it", which is the question a bin
/// code is scanned to ask. The bin map's card reads it too (D221).
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
                let (contents, contents_total) = contents_of(tx, id).await?;
                Ok(BinView {
                    location_id: id,
                    code: r.get(0),
                    kind: r.get(1),
                    active: r.get(2),
                    cell,
                    place,
                    contents,
                    contents_total,
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
    /// Its first label is at the right end of its front (D220).
    pub from_right: bool,
    /// How many bins share a bay at each level, lowest first: where the map
    /// splits a bay (D208).
    pub positions: Vec<i32>,
    pub pattern: Option<String>,
    /// How many of its levels, from the floor up, can be reached without a
    /// forklift (D180).
    pub reach_levels: i32,
    /// Bins in its own cells, not counting places inside it.
    pub bins: i64,
    /// Its box in its parent's cells, as drawn: where its front-left corner
    /// is, how big it is, and how far it is turned, in degrees anticlockwise
    /// (D209).
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub length: f64,
    pub depth: f64,
    pub height: f64,
    pub turn: f64,
    /// It has an outline of its own, so it is moved and turned, not resized.
    pub outlined: bool,
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
    /// The layout as read, as a fingerprint: an edit made against an older
    /// one is refused rather than written over somebody else's (D209).
    pub version: String,
    /// How many millimetres one cell is, once the site says (D210). Until
    /// then the layout is not to scale, and is shown in cells.
    pub cell_mm: Option<i32>,
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
                frame: *f,
            });
            for k in kids.get(&id).into_iter().flatten().rev() {
                stack.push((*k, nesting + 1));
            }
        }
    }
    out
}

/// A fingerprint of every place on the site as its row stands, so two people
/// editing at once can't write over each other unknowingly (D209). Drafting
/// and reach change the rows too, and so change it.
pub(crate) async fn layout_version(tx: &Transaction<'_>, site: Uuid) -> Result<String, ApiError> {
    Ok(tx
        .query_one(
            "SELECT md5(coalesce(string_agg(p::text, '|' ORDER BY p.id), ''))
               FROM place p WHERE p.site_id = $1",
            &[&site],
        )
        .await?
        .get(0))
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
                let (site_code, cell_mm): (String, Option<i32>) = tx
                    .query_opt("SELECT code, cell_mm FROM site WHERE id = $1", &[&site])
                    .await?
                    .map(|r| (r.get(0), r.get(1)))
                    .ok_or(ApiError::NotFound)?;
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
                        from_right: p.grid.from_right,
                        positions: (1..=p.grid.levels).map(|l| p.grid.positions_at(l)).collect(),
                        pattern: p.pattern,
                        reach_levels: p.reach_levels,
                        x: p.x,
                        y: p.y,
                        z: p.z,
                        length: p.length,
                        depth: p.depth,
                        height: p.height,
                        turn: p.turn,
                        outlined: p.outline.is_some(),
                    })
                    .collect();
                let version = layout_version(tx, site).await?;
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
                    version,
                    cell_mm,
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
    /// When NetSuite's count was taken, the newest at the site: the age of
    /// what the map colours by (D212).
    pub reported_as_at: Option<DateTime<Utc>>,
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
                let reported_as_at: Option<DateTime<Utc>> = tx
                    .query_one("SELECT max(as_at) FROM reported_stock WHERE site_id = $1", &[&site])
                    .await?
                    .get(0);
                Ok(MapBins { bins, unplaced, reported_as_at })
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
    /// True when it was made numbered from the right end of its front (D220).
    pub from_right: bool,
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
    /// Places numbered from the right end of their front, leftwards (D220),
    /// by the names the preview gave them.
    #[serde(default)]
    pub from_right: Vec<String>,
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

/// Where each name a pattern gives is on this grid: the cell it spells.
fn cells_named(pattern: &Pattern, grid: &Grid) -> Result<HashMap<String, GridCell>, String> {
    Ok(pattern.names(grid)?.into_iter().map(|(cell, name)| (name, cell)).collect())
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
/// back, and a place named in `from_right` is numbered from the right end of
/// its front. The names are worked out the same way whatever is asked, so the
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
    for wanted in asked.leave_out.iter().chain(&asked.two_sided).chain(&asked.from_right) {
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

    // What will be made: each family's place, with two sides and numbered
    // from the right where asked. Either is the same codes read on another
    // grid, so its bins are found again by name, each in the cell its pattern
    // spells it in.
    let mut making: Vec<(&layout::Drafted, String, Grid, Vec<(&str, GridCell)>)> = vec![];
    for (d, name) in named {
        let mut grid = if asked.two_sided.contains(&name) {
            layout::two_sides(&d.grid)
                .filter(|_| d.solid)
                .ok_or_else(|| ApiError::Rejected(format!("{name} has no bays to put on two sides")))?
        } else {
            d.grid.clone()
        };
        grid.from_right = asked.from_right.contains(&name);
        if grid == d.grid {
            let bins = d.bins.iter().map(|(code, cell)| (code.as_str(), *cell)).collect();
            making.push((d, name, grid, bins));
            continue;
        }
        let by_name = Pattern::parse(&d.pattern)
            .and_then(|p| cells_named(&p, &grid))
            .map_err(|e| ApiError::Rejected(format!("{name} cannot be read that way: {e}")))?;
        let bins: Vec<(&str, GridCell)> = d
            .bins
            .iter()
            .filter_map(|(code, _)| by_name.get(code).map(|cell| (code.as_str(), *cell)))
            .collect();
        if bins.len() != d.bins.len() {
            return Err(ApiError::Rejected(format!("{name} has codes it would not name that way")));
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
                                        first_bay, bay_step, first_level, sides, from_right)
                     VALUES ($1, $2, $3, $4, $5, 1, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                             $15, $16, $17, $18, $19)
                     RETURNING id",
                    &[
                        &tenant, &site, &container_id, &name, &d.solid, &at, &length, &depth,
                        &height, &grid.bays, &grid.levels, &grid.rows, &positions, &d.pattern,
                        &grid.first_bay, &grid.bay_step, &grid.first_level, &sides,
                        &grid.from_right,
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
                from_right: grid.from_right,
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

// ---------------------------------------------------------------------------
// The plan editor (D209)
// ---------------------------------------------------------------------------

/// A place's box in its parent's cells.
#[derive(Deserialize, Debug, Clone, Copy)]
pub struct PlaceBox {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub z: f64,
    pub length: f64,
    pub depth: f64,
    pub height: f64,
    /// Degrees anticlockwise; whole degrees are kept.
    #[serde(default)]
    pub turn: f64,
}

/// A place as the editor left it. Its grid is not here, but for which end its
/// numbering starts: changing bays, levels or the naming pattern would rename
/// bins or put them in other places, and the editor does neither.
#[derive(Deserialize, Debug)]
pub struct PlaceChanged {
    pub place_id: Uuid,
    pub name: String,
    pub solid: bool,
    #[serde(flatten)]
    pub at: PlaceBox,
    /// Numbered from the right end of its front (D220). Absent: as it was.
    #[serde(default)]
    pub from_right: Option<bool>,
}

/// A place drawn in the editor: a wall, a column, a dock, a packing station.
#[derive(Deserialize, Debug)]
pub struct PlaceAdded {
    /// Minted by the client, so the place it drew is the one it can name.
    pub place_id: Uuid,
    /// The place it is inside, or none: standing on the site.
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub solid: bool,
    #[serde(flatten)]
    pub at: PlaceBox,
}

/// A bin from the tray put on the plan as a spot of its own (D211): the
/// packing bench, a dock door, a floor bay. One cell, named for the bin.
#[derive(Deserialize, Debug)]
pub struct SpotAdded {
    /// Minted by the client, as an added place's is.
    pub place_id: Uuid,
    /// The bin it holds, which is on no layout yet.
    pub location_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub solid: bool,
    #[serde(flatten)]
    pub at: PlaceBox,
}

#[derive(Deserialize, Debug)]
pub struct LayoutEdit {
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    /// The layout's fingerprint when the editor read it (`LayoutView::version`).
    pub version: String,
    #[serde(default)]
    pub changed: Vec<PlaceChanged>,
    #[serde(default)]
    pub added: Vec<PlaceAdded>,
    #[serde(default)]
    pub removed: Vec<Uuid>,
    #[serde(default)]
    pub spots: Vec<SpotAdded>,
}

#[derive(Serialize, Debug)]
pub struct LayoutEdited {
    pub changed: i64,
    pub added: i64,
    pub removed: i64,
    /// Bins from the tray put on the plan.
    pub placed: i64,
    /// The layout's fingerprint now, to edit on from.
    pub version: String,
    /// This act had already been saved; nothing was written again.
    pub replay: bool,
}

/// A box worth keeping, or what is wrong with it.
fn checked(at: PlaceBox, what: &str) -> Result<PlaceBox, ApiError> {
    let all = [at.x, at.y, at.z, at.length, at.depth, at.height, at.turn];
    if all.iter().any(|v| !v.is_finite()) {
        return Err(ApiError::Rejected(format!("{what} has a position or size that isn't a number")));
    }
    if at.length <= 0.0 || at.depth <= 0.0 || at.height <= 0.0 {
        return Err(ApiError::Rejected(format!("{what} needs a length, a depth and a height above nothing")));
    }
    Ok(PlaceBox { turn: at.turn.round().rem_euclid(360.0), ..at })
}

/// How a place is kept in its history: its name, kind and box, and which end
/// it is numbered from.
fn as_kept(parent: Option<Uuid>, name: &str, solid: bool, at: &PlaceBox, from_right: bool) -> serde_json::Value {
    serde_json::json!({
        "parent_id": parent, "name": name, "solid": solid,
        "x": at.x, "y": at.y, "z": at.z,
        "length": at.length, "depth": at.depth, "height": at.height, "turn": at.turn,
        "from_right": from_right,
    })
}

/// Number a place from the other end of its front (D220).
///
/// **Nothing moves on the floor.** The place's labels were the wrong way
/// round, so each bin, keeping its name, goes to the cell that name is on
/// now: the mirror of its column, on the same side and level. A bin its
/// pattern does not name is refused rather than left where its labels no
/// longer say.
async fn renumber(tx: &Transaction<'_>, was: &Row, from_right: bool) -> Result<(), ApiError> {
    let bins: Vec<(Uuid, String)> = tx
        .query("SELECT id, code FROM location WHERE place_id = $1", &[&was.id])
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    if bins.is_empty() {
        return Ok(());
    }
    let name = &was.name;
    let refused = |why: String| ApiError::Rejected(format!("{name} can't be numbered from its other end: {why}"));
    let pattern = was
        .pattern
        .as_deref()
        .and_then(|s| Pattern::parse(s).ok())
        .ok_or_else(|| refused("it has no pattern its bins are named by".into()))?;
    let by_name = cells_named(&pattern, &Grid { from_right, ..was.grid.clone() }).map_err(refused)?;
    let cells = bins
        .iter()
        .map(|(id, code)| match by_name.get(code) {
            Some(cell) => Ok((*id, was.id, *cell)),
            None => Err(refused(format!("{code} is in it, and isn't a name its pattern gives"))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Off and back on: a swap made in one statement collides with itself on
    // location_slot_key.
    tx.execute(
        "UPDATE location
            SET place_id = NULL, slot_side = NULL, slot_bay = NULL, slot_level = NULL,
                slot_row = NULL, slot_position = NULL
          WHERE place_id = $1",
        &[&was.id],
    )
    .await?;
    put(tx, &cells).await
}

/// A database refusal the person can act on, in their words.
fn said(e: tokio_postgres::Error, name: &str) -> ApiError {
    match e.code() {
        Some(c) if *c == tokio_postgres::error::SqlState::UNIQUE_VIOLATION => {
            ApiError::Rejected(format!("there is already a place called {name} there"))
        }
        Some(c) if *c == tokio_postgres::error::SqlState::CHECK_VIOLATION => {
            ApiError::Rejected(format!("{name} can't be drawn like that"))
        }
        _ => ApiError::Database(e),
    }
}

/// Save what the plan editor changed, as one act (D209).
///
/// **Bins never move.** A place's box is where it is drawn; a bin's cell is a
/// bay and a level of its place, so moving, turning or stretching a rack
/// carries its bins with it. Of its grid, only which end its numbering starts
/// can be changed here, and that moves no bin on the floor: each keeps its
/// name, in the cell the name is on now (D220).
///
/// **Against the layout the editor read.** If the layout has changed since,
/// by another editor, a draft or a rack's reach, the save is refused, so
/// nobody's change is overwritten without them knowing.
#[post("/layout/edit")]
pub async fn edit_layout(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<LayoutEdit>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let site = working_site(who.site_id)?;
    let body = body.into_inner();
    let ev = NewClientEvent {
        tenant_id: who.tenant_id,
        client_event_id: body.client_event_id,
        site_id: who.site_id,
        recorded_by_id: who.person_id,
        submitted_at: body.occurred_at,
    };
    let tenant = who.tenant_id;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                // One save at a time per site, so two can't both pass the
                // version check below and write over each other.
                tx.execute("SELECT pg_advisory_xact_lock(hashtext('layout:' || $1::uuid::text))", &[&site])
                    .await?;
                // A retry of a save that landed answers with what it did.
                if claim_act(tx, &ev).await?.is_replay() {
                    let n = |kind: &'static str| async move {
                        tx.query_one(
                            "SELECT count(*) FROM place_change WHERE client_event_id = $1 AND change = $2",
                            &[&ev.client_event_id, &kind],
                        )
                        .await
                        .map(|r| r.get::<_, i64>(0))
                    };
                    let placed: i64 = tx
                        .query_one(
                            "SELECT count(*) FROM place_change WHERE client_event_id = $1 AND change = 'added' AND after ? 'bin'",
                            &[&ev.client_event_id],
                        )
                        .await?
                        .get(0);
                    return Ok(LayoutEdited {
                        changed: n("changed").await?,
                        added: n("added").await? - placed,
                        removed: n("removed").await?,
                        placed,
                        version: layout_version(tx, site).await?,
                        replay: true,
                    });
                }
                if layout_version(tx, site).await? != body.version {
                    return Err(ApiError::Rejected(
                        "the layout has changed since you opened it; open it again to see what changed, then make your change".into(),
                    ));
                }
                let rows = site_places(tx, site).await?;
                let by_id: HashMap<Uuid, &Row> = rows.iter().map(|p| (p.id, p)).collect();
                let record = |place: Uuid, change: &'static str, before: Option<serde_json::Value>, after: Option<serde_json::Value>| async move {
                    // As text, cast: the driver is built without JSON.
                    let (before, after) = (before.map(|v| v.to_string()), after.map(|v| v.to_string()));
                    tx.execute(
                        "INSERT INTO place_change (tenant_id, site_id, place_id, client_event_id, change, before, after)
                         VALUES ($1, $2, $3, $4, $5, $6::text::jsonb, $7::text::jsonb)",
                        &[&tenant, &site, &place, &ev.client_event_id, &change, &before, &after],
                    )
                    .await
                };

                for c in &body.changed {
                    let was = *by_id.get(&c.place_id).ok_or(ApiError::NotFound)?;
                    let name = c.name.trim();
                    if name.is_empty() {
                        return Err(ApiError::Rejected("a place needs a name".into()));
                    }
                    let at = checked(c.at, name)?;
                    if was.outline.is_some() && (at.length != was.length || at.depth != was.depth) {
                        return Err(ApiError::Rejected(format!("{name} has its own outline, so it is moved and turned, not resized")));
                    }
                    if was.grid.sides == 2 && !c.solid {
                        return Err(ApiError::Rejected(format!("{name} has bins on two sides, so it stays solid")));
                    }
                    let before = PlaceBox { x: was.x, y: was.y, z: was.z, length: was.length, depth: was.depth, height: was.height, turn: was.turn };
                    let from_right = c.from_right.unwrap_or(was.grid.from_right);
                    tx.execute(
                        "UPDATE place SET name = $2, solid = $3, x = $4, y = $5, z = $6,
                                          length = $7, depth = $8, height = $9, turn = $10,
                                          from_right = $11
                          WHERE id = $1",
                        &[&c.place_id, &name, &c.solid, &at.x, &at.y, &at.z, &at.length, &at.depth, &at.height, &(at.turn as i16), &from_right],
                    )
                    .await
                    .map_err(|e| said(e, name))?;
                    if from_right != was.grid.from_right {
                        renumber(tx, was, from_right).await?;
                    }
                    record(
                        c.place_id,
                        "changed",
                        Some(as_kept(was.parent_id, &was.name, was.solid, &before, was.grid.from_right)),
                        Some(as_kept(was.parent_id, name, c.solid, &at, from_right)),
                    )
                    .await?;
                }

                for a in &body.added {
                    let name = a.name.trim();
                    if name.is_empty() {
                        return Err(ApiError::Rejected("a place needs a name".into()));
                    }
                    if let Some(parent) = a.parent_id {
                        if !by_id.contains_key(&parent) {
                            return Err(ApiError::Rejected(format!("{name} is inside a place that isn't on this site")));
                        }
                    }
                    let at = checked(a.at, name)?;
                    tx.execute(
                        "INSERT INTO place (id, tenant_id, site_id, parent_id, name, solid,
                                            x, y, z, length, depth, height, turn)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
                        &[&a.place_id, &tenant, &site, &a.parent_id, &name, &a.solid, &at.x, &at.y, &at.z, &at.length, &at.depth, &at.height, &(at.turn as i16)],
                    )
                    .await
                    .map_err(|e| said(e, name))?;
                    record(a.place_id, "added", None, Some(as_kept(a.parent_id, name, a.solid, &at, false))).await?;
                }

                // A bin from the tray, on a spot of its own: one cell, named for
                // it. Only a bin on no layout: one already in a cell stays put.
                for spot in &body.spots {
                    let code: String = tx
                        .query_opt(
                            "SELECT code FROM location WHERE id = $1 AND site_id = $2 AND active",
                            &[&spot.location_id, &site],
                        )
                        .await?
                        .ok_or(ApiError::NotFound)?
                        .get(0);
                    if let Some(parent) = spot.parent_id {
                        if !by_id.contains_key(&parent) {
                            return Err(ApiError::Rejected(format!("{code} is inside a place that isn't on this site")));
                        }
                    }
                    let at = checked(spot.at, &code)?;
                    tx.execute(
                        "INSERT INTO place (id, tenant_id, site_id, parent_id, name, solid,
                                            x, y, z, length, depth, height, turn)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
                        &[&spot.place_id, &tenant, &site, &spot.parent_id, &code, &spot.solid, &at.x, &at.y, &at.z, &at.length, &at.depth, &at.height, &(at.turn as i16)],
                    )
                    .await
                    .map_err(|e| said(e, &code))?;
                    let moved = tx
                        .execute(
                            "UPDATE location
                                SET place_id = $2, slot_side = 1, slot_bay = 1, slot_level = 1, slot_row = 1, slot_position = 1
                              WHERE id = $1 AND place_id IS NULL",
                            &[&spot.location_id, &spot.place_id],
                        )
                        .await?;
                    if moved == 0 {
                        return Err(ApiError::Rejected(format!("{code} is on the layout already, so it stays where it is")));
                    }
                    let mut after = as_kept(spot.parent_id, &code, spot.solid, &at, false);
                    after["bin"] = serde_json::json!(code);
                    record(spot.place_id, "added", None, Some(after)).await?;
                }

                for id in &body.removed {
                    let was = *by_id.get(id).ok_or(ApiError::NotFound)?;
                    let bins: i64 = tx
                        .query_one("SELECT count(*) FROM location WHERE place_id = $1", &[id])
                        .await?
                        .get(0);
                    if bins > 0 {
                        return Err(ApiError::Rejected(format!(
                            "{} still has {bins} bin{} in it, so it stays",
                            was.name,
                            if bins == 1 { "" } else { "s" }
                        )));
                    }
                    if rows.iter().any(|p| p.parent_id == Some(*id) && !body.removed.contains(&p.id)) {
                        return Err(ApiError::Rejected(format!("{} has places inside it, so it stays", was.name)));
                    }
                }
                // Inside first, so a place is never removed from under another.
                let mut gone: Vec<&Row> = body.removed.iter().filter_map(|id| by_id.get(id).copied()).collect();
                let depth_of = |p: &Row| {
                    let mut n = 0;
                    let mut at = p.parent_id;
                    while let Some(id) = at {
                        n += 1;
                        at = by_id.get(&id).and_then(|q| q.parent_id);
                        if n > rows.len() {
                            break;
                        }
                    }
                    n
                };
                gone.sort_by_key(|p| std::cmp::Reverse(depth_of(p)));
                for was in gone {
                    tx.execute("DELETE FROM place WHERE id = $1", &[&was.id]).await?;
                    let before = PlaceBox { x: was.x, y: was.y, z: was.z, length: was.length, depth: was.depth, height: was.height, turn: was.turn };
                    record(was.id, "removed", Some(as_kept(was.parent_id, &was.name, was.solid, &before, was.grid.from_right)), None).await?;
                }

                Ok(LayoutEdited {
                    changed: body.changed.len() as i64,
                    added: body.added.len() as i64,
                    removed: body.removed.len() as i64,
                    placed: body.spots.len() as i64,
                    version: layout_version(tx, site).await?,
                    replay: false,
                })
            })
        })
        .await?;
    tracing::info!(tenant_id = %tenant, changed = out.changed, added = out.added, removed = out.removed, "the layout was edited");
    Ok(HttpResponse::Ok().json(out))
}

// ---------------------------------------------------------------------------
// The scale (D210)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct ScaleRequest {
    /// How many millimetres one cell is.
    pub cell_mm: i32,
}

#[derive(Serialize, Debug)]
pub struct ScaleSet {
    pub cell_mm: i32,
}

/// Say how long one of the site's cells is (D210). Once.
///
/// **Set once, because everything after it is measured in it.** A rack placed
/// 4.2 m from the wall is 4.2 cells there with a one-metre cell; changing the
/// cell afterwards would move every measured place without anybody moving it.
#[post("/layout/scale")]
pub async fn set_scale(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<ScaleRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let site = working_site(who.site_id)?;
    let cell_mm = body.cell_mm;
    if !(10..=100_000).contains(&cell_mm) {
        return Err(ApiError::Rejected("a cell is between a centimetre and a hundred metres".into()));
    }
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let was: Option<i32> = tx
                    .query_opt("SELECT cell_mm FROM site WHERE id = $1 FOR UPDATE", &[&site])
                    .await?
                    .ok_or(ApiError::NotFound)?
                    .get(0);
                match was {
                    Some(same) if same == cell_mm => {}
                    Some(_) => {
                        return Err(ApiError::Rejected(
                            "the scale is set already, and places are measured in it; move the places instead".into(),
                        ))
                    }
                    None => {
                        tx.execute("UPDATE site SET cell_mm = $2 WHERE id = $1", &[&site, &cell_mm]).await?;
                    }
                }
                Ok(ScaleSet { cell_mm })
            })
        })
        .await?;
    tracing::info!(%site, cell_mm, by = %who.person_id, "the site's scale was set");
    Ok(HttpResponse::Ok().json(out))
}
