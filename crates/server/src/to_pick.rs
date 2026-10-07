//! A batch of orders to pick: which are still waiting, where each line is
//! taken from, and, for picking on paper, who walks which. D231, D230.
//!
//! The batch is what was pasted from the sheet the tickets went out on, or,
//! with nothing pasted, everything NetSuite has open here. Each order says
//! where it stands: still **waiting**, **part picked** (NetSuite has picked
//! some and has more to pick), or gone on: **picked**, **packed**, **shipped**,
//! **closed**, at another warehouse, or unknown.
//!
//! **Where from** is the item list's rule (D180, [`crate::items::piles`]),
//! taken in turn: as much as the first bin holds, then the next, until the
//! line is covered, the way NetSuite's Inventory Detail lists a line over its
//! bins. Racking only: goods already at the bench are another order's. The
//! batch is shared out in the order shown, so two orders are never both sent
//! for the same last five in a bin; a line no bin covers says how many short.
//!
//! **Who walks which** is [`crate::pick_groups`], over the walk's own floor
//! ([`walk_route::distances`]).
//!
//! A read. Nothing here is recorded: the picks are NetSuite's (D212).

use std::collections::HashMap;

use actix_web::{get, web, HttpRequest, HttpResponse};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::error::ApiError;
use crate::importing::orders::{roles, Role, Sent};
use crate::layout::GridCell;
use crate::pick_groups::{self, Pick, Settings};
use crate::pictures::{self, Picture};
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::walk_route::{self, Whereabouts};
use crate::AppState;

/// The most orders one batch is asked about.
const MOST_ASKED: usize = 200;
/// The most waiting orders shared out at once. Nobody shares out a whole
/// warehouse's open orders, and planning them would take too long to wait for.
pub const MOST_PLANNED: usize = 40;
/// A walking pace and the time a stop takes, for the minutes a trip is said
/// to take once the site is measured, and to weigh stops against walking
/// when trips are shared out. Estimates, said as "about".
const WALK_M_PER_S: f64 = 1.0;
const STOP_S: f64 = 20.0;

#[derive(Deserialize)]
pub struct ToPickQuery {
    /// The orders asked about, as pasted, split by commas, spaces or tabs.
    /// Absent or empty: every order NetSuite has open here.
    pub orders: Option<String>,
    /// People picking, for sharing the batch out: 1 by default.
    pub pickers: Option<usize>,
    /// The most orders one trip takes.
    pub per_trip: Option<usize>,
    /// A shelf wanted by orders on different trips is walked to by one.
    #[serde(default)]
    pub gather: bool,
    /// Printing only: a walk sheet on top of each trip's tickets.
    #[serde(default)]
    pub walk: bool,
}

impl ToPickQuery {
    /// How the batch is shared out, as asked.
    pub fn settings(&self) -> Settings {
        Settings {
            pickers: self.pickers.unwrap_or(1).clamp(1, 20),
            per_trip: self.per_trip.filter(|&n| n > 0),
            gather: self.gather,
            stop_cost: 0.0,
        }
    }
}

#[derive(Serialize, Debug)]
pub struct ToPick {
    /// When NetSuite last said what it has open here; none when it hasn't.
    pub orders_as_at: Option<DateTime<Utc>>,
    /// How old the bins' counts are (D212).
    pub balance_as_at: Option<DateTime<Utc>>,
    pub orders: Vec<AskedOrder>,
    /// Who walks which, when any order is waiting.
    pub plan: Option<PickPlan>,
}

/// An order asked about, and where it stands.
#[derive(Serialize, Debug)]
pub struct AskedOrder {
    /// As pasted, or its number when nothing was.
    pub asked: String,
    /// The number NetSuite knows it by, when either system knows it.
    pub number: Option<String>,
    /// `waiting`, `part_picked`, `picked`, `packed`, `shipped`, `closed`,
    /// `elsewhere` or `unknown`.
    pub state: String,
    pub ordered_on: Option<NaiveDate>,
    pub customer: Option<String>,
    pub ship_to: Option<String>,
    /// What the order says to the people picking it, and what the business
    /// notes of the customer for its own people. Printed as written.
    pub picking_instructions: Option<String>,
    pub customer_notes: Option<String>,
    pub ship_via: Option<String>,
    pub po_ref: Option<String>,
    /// NetSuite's word for it.
    pub status: Option<String>,
    pub lines: Vec<ToPickLine>,
}

#[derive(Serialize, Debug)]
pub struct ToPickLine {
    pub line_key: String,
    pub line_no: Option<i32>,
    pub item_id: Option<Uuid>,
    pub code: String,
    pub description: Option<String>,
    /// The supplier's code printed on it (Art No.).
    pub art_no: Option<String>,
    pub picture: Option<Picture>,
    pub ordered: f64,
    pub committed: Option<f64>,
    /// Left to pick here, as NetSuite says.
    pub to_pick: f64,
    /// A kit's own line: ordered, never picked; its parts are (D223).
    pub kit: bool,
    /// The kit a part belongs to, by its code.
    pub part_of: Option<String>,
    /// Where to take it from, in the order to go.
    pub takes: Vec<Take>,
    /// What no bin NetSuite reports covers, after the orders before it.
    pub short: f64,
}

#[derive(Serialize, Debug, Clone)]
pub struct Take {
    pub location_id: Uuid,
    pub bin: String,
    pub quantity: f64,
    /// What NetSuite (or this system) says the bin holds, before this batch.
    pub on_hand: f64,
    pub within_reach: bool,
}

/// The batch shared out.
#[derive(Serialize, Debug)]
pub struct PickPlan {
    /// `pack`: trips start and end at the packing bench; `free`: the bench
    /// isn't on the layout; `none`: no bin of the batch is on it.
    pub from: String,
    pub cell_mm: Option<i32>,
    pub pickers: Vec<Vec<PlannedTrip>>,
}

#[derive(Serialize, Debug)]
pub struct PlannedTrip {
    /// The orders it carries back, by number.
    pub orders: Vec<String>,
    /// Its walk in the site's cells, and about how long it takes once the
    /// site is measured.
    pub walked: f64,
    pub minutes: Option<f64>,
    pub stops: Vec<PlannedStop>,
    /// Where it goes on the floor, from the bench and back, in the site's
    /// cells: the walk's route, as the map draws it. Empty off the layout.
    pub path: Vec<[f64; 2]>,
}

/// One shelf and one product, and how many for each order.
#[derive(Serialize, Debug)]
pub struct PlannedStop {
    /// The bin, for the map to show.
    pub location_id: Option<Uuid>,
    pub bin: Option<String>,
    pub within_reach: Option<bool>,
    pub code: String,
    pub description: Option<String>,
    pub takes: Vec<StopTake>,
    /// Its bin isn't on the layout, or there is no bin to go to: walked last.
    pub off_route: bool,
}

#[derive(Serialize, Debug)]
pub struct StopTake {
    pub order: String,
    pub quantity: f64,
    /// For an order on another trip: taken here, sorted at the bench.
    pub gathered: bool,
}

#[get("/sites/{id}/to-pick")]
pub async fn to_pick(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    q: web::Query<ToPickQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let site = path.into_inner();
    let asked = asked(q.orders.as_deref());
    let settings = q.settings();
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope.run(move |tx| Box::pin(async move { read(tx, site, &asked, settings).await })).await?;
    Ok(HttpResponse::Ok().json(out))
}

/// The orders as pasted: split, trimmed, each once, in the order given.
pub fn asked(pasted: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for t in pasted.unwrap_or("").split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
        let t = t.trim();
        if !t.is_empty() && !out.iter().any(|o| key(o) == key(t)) {
            out.push(t.to_string());
        }
        if out.len() == MOST_ASKED {
            break;
        }
    }
    out
}

/// What an order number is matched by: its digits, so S268281 and 268281
/// are one order; a number with no digits by itself, in capitals.
pub fn key(number: &str) -> String {
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() {
        number.trim().to_uppercase()
    } else {
        digits
    }
}

/// One line of the report, as read.
struct Reported {
    line_key: String,
    line_no: Option<i32>,
    item_id: Option<Uuid>,
    code: String,
    description: Option<String>,
    art_no: Option<String>,
    item_type: Option<String>,
    kit_line: Option<String>,
    ordered: f64,
    committed: Option<f64>,
    to_pick: f64,
}

/// A bin an item can be taken from, in its turn.
struct Pile {
    location_id: Uuid,
    bin: String,
    reach: bool,
    qty: f64,
}

pub async fn read(tx: &Transaction<'_>, site: Uuid, asked: &[String], settings: Settings) -> Result<ToPick, ApiError> {
    tx.query_opt("SELECT 1 FROM site WHERE id = $1", &[&site]).await?.ok_or(ApiError::NotFound)?;

    // What NetSuite has open here, oldest order first.
    let rows = tx
        .query(
            "SELECT order_number, ordered_on, customer, ship_to, picking_instructions, ship_via, po_ref, status,
                    line_key, line_no, item_id, item_code, description, item_type, kit_line,
                    ordered::float8, committed::float8, to_pick::float8, as_at, art_no, customer_notes
               FROM reported_order_line
              WHERE site_id = $1
              ORDER BY ordered_on NULLS LAST, order_number, line_no NULLS LAST, line_key",
            &[&site],
        )
        .await?;
    let orders_as_at: Option<DateTime<Utc>> = rows.iter().map(|r| r.get::<_, DateTime<Utc>>(18)).max();
    let mut open: Vec<(AskedOrder, Vec<Reported>)> = vec![];
    let mut open_at: HashMap<String, usize> = HashMap::new();
    for r in &rows {
        let number: String = r.get(0);
        let i = *open_at.entry(key(&number)).or_insert_with(|| {
            open.push((
                AskedOrder {
                    asked: number.clone(),
                    number: Some(number.clone()),
                    state: "waiting".into(),
                    ordered_on: r.get(1),
                    customer: r.get(2),
                    ship_to: r.get(3),
                    picking_instructions: r.get(4),
                    customer_notes: r.get(20),
                    ship_via: r.get(5),
                    po_ref: r.get(6),
                    status: r.get(7),
                    lines: vec![],
                },
                vec![],
            ));
            open.len() - 1
        });
        open[i].1.push(Reported {
            line_key: r.get(8),
            line_no: r.get(9),
            item_id: r.get(10),
            code: r.get(11),
            description: r.get(12),
            item_type: r.get(13),
            kit_line: r.get(14),
            ordered: r.get(15),
            committed: r.get(16),
            to_pick: r.get(17),
            art_no: r.get(19),
        });
    }

    // The batch: as asked, or everything open.
    let keys: Vec<String> = if asked.is_empty() { open.iter().map(|o| key(&o.0.asked)).collect() } else { asked.iter().map(|a| key(a)).collect() };
    // What Spork knows of each: whether NetSuite has picked any of it here
    // (a fulfilment, D172), and how those it has went on (D187).
    let known: HashMap<String, (String, bool, bool, Vec<String>)> = tx
        .query(
            "SELECT o.confirmation_number,
                    bool_or(f.id IS NOT NULL),
                    bool_or(f.id IS NOT NULL AND f.state <> 'cancelled' AND f.closed_elsewhere IS NULL),
                    coalesce(array_agg(DISTINCT f.closed_elsewhere) FILTER (WHERE f.closed_elsewhere IS NOT NULL), '{}')
               FROM \"order\" o
               LEFT JOIN fulfilment f ON f.order_id = o.id AND f.site_id = $1
              WHERE regexp_replace(o.confirmation_number, '[^0-9]', '', 'g') = ANY($2)
                 OR upper(o.confirmation_number) = ANY($2)
              GROUP BY o.confirmation_number",
            &[&site, &keys],
        )
        .await?
        .iter()
        .map(|r| {
            let number: String = r.get(0);
            (key(&number), (number, r.get(1), r.get(2), r.get(3)))
        })
        .collect();

    let mut batch: Vec<(AskedOrder, Vec<Reported>)> = vec![];
    let mut taken: Vec<Option<(AskedOrder, Vec<Reported>)>> = open.into_iter().map(Some).collect();
    for (i, k) in keys.iter().enumerate() {
        let shown = if asked.is_empty() { None } else { Some(asked[i].clone()) };
        if let Some(found) = open_at.get(k).and_then(|&at| taken[at].take()) {
            let (mut order, lines) = found;
            if let Some(a) = shown {
                order.asked = a;
            }
            if known.get(k).is_some_and(|kn| kn.1) {
                order.state = "part_picked".into();
            }
            batch.push((order, lines));
            continue;
        }
        let asked_as = shown.unwrap_or_else(|| k.clone());
        let (number, state) = match known.get(k) {
            None => (None, "unknown"),
            Some((n, here, open_here, closed)) => (
                Some(n.clone()),
                if *open_here {
                    "picked"
                } else if closed.iter().any(|c| c == "shipped") {
                    "shipped"
                } else if closed.iter().any(|c| c == "packed") {
                    "packed"
                } else if *here {
                    "closed"
                } else {
                    "elsewhere"
                },
            ),
        };
        batch.push((
            AskedOrder {
                asked: asked_as,
                number,
                state: state.into(),
                ordered_on: None,
                customer: None,
                ship_to: None,
                picking_instructions: None,
                customer_notes: None,
                ship_via: None,
                po_ref: None,
                status: None,
                lines: vec![],
            },
            vec![],
        ));
    }

    // Every bin each item could come from here, racking only, in its turn.
    let items: Vec<Uuid> = {
        let mut v: Vec<Uuid> = batch.iter().flat_map(|(_, ls)| ls.iter().filter_map(|l| l.item_id)).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let kinds: Vec<String> = crate::places::SOLID_KINDS.iter().map(|k| k.to_string()).collect();
    let mut piles: HashMap<Uuid, Vec<Pile>> = HashMap::new();
    let mut whereabouts: HashMap<Uuid, Option<Whereabouts>> = HashMap::new();
    for r in tx
        .query(
            &format!(
                "SELECT i.id, p.location_id, p.code, p.reach, p.qty::float8,
                        l.place_id, l.slot_side, l.slot_bay, l.slot_level, l.slot_row, l.slot_position
                   FROM unnest($1::uuid[]) AS i(id)
                  CROSS JOIN LATERAL ({}) p
                   JOIN location l ON l.id = p.location_id
                  WHERE l.kind::text = ANY($3)
                  ORDER BY i.id, p.nth",
                crate::items::piles("i.id", "$2")
            ),
            &[&items, &site, &kinds],
        )
        .await?
    {
        let location_id: Uuid = r.get(1);
        piles.entry(r.get(0)).or_default().push(Pile { location_id, bin: r.get(2), reach: r.get(3), qty: r.get(4) });
        whereabouts.entry(location_id).or_insert_with(|| {
            Some(Whereabouts {
                place_id: r.get::<_, Option<Uuid>>(5)?,
                cell: GridCell {
                    side: r.get::<_, Option<i16>>(6).unwrap_or(1) as i32,
                    bay: r.get::<_, Option<i32>>(7)?,
                    level: r.get::<_, Option<i32>>(8)?,
                    row: r.get::<_, Option<i32>>(9)?,
                    position: r.get::<_, Option<i32>>(10)?,
                },
            })
        });
    }
    let pictured = pictures::of(tx, &items).await?;

    // Each line over its bins, the batch in the order shown.
    let mut left: HashMap<(Uuid, Uuid), f64> = HashMap::new();
    let mut orders: Vec<AskedOrder> = vec![];
    for (mut order, lines) in batch {
        let sent: Vec<Sent> = lines
            .iter()
            .map(|l| Sent {
                key: Some(l.line_key.as_str()),
                kit_line: l.kit_line.as_deref(),
                item_type: l.item_type.as_deref(),
                item: l.code.clone(),
                line_no: l.line_no.unwrap_or(0),
            })
            .collect();
        let role = roles(&sent);
        // A kit is picked as its parts, when the order has them; a kit line
        // with none of its parts on it is picked as it is, so it isn't lost.
        let has_parts: Vec<bool> = sent
            .iter()
            .map(|s| role.iter().any(|r| *r == Role::PartOf { item: s.item.clone(), line_no: s.line_no }))
            .collect();
        for ((l, role), parts) in lines.into_iter().zip(role).zip(has_parts) {
            let kit = role == Role::Kit && parts;
            let part_of = match &role {
                Role::PartOf { item, .. } => Some(item.clone()),
                _ => None,
            };
            let mut takes = vec![];
            let mut need = if kit { 0.0 } else { l.to_pick };
            if let Some(item) = l.item_id {
                for p in piles.get(&item).map(Vec::as_slice).unwrap_or(&[]) {
                    if need <= 0.0 {
                        break;
                    }
                    let here = left.entry((item, p.location_id)).or_insert(p.qty);
                    let take = need.min(*here);
                    if take > 0.0 {
                        takes.push(Take { location_id: p.location_id, bin: p.bin.clone(), quantity: take, on_hand: p.qty, within_reach: p.reach });
                        *here -= take;
                        need -= take;
                    }
                }
            }
            order.lines.push(ToPickLine {
                line_key: l.line_key,
                line_no: l.line_no,
                item_id: l.item_id,
                picture: l.item_id.and_then(|i| pictured.get(&i).cloned()),
                code: l.code,
                description: l.description,
                art_no: l.art_no,
                ordered: l.ordered,
                committed: l.committed,
                to_pick: l.to_pick,
                kit,
                part_of,
                takes,
                short: need.max(0.0),
            });
        }
        orders.push(order);
    }

    let plan = plan(tx, site, &orders, &whereabouts, settings).await?;
    let balance_as_at: Option<DateTime<Utc>> =
        tx.query_one("SELECT max(as_at) FROM reported_stock WHERE site_id = $1", &[&site]).await?.get(0);
    Ok(ToPick { orders_as_at, balance_as_at, orders, plan })
}

/// One pick for the planner, in words.
struct Planned<'a> {
    order: &'a str,
    location: Option<Uuid>,
    bin: Option<&'a str>,
    reach: Option<bool>,
    code: &'a str,
    description: Option<&'a str>,
    quantity: f64,
}

/// Share the waiting orders out (D230).
async fn plan(
    tx: &Transaction<'_>,
    site: Uuid,
    orders: &[AskedOrder],
    whereabouts: &HashMap<Uuid, Option<Whereabouts>>,
    mut settings: Settings,
) -> Result<Option<PickPlan>, ApiError> {
    let waiting: Vec<&AskedOrder> =
        orders.iter().filter(|o| (o.state == "waiting" || o.state == "part_picked") && o.number.is_some()).collect();
    if waiting.len() > MOST_PLANNED {
        return Ok(None);
    }
    // Every take, and what no bin covers, as a pick.
    let mut picks: Vec<(usize, Planned)> = vec![];
    for (i, o) in waiting.iter().enumerate() {
        let number = o.number.as_deref().unwrap_or(&o.asked);
        for l in o.lines.iter().filter(|l| !l.kit) {
            for t in &l.takes {
                picks.push((
                    i,
                    Planned {
                        order: number,
                        location: Some(t.location_id),
                        bin: Some(&t.bin),
                        reach: Some(t.within_reach),
                        code: &l.code,
                        description: l.description.as_deref(),
                        quantity: t.quantity,
                    },
                ));
            }
            if l.short > 0.0 {
                picks.push((
                    i,
                    Planned {
                        order: number,
                        location: None,
                        bin: None,
                        reach: None,
                        code: &l.code,
                        description: l.description.as_deref(),
                        quantity: l.short,
                    },
                ));
            }
        }
    }
    if picks.is_empty() {
        return Ok(None);
    }

    // The walk between the batch's bins.
    let locations: Vec<Uuid> = {
        let mut v: Vec<Uuid> = picks.iter().filter_map(|p| p.1.location).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let bins: Vec<Option<Whereabouts>> = locations.iter().map(|l| whereabouts.get(l).copied().flatten()).collect();
    let walk = walk_route::distances(tx, site, &bins).await?;
    let (cost, node_of, from, cell_mm) = match &walk {
        Some(d) => (d.cost.clone(), d.node_of.clone(), d.from.clone(), d.cell_mm),
        None => (vec![vec![0.0]], vec![None; locations.len()], "none".to_string(), None),
    };
    // A stop's time as a walk, in cells: a cell taken as a metre until the
    // site is measured.
    let metres_per_cell = cell_mm.map(|mm| mm as f64 / 1000.0).unwrap_or(1.0);
    settings.stop_cost = STOP_S * WALK_M_PER_S / metres_per_cell;

    // Which shelf and product each pick is. What no bin covers is each its
    // own: there is no shelf to share a walk to.
    let mut whats: Vec<Option<(Uuid, &str)>> = vec![];
    let input: Vec<Pick> = picks
        .iter()
        .map(|(o, p)| {
            let what = p.location.map(|l| (l, p.code));
            let w = what.and_then(|w| whats.iter().position(|x| *x == Some(w))).unwrap_or_else(|| {
                whats.push(what);
                whats.len() - 1
            });
            let stop = p.location.and_then(|l| locations.iter().position(|x| *x == l)).and_then(|i| node_of[i]);
            Pick { order: *o, stop, what: w }
        })
        .collect();
    let planned = pick_groups::plan(&cost, waiting.len(), &input, &settings);

    let stops = |indices: &[usize], trip_orders: &[usize], off_route: bool| -> Vec<PlannedStop> {
        let mut out: Vec<(usize, PlannedStop)> = vec![];
        for &i in indices {
            let (o, p) = &picks[i];
            let w = input[i].what;
            let take = StopTake { order: p.order.to_string(), quantity: p.quantity, gathered: !trip_orders.contains(o) };
            match out.iter_mut().find(|(x, _)| *x == w) {
                Some((_, s)) => s.takes.push(take),
                None => out.push((
                    w,
                    PlannedStop {
                        location_id: p.location,
                        bin: p.bin.map(String::from),
                        within_reach: p.reach,
                        code: p.code.to_string(),
                        description: p.description.map(String::from),
                        takes: vec![take],
                        off_route,
                    },
                )),
            }
        }
        out.into_iter().map(|(_, s)| s).collect()
    };
    let pickers = planned
        .pickers
        .iter()
        .map(|trips| {
            trips
                .iter()
                .map(|t| {
                    let mut route: Vec<PlannedStop> = t.stops.iter().flat_map(|(_, at)| stops(at, &t.orders, false)).collect();
                    route.extend(stops(&t.off_route, &t.orders, true));
                    let minutes = cell_mm.map(|_| (t.walked * metres_per_cell / WALK_M_PER_S + STOP_S * route.len() as f64) / 60.0);
                    let nodes: Vec<usize> = t.stops.iter().map(|s| s.0).collect();
                    PlannedTrip {
                        orders: t.orders.iter().map(|&o| waiting[o].number.clone().unwrap_or_default()).collect(),
                        walked: t.walked,
                        minutes,
                        stops: route,
                        path: walk.as_ref().map(|d| d.line(&nodes)).unwrap_or_default(),
                    }
                })
                .collect()
        })
        .collect();
    Ok(Some(PickPlan { from, cell_mm, pickers }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pasted_row_is_its_orders_once_each() {
        let row = "S268281\tS268299\t 268281 ,S268300;;\n";
        assert_eq!(asked(Some(row)), vec!["S268281", "S268299", "S268300"], "268281 is S268281 again");
        assert!(asked(None).is_empty());
    }

    #[test]
    fn an_order_is_matched_by_its_digits() {
        assert_eq!(key("S268281"), "268281");
        assert_eq!(key(" 268281 "), "268281");
        assert_eq!(key("rush"), "RUSH");
    }
}
