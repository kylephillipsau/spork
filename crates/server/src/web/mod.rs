//! The pack station, server-rendered.
//!
//! Milestone 2 of the demo scope: sign in, find the order, pack, print. These
//! pages replace stages 1 to 6 of the recorded process, which today span two
//! systems and about a dozen screens.
//!
//! # Why these render here rather than in a client
//!
//! The client stack in `architecture.md` says React, and that is prose rather
//! than a numbered decision — the same sentence in the process document also
//! says Diesel, which this project has never used. What decided it for the demo
//! is narrower: **stage 5 is a printed A4 sheet folded onto a pallet.** A
//! document is the thing HTML is actually for, and one binary with no asset
//! pipeline puts the remaining time into screens.
//!
//! # What these pages may not do
//!
//! **Write.** Every mutation goes to the JSON API that already exists, is
//! authenticated, and is covered by the walk in `tests/pack_walk_http.rs`. A
//! second write path rendering its own SQL is how a packing list starts
//! disagreeing with the ledger it claims to describe. So these handlers read,
//! render, and hand the browser a `fetch` for anything that changes.


use actix_web::{get, web, HttpRequest, HttpResponse, Responder};

mod style;
use maud::{html, Markup, DOCTYPE};
use uuid::Uuid;

use crate::auth::Caller;
use crate::error::ApiError;
use crate::bench::PackedRow;
use crate::tenancy::TenantScope;
use crate::AppState;

/// The stylesheet, which is now a print stylesheet.
///
/// It served nine screens and serves one. What is left of it that matters is
/// the `@media print` block: A4, black on white, rules at 1.5pt and a docket
/// that will not break across a page.
#[get("/print/style.css")]
pub async fn stylesheet() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/css; charset=utf-8")
        .insert_header(("cache-control", "public, max-age=300"))
        .body(style::CSS)
}

/// An A4 sheet, and nothing else.
///
/// **This was `shell`, and it carried a navigation bar.** Nine maud screens
/// shared it; one remains, and a packing list folded onto a pallet has no use
/// for links to Packing, Findings, Weigh and Keys — which is just as well,
/// because those pages are gone and the links would have pointed at nothing.
/// The print rules hid the bar on paper anyway, so what it really was is chrome
/// that only ever showed on a screen nobody was meant to be reading this on.
fn document(title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · Nylonite" }
                link rel="stylesheet" href="/print/style.css";
            }
            body {
                main { (body) }
            }
        }
    }
}

/// A signed-out page is a redirect, not an error.
///
/// The API answers 401 because a machine should be told plainly; a person who
/// walks up to a browser should be shown the form.
async fn require_caller(
    state: &web::Data<AppState>,
    req: &HttpRequest,
) -> Result<Caller, HttpResponse> {
    match crate::routes::caller(state, req).await {
        Ok(c) => Ok(c),
        Err(_) => Err(HttpResponse::SeeOther()
            .insert_header(("location", "/sign-in"))
            .finish()),
    }
}

// ---------------------------------------------------------------------------
// Stage 5: the packing list, which is the thing NetSuite cannot print
// ---------------------------------------------------------------------------

// **`/print`, which is where D113 always said this was going.** `assets.rs`
// wrote the sentence before there was anything to move: *"when those narrow to
// `/print/*`, `/app` is free for this."* The client has the root now, the other
// maud pages are still ports in progress at `/app`, and this one is not a port
// at all — it is an A4 sheet folded onto a pallet, which is what HTML is
// actually for and why it stays.
#[get("/print/packing-list/{id}")]
pub async fn packing_list_page(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = match require_caller(&state, &req).await {
        Ok(c) => c,
        Err(redirect) => return Ok(redirect),
    };
    let package_id = path.into_inner();
    let carton = carton_view(&state, &who, package_id).await?;

    Ok(HttpResponse::Ok().content_type("text/html; charset=utf-8").body(
        document(
            "Packing list",
            html! {
                div class="noprint" {
                    h1 { "Packing list" }
                    p { button onclick="window.print()" { "Print" } }
                }

                div class="docket" {
                    div class="band" {
                        span class="seq" { "Carton " (carton.sequence) }
                        @if let Some(t) = &carton.package_type { span class="of" { (t) } }
                        span class="right" {
                            @if let Some(r) = &carton.fulfilment_reference {
                                code { (r) }
                                @if carton.order_reference.is_some() { " · " }
                            }
                            @if let Some(r) = &carton.order_reference { code { (r) } }
                        }
                    }
                    div class="body" {
                        @if carton.lines.is_empty() {
                            p class="muted" { "Empty." }
                        } @else {
                            table {
                                thead { tr {
                                    th { "Item" }
                                    th { "Lot" }
                                    th class="r" { "Qty" }
                                } }
                                tbody {
                                    @for l in &carton.lines {
                                        tr {
                                            td {
                                                code { (l.item_code) }
                                                @if let Some(d) = &l.description {
                                                    div class="muted" { (d) }
                                                }
                                            }
                                            td {
                                                @match &l.lot_code {
                                                    Some(c) => code { (c) },
                                                    None => span class="muted" { "—" },
                                                }
                                            }
                                            td class="r num" { (l.quantity) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div class="stamp" {
                        div {
                            span class="k" { "Gross weight" }
                            span class="v" {
                                @match carton.gross_weight_g {
                                    Some(g) => { (format!("{:.2}", g as f64 / 1000.0)) " kg" },
                                    None => "not weighed",
                                }
                            }
                        }
                        div {
                            span class="k" { "Size" }
                            span class="v" {
                                @if let (Some(l), Some(w), Some(h)) =
                                    (carton.length_mm, carton.width_mm, carton.height_mm) {
                                    (l) " × " (w) " × " (h) " mm"
                                } @else {
                                    "not measured"
                                }
                            }
                        }
                        div {
                            span class="k" { "Sealed" }
                            span class="v" { @if carton.sealed { "Sealed" } @else { "Open" } }
                        }
                    }
                }

                // The only amber in the interface, and it means the same thing
                // here as it does everywhere: two records disagree.
                @for w in &carton.warnings {
                    div class="finding" {
                        div class="kind" { "Finding" }
                        div { (w) }
                    }
                }
            },
        )
        .into_string(),
    ))
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------



struct CartonView {
    sequence: String,
    package_type: Option<String>,
    fulfilment_reference: Option<String>,
    order_reference: Option<String>,
    sealed: bool,
    gross_weight_g: Option<i64>,
    length_mm: Option<i32>,
    width_mm: Option<i32>,
    height_mm: Option<i32>,
    lines: Vec<PackedRow>,
    warnings: Vec<String>,
}

/// The same two folds `GET /packages/{id}/contents` returns, rendered.
///
/// Reading rather than calling the endpoint keeps this a page and that an API;
/// what matters is that neither writes, so there is one write path and this is
/// not it.
async fn carton_view(
    state: &web::Data<AppState>,
    who: &Caller,
    package_id: Uuid,
) -> Result<CartonView, ApiError> {
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let head = tx
                    .query_opt(
                        // **Measured first, stated second.** A preset says how
                        // big the box is and that is true of every one of them
                        // until somebody measures this one; a carton that came
                        // back crushed and was re-measured prints what the tape
                        // said, not what the catalogue says it should have been.
                        // Before this the sheet read "not measured" for a box
                        // whose size has been on file since migration 67.
                        "SELECT coalesce(p.sequence::text, '—'), pt.name,
                                p.sealed_at IS NOT NULL, p.gross_weight_g,
                                coalesce(p.length_mm, pt.length_mm),
                                coalesce(p.width_mm, pt.width_mm),
                                coalesce(p.height_mm, pt.height_mm),
                                f.reference
                           FROM package p
                           LEFT JOIN package_type pt ON pt.id = p.package_type_id
                           LEFT JOIN fulfilment f ON f.id = p.fulfilment_id
                          WHERE p.id = $1",
                        &[&package_id],
                    )
                    .await?
                    .ok_or(ApiError::NotFound)?;

                // Netted through `stock_movement_effective` (D103), so a pick
                // that was partly reversed prints what is left rather than the
                // sum of both movements.
                let rows = tx
                    .query(
                        "SELECT i.code, i.description, l.code,
                                sum(e.effective_quantity)::bigint,
                                o.confirmation_number
                           FROM stock_movement m
                           JOIN stock_movement_effective e ON e.movement_id = m.id
                           JOIN item i ON i.id = m.item_id
                           JOIN fulfilment_line fl ON fl.id = m.fulfilment_line_id
                           JOIN order_line ol ON ol.id = fl.order_line_id
                           JOIN \"order\" o ON o.id = ol.order_id
                           LEFT JOIN lot l ON l.id = m.to_lot_id
                          WHERE m.to_package_id = $1 AND m.fulfilment_line_id IS NOT NULL
                          GROUP BY i.code, i.description, l.code, o.confirmation_number
                         HAVING sum(e.effective_quantity) <> 0
                          ORDER BY i.code, l.code NULLS FIRST",
                        &[&package_id],
                    )
                    .await?;

                let cells: i64 = tx
                    .query_one(
                        "SELECT coalesce(sum(quantity), 0)::bigint FROM package_content
                          WHERE package_id = $1",
                        &[&package_id],
                    )
                    .await?
                    .get(0);

                let lines: Vec<PackedRow> = rows
                    .iter()
                    .map(|r| PackedRow {
                        item_code: r.get(0),
                        description: r.get(1),
                        lot_code: r.get(2),
                        quantity: r.get(3),
                        picks: vec![],
                    })
                    .collect();
                let from_lines: i64 = lines.iter().map(|l| l.quantity).sum();

                let mut warnings = vec![];
                if from_lines != cells {
                    warnings.push(format!(
                        "Picks account for {from_lines} units. Stock on hand holds {cells}."
                    ));
                }

                Ok(CartonView {
                    sequence: head.get(0),
                    package_type: head.get(1),
                    fulfilment_reference: head.get(7),
                    order_reference: rows.first().and_then(|r| r.get(4)),
                    sealed: head.get(2),
                    gross_weight_g: head.get(3),
                    length_mm: head.get(4),
                    width_mm: head.get(5),
                    height_mm: head.get(6),
                    lines,
                    warnings,
                })
            })
        })
        .await
}

/// The pages, registered beside the API rather than instead of it.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(stylesheet).service(packing_list_page);
}
