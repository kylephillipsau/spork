//! An item's own page: what it is, what it looks like, what it measures, and
//! where it is.
//!
//! **Where a scanned item lands.** D111's locator resolves an item code or
//! barcode to its id, and the question a person scans a product to ask is
//! *what is this and where does it live*. Until this, an item scan went to the
//! capture worklist or nowhere.
//!
//! # Two answers to "where", kept apart
//!
//! **Where this system holds it** is `stock`, the fold of its own ledger.
//! **Where NetSuite says it is** is `reported_stock`, a snapshot somebody
//! exported. Migration 86 is emphatic that the second is not the first, and a
//! page that merged them would be this system asserting it holds goods it never
//! recorded receiving. So they are two lists, the second carries its age, and
//! a screen says which is which.
//!
//! # Where an item's properties are seen and changed
//!
//! The page carries every subject that gets measured for the item (its carton,
//! its each, its family's carton, its parts) with what is known of each and
//! the newest photograph of each face, so a screen can show them and offer to
//! weigh, measure or photograph each one. It is the capture worklist's own
//! enumeration ([`crate::capture::subjects_for_item`]), so the item and the
//! to-do list cannot disagree about what there is to do.
//!
//! Nothing here writes: weighing, measuring and photographing are
//! `POST /weighings`, `POST /observations` and its images, as they were.

use actix_web::{get, web, HttpRequest, HttpResponse};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::capture::{self, CaptureSubject};
use crate::error::ApiError;
use crate::pictures::{self, Picture};
use crate::routes::{caller, measurements_of, ItemMeasurements};
use crate::tenancy::TenantScope;
use crate::AppState;

/// The style an item is a variant of (D108).
#[derive(Serialize, Debug)]
pub struct ItemStyleRef {
    pub code: String,
    pub description: Option<String>,
    /// Codes on file that are variants of it, this one included.
    pub variants: i64,
    /// The variant whose picture stands for the family (D188).
    pub picture_item_id: Option<Uuid>,
}

/// What a carton of it holds, as the case pack in force says.
///
/// Both counts are nullable because a carton can be known to exist before
/// anybody has said what is in it, which is what the prepack loader records
/// for a carton with no stated count.
#[derive(Serialize, Debug)]
pub struct ItemPacking {
    pub units_per_inner: Option<i32>,
    pub inners_per_carton: Option<i32>,
    pub effective_from: NaiveDate,
}

/// Some of it, on a shelf or at a warehouse, as this system's ledger holds it.
#[derive(Serialize, Debug)]
pub struct ItemHeld {
    pub site_code: Option<String>,
    /// The bin, when it is on one. Absent when it is inside a package.
    pub location_id: Option<Uuid>,
    pub bin_code: Option<String>,
    /// Whether that bin can be reached from the floor (D180); absent with no bin.
    pub within_reach: Option<bool>,
    pub quantity: i64,
    pub allocated_quantity: i64,
}

/// Some of it, as NetSuite's inventory balance reported it.
#[derive(Serialize, Debug)]
pub struct ItemReported {
    pub site_code: String,
    /// Absent when the report named a warehouse and no shelf.
    pub location_id: Option<Uuid>,
    pub bin_code: Option<String>,
    /// Whether that bin can be reached from the floor (D180); absent with no bin.
    pub within_reach: Option<bool>,
    /// Text, because `on_hand` is numeric and a quantity can be fractional.
    pub on_hand: String,
    pub available: Option<String>,
    pub status: Option<String>,
    /// When the report was taken, which is how old this is.
    pub as_at: DateTime<Utc>,
    /// Which feed it came from.
    pub source: String,
}

/// Which level of it is one in NetSuite (D218).
#[derive(Serialize, Debug)]
pub struct ItemUnit {
    /// `each`, `inner` or `carton`.
    pub level: String,
    /// Said in Spork, rather than taken from NetSuite's Pack Unit.
    pub said: bool,
    /// NetSuite's Pack Unit, as it names it: "CTN", "Box", "Pair".
    pub netsuite_unit: Option<String>,
}

/// Something said on the floor against NetSuite's bins, still open (D215).
#[derive(Serialize, Debug)]
pub struct ItemFlag {
    pub discrepancy_id: Uuid,
    /// `not_in_listed_bin` or `found_in_unlisted_bin`.
    pub kind: String,
    pub location_id: Option<Uuid>,
    pub bin_code: Option<String>,
    /// How many were found there, when they were counted.
    pub found: Option<String>,
    pub detected_at: DateTime<Utc>,
    pub detected_by: Option<String>,
}

/// One item, with everything a person needs to find it and know it.
#[derive(Serialize, Debug)]
pub struct ItemView {
    pub item_id: Uuid,
    pub code: String,
    pub description: String,
    pub active: bool,
    pub style: Option<ItemStyleRef>,
    /// The front, own before its style's, and saying whose (D141).
    pub picture: Option<Picture>,
    pub measurements: Vec<ItemMeasurements>,
    pub packing: Option<ItemPacking>,
    /// Which level of it is one in NetSuite: what it is sold as (D218).
    pub unit: ItemUnit,
    /// This system's own record, in walking order.
    pub held: Vec<ItemHeld>,
    /// What NetSuite last reported, in walking order.
    pub reported: Vec<ItemReported>,
    /// What has been said against those bins on the floor and not yet put
    /// right in NetSuite, newest first (D215).
    pub flags: Vec<ItemFlag>,
    /// What gets measured for it, each with what is known, in the order to
    /// offer them: its carton, its each, its family's carton, its parts.
    pub subjects: Vec<CaptureSubject>,
    /// Its newest box drawing (D186), and the cuts it was drawn from.
    pub box_picture: Option<BoxPicture>,
    /// The newest photograph of each face of each of those subjects. Their own
    /// only: a photograph is what one look at one box saw, and does not
    /// inherit (D132, D141).
    pub photos: Vec<SubjectPhoto>,
}

/// One face of one subject, as last photographed.
///
/// Named by the subject's own identity, the pair `POST /observations` takes,
/// so a screen can put it beside the subject it shows.
#[derive(Serialize, Debug)]
pub struct SubjectPhoto {
    pub item_id: Option<Uuid>,
    pub item_style_id: Option<Uuid>,
    pub item_part_id: Option<Uuid>,
    /// A run of the item that looks different (D182).
    pub lot_id: Option<Uuid>,
    pub packaging_level: Option<String>,
    pub face: String,
    /// The photograph, which a cut points at (D176).
    pub image_id: Uuid,
    /// `GET /images/{digest}` serves it.
    pub digest: String,
    pub captured_at: DateTime<Utc>,
    /// Its newest cut to the face, when somebody has marked one: for a side
    /// said to look like another, that other's.
    pub cut: Option<PhotoCut>,
    /// The side it was said to look like, rather than photographed (D183).
    pub same_as: Option<String>,
}

/// An item drawn as its box (D186).
#[derive(Serialize, Debug)]
pub struct BoxPicture {
    pub digest: String,
    /// The front, right and top cuts it was drawn from.
    pub made_from: Vec<String>,
}

/// A photograph cut to its face and straightened (D176).
#[derive(Serialize, Debug)]
pub struct PhotoCut {
    /// The straightened face. `GET /images/{digest}` serves it.
    pub digest: String,
    /// Top-left, top-right, bottom-right, bottom-left of the face, x then y,
    /// as fractions of the photograph shown the right way up.
    pub corners: Vec<f64>,
}

#[get("/items/{item_id}")]
pub async fn item_page(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let id = path.into_inner();
    let site = who.site_id;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let view = scope
        .run(|tx| {
            Box::pin(async move {
                let Some(r) = tx
                    .query_opt(
                        "SELECT i.code, i.description, i.active, s.code, s.description,
                                (SELECT count(*) FROM item v WHERE v.style_id = s.id),
                                s.picture_item_id
                           FROM item i
                           LEFT JOIN item_style s ON s.id = i.style_id
                          WHERE i.id = $1",
                        &[&id],
                    )
                    .await?
                else {
                    return Err(ApiError::NotFound);
                };
                let style = r.get::<_, Option<String>>(3).map(|code| ItemStyleRef {
                    code,
                    description: r.get(4),
                    variants: r.get(5),
                    picture_item_id: r.get(6),
                });

                let picture = pictures::of(tx, &[id]).await?.remove(&id);

                let measurements = measurements_of(tx, id).await?;

                // The newest case pack is the one in force, which is the rule
                // `receiving` reads it by.
                let packing = tx
                    .query_opt(
                        "SELECT units_per_inner, inners_per_carton, effective_from
                           FROM item_packing_config
                          WHERE item_id = $1 AND effective_from <= CURRENT_DATE
                          ORDER BY effective_from DESC, id DESC
                          LIMIT 1",
                        &[&id],
                    )
                    .await?
                    .map(|p| ItemPacking {
                        units_per_inner: p.get(0),
                        inners_per_carton: p.get(1),
                        effective_from: p.get(2),
                    });

                let held = tx
                    .query(
                        &format!(
                            "SELECT si.code, l.id, l.code, sum(st.quantity)::bigint,
                                    sum(st.allocated_quantity)::bigint,
                                    CASE WHEN l.id IS NOT NULL THEN {reach} END
                               FROM stock st
                               LEFT JOIN location l ON l.id = st.resolved_location_id
                               LEFT JOIN site si ON si.id = st.site_id
                              WHERE st.item_id = $1 AND st.quantity <> 0
                              GROUP BY si.code, l.id, l.code, l.pick_sequence
                              ORDER BY si.code, l.pick_sequence NULLS LAST, l.code",
                            reach = crate::places::within_reach("l")
                        ),
                        &[&id],
                    )
                    .await?
                    .iter()
                    .map(|h| ItemHeld {
                        site_code: h.get(0),
                        location_id: h.get(1),
                        bin_code: h.get(2),
                        within_reach: h.get(5),
                        quantity: h.get(3),
                        allocated_quantity: h.get(4),
                    })
                    .collect();

                let reported = tx
                    .query(
                        &format!(
                            "SELECT si.code, l.id, l.code, rs.on_hand::text, rs.available::text,
                                    rs.status, rs.as_at, rs.source,
                                    CASE WHEN l.id IS NOT NULL THEN {reach} END
                               FROM reported_stock rs
                               JOIN site si ON si.id = rs.site_id
                               LEFT JOIN location l ON l.id = rs.location_id
                              WHERE rs.item_id = $1
                              ORDER BY si.code, l.pick_sequence NULLS LAST, l.code NULLS LAST",
                            reach = crate::places::within_reach("l")
                        ),
                        &[&id],
                    )
                    .await?
                    .iter()
                    .map(|x| ItemReported {
                        site_code: x.get(0),
                        location_id: x.get(1),
                        bin_code: x.get(2),
                        within_reach: x.get(8),
                        on_hand: x.get(3),
                        available: x.get(4),
                        status: x.get(5),
                        as_at: x.get(6),
                        source: x.get(7),
                    })
                    .collect();

                let unit = tx
                    .query_opt(
                        "SELECT level::text, said, netsuite_unit FROM item_unit_level WHERE item_id = $1",
                        &[&id],
                    )
                    .await?
                    .map(|u| ItemUnit { level: u.get(0), said: u.get(1), netsuite_unit: u.get(2) })
                    .unwrap_or(ItemUnit { level: "each".into(), said: false, netsuite_unit: None });

                let flags = tx
                    .query(
                        "SELECT d.id, d.kind::text, l.id, l.code, d.observed_quantity::text, d.detected_at,
                                fb.display_name
                           FROM discrepancy d
                           LEFT JOIN location l ON l.id = d.holder_location_id
                           LEFT JOIN person fb ON fb.id = d.detected_by_id
                          WHERE d.item_id = $1 AND d.kind::text = ANY($2)
                            AND d.state IN ('open', 'investigating')
                          ORDER BY d.detected_at DESC",
                        &[&id, &crate::listed::KINDS.as_slice()],
                    )
                    .await?
                    .iter()
                    .map(|f| ItemFlag {
                        discrepancy_id: f.get(0),
                        kind: f.get(1),
                        location_id: f.get(2),
                        bin_code: f.get(3),
                        found: f.get(4),
                        detected_at: f.get(5),
                        detected_by: f.get(6),
                    })
                    .collect();

                let subjects = capture::subjects_for_item(tx, site, id).await?;

                let box_picture = tx
                    .query_opt(
                        "SELECT digest, made_from FROM box_picture WHERE item_id = $1
                          ORDER BY recorded_at DESC, id DESC LIMIT 1",
                        &[&id],
                    )
                    .await?
                    .map(|b| BoxPicture { digest: b.get(0), made_from: b.get(1) });

                let photos = tx
                    .query(
                        "SELECT DISTINCT ON (o.id, oi.face)
                                o.item_id, o.item_style_id, o.item_part_id,
                                o.packaging_level::text, oi.face, oi.digest, oi.captured_at,
                                oi.id, cut.digest, cut.corners, o.lot_id, src.face
                           FROM observable o
                           JOIN observation_event e ON e.observable_id = o.id
                           JOIN observation_image oi ON oi.observation_event_id = e.id
                           LEFT JOIN observation_image src ON src.id = oi.same_as_id
                           LEFT JOIN LATERAL (
                                SELECT c.digest, c.corners
                                  FROM observation_image_cut c
                                 WHERE c.observation_image_id = coalesce(oi.same_as_id, oi.id)
                                 ORDER BY c.recorded_at DESC, c.id DESC
                                 LIMIT 1
                           ) cut ON true
                          WHERE (o.item_id = $1
                             OR o.item_style_id = (SELECT style_id FROM item WHERE id = $1)
                             OR o.item_part_id IN (SELECT id FROM item_part WHERE item_id = $1)
                             OR o.lot_id IN (SELECT id FROM lot WHERE item_id = $1))
                            AND NOT EXISTS (SELECT 1 FROM observation_image_move mv WHERE mv.observation_image_id = oi.id)
                          ORDER BY o.id, oi.face, oi.captured_at DESC, oi.id DESC",
                        &[&id],
                    )
                    .await?
                    .iter()
                    .map(|x| SubjectPhoto {
                        item_id: x.get(0),
                        item_style_id: x.get(1),
                        item_part_id: x.get(2),
                        lot_id: x.get(10),
                        packaging_level: x.get(3),
                        face: x.get(4),
                        image_id: x.get(7),
                        digest: x.get(5),
                        captured_at: x.get(6),
                        cut: x.get::<_, Option<String>>(8).map(|digest| PhotoCut {
                            digest,
                            corners: x.get(9),
                        }),
                        same_as: x.get(11),
                    })
                    .collect();

                Ok(ItemView {
                    item_id: id,
                    code: r.get(0),
                    description: r.get(1),
                    active: r.get(2),
                    style,
                    picture,
                    measurements,
                    packing,
                    unit,
                    held,
                    reported,
                    flags,
                    subjects,
                    box_picture,
                    photos,
                })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(view))
}

// ---------------------------------------------------------------------------
// The list
// ---------------------------------------------------------------------------

/// The methods that make a figure a measurement, rather than one copied.
///
/// **A figure copied from a list is not a measurement**, which is the whole
/// distinction this column is for. The prepack list gives a thousand items a
/// carton size and weight, `transcribed`; nobody here put any of those cartons
/// on a scale. Somebody asking "what have we not measured yet" is asking about
/// the second kind, so an item with only listed figures still needs measuring.
const MEASURED_METHODS: &str = "('instrument', 'scan', 'keyed', 'derived', 'photographed')";

#[derive(serde::Deserialize, Debug)]
pub struct ItemsQuery {
    /// Code, description or barcode. A barcode is normalised the way a scan is.
    pub q: Option<String>,
    /// `here`: on hand at the caller's site, by either record.
    pub stock: Option<String>,
    /// `weighing`: no weight measured here, whatever a list says.
    /// `measuring`: no size measured here. `photo`: no picture of its front,
    /// its own or its family's. `packing`: still to pack on an open order, and
    /// its each has no size recorded, its own or its family's, nor is said to
    /// have none, nor ships as it is: what the pack bench's suggestion cannot
    /// place (D195, D197).
    pub needs: Option<String>,
    /// The other way round: `measured`, a weight or a size measured here (or
    /// said to have none); `photographed`, a picture of it, its own or its
    /// family's; `both`, the two together. What has been done, to look over.
    pub has: Option<String>,
    /// Only the items on this list (D179).
    pub list: Option<Uuid>,
    /// `code` (the default), `demand` (most ordered first), `packing` (on the
    /// most open orders still to pack first, D197), `walk` (in the order the
    /// bins are walked, by where most of it is) or `list` (as on the list asked
    /// for, which is the order on its paper).
    pub order: Option<String>,
    /// Where the page before ended, as its `next` said: the last code in code
    /// order, or how many came before in the other two.
    pub after: Option<String>,
    pub limit: Option<i64>,
}

/// How often it is ordered: the lines naming it. A join on `n`.
const DEMAND: &str = "LEFT JOIN LATERAL (
                          SELECT count(*)::bigint AS lines FROM order_line ol WHERE ol.item_id = n.id
                      ) dem ON true";

/// The bin to go to for it at the caller's site (`$4`): the biggest pile **in
/// reach of the floor** (D180), and only when no bin in reach holds any, the
/// biggest pile anywhere. Somebody sent to weigh one or pick one wants the
/// shelf they can get to without a forklift. This system's own ledger before
/// NetSuite's report, as everywhere. A join on `n`, answering `pile.code`,
/// `pile.pick_sequence` and `pile.reach`.
fn pile() -> String {
    format!(
        "LEFT JOIN LATERAL (
             SELECT l.code, l.pick_sequence, {reach} AS reach
               FROM (SELECT coalesce(s.holder_location_id, s.resolved_location_id)
                              AS location_id,
                            sum(s.quantity)::numeric AS qty, 0 AS rank
                       FROM stock s
                      WHERE s.item_id = n.id AND s.quantity > 0
                        AND ($4::uuid IS NULL OR s.site_id = $4)
                      GROUP BY 1
                     UNION ALL
                     SELECT rs.location_id, sum(rs.on_hand), 1
                       FROM reported_stock rs
                      WHERE rs.item_id = n.id AND rs.on_hand > 0
                        AND rs.location_id IS NOT NULL
                        AND ($4::uuid IS NULL OR rs.site_id = $4)
                      GROUP BY 1) piles
               JOIN location l ON l.id = piles.location_id
              ORDER BY {reach} DESC, piles.rank, piles.qty DESC, l.code
              LIMIT 1
         ) pile ON true",
        reach = crate::places::within_reach("l")
    )
}

/// Where an item is on the list asked for (`$11`): its place on the paper. A
/// join on `n`.
const LISTED: &str = "LEFT JOIN item_list_entry le ON le.item_list_id = $11 AND le.item_id = n.id";

/// What is still to pack of each item on an open order (D197): lines on a
/// fulfilment that is not cancelled nor closed in the other system, less what
/// is picked or in a carton, as the packing queue counts them. A `WITH` arm.
const TO_PACK: &str = "to_pack AS (
         SELECT ol.item_id, count(*)::bigint AS lines,
                sum(fl.quantity - greatest(fl.picked_quantity, fl.packed_quantity))::bigint AS units
           FROM fulfilment f
           JOIN fulfilment_line fl ON fl.fulfilment_id = f.id
           JOIN order_line ol ON ol.id = fl.order_line_id
          WHERE f.state <> 'cancelled'
            AND f.closed_elsewhere IS NULL
            AND fl.quantity > greatest(fl.picked_quantity, fl.packed_quantity)
          GROUP BY ol.item_id
     )";

/// Most needed at the pack bench first: on the most open orders, then the
/// most units. A join on `n`.
const PACKING: &str = "LEFT JOIN to_pack tp ON tp.item_id = n.id";

/// How the list is ordered, and so how it pages.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Order {
    Code,
    Demand,
    Packing,
    Walk,
    Listed,
}

/// One item in the list.
#[derive(Serialize, Debug)]
pub struct ItemRow {
    pub item_id: Uuid,
    pub code: String,
    pub description: String,
    pub active: bool,
    pub style_code: Option<String>,
    pub picture: Option<Picture>,
    /// Its weight: `measured` (or declared to have none), `listed` (copied
    /// from a list, never measured here) or `none`. Its own or its family's.
    pub weight: String,
    /// Its size, in the same words.
    pub size: String,
    /// Order lines naming it: how much it matters to get right.
    pub demand: i64,
    /// Units still to pack on open orders, and on how many lines (D197).
    pub to_pack: i64,
    pub to_pack_lines: i64,
    /// The bin to go to for it here: the one in reach of the floor holding
    /// the most of it, or the biggest pile when none in reach holds any
    /// (D180). This system's ledger first, then NetSuite's report.
    pub bin_code: Option<String>,
    /// Whether that bin can be reached from the floor; absent with no bin.
    pub bin_within_reach: Option<bool>,
    /// What NetSuite last reported on hand at this site, as text, when it
    /// reported any; and on how many shelves.
    pub reported_on_hand: Option<String>,
    pub reported_bins: i64,
    /// What this system's own ledger holds at this site.
    pub held: i64,
    /// Its place on the list asked for, from 1; absent when no list was.
    pub list_position: Option<i32>,
}

#[derive(Serialize, Debug)]
pub struct ItemsList {
    pub items: Vec<ItemRow>,
    /// How many match, across every page.
    pub total: i64,
    /// Pass as `after` for the next page; absent on the last.
    pub next: Option<String>,
}

/// Items, searched and filtered, fifty at a time: in code order, most ordered
/// first, or in walking order.
///
/// **The stock columns are this site's**, the site the session works at, and
/// every site's when it names none. NetSuite's report and this system's ledger
/// stay two columns for the item page's reason (migration 86).
#[get("/items")]
pub async fn item_list(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<ItemsQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let ask = ListAsk::read(&query, who.site_id)?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);

    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let (items, total, more) = list_rows(tx, &ask, limit).await?;
                let next = match (more, ask.order) {
                    (false, _) => None,
                    (true, Order::Code) => items.last().map(|i| i.code.clone()),
                    (true, _) => Some((ask.offset + limit).to_string()),
                };
                Ok(ItemsList { items, total, next })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}

/// The list asked for, read from its query once: what the page and the export
/// (D216) both filter and order by.
pub(crate) struct ListAsk {
    like: Option<String>,
    codes: Vec<String>,
    here: bool,
    site: Option<Uuid>,
    weighing: bool,
    measuring: bool,
    photo: bool,
    packing: bool,
    has_measured: bool,
    has_photo: bool,
    list: Option<Uuid>,
    order: Order,
    after_code: Option<String>,
    offset: i64,
}

impl ListAsk {
    pub(crate) fn read(query: &ItemsQuery, site: Option<Uuid>) -> Result<ListAsk, ApiError> {
        let asked = query
            .q
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(str::to_string);
        // The wildcards a person did not mean are dropped, not escaped: nobody
        // searches for a percent sign in a stock code.
        let like = asked.as_ref().map(|q| format!("%{}%", q.replace(['%', '_'], "")));
        let codes: Vec<String> = asked
            .iter()
            .flat_map(|q| [Some(q.clone()), crate::barcodes::normalise_gtin(q)])
            .flatten()
            .collect();
        let here = query.stock.as_deref() == Some("here");
        let (weighing, measuring, photo, packing) = match query.needs.as_deref() {
            Some("weighing") => (true, false, false, false),
            Some("measuring") => (false, true, false, false),
            Some("photo") => (false, false, true, false),
            Some("packing") => (false, false, false, true),
            Some(other) => {
                return Err(ApiError::Rejected(format!(
                    "needs is weighing, measuring, photo or packing, not {other}"
                )))
            }
            None => (false, false, false, false),
        };
        let (has_measured, has_photo) = match query.has.as_deref() {
            Some("measured") => (true, false),
            Some("photographed") => (false, true),
            Some("both") => (true, true),
            Some(other) => {
                return Err(ApiError::Rejected(format!(
                    "has is measured, photographed or both, not {other}"
                )))
            }
            None => (false, false),
        };
        let list = query.list;
        let order = match query.order.as_deref() {
            None | Some("code") => Order::Code,
            Some("demand") => Order::Demand,
            Some("packing") => Order::Packing,
            Some("walk") => Order::Walk,
            Some("list") if list.is_some() => Order::Listed,
            Some("list") => return Err(ApiError::Rejected("order=list needs a list".into())),
            Some(other) => {
                return Err(ApiError::Rejected(format!(
                    "order is code, demand, packing, walk or list, not {other}"
                )))
            }
        };
        let after = query.after.clone().filter(|a| !a.is_empty());
        // Code order pages by the last code, which stays right while rows are
        // added; the other two by position, because a count of order lines or a
        // bin's place in the walk is no key to start from.
        let (after_code, offset): (Option<String>, i64) = match order {
            Order::Code => (after, 0),
            _ => match after.as_deref().map(str::parse::<i64>) {
                None => (None, 0),
                Some(Ok(n)) if n >= 0 => (None, n),
                Some(_) => return Err(ApiError::Rejected("after is where the last page ended".into())),
            },
        };
        Ok(ListAsk {
            like,
            codes,
            here,
            site,
            weighing,
            measuring,
            photo,
            packing,
            has_measured,
            has_photo,
            list,
            order,
            after_code,
            offset,
        })
    }
}

/// The rows of the list asked for, at most `limit`, in its order; how many
/// match in all; and whether there are more than `limit`.
pub(crate) async fn list_rows(
    tx: &tokio_postgres::Transaction<'_>,
    ask: &ListAsk,
    limit: i64,
) -> Result<(Vec<ItemRow>, i64, bool), ApiError> {
    let ListAsk {
        like,
        codes,
        here,
        site,
        weighing,
        measuring,
        photo,
        packing,
        has_measured,
        has_photo,
        list,
        order,
        after_code,
        offset,
    } = ask;
    let order = *order;
    let pile = pile();
    let (sort_join, sort_by) = match order {
        Order::Code => ("", "n.code"),
        Order::Demand => (DEMAND, "dem.lines DESC, n.code"),
        Order::Packing => (PACKING, "tp.lines DESC NULLS LAST, tp.units DESC NULLS LAST, n.code"),
        Order::Walk => (pile.as_str(), "pile.pick_sequence NULLS LAST, pile.code NULLS LAST, n.code"),
        Order::Listed => (LISTED, "le.position, n.code"),
    };
    // Asked as yes or no: whether a measured figure exists.
    // **Measured means the unit is** (D219): an item sold by the carton
    // needs its carton weighed, not a single glove, and is measured when its
    // carton is. Needs, has, and the row's own words all say the same.
    let measured = |metrics: &str| {
        format!(
            "EXISTS (SELECT 1 FROM observable o
                       JOIN observation_current oc ON oc.observable_id = o.id
                       JOIN metric m ON m.id = oc.metric_id
                      WHERE (o.item_id = c.id
                             OR (c.style_id IS NOT NULL AND o.item_style_id = c.style_id))
                        AND m.code IN ({metrics})
                        AND o.packaging_level::text = c.unit
                        AND (oc.absent_reason IS NOT NULL
                             OR oc.method::text IN {MEASURED_METHODS}))"
        )
    };
    let weighed = measured("'gross_weight'");
    let sized = measured("'length', 'width', 'height'");
    // Its unit has all three lengths recorded, its own or its family's, or
    // is said to have none (D138): the pack bench can place what was
    // ordered, or knows to put it in loose (D197, D218).
    let unit_sized = "EXISTS (SELECT 1 FROM observable o
                       JOIN observation_current oc ON oc.observable_id = o.id
                       JOIN metric m ON m.id = oc.metric_id
                      WHERE (o.item_id = c.id
                             OR (c.style_id IS NOT NULL AND o.item_style_id = c.style_id))
                        AND o.packaging_level::text = c.unit
                        AND m.code IN ('length', 'width', 'height')
                        AND (oc.value_numeric IS NOT NULL OR oc.absent_reason IS NOT NULL)
                     HAVING count(DISTINCT m.code) = 3)";
    let sql = format!(
        "WITH {picture},
         {TO_PACK},
         -- Which level of each item is one in NetSuite (D218), read once.
         unit AS MATERIALIZED (
             SELECT item_id, level::text AS level FROM item_unit_level
         ),
         candidates AS (
             SELECT i.id, i.code, i.description, i.active, i.style_id,
                    coalesce(u.level, 'each') AS unit
               FROM item i
               LEFT JOIN unit u ON u.item_id = i.id
              WHERE ($1::text IS NULL OR i.code ILIKE $1 OR i.description ILIKE $1
                     OR EXISTS (SELECT 1 FROM item_barcode b
                                 WHERE b.item_id = i.id AND b.barcode = ANY($2::text[])))
                AND (NOT $3::bool
                     OR EXISTS (SELECT 1 FROM reported_stock rs
                                 WHERE rs.item_id = i.id AND rs.on_hand > 0
                                   AND ($4::uuid IS NULL OR rs.site_id = $4))
                     OR EXISTS (SELECT 1 FROM stock s
                                 WHERE s.item_id = i.id AND s.quantity > 0
                                   AND ($4::uuid IS NULL OR s.site_id = $4)))
                AND ($11::uuid IS NULL
                     OR EXISTS (SELECT 1 FROM item_list_entry e
                                 WHERE e.item_list_id = $11 AND e.item_id = i.id))
         ),
         needed AS (
             SELECT c.* FROM candidates c
              WHERE (NOT $5::bool OR NOT {weighed})
                AND (NOT $6::bool OR NOT {sized})
                AND (NOT $7::bool
                     OR NOT EXISTS (SELECT 1 FROM picture p WHERE p.item_id = c.id))
                AND (NOT $12::bool OR {weighed} OR {sized})
                AND (NOT $13::bool
                     OR EXISTS (SELECT 1 FROM picture p WHERE p.item_id = c.id))
                AND (NOT $14::bool
                     OR (EXISTS (SELECT 1 FROM to_pack tp WHERE tp.item_id = c.id)
                         AND NOT {unit_sized}
                         AND NOT (SELECT s.as_it_is
                                    FROM ships_as_is(c.id, NULL, NULL, NULL, c.unit::packaging_level) s)))
         ),
         page AS (
             SELECT n.*, row_number() OVER (ORDER BY {sort_by}) AS ordinal
               FROM needed n {sort_join}
              WHERE ($8::text IS NULL OR n.code > $8)
              ORDER BY {sort_by}
              LIMIT $9 OFFSET $10
         )
         SELECT n.id, n.code, n.description, n.active, st.code, fig.weight, fig.size,
                pic.digest, pic.source, rep.on_hand, rep.bins, held.q,
                (SELECT count(*) FROM needed), dem.lines, pile.code,
                (SELECT e.position FROM item_list_entry e
                  WHERE e.item_list_id = $11 AND e.item_id = n.id),
                pile.reach, coalesce(tpk.units, 0), coalesce(tpk.lines, 0)
           FROM page n
           LEFT JOIN to_pack tpk ON tpk.item_id = n.id
           LEFT JOIN item_style st ON st.id = n.style_id
           LEFT JOIN picture pic ON pic.item_id = n.id
           {DEMAND}
           {pile}
           -- Its weight and its size, its own or its family's:
           -- 2 measured (or said to have none), 1 only copied.
           LEFT JOIN LATERAL (
               SELECT max(CASE WHEN m.code = 'gross_weight' THEN grade END) AS weight,
                      max(CASE WHEN m.code IN ('length', 'width', 'height') THEN grade END)
                        AS size
                 FROM (SELECT oc.metric_id,
                              CASE WHEN oc.absent_reason IS NOT NULL
                                     OR oc.method::text IN {MEASURED_METHODS}
                                   THEN 2 ELSE 1 END AS grade
                         FROM observable o
                         JOIN observation_current oc ON oc.observable_id = o.id
                        WHERE (o.item_id = n.id
                               OR (n.style_id IS NOT NULL AND o.item_style_id = n.style_id))
                          -- The unit's, as what still needs doing is (D219).
                          AND o.packaging_level::text = n.unit) g
                 JOIN metric m ON m.id = g.metric_id
           ) fig ON true
           LEFT JOIN LATERAL (
               SELECT sum(rs.on_hand)::text AS on_hand,
                      count(DISTINCT rs.location_id) AS bins
                 FROM reported_stock rs
                WHERE rs.item_id = n.id AND ($4::uuid IS NULL OR rs.site_id = $4)
           ) rep ON true
           LEFT JOIN LATERAL (
               SELECT coalesce(sum(s.quantity), 0)::bigint AS q
                 FROM stock s
                WHERE s.item_id = n.id AND s.quantity > 0
                  AND ($4::uuid IS NULL OR s.site_id = $4)
           ) held ON true
          ORDER BY n.ordinal",
        picture = pictures::PICTURE_CTE
    );
    let rows = tx
        .query(
            &sql,
            &[
                &like, &codes, &here, &site, &weighing, &measuring, &photo,
                &after_code, &(limit + 1), &offset, &list, &has_measured, &has_photo, &packing,
            ],
        )
        .await?;
    let total: i64 = rows.first().map(|r| r.get(12)).unwrap_or(0);
    let more = rows.len() as i64 > limit;
    let said = |n: Option<i32>| {
        match n {
            Some(2) => "measured",
            Some(_) => "listed",
            None => "none",
        }
        .to_string()
    };
    let items: Vec<ItemRow> = rows
        .iter()
        .take(limit as usize)
        .map(|r| ItemRow {
            item_id: r.get(0),
            code: r.get(1),
            description: r.get(2),
            active: r.get(3),
            style_code: r.get(4),
            weight: said(r.get(5)),
            size: said(r.get(6)),
            picture: pictures::from_row(r.get(7), r.get(8)),
            reported_on_hand: r.get(9),
            reported_bins: r.get(10),
            held: r.get(11),
            demand: r.get(13),
            bin_code: r.get(14),
            list_position: r.get(15),
            bin_within_reach: r.get(16),
            to_pack: r.get(17),
            to_pack_lines: r.get(18),
        })
        .collect();
    Ok((items, total, more))
}
