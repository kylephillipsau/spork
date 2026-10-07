//! Picking tickets and walk sheets, printed (D231, D230).
//!
//! One order to an A4 landscape page, so the ticket goes with its goods to the
//! bench: the order number large enough to read across it and again at the
//! foot, its barcode for the handheld, the picking instructions and customer
//! notes NetSuite's own ticket stopped printing, and each line's bin as the
//! biggest thing on it, in walking order. With `walk`, each trip's walk sheet
//! goes on top of its tickets: every stop in order, how many for each order,
//! and what is carried back for another group.
//!
//! The batch is [`to_pick::read`]'s, the screen's own, so the paper and the
//! screen cannot disagree.

use std::collections::HashMap;

use actix_web::{get, web, HttpRequest, HttpResponse};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use uuid::Uuid;

use crate::error::ApiError;
use crate::tenancy::TenantScope;
use crate::to_pick::{self, AskedOrder, PlannedTrip, ToPick, ToPickLine, ToPickQuery};
use crate::AppState;

#[get("/print/pick-tickets/{site}")]
pub async fn pick_tickets(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    q: web::Query<ToPickQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = match super::require_caller(&state, &req).await {
        Ok(c) => c,
        Err(redirect) => return Ok(redirect),
    };
    let site = path.into_inner();
    let asked = to_pick::asked(q.orders.as_deref());
    let settings = q.settings();
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let (batch, place, printed) = scope
        .run(move |tx| {
            Box::pin(async move {
                let batch = to_pick::read(tx, site, &asked, settings).await?;
                let r = tx
                    .query_one(
                        "SELECT name, to_char(now() AT TIME ZONE timezone, 'FMDD/FMMM/YYYY HH24:MI') FROM site WHERE id = $1",
                        &[&site],
                    )
                    .await?;
                Ok((batch, r.get::<_, String>(0), r.get::<_, String>(1)))
            })
        })
        .await?;
    let sheets = Sheets::new(&batch, settings.pickers > 1);
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(page(&sheets, &place, &printed, q.walk).into_string()))
}

/// A take, by the order it is for, its bin and its item.
type Key<'a> = (&'a str, &'a str, &'a str);

/// The batch laid out for paper: which trip and group each order is on, where
/// each take comes in the walk, and which takes another group collects.
struct Sheets<'a> {
    /// Each trip with its group's name and its number in the group.
    trips: Vec<(String, usize, &'a PlannedTrip)>,
    /// Waiting orders by number.
    orders: HashMap<&'a str, &'a AskedOrder>,
    /// An order's group, when the batch is shared between pickers.
    group_of: HashMap<&'a str, String>,
    /// Where a take comes in its trip's walk.
    rank: HashMap<Key<'a>, usize>,
    /// Takes another group collects, and which.
    collected: HashMap<Key<'a>, String>,
    /// Waiting orders the plan has no trip for, in the batch's order.
    loose: Vec<&'a AskedOrder>,
    grouped: bool,
    /// Trips start and end at the packing bench.
    from_bench: bool,
}

impl<'a> Sheets<'a> {
    fn new(batch: &'a ToPick, grouped: bool) -> Self {
        let waiting: Vec<&AskedOrder> = batch
            .orders
            .iter()
            .filter(|o| (o.state == "waiting" || o.state == "part_picked") && o.number.is_some())
            .collect();
        let orders: HashMap<&str, &AskedOrder> = waiting.iter().map(|o| (o.number.as_deref().unwrap_or(""), *o)).collect();
        let mut trips = vec![];
        if let Some(plan) = &batch.plan {
            for (g, group) in plan.pickers.iter().enumerate() {
                for (t, trip) in group.iter().enumerate() {
                    trips.push((group_name(g), t + 1, trip));
                }
            }
        }
        let mut group_of = HashMap::new();
        let mut rank = HashMap::new();
        let mut collected = HashMap::new();
        for (group, _, trip) in &trips {
            for o in &trip.orders {
                group_of.insert(o.as_str(), group.clone());
            }
            for (n, stop) in trip.stops.iter().enumerate() {
                let (Some(bin), code) = (stop.bin.as_deref(), stop.code.as_str()) else { continue };
                for take in &stop.takes {
                    rank.insert((take.order.as_str(), bin, code), n);
                    if take.gathered {
                        collected.insert((take.order.as_str(), bin, code), group.clone());
                    }
                }
            }
        }
        let planned: Vec<&str> = trips.iter().flat_map(|t| t.2.orders.iter().map(String::as_str)).collect();
        let loose = waiting.into_iter().filter(|o| !planned.contains(&o.number.as_deref().unwrap_or(""))).collect();
        let from_bench = batch.plan.as_ref().is_some_and(|p| p.from == "pack");
        Sheets { trips, orders, group_of, rank, collected, loose, grouped, from_bench }
    }
}

/// Group A, B, C… in the order the planner gave them.
fn group_name(i: usize) -> String {
    format!("Group {}", char::from(b'A' + (i % 26) as u8))
}

fn page(sheets: &Sheets, place: &str, printed: &str, walk: bool) -> Markup {
    let ticket_head = format!("Pick ticket · {place} · printed {printed}");
    let walk_head = format!("Walk sheet · {place} · printed {printed}");
    let count = sheets.trips.iter().map(|t| t.2.orders.len()).sum::<usize>() + sheets.loose.len();
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Pick tickets · Spork" }
                style { (PreEscaped(CSS)) }
            }
            body {
                div class="bar" {
                    strong { (count) @if count == 1 { " ticket" } @else { " tickets" } }
                    @if walk { span { "with a walk sheet on each trip" } }
                    button onclick="window.print()" { "Print" }
                }
                @if count == 0 {
                    section class="page" { p class="none" { "Nothing waiting to pick in this batch." } }
                }
                @for (group, n, trip) in &sheets.trips {
                    // A trip whose every pick another group collects has nothing to walk.
                    @if walk && !trip.stops.is_empty() { (walk_sheet(sheets, &walk_head, group, *n, trip)) }
                    @for number in &trip.orders {
                        @if let Some(order) = sheets.orders.get(number.as_str()) {
                            (ticket(sheets, &ticket_head, order))
                        }
                    }
                }
                @for order in &sheets.loose { (ticket(sheets, &ticket_head, order)) }
            }
        }
    }
}

/// One row of a ticket.
enum Row<'a> {
    /// A kit's own line, above its parts.
    Kit(&'a ToPickLine),
    Take { line: &'a ToPickLine, bin: &'a str, quantity: f64, high: bool, by: Option<&'a String> },
    /// What no bin covers.
    Short(&'a ToPickLine),
}

fn ticket(sheets: &Sheets, head: &str, order: &AskedOrder) -> Markup {
    let number = order.number.as_deref().unwrap_or(&order.asked);
    // Lines in walking order: each by its first take's place in the walk, and
    // what no bin covers last. A kit's line goes above its first part.
    let place = |l: &ToPickLine| {
        l.takes.iter().filter_map(|t| sheets.rank.get(&(number, t.bin.as_str(), l.code.as_str()))).min().copied().unwrap_or(usize::MAX)
    };
    let mut goods: Vec<&ToPickLine> = order.lines.iter().filter(|l| !l.kit).collect();
    goods.sort_by_key(|l| place(l));
    let mut rows = vec![];
    let mut kits_shown: Vec<&str> = vec![];
    for line in &goods {
        if let Some(kit) = line.part_of.as_deref().filter(|k| !kits_shown.contains(k)) {
            if let Some(k) = order.lines.iter().find(|k| k.kit && k.code == kit) {
                rows.push(Row::Kit(k));
            }
            kits_shown.push(kit);
        }
        for t in &line.takes {
            let by = sheets.collected.get(&(number, t.bin.as_str(), line.code.as_str()));
            rows.push(Row::Take { line, bin: &t.bin, quantity: t.quantity, high: !t.within_reach, by });
        }
        if line.short > 0.0 {
            rows.push(Row::Short(line));
        }
    }
    let units: f64 = goods.iter().map(|l| l.to_pick).sum();
    let address = order.ship_to.as_deref().map(address);
    html! {
        section class="page" {
            div class="runhead" { span { (head) } }
            article class="ticket" {
                header class="top" {
                    div { div class="num" { (number) } (barcode(number)) }
                    div class="who" {
                        div class="name" { (order.customer.as_deref().unwrap_or("")) }
                        @if let Some(a) = &address { div class="addr" { (a) } }
                        @if order.picking_instructions.is_some() || order.customer_notes.is_some() {
                            div class="notes" {
                                @if let Some(p) = &order.picking_instructions { div { b { "Picking instructions" } (p) } }
                                @if let Some(c) = &order.customer_notes { div { b { "Customer notes" } (c) } }
                            }
                        }
                    }
                    dl class="facts" {
                        @if let Some(d) = order.ordered_on { dt { "Date" } dd { (d.format("%-d/%-m/%Y")) } }
                        @if let Some(v) = &order.ship_via { dt { "Ship via" } dd { (v) } }
                        @if let Some(p) = &order.po_ref { dt { "PO" } dd class="mono" { (p) } }
                        @if sheets.grouped {
                            @if let Some(g) = sheets.group_of.get(number) { dt { "Group" } dd { (g.trim_start_matches("Group ")) } }
                        }
                    }
                }
                table class="lines" {
                    thead { tr { th class="box" { "Got" } th class="bin" { "Bin" } th class="qty" { "Qty" } th class="code" { "Item" } th { "Description" } th class="art" { "Art no." } } }
                    tbody {
                        @for row in &rows {
                            @match row {
                                Row::Kit(k) => {
                                    tr class="kit" { td {} td colspan="5" { (k.code) " × " (qty(k.to_pick)) @if let Some(d) = &k.description { " · " (d) } ", picked as its parts:" } }
                                }
                                Row::Take { line, bin, quantity, by: Some(group), .. } => {
                                    tr class="elsewhere" {
                                        td class="box" {} td class="bin" { (bin) } td class="qty" { (qty(*quantity)) } td class="code" { (line.code) }
                                        td class="desc" { (line.description.as_deref().unwrap_or("")) span class="also" { "Collected by " (group) ". It comes to you at the bench." } }
                                        td class="art" { (line.art_no.as_deref().unwrap_or("")) }
                                    }
                                }
                                Row::Take { line, bin, quantity, high, by: None } => {
                                    tr class=[line.part_of.as_ref().map(|_| "part")] {
                                        td class="box" { span {} } td class="bin" { (bin) @if *high { span class="high" { "HIGH" } } } td class="qty" { (qty(*quantity)) } td class="code" { (line.code) }
                                        td class="desc" { (line.description.as_deref().unwrap_or("")) (ordered_note(line)) }
                                        td class="art" { (line.art_no.as_deref().unwrap_or("")) }
                                    }
                                }
                                Row::Short(line) => {
                                    tr class="short" {
                                        td class="box" { span {} } td class="bin" { "No stock in a bin" } td class="qty" { (qty(line.short)) } td class="code" { (line.code) }
                                        td class="desc" { (line.description.as_deref().unwrap_or("")) span class="also" { "NetSuite shows none on a shelf here. Check before you go." } }
                                        td class="art" { (line.art_no.as_deref().unwrap_or("")) }
                                    }
                                }
                            }
                        }
                    }
                }
                footer class="foot" {
                    div class="again" {
                        span class="mono" { (number) }
                        (order.customer.as_deref().unwrap_or("")) " · " (goods.len()) @if goods.len() == 1 { " line" } @else { " lines" } " · " (qty(units)) " units"
                    }
                    div class="sign" { span { "Picked by " i {} } span { "Checked by " i {} } }
                }
            }
        }
    }
}

/// "12 ordered, 10 to pick", when what is to pick isn't what was ordered.
fn ordered_note(line: &ToPickLine) -> Markup {
    html! {
        @if line.to_pick < line.ordered {
            span class="also" { (qty(line.ordered)) " ordered, " (qty(line.to_pick)) " to pick" }
        }
    }
}

fn walk_sheet(sheets: &Sheets, head: &str, group: &str, n: usize, trip: &PlannedTrip) -> Markup {
    let title = if sheets.grouped { format!("{group} · trip {n}") } else { format!("Trip {n}") };
    let units: f64 = trip.stops.iter().flat_map(|s| &s.takes).map(|t| t.quantity).sum();
    let theirs: f64 = trip.stops.iter().flat_map(|s| &s.takes).filter(|t| t.gathered).map(|t| t.quantity).sum();
    // Stops off the floor plan are walked last; said so only when some aren't.
    let mixed = trip.stops.iter().any(|s| s.off_route) && trip.stops.iter().any(|s| !s.off_route);
    html! {
        section class="page" {
            div class="runhead" { span { (head) } span { (title) } }
            header class="walkhead" {
                h2 { (title) }
                div class="sum" {
                    b { (trip.stops.len()) " stops" } " · " (qty(units)) " units"
                    @if theirs > 0.0 { ", " (qty(theirs)) " of them for other groups" }
                    @if let Some(m) = trip.minutes {
                        br; "About " (m.round().max(1.0)) " min"
                        @if sheets.from_bench { ", from the packing bench and back" }
                    }
                }
                div class="orders" {
                    @for o in &trip.orders {
                        div { span class="mono" { (o) } span { (sheets.orders.get(o.as_str()).and_then(|x| x.customer.as_deref()).unwrap_or("")) } }
                    }
                }
            }
            table class="walk" {
                thead { tr { th class="n" { "Stop" } th class="bin" { "Bin" } th { "Item" } th { "For each order" } th class="total" { "All" } th class="box" { "Got" } } }
                tbody {
                    @for (i, stop) in trip.stops.iter().enumerate() {
                        @if mixed && stop.off_route && !trip.stops[i - 1].off_route {
                            tr class="offhead" { td colspan="6" { "Not on the floor plan yet" } }
                        }
                        tr {
                            td class="n" { (i + 1) }
                            td class="bin" {
                                @match &stop.bin { Some(b) => { (b) }, None => { span class="none" { "No bin" } } }
                                @if stop.within_reach == Some(false) { span class="high" { "HIGH" } }
                            }
                            td class="code" { (stop.code) span class="desc" { (stop.description.as_deref().unwrap_or("")) } }
                            td class="split" {
                                @for t in stop.takes.iter().filter(|t| !t.gathered) {
                                    span class="t" { span class="mono" { (t.order) } "×" (qty(t.quantity)) }
                                }
                                @for t in stop.takes.iter().filter(|t| t.gathered) {
                                    span class="bench" { (qty(t.quantity)) " more for " (t.order)
                                        @if let Some(g) = sheets.group_of.get(t.order.as_str()) { " (" (g) ")" }
                                        ", to the bench" }
                                }
                            }
                            td class="total" { (qty(stop.takes.iter().map(|t| t.quantity).sum())) }
                            td class="box" { span {} }
                        }
                    }
                }
            }
            @if theirs > 0.0 {
                p class="walknote" { "Back at the bench, put each order's goods with its ticket, and hand what is marked for another group to whoever packs it." }
            }
        }
    }
}

/// A quantity as a person writes it: no ".0" on a whole one.
fn qty(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// The ship-to as one line, its country left off when it's this one.
fn address(ship_to: &str) -> String {
    let lines: Vec<&str> = ship_to.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let lines = match lines.split_last() {
        Some((last, rest)) if last.eq_ignore_ascii_case("australia") => rest.to_vec(),
        _ => lines,
    };
    lines.join(", ")
}

// ---------------------------------------------------------------------------
// Code 128, set B: the order number as the handheld scans it
// ---------------------------------------------------------------------------

/// Each symbol's bars and spaces, in modules: 0 to 102, the three starts, and stop.
const CODE128: [&str; 107] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312", "132212", "221213", "221312",
    "231212", "112232", "122132", "122231", "113222", "123122", "123221", "223211", "221132", "221231", "213212",
    "223112", "312131", "311222", "321122", "321221", "312212", "322112", "322211", "212123", "212321", "232121",
    "111323", "131123", "131321", "112313", "132113", "132311", "211313", "231113", "231311", "112133", "112331",
    "132131", "113123", "113321", "133121", "313121", "211331", "231131", "213113", "213311", "213131", "311123",
    "311321", "331121", "312113", "312311", "332111", "314111", "221411", "431111", "111224", "111422", "121124",
    "121421", "141122", "141221", "112214", "112412", "122114", "122411", "142112", "142211", "241211", "221114",
    "413111", "241112", "134111", "111242", "121142", "121241", "114212", "124112", "124211", "411212", "421112",
    "421211", "212141", "214121", "412121", "111143", "111341", "131141", "114113", "114311", "411113", "411311",
    "113141", "114131", "311141", "411131", "211412", "211214", "211232", "2331112",
];
const START_B: usize = 104;
const STOP: usize = 106;

/// The bars of `text` in Code 128 set B, as (start, width) in modules, and the
/// whole width with its quiet zones. Nothing for a character set B lacks.
fn code128(text: &str) -> Option<(Vec<(u32, u32)>, u32)> {
    let values: Vec<usize> = text.bytes().map(|b| (32..=126).contains(&b).then_some(usize::from(b) - 32)).collect::<Option<_>>()?;
    let check = (START_B + values.iter().enumerate().map(|(i, v)| (i + 1) * v).sum::<usize>()) % 103;
    let quiet = 10;
    let mut x = quiet;
    let mut bars = vec![];
    for symbol in std::iter::once(START_B).chain(values).chain([check, STOP]) {
        for (k, w) in CODE128[symbol].bytes().map(|b| u32::from(b - b'0')).enumerate() {
            if k % 2 == 0 {
                bars.push((x, w));
            }
            x += w;
        }
    }
    Some((bars, x + quiet))
}

fn barcode(text: &str) -> Markup {
    match code128(text) {
        None => html! {},
        Some((bars, width)) => html! {
            svg class="barcode" viewBox=(format!("0 0 {width} 40")) preserveAspectRatio="none"
                style=(format!("width:{:.1}mm", f64::from(width) * 0.38)) role="img" aria-label=(format!("Barcode {text}")) {
                @for (x, w) in &bars { rect x=(x) y="0" width=(w) height="40" {} }
            }
        },
    }
}

const CSS: &str = r#"
@page { size: A4 landscape; margin: 9mm 10mm; }
:root {
  --ink: #000; --soft: #3a3a3a; --faint: #6b6b6b; --hair: #b5b5b5;
  --sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  --mono: ui-monospace, "SF Mono", SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace;
}
* { box-sizing: border-box; }
body { margin: 0; background: #e6e8e3; color: var(--ink); font-family: var(--sans); font-size: 10.5pt; font-variant-numeric: tabular-nums; }
.bar { position: sticky; top: 0; z-index: 1; display: flex; gap: 14px; align-items: center; padding: 10px 16px; background: #fff; border-bottom: 1px solid #c9ccc5; font-size: 14px; }
.bar button { margin-left: auto; font: 600 15px var(--sans); padding: 8px 20px; cursor: pointer; }
.page { width: 297mm; min-height: 210mm; margin: 14px auto; padding: 9mm 10mm; background: #fff; box-shadow: 0 1px 2px rgba(0,0,0,.18), 0 8px 24px rgba(0,0,0,.12); display: flex; flex-direction: column; break-after: page; }
.page:last-child { break-after: auto; }
@media print {
  body { background: #fff; }
  .bar { display: none; }
  .page { width: auto; min-height: 192mm; margin: 0; padding: 0; box-shadow: none; }
}
.none { color: var(--faint); }
.mono { font-family: var(--mono); }
.runhead { display: flex; justify-content: space-between; font-size: 8pt; color: var(--faint); padding-bottom: 3mm; }

.ticket { flex: 1; display: flex; flex-direction: column; border: 1.5pt solid var(--ink); }
.top { display: grid; grid-template-columns: 72mm 1fr auto; gap: 3mm 8mm; padding: 4mm 5mm 3.5mm; border-bottom: 1.5pt solid var(--ink); }
.num { font-family: var(--mono); font-size: 40pt; font-weight: 700; line-height: .95; letter-spacing: -.03em; }
.barcode { display: block; height: 12mm; margin-top: 2.5mm; }
.who { min-width: 0; display: grid; gap: 1.2mm; align-content: start; }
.who .name { font-size: 18pt; font-weight: 700; line-height: 1.1; }
.who .addr { font-size: 10pt; color: var(--soft); }
.notes { margin-top: 1.5mm; border: 1.5pt solid var(--ink); padding: 1.8mm 2.5mm; font-size: 11pt; font-weight: 600; line-height: 1.35; display: grid; gap: 1.5mm; white-space: pre-line; }
.notes b { display: block; font-size: 7.5pt; font-weight: 700; text-transform: uppercase; letter-spacing: .08em; margin-bottom: .6mm; white-space: normal; }
.facts { display: grid; grid-template-columns: auto auto; gap: 1.2mm 4mm; font-size: 10pt; align-content: start; margin: 0; }
.facts dt { color: var(--faint); }
.facts dd { margin: 0; font-weight: 700; text-align: right; }

table { width: 100%; border-collapse: collapse; }
th { font-size: 7.5pt; text-transform: uppercase; letter-spacing: .06em; color: var(--faint); font-weight: 600; text-align: left; padding: 2mm 2.5mm 1mm; border-bottom: .75pt solid var(--ink); }
td { padding: 2.2mm 2.5mm; border-bottom: .5pt solid var(--hair); vertical-align: baseline; }
tr { break-inside: avoid; }
.box { width: 11mm; }
td.box span { display: inline-block; width: 8mm; height: 6.5mm; border: 1pt solid var(--ink); vertical-align: middle; }
.lines .bin { width: 42mm; white-space: nowrap; }
td.bin { font-family: var(--mono); font-size: 18pt; font-weight: 700; }
.qty { width: 15mm; text-align: right; }
td.qty { font-size: 18pt; font-weight: 700; }
.lines .code { width: 42mm; white-space: nowrap; }
td.code { font-family: var(--mono); font-size: 12pt; font-weight: 700; }
td.desc { font-size: 10.5pt; }
.also { display: block; font-size: 8.5pt; color: var(--faint); margin-top: .6mm; }
.art { width: 30mm; white-space: nowrap; }
td.art { font-family: var(--mono); font-size: 9pt; color: var(--soft); }
.high { display: inline-block; font-family: var(--sans); font-size: 7.5pt; font-weight: 700; letter-spacing: .05em; border: 1pt solid var(--ink); padding: 0 1.2mm; vertical-align: 5pt; margin-left: 1.5mm; }
.kit td { padding-top: 2.4mm; padding-bottom: .4mm; font-size: 10pt; font-weight: 700; border-bottom: none; }
.part td.code { padding-left: 6mm; }
.short td { background: repeating-linear-gradient(135deg, #fff 0 2mm, #ececec 2mm 4mm); }
.short td.bin { font-family: var(--sans); font-size: 10pt; white-space: normal; }
.elsewhere td { color: var(--faint); }
.elsewhere td.bin, .elsewhere td.qty, .elsewhere td.code { font-weight: 400; }
.foot { margin-top: auto; display: grid; grid-template-columns: 1fr auto; align-items: end; gap: 3mm 8mm; border-top: 1.5pt solid var(--ink); padding: 3mm 5mm 3.5mm; }
.again { font-size: 9pt; color: var(--soft); }
.again .mono { display: block; font-size: 20pt; font-weight: 700; color: var(--ink); letter-spacing: -.02em; }
.sign { display: flex; gap: 10mm; font-size: 9.5pt; }
.sign span { display: flex; gap: 2mm; align-items: end; }
.sign i { display: inline-block; width: 48mm; border-bottom: .75pt solid var(--ink); }

.walkhead { display: grid; grid-template-columns: 1fr auto; gap: 2mm 6mm; border: 1.5pt solid var(--ink); padding: 4mm 5mm; }
.walkhead h2 { margin: 0; font-size: 22pt; line-height: 1.1; }
.walkhead .sum { text-align: right; font-size: 10pt; line-height: 1.45; }
.walkhead .sum b { font-size: 13pt; }
.orders { grid-column: 1 / -1; display: grid; grid-template-columns: repeat(auto-fill, minmax(70mm, 1fr)); gap: 2mm 5mm; border-top: .75pt solid var(--ink); padding-top: 2.5mm; }
.orders > div { display: flex; gap: 2.5mm; align-items: baseline; font-size: 10.5pt; }
.orders .mono { font-weight: 700; font-size: 13pt; }
table.walk { margin-top: 5mm; border: 1.5pt solid var(--ink); }
.walk .n { width: 9mm; }
.walk td.n { font-size: 10pt; color: var(--faint); }
.walk .bin { width: 44mm; white-space: nowrap; }
.walk .desc { display: block; font-family: var(--sans); font-size: 9pt; font-weight: 400; color: var(--soft); margin-top: .6mm; }
.walk td.split { white-space: nowrap; }
.walk .t { display: inline-flex; align-items: baseline; gap: 1.5mm; margin-right: 5mm; font-size: 13pt; font-weight: 700; }
.walk .t .mono { font-size: 10.5pt; }
.total { width: 14mm; text-align: right; }
td.total { font-size: 15pt; font-weight: 700; }
.bench { display: block; margin-top: 1.2mm; font-size: 9pt; font-weight: 700; white-space: normal; }
.bench::before { content: "\2192\00a0"; }
.offhead td { font-size: 7.5pt; text-transform: uppercase; letter-spacing: .06em; color: var(--faint); padding-top: 3mm; border-bottom: .75pt solid var(--ink); }
.walknote { margin-top: 4mm; font-size: 10.5pt; border-left: 2.5pt solid var(--ink); padding-left: 3mm; line-height: 1.45; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code128_is_start_characters_check_and_stop() {
        let (bars, width) = code128("S100231").unwrap();
        // Start, seven characters, check and stop: 10 symbols of 11 modules,
        // the stop 2 more, and two quiet zones of 10.
        assert_eq!(width, 10 * 11 + 2 + 20);
        assert_eq!(bars.len(), 10 * 3 + 1, "three bars a symbol, four in the stop");
        assert!(code128("S100231é").is_none(), "set B has no é");
    }
}
