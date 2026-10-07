//! The item list, exported (D216): a row per item with everything recorded of
//! it, as CSV or as a workbook with its picture in its row.
//!
//! **What the list shows, every row of it.** The export takes the list's own
//! question (`GET /items`'s search, filters and order, [`items::ListAsk`]) and
//! answers it whole instead of fifty at a time. Nothing asked is the whole
//! catalogue.
//!
//! **The item page's subjects, not a second idea of them.** Each item's each,
//! inner pack and carton are what its page shows, assembled by the same code
//! ([`capture::subjects_for_items`]): its family's carton when it has none of
//! its own, the variant standing for its carton (D184). Runs that look
//! different (D182) are on the item's page, not in a row.
//!
//! **Its picture** is its newest box drawing (D186), or where it has none (a
//! bucket, a bag, a box not yet cut) its front photo, its own or its family's.
//! A workbook holds it shrunk to the row; both formats link to it, a link that
//! opens in a browser signed in to Spork.

use std::collections::HashMap;

use actix_web::{get, http::header, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::capture::{self, CaptureSubject};
use crate::error::ApiError;
use crate::images;
use crate::items::{self, ItemsQuery};
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// An observable, by the subject arms it names and its level.
type ObservableKey = (Option<Uuid>, Option<Uuid>, Option<Uuid>, Option<String>);
/// Who recorded an observable's newest figure, and how it was arranged.
type Recorded = (Option<String>, Option<String>);
/// A level of a row, by its name: the each, the inner pack, the carton.
type LevelOf = fn(&ExportRow) -> Option<&ExportLevel>;

/// Every row: a catalogue is a few thousand, and an export is all of it.
const EVERY_ROW: i64 = 1_000_000;
/// How big a picture is in the workbook, on its longest side, in pixels.
const THUMBNAIL_PX: u32 = 160;

#[derive(Deserialize, Debug)]
pub struct ExportFormat {
    /// `csv`, `xlsx`, or `json` (the rows as data, which the tests read).
    pub format: Option<String>,
}

/// One level of an item as recorded: its each, inner pack or carton.
#[derive(Serialize, Debug, Default)]
pub struct ExportLevel {
    pub weight_g: Option<i64>,
    /// Said to have no weight (D138).
    pub weight_absent: bool,
    pub length_mm: Option<i64>,
    pub width_mm: Option<i64>,
    pub height_mm: Option<i64>,
    /// Said to have no size (D138).
    pub dimensions_absent: bool,
    /// A round thing's (D213).
    pub diameter_mm: Option<i64>,
    pub base_diameter_mm: Option<i64>,
    pub top_height_mm: Option<i64>,
    /// GS1's name for what it is packed in (D191).
    pub packed_in: Option<String>,
    pub ships_as_is: bool,
    pub upright: bool,
    /// How it was arranged when it was measured (D138).
    pub arranged: Option<String>,
    /// The faces photographed, in the order they are asked for.
    pub photos: Vec<String>,
    /// When its newest figure was recorded, in the site's time, how, and by whom.
    pub measured: Option<String>,
    pub method: Option<String>,
    pub by: Option<String>,
    /// Whose figures: `own`, `style` (its family's), `variant` or `mixed`.
    pub source: Option<String>,
}

/// Its picture, and what it is.
#[derive(Serialize, Debug)]
pub struct ExportPicture {
    pub digest: String,
    /// `drawing`, `photo`, or `family photo`.
    pub kind: String,
}

/// One item, with everything recorded of it.
#[derive(Serialize, Debug)]
pub struct ExportRow {
    pub item_id: Uuid,
    pub code: String,
    pub description: String,
    pub active: bool,
    pub family: Option<String>,
    /// What NetSuite says it is sold in, and its supplier's part number (D217).
    pub selling_unit: Option<String>,
    /// Which level is one of that (D218): `each`, `inner` or `carton`.
    pub unit_level: String,
    /// How many of the each one of that is, where it is counted in them: two
    /// for a pair, packed as one or not (D233).
    pub unit_singles: Option<i32>,
    pub supplier_part: Option<String>,
    /// The bin to go to for it here (D180), and what NetSuite has in it.
    pub bin: Option<String>,
    pub bin_on_hand: Option<String>,
    /// NetSuite's balance at this site: in all, and bin by bin.
    pub netsuite_on_hand: Option<String>,
    pub netsuite_bins: Option<String>,
    /// What Spork's own ledger holds here.
    pub spork_holds: i64,
    pub barcodes: Option<String>,
    /// What its carton holds (D178, D185): the whole carton, and in packs.
    pub carton_holds: Option<i64>,
    pub packs_per_carton: Option<i32>,
    pub each_per_pack: Option<i32>,
    pub each: Option<ExportLevel>,
    pub inner: Option<ExportLevel>,
    pub carton: Option<ExportLevel>,
    /// Its parts (D139), each in a few words.
    pub parts: Option<String>,
    /// Said on the floor against NetSuite's bins and still open (D215).
    pub open_flags: Option<String>,
    pub picture: Option<ExportPicture>,
}

/// The list as it is asked, every row of it, with all that is recorded.
#[get("/items/export")]
pub async fn export_items(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<ItemsQuery>,
    format: web::Query<ExportFormat>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let format = match format.format.as_deref() {
        None | Some("csv") => Kind::Csv,
        Some("xlsx") => Kind::Xlsx,
        Some("json") => Kind::Json,
        Some("pdf") => Kind::Pdf,
        Some(other) => {
            return Err(ApiError::Rejected(format!(
                "format is csv, xlsx, pdf or json, not {other}"
            )))
        }
    };
    let ask = items::ListAsk::read(&query, who.site_id)?;
    let site = who.site_id;

    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let (rows, context) = scope
        .run(move |tx| Box::pin(async move { rows_of(tx, &ask, site).await }))
        .await?;

    // Links open the picture where the API is served: `/api/images/…` behind
    // the server, `/images/…` where the routes are mounted bare.
    let base = {
        let info = req.connection_info();
        let api = req.path().strip_suffix("/items/export").unwrap_or("");
        format!("{}://{}{api}/images/", info.scheme(), info.host())
    };
    let link = |digest: &str| format!("{base}{digest}");
    let name = format!("spork-items-{}", context.today);
    match format {
        Kind::Json => Ok(HttpResponse::Ok().json(rows)),
        Kind::Csv => Ok(download(
            csv_of(&rows, &link)?,
            "text/csv; charset=utf-8",
            &format!("{name}.csv"),
        )),
        Kind::Pdf => Ok(download(
            crate::sheet::render(&sheet_of(&rows, &context)),
            "application/pdf",
            &format!(
                "{}_Weights_Dims_Capture_{}_{}.pdf",
                file_word(&context.tenant),
                file_word(&context.site),
                context.today
            ),
        )),
        Kind::Xlsx => {
            let pictures = thumbnails(&rows).await;
            Ok(download(
                workbook_of(&rows, &pictures, &link)?,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                &format!("{name}.xlsx"),
            ))
        }
    }
}

/// A name as a file's: its words joined by underscores, and nothing a file
/// system would refuse.
fn file_word(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// The capture sheet the list replaces, with what is recorded in its boxes
/// (D217): for the unit NetSuite says it is sold in, to one decimal.
fn sheet_of(rows: &[ExportRow], context: &Context) -> crate::sheet::Sheet {
    crate::sheet::Sheet {
        title: format!("WEIGHTS & DIMENSIONS CAPTURE — {}", context.site.to_uppercase()),
        subtitle: format!(
            "NetSuite data refreshed: {} · {} {} · measure the selling unit shown in Unit · centimetres and kilograms, one decimal",
            context.refreshed.as_deref().unwrap_or("not yet"),
            rows.len(),
            if rows.len() == 1 { "item" } else { "items" },
        ),
        rows: rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let level = selling_level(r);
                let one = |v: Option<i64>, absent: bool, per: f64| match (v, absent) {
                    (Some(v), _) => Some(format!("{:.1}", v as f64 / per)),
                    (None, true) => Some("none".into()),
                    _ => None,
                };
                crate::sheet::SheetRow {
                    seq: i + 1,
                    bin: r.bin.clone().unwrap_or_else(|| "-".into()),
                    code: r.code.clone(),
                    description: r.description.clone(),
                    supplier_part: r.supplier_part.clone().unwrap_or_else(|| "-".into()),
                    unit: r.selling_unit.clone().unwrap_or_else(|| "not set".into()),
                    // What NetSuite has in the bin named: the shelf the
                    // sheet sends somebody to. With no bin, all of it here.
                    soh: match (&r.bin, &r.bin_on_hand) {
                        (Some(_), Some(n)) => whole(n),
                        (Some(_), None) => "0".into(),
                        (None, _) => r.netsuite_on_hand.as_deref().map(whole).unwrap_or_else(|| "0".into()),
                    },
                    boxes: match level {
                        Some(l) => [
                            one(l.length_mm, l.dimensions_absent, 10.0),
                            one(l.width_mm, l.dimensions_absent, 10.0),
                            one(l.height_mm, l.dimensions_absent, 10.0),
                            one(l.weight_g, l.weight_absent, 1000.0),
                        ],
                        None => [None, None, None, None],
                    },
                }
            })
            .collect(),
    }
}

/// The level whose figures go in the boxes: the one NetSuite counts one of
/// (D218), the carton for a CTN, the pack for a box of 100, the each for the
/// rest, as said or as NetSuite's Pack Unit has it. A pair not packed as one
/// is two of the each (D233), which no card holds, so its boxes are empty.
fn selling_level(r: &ExportRow) -> Option<&ExportLevel> {
    match r.unit_level.as_str() {
        "carton" => r.carton.as_ref(),
        "inner" => r.inner.as_ref(),
        _ if r.unit_singles.is_some_and(|n| n > 1) => None,
        _ => r.each.as_ref(),
    }
}

/// A count as a person writes it: "19", not "19.000".
fn whole(n: &str) -> String {
    if n.contains('.') {
        n.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        n.to_string()
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Csv,
    Xlsx,
    Pdf,
    Json,
}

fn download(body: Vec<u8>, kind: &str, file: &str) -> HttpResponse {
    HttpResponse::Ok()
        .content_type(kind)
        .insert_header((
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{file}\""),
        ))
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .body(body)
}

/// The rows, read in one transaction; and today's date at the site, for the
/// file's name.
/// What a file is named and headed by, beside its rows.
struct Context {
    /// Today, at the site.
    today: String,
    tenant: String,
    /// The site's name, or "every site".
    site: String,
    /// When NetSuite's balance here was last loaded, in the site's own time
    /// and zone: "1 Oct 2026, 6:46 AM AEST".
    refreshed: Option<String>,
}

async fn rows_of(
    tx: &tokio_postgres::Transaction<'_>,
    ask: &items::ListAsk,
    site: Option<Uuid>,
) -> Result<(Vec<ExportRow>, Context), ApiError> {
    let (listed, _, _) = items::list_rows(tx, ask, EVERY_ROW).await?;
    let ids: Vec<Uuid> = listed.iter().map(|i| i.item_id).collect();
    let tz: String = match site {
        Some(s) => tx
            .query_opt("SELECT timezone FROM site WHERE id = $1", &[&s])
            .await?
            .map(|r| r.get(0))
            .unwrap_or_else(|| "UTC".into()),
        None => "UTC".into(),
    };
    let today: String = tx
        .query_one(
            "SELECT to_char(now() AT TIME ZONE $1, 'YYYY-MM-DD')",
            &[&tz],
        )
        .await?
        .get(0);

    let mut subjects = capture::subjects_for_items(tx, site, &ids).await?;
    // NetSuite's newest word on what each is sold in (D217).
    let details: HashMap<Uuid, (Option<String>, Option<String>)> = tx
        .query(
            "SELECT DISTINCT ON (item_id) item_id, selling_unit, supplier_part FROM reported_item
              WHERE item_id = ANY($1) ORDER BY item_id, as_at DESC",
            &[&ids],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), (r.get(1), r.get(2))))
        .collect();
    // Which level of each is one in NetSuite (D218), and how many single ones (D233).
    let units: HashMap<Uuid, (String, Option<i32>)> = tx
        .query("SELECT item_id, level::text, singles FROM item_unit_level WHERE item_id = ANY($1)", &[&ids])
        .await?
        .iter()
        .map(|r| (r.get(0), (r.get(1), r.get(2))))
        .collect();

    let by_item = |rows: Vec<tokio_postgres::Row>| -> HashMap<Uuid, String> {
        rows.iter().map(|r| (r.get(0), r.get(1))).collect()
    };
    let netsuite_bins = by_item(
        tx.query(
            "SELECT item_id, string_agg(code || ' (' || on_hand || ')', '; ' ORDER BY seq NULLS LAST, code)
               FROM (SELECT DISTINCT ON (rs.item_id, l.id)
                            rs.item_id, l.code, l.pick_sequence AS seq, rs.on_hand::text AS on_hand,
                            rs.on_hand AS n
                       FROM reported_stock rs JOIN location l ON l.id = rs.location_id
                      WHERE rs.item_id = ANY($1) AND ($2::uuid IS NULL OR rs.site_id = $2)
                      ORDER BY rs.item_id, l.id, rs.as_at DESC) newest
              WHERE n > 0
              GROUP BY item_id",
            &[&ids, &site],
        )
        .await?,
    );
    // What NetSuite has in each bin of each item here, newest word per bin.
    let in_bins: HashMap<(Uuid, String), String> = tx
        .query(
            "SELECT DISTINCT ON (rs.item_id, l.id) rs.item_id, l.code, rs.on_hand::text
               FROM reported_stock rs JOIN location l ON l.id = rs.location_id
              WHERE rs.item_id = ANY($1) AND ($2::uuid IS NULL OR rs.site_id = $2)
              ORDER BY rs.item_id, l.id, rs.as_at DESC",
            &[&ids, &site],
        )
        .await?
        .iter()
        .map(|r| ((r.get(0), r.get(1)), r.get(2)))
        .collect();
    let barcodes = by_item(
        tx.query(
            "SELECT item_id, string_agg(barcode || coalesce(' (' || packaging_level::text || ')', ''), '; '
                                        ORDER BY packaging_level NULLS FIRST, barcode)
               FROM item_barcode
              WHERE item_id = ANY($1) AND effective @> current_date
              GROUP BY item_id",
            &[&ids],
        )
        .await?,
    );
    let flags = by_item(
        tx.query(
            "SELECT d.item_id,
                    string_agg(CASE d.kind::text WHEN 'not_in_listed_bin' THEN 'Not in ' ELSE 'Found in ' END
                               || l.code, '; ' ORDER BY d.detected_at)
               FROM discrepancy d JOIN location l ON l.id = d.holder_location_id
              WHERE d.item_id = ANY($1) AND d.kind::text = ANY($2)
                AND d.state IN ('open', 'investigating')
              GROUP BY d.item_id",
            &[&ids, &crate::listed::KINDS.as_slice()],
        )
        .await?,
    );
    let drawings = by_item(
        tx.query(
            "SELECT DISTINCT ON (item_id) item_id, digest FROM box_picture
              WHERE item_id = ANY($1) ORDER BY item_id, recorded_at DESC, id DESC",
            &[&ids],
        )
        .await?,
    );
    let packing: HashMap<Uuid, (Option<i32>, Option<i32>)> = tx
        .query(
            "SELECT DISTINCT ON (item_id) item_id, units_per_inner, inners_per_carton
               FROM item_packing_config
              WHERE item_id = ANY($1) AND effective_from <= current_date
              ORDER BY item_id, effective_from DESC, id DESC",
            &[&ids],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), (r.get(1), r.get(2))))
        .collect();
    let family_of: HashMap<Uuid, Uuid> = tx
        .query(
            "SELECT id, style_id FROM item WHERE id = ANY($1) AND style_id IS NOT NULL",
            &[&ids],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    let packaging_names: HashMap<String, String> = tx
        .query("SELECT code, name FROM packaging_type", &[])
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    // Who recorded each observable's newest figure, and how it was arranged.
    let recorded: HashMap<ObservableKey, Recorded> =
        tx.query(
            "SELECT o.item_id, o.item_style_id, o.lot_id, o.packaging_level::text,
                    (array_agg(p.display_name ORDER BY oe.observed_at DESC))[1],
                    (array_agg(pr.label ORDER BY oe.observed_at DESC) FILTER (WHERE pr.id IS NOT NULL))[1]
               FROM observable o
               JOIN observation_current oc ON oc.observable_id = o.id
               JOIN observation ob ON ob.id = oc.observation_id
               JOIN observation_event oe ON oe.id = ob.observation_event_id
               LEFT JOIN person p ON p.id = oe.recorded_by_id
               LEFT JOIN presentation pr ON pr.id = oe.presentation_id
              WHERE oc.absent_reason IS NULL
                AND (o.item_id = ANY($1) OR o.item_style_id = ANY($2) OR o.lot_id IS NOT NULL)
              GROUP BY 1, 2, 3, 4",
            &[&ids, &family_of.values().copied().collect::<Vec<_>>()],
        )
        .await?
        .iter()
        .map(|r| ((r.get(0), r.get(1), r.get(2), r.get(3)), (r.get(4), r.get(5))))
        .collect();

    // Every time shown, in the site's own time, in one go.
    let mut times: Vec<DateTime<Utc>> = vec![];
    for found in subjects.values() {
        times.extend(found.iter().filter_map(|s| s.observed_at));
    }
    let local: HashMap<DateTime<Utc>, String> = tx
        .query(
            "SELECT t, to_char(t AT TIME ZONE $2, 'YYYY-MM-DD HH24:MI') FROM unnest($1::timestamptz[]) t",
            &[&times, &tz],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();

    let level = |item: Uuid, s: &CaptureSubject| -> ExportLevel {
        // Whose figures they are says whose look recorded them.
        let key = match s.source.as_deref() {
            Some("style") => (
                None,
                family_of.get(&item).copied(),
                None,
                s.packaging_level.clone(),
            ),
            Some("variant") => (None, None, s.variant_lot_id, None),
            _ => (
                s.item_id,
                s.item_style_id,
                s.lot_id,
                s.packaging_level.clone(),
            ),
        };
        let (by, arranged) = recorded.get(&key).cloned().unwrap_or((None, None));
        ExportLevel {
            weight_g: s.gross_weight_g,
            weight_absent: s.weight_absent,
            length_mm: s.length_mm,
            width_mm: s.width_mm,
            height_mm: s.height_mm,
            dimensions_absent: s.dimensions_absent,
            diameter_mm: s.diameter_mm,
            base_diameter_mm: s.base_diameter_mm,
            top_height_mm: s.top_height_mm,
            packed_in: s
                .packed_in
                .as_ref()
                .map(|c| packaging_names.get(c).cloned().unwrap_or_else(|| c.clone())),
            ships_as_is: s.ships_as_is,
            upright: s.upright,
            arranged,
            photos: s.faces.clone(),
            measured: s.observed_at.and_then(|t| local.get(&t).cloned()),
            method: s.method.clone(),
            by,
            source: s.source.clone(),
        }
    };

    let rows = listed
        .into_iter()
        .map(|i| {
            let found = subjects.remove(&i.item_id).unwrap_or_default();
            let at = |lv: &str| {
                found
                    .iter()
                    .find(|s| {
                        s.item_id == Some(i.item_id)
                            && s.lot_id.is_none()
                            && s.packaging_level.as_deref() == Some(lv)
                    })
                    .map(|s| level(i.item_id, s))
            };
            let parts: Vec<String> = found
                .iter()
                .filter(|s| s.item_part_id.is_some())
                .map(part_in_words)
                .collect();
            let (per, packs) = packing.get(&i.item_id).copied().unwrap_or((None, None));
            let picture = match (drawings.get(&i.item_id), &i.picture) {
                (Some(d), _) => Some(ExportPicture {
                    digest: d.clone(),
                    kind: "drawing".into(),
                }),
                (None, Some(p)) => Some(ExportPicture {
                    digest: p.digest.clone(),
                    kind: if p.source == "style" {
                        "family photo"
                    } else {
                        "photo"
                    }
                    .into(),
                }),
                (None, None) => None,
            };
            let (selling_unit, supplier_part) =
                details.get(&i.item_id).cloned().unwrap_or((None, None));
            ExportRow {
                item_id: i.item_id,
                selling_unit,
                unit_level: units.get(&i.item_id).map_or_else(|| "each".into(), |u| u.0.clone()),
                unit_singles: units.get(&i.item_id).map_or(Some(1), |u| u.1),
                supplier_part,
                each: at("each"),
                inner: at("inner"),
                carton: at("carton"),
                parts: (!parts.is_empty()).then(|| parts.join("; ")),
                netsuite_bins: netsuite_bins.get(&i.item_id).cloned(),
                barcodes: barcodes.get(&i.item_id).cloned(),
                open_flags: flags.get(&i.item_id).cloned(),
                carton_holds: packs.map(|n| n as i64 * per.unwrap_or(1) as i64),
                packs_per_carton: packs.filter(|_| per.unwrap_or(1) > 1),
                each_per_pack: per.filter(|&p| p > 1),
                picture,
                code: i.code,
                description: i.description,
                active: i.active,
                family: i.style_code,
                bin_on_hand: i
                    .bin_code
                    .as_ref()
                    .and_then(|b| in_bins.get(&(i.item_id, b.clone())).cloned()),
                bin: i.bin_code,
                netsuite_on_hand: i.reported_on_hand,
                spork_holds: i.held,
            }
        })
        .collect();

    let tenant: String = tx
        .query_one("SELECT name FROM tenant WHERE id = current_tenant()", &[])
        .await?
        .get(0);
    let site_name: String = match site {
        Some(s) => tx
            .query_opt("SELECT name FROM site WHERE id = $1", &[&s])
            .await?
            .map(|r| r.get(0)),
        None => None,
    }
    .unwrap_or_else(|| "every site".into());
    // Last, because it sets the transaction's zone to name it ("AEST").
    tx.execute("SELECT set_config('timezone', $1, true)", &[&tz])
        .await?;
    let refreshed: Option<String> = tx
        .query_one(
            "SELECT to_char(max(as_at), 'FMDD Mon YYYY, FMHH12:MI AM TZ') FROM reported_stock
              WHERE $1::uuid IS NULL OR site_id = $1",
            &[&site],
        )
        .await?
        .get(0);
    Ok((
        rows,
        Context {
            today,
            tenant,
            site: site_name,
            refreshed,
        },
    ))
}

/// A part in a few words: "Handle: 0.600 kg, 120.0 × 6.0 × 4.0 cm".
fn part_in_words(s: &CaptureSubject) -> String {
    let mut said = vec![];
    if let Some(g) = s.gross_weight_g {
        said.push(format!("{} kg", kg(g)));
    }
    if let (Some(l), Some(w), Some(h)) = (s.length_mm, s.width_mm, s.height_mm) {
        said.push(format!("{} × {} × {} cm", cm(l), cm(w), cm(h)));
    } else if s.dimensions_absent {
        said.push("no size".into());
    }
    let name = s.part_label.clone().unwrap_or_else(|| "Part".into());
    if said.is_empty() {
        format!("{name}: not measured")
    } else {
        format!("{name}: {}", said.join(", "))
    }
}

fn kg(g: i64) -> String {
    format!("{:.3}", g as f64 / 1000.0)
}

fn cm(mm: i64) -> String {
    format!("{:.1}", mm as f64 / 10.0)
}

// ---------------------------------------------------------------------------
// The columns, once, for both formats
// ---------------------------------------------------------------------------

/// What one cell holds.
enum Cell {
    Empty,
    Text(String),
    /// A figure, and how many places it is shown to.
    Number(f64, usize),
    Whole(i64),
    /// A link, and what it reads as.
    Link(String, String),
}

type Column<'a> = (String, Box<dyn Fn(&ExportRow) -> Cell + 'a>);

fn text(v: Option<&String>) -> Cell {
    v.map(|t| Cell::Text(t.clone())).unwrap_or(Cell::Empty)
}

fn yes(v: bool) -> Cell {
    if v {
        Cell::Text("yes".into())
    } else {
        Cell::Empty
    }
}

/// A weight in kilograms, or "none" where it was said to have none.
fn weight(l: &ExportLevel) -> Cell {
    match (l.weight_g, l.weight_absent) {
        (Some(g), _) => Cell::Number(g as f64 / 1000.0, 3),
        (None, true) => Cell::Text("none".into()),
        _ => Cell::Empty,
    }
}

/// A length in centimetres, or "none" where it was said to have no size.
fn length(v: Option<i64>, absent: bool) -> Cell {
    match (v, absent) {
        (Some(mm), _) => Cell::Number(mm as f64 / 10.0, 1),
        (None, true) => Cell::Text("none".into()),
        _ => Cell::Empty,
    }
}

const METHODS: [(&str, &str); 5] = [
    ("instrument", "Measured"),
    ("estimated", "Estimated"),
    ("transcribed", "Copied from a list"),
    ("asserted", "Stated"),
    ("photographed", "From a photo"),
];

const SOURCES: [(&str, &str); 4] = [
    ("own", "Its own"),
    ("style", "Its family's"),
    ("variant", "Its variant's"),
    ("mixed", "Partly its family's"),
];

fn words(table: &[(&str, &str)], v: Option<&String>) -> Cell {
    match v {
        Some(v) => Cell::Text(
            table
                .iter()
                .find(|(k, _)| k == v)
                .map(|(_, w)| w.to_string())
                .unwrap_or(v.clone()),
        ),
        None => Cell::Empty,
    }
}

fn columns<'a>(link: &'a dyn Fn(&str) -> String) -> Vec<Column<'a>> {
    let mut c: Vec<Column<'a>> = vec![
        ("Code".into(), Box::new(|r| Cell::Text(r.code.clone()))),
        (
            "Description".into(),
            Box::new(|r| Cell::Text(r.description.clone())),
        ),
        ("Unit".into(), Box::new(|r| text(r.selling_unit.as_ref()))),
        (
            "Supplier Part No.".into(),
            Box::new(|r| text(r.supplier_part.as_ref())),
        ),
        (
            "Active".into(),
            Box::new(|r| Cell::Text(if r.active { "yes" } else { "no" }.into())),
        ),
        ("Family".into(), Box::new(|r| text(r.family.as_ref()))),
        ("Bin".into(), Box::new(|r| text(r.bin.as_ref()))),
        (
            "Bin on hand".into(),
            Box::new(|r| {
                r.bin_on_hand
                    .as_ref()
                    .and_then(|n| n.parse::<f64>().ok())
                    .map(|n| Cell::Number(n, 0))
                    .unwrap_or(Cell::Empty)
            }),
        ),
        (
            "NetSuite on hand".into(),
            Box::new(|r| {
                r.netsuite_on_hand
                    .as_ref()
                    .and_then(|n| n.parse::<f64>().ok())
                    .map(|n| Cell::Number(n, 0))
                    .unwrap_or(Cell::Empty)
            }),
        ),
        (
            "NetSuite bins".into(),
            Box::new(|r| text(r.netsuite_bins.as_ref())),
        ),
        (
            "Spork holds".into(),
            Box::new(|r| {
                if r.spork_holds == 0 {
                    Cell::Empty
                } else {
                    Cell::Whole(r.spork_holds)
                }
            }),
        ),
        ("Barcodes".into(), Box::new(|r| text(r.barcodes.as_ref()))),
        (
            "Carton holds (each)".into(),
            Box::new(|r| r.carton_holds.map(Cell::Whole).unwrap_or(Cell::Empty)),
        ),
        (
            "Packs per carton".into(),
            Box::new(|r| {
                r.packs_per_carton
                    .map(|n| Cell::Whole(n as i64))
                    .unwrap_or(Cell::Empty)
            }),
        ),
        (
            "Each per pack".into(),
            Box::new(|r| {
                r.each_per_pack
                    .map(|n| Cell::Whole(n as i64))
                    .unwrap_or(Cell::Empty)
            }),
        ),
    ];
    let levels: [(&str, LevelOf); 3] = [
        ("Each", |r| r.each.as_ref()),
        ("Inner pack", |r| r.inner.as_ref()),
        ("Carton", |r| r.carton.as_ref()),
    ];
    for (name, of) in levels {
        let at = move |f: fn(&ExportLevel) -> Cell| -> Box<dyn Fn(&ExportRow) -> Cell + 'a> {
            Box::new(move |r: &ExportRow| of(r).map(f).unwrap_or(Cell::Empty))
        };
        c.push((format!("{name} weight (kg)"), at(weight)));
        c.push((
            format!("{name} length (cm)"),
            at(|l| length(l.length_mm, l.dimensions_absent)),
        ));
        c.push((
            format!("{name} width (cm)"),
            at(|l| length(l.width_mm, l.dimensions_absent)),
        ));
        c.push((
            format!("{name} height (cm)"),
            at(|l| length(l.height_mm, l.dimensions_absent)),
        ));
        c.push((
            format!("{name} across the top (cm)"),
            at(|l| length(l.diameter_mm, false)),
        ));
        c.push((
            format!("{name} across the base (cm)"),
            at(|l| length(l.base_diameter_mm, false)),
        ));
        c.push((
            format!("{name} top part's height (cm)"),
            at(|l| length(l.top_height_mm, false)),
        ));
        c.push((
            format!("{name} packed in"),
            at(|l| text(l.packed_in.as_ref())),
        ));
        c.push((format!("{name} ships as it is"), at(|l| yes(l.ships_as_is))));
        c.push((format!("{name} keep this way up"), at(|l| yes(l.upright))));
        c.push((
            format!("{name} arranged"),
            at(|l| text(l.arranged.as_ref())),
        ));
        c.push((
            format!("{name} photos"),
            at(|l| {
                if l.photos.is_empty() {
                    Cell::Empty
                } else {
                    Cell::Text(l.photos.join(", "))
                }
            }),
        ));
        c.push((
            format!("{name} recorded"),
            at(|l| text(l.measured.as_ref())),
        ));
        c.push((
            format!("{name} how"),
            at(|l| words(&METHODS, l.method.as_ref())),
        ));
        c.push((format!("{name} by"), at(|l| text(l.by.as_ref()))));
        c.push((
            format!("{name} figures"),
            at(|l| words(&SOURCES, l.source.as_ref())),
        ));
    }
    c.push(("Parts".into(), Box::new(|r| text(r.parts.as_ref()))));
    c.push((
        "Open bin flags".into(),
        Box::new(|r| text(r.open_flags.as_ref())),
    ));
    c.push((
        "Picture is".into(),
        Box::new(|r| {
            r.picture
                .as_ref()
                .map(|p| Cell::Text(p.kind.clone()))
                .unwrap_or(Cell::Empty)
        }),
    ));
    c.push((
        "Picture (opens signed in to Spork)".into(),
        Box::new(move |r| {
            r.picture
                .as_ref()
                .map(|p| Cell::Link(link(&p.digest), "Open".into()))
                .unwrap_or(Cell::Empty)
        }),
    ));
    c
}

// ---------------------------------------------------------------------------
// CSV
// ---------------------------------------------------------------------------

/// UTF-8 with its byte-order mark, so a spreadsheet opening it reads `×` and
/// `’` as themselves.
fn csv_of(rows: &[ExportRow], link: &dyn Fn(&str) -> String) -> Result<Vec<u8>, ApiError> {
    let cols = columns(link);
    let mut out = csv::Writer::from_writer(b"\xEF\xBB\xBF".to_vec());
    let fail = |e: csv::Error| ApiError::Rejected(format!("could not write the CSV: {e}"));
    out.write_record(cols.iter().map(|(h, _)| h.as_str()))
        .map_err(fail)?;
    for row in rows {
        out.write_record(cols.iter().map(|(_, f)| match f(row) {
            Cell::Empty => String::new(),
            Cell::Text(t) => t,
            Cell::Number(n, places) => format!("{n:.places$}"),
            Cell::Whole(n) => n.to_string(),
            Cell::Link(url, _) => url,
        }))
        .map_err(fail)?;
    }
    out.into_inner()
        .map_err(|e| ApiError::Rejected(format!("could not write the CSV: {e}")))
}

// ---------------------------------------------------------------------------
// The workbook
// ---------------------------------------------------------------------------

/// Each picture, shrunk to the row and written as a PNG: drawings and cuts are
/// WebP, which a workbook can't hold. One that can't be read is left out.
async fn thumbnails(rows: &[ExportRow]) -> HashMap<String, Vec<u8>> {
    let root = images::directory();
    let mut raw: Vec<(String, Vec<u8>)> = vec![];
    for p in rows.iter().filter_map(|r| r.picture.as_ref()) {
        if raw.iter().any(|(d, _)| d == &p.digest) {
            continue;
        }
        if let Ok(Some(bytes)) = images::get(&root, &p.digest).await {
            raw.push((p.digest.clone(), bytes));
        }
    }
    tokio::task::spawn_blocking(move || {
        raw.into_iter()
            .filter_map(|(digest, bytes)| {
                let picture = image::load_from_memory(&bytes)
                    .ok()?
                    .thumbnail(THUMBNAIL_PX, THUMBNAIL_PX);
                let mut png = std::io::Cursor::new(vec![]);
                picture.write_to(&mut png, image::ImageFormat::Png).ok()?;
                Some((digest, png.into_inner()))
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// The rows as a workbook: a header row that stays put, a filter on it, and
/// each item's picture in the first column of its row.
fn workbook_of(
    rows: &[ExportRow],
    pictures: &HashMap<String, Vec<u8>>,
    link: &dyn Fn(&str) -> String,
) -> Result<Vec<u8>, ApiError> {
    use rust_xlsxwriter::{Format, Image, Url, Workbook};
    let fail = |e: rust_xlsxwriter::XlsxError| {
        ApiError::Rejected(format!("could not write the workbook: {e}"))
    };
    let cols = columns(link);
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.set_name("Items").map_err(fail)?;
    let bold = Format::new().set_bold().set_text_wrap();
    let whole = Format::new().set_num_format("0");
    let tenths = Format::new().set_num_format("0.0");
    let thousandths = Format::new().set_num_format("0.000");
    let shown = |places: usize| match places {
        0 => &whole,
        1 => &tenths,
        _ => &thousandths,
    };
    let middle = Format::new().set_align(rust_xlsxwriter::FormatAlign::VerticalCenter);

    // The picture first, then every column.
    sheet
        .write_string_with_format(0, 0, "Picture", &bold)
        .map_err(fail)?;
    sheet.set_column_width(0, 24).map_err(fail)?;
    for (at, (head, _)) in cols.iter().enumerate() {
        let col = at as u16 + 1;
        sheet
            .write_string_with_format(0, col, head, &bold)
            .map_err(fail)?;
        sheet
            .set_column_width(col, if head == "Description" { 36 } else { 14 })
            .map_err(fail)?;
    }
    sheet.set_row_height(0, 45).map_err(fail)?;
    for (n, row) in rows.iter().enumerate() {
        let r = n as u32 + 1;
        if let Some(png) = row.picture.as_ref().and_then(|p| pictures.get(&p.digest)) {
            sheet.set_row_height(r, 90).map_err(fail)?;
            let picture = Image::new_from_buffer(png).map_err(fail)?;
            sheet
                .insert_image_fit_to_cell(r, 0, &picture, true)
                .map_err(fail)?;
        }
        for (at, (_, cell)) in cols.iter().enumerate() {
            let col = at as u16 + 1;
            match cell(row) {
                Cell::Empty => {}
                Cell::Text(t) => {
                    sheet
                        .write_string_with_format(r, col, t, &middle)
                        .map_err(fail)?;
                }
                Cell::Number(v, p) => {
                    sheet
                        .write_number_with_format(r, col, v, shown(p))
                        .map_err(fail)?;
                }
                Cell::Whole(v) => {
                    sheet
                        .write_number_with_format(r, col, v as f64, &whole)
                        .map_err(fail)?;
                }
                Cell::Link(url, said) => {
                    sheet
                        .write_url_with_text(r, col, Url::new(url), said)
                        .map_err(fail)?;
                }
            }
        }
    }
    let last_col = cols.len() as u16;
    sheet.set_freeze_panes(1, 2).map_err(fail)?;
    sheet
        .autofilter(0, 0, rows.len() as u32, last_col)
        .map_err(fail)?;
    book.save_to_buffer().map_err(fail)
}
