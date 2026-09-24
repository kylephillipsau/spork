//! Read endpoints, and the first write path that records a pick as a ledger row.
//!
//! Note what is missing from every stock and fulfilment query: `WHERE tenant_id`.
//! It does not need one, and adding one would be worse than redundant. Row level
//! security applies the predicate, so a handler that forgets it returns nothing
//! rather than everything. The database is the boundary; the query is just a
//! query.
//!
//! The pick endpoint INSERTs `stock_movement` and never UPDATEs progress columns.
//! Progress is a fold; the app does not write it.

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    adjusting::{self, Direction, ProposedAdjustment},
    auth,
    allocating::{self, ProposedAllocation, ProposedRelease},
    client_events::{self, ActInsert, NewClientEvent},
    correction::{self, ProposedCorrection, RevisionClass, Side},
    counting::{self, ProposedCount},
    despatching::{self, ProposedDespatch, ProposedOpen, ProposedSeal},
    error::ApiError,
    ledger_views::{self, Progress, ProjectionProgress},
    moving::{self, ProposedMove},
    observing,
    packages::{self, ProposedContain, ProposedPackage, ProposedPlace},
    passkeys,
    revalidation,
    receiving::{self, Arrival, Claim},
    tenancy::TenantScope,
    AppState,
};
use webauthn_rs::prelude::{
    CredentialID, DiscoverableAuthentication, DiscoverableKey, Passkey, PasskeyAuthentication,
    PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential,
};

/// The tenant header, which is a placeholder for authentication until one exists.
///
/// Whatever eventually establishes identity hands this a tenant and nothing else
/// in these handlers changes.
// `tenant_from` used to read `x-tenant-id` here and is deliberately gone.
//
// **The header was the whole tenancy boundary and nothing verified it.** Every
// guard underneath was exact about a value any caller could type: `SET LOCAL`
// per transaction, RLS forced, `spork_app` refused a bypass role at startup,
// S51 and J64 keeping references inside a tenant. Deleting the function rather
// than leaving it unused is the point — a helper that reads a tenant from a
// header is one somebody reaches for again. The tenant comes from the session
// now, which is the only thing that has been checked.


// ---------------------------------------------------------------------------
// Stock on hand
// ---------------------------------------------------------------------------

#[derive(Serialize, Debug, PartialEq)]
pub struct StockRow {
    /// **The cell, so a screen can act on the row it just read.** Absent until
    /// the HTTP walk tried to allocate from a cell this endpoint had returned
    /// and found it had handed back a description rather than a thing. Every
    /// write path that names stock takes this id.
    pub stock_id: Uuid,
    /// The bin or carton, for the same reason.
    pub holder_location_id: Option<Uuid>,
    pub holder_package_id: Option<Uuid>,
    pub item_code: String,
    /// Resolved location (package placement folded in for carton-held cells).
    pub location_code: Option<String>,
    /// Set when the cell is held in a package rather than a bin.
    pub package_barcode: Option<String>,
    pub quantity: i64,
    pub available: i64,
}

/// Stock on hand for the requesting tenant, including cells held in packages.
#[get("/stock")]
pub async fn stock_on_hand(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let rows = scope
        .run(|tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        "SELECT i.code,
                                l.code,
                                p.barcode,
                                s.quantity,
                                s.available_quantity,
                                s.id,
                                s.holder_location_id,
                                s.holder_package_id
                           FROM stock s
                           JOIN item i ON i.id = s.item_id
                           LEFT JOIN location l ON l.id = s.resolved_location_id
                           LEFT JOIN package p ON p.id = s.holder_package_id
                          WHERE s.quantity <> 0
                          ORDER BY i.code, l.code NULLS LAST, p.barcode NULLS LAST",
                        &[],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| StockRow {
                        item_code: r.get(0),
                        location_code: r.get(1),
                        package_barcode: r.get(2),
                        quantity: r.get(3),
                        available: r.get(4),
                        stock_id: r.get(5),
                        holder_location_id: r.get(6),
                        holder_package_id: r.get(7),
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(rows))
}

// ---------------------------------------------------------------------------
// Findings (discrepancy queue)
// ---------------------------------------------------------------------------

/// Shared SELECT for the findings queue / single finding / transition responses.
const FINDING_SELECT: &str = "SELECT d.id,
                d.kind::text,
                d.state::text,
                d.item_id,
                i.code,
                d.holder_location_id,
                l.code,
                d.holder_package_id,
                p.barcode,
                d.expected_quantity::text,
                d.observed_quantity::text,
                d.variance::text,
                d.detail,
                d.stock_count_id,
                d.stock_movement_id,
                d.detected_at,
                d.detected_by_id,
                d.resolving_movement_id,
                d.resolved_at,
                d.resolved_by_id,
                d.resolution_reason,
                -- **Who, by name, and not only by id.** D11 makes the actor the
                -- non-repudiable floor of every fact, and an interface that can
                -- only show a uuid cannot make that visible to the person who
                -- has to act on it. The maud page resolved these and the JSON
                -- did not, so the first React screen over this read would have
                -- been a step backwards on the one attribute D11 is emphatic
                -- about. NULL means a scheduled check found it, which is a real
                -- and different answer from somebody-unnamed.
                fb.display_name,
                rb.display_name,
                -- **The pictures somebody offered in support of this** (D140).
                -- An aggregate rather than a second round trip: the findings
                -- rail draws them beside the numbers, and a screen fetching
                -- each finding's evidence one at a time is the shape the
                -- capture worklist was written to avoid.
                --
                -- Digests only. Which look produced them and who took it are
                -- questions `evidence` can answer and this list does not ask.
                coalesce(
                    (SELECT array_agg(oi.digest ORDER BY oi.captured_at, oi.id)
                       FROM evidence ev
                       JOIN observation_image oi
                         ON oi.observation_event_id = ev.observation_event_id
                      WHERE ev.discrepancy_id = d.id),
                    ARRAY[]::text[])
           FROM discrepancy d
           LEFT JOIN item i ON i.id = d.item_id
           LEFT JOIN location l ON l.id = d.holder_location_id
           LEFT JOIN package p ON p.id = d.holder_package_id
           LEFT JOIN person fb ON fb.id = d.detected_by_id
           LEFT JOIN person rb ON rb.id = d.resolved_by_id";

#[derive(Serialize, Debug, PartialEq)]
pub struct DiscrepancyRow {
    pub id: Uuid,
    pub kind: String,
    pub state: String,
    pub item_id: Option<Uuid>,
    pub item_code: Option<String>,
    pub holder_location_id: Option<Uuid>,
    pub location_code: Option<String>,
    pub holder_package_id: Option<Uuid>,
    pub package_barcode: Option<String>,
    /// Decimal quantities as strings (no rust_decimal dependency).
    pub expected_quantity: Option<String>,
    pub observed_quantity: Option<String>,
    pub variance: Option<String>,
    pub detail: Option<String>,
    pub stock_count_id: Option<Uuid>,
    pub stock_movement_id: Option<Uuid>,
    pub detected_at: DateTime<Utc>,
    pub detected_by_id: Option<Uuid>,
    pub resolving_movement_id: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by_id: Option<Uuid>,
    pub resolution_reason: Option<String>,
    /// Who found it. NULL is a scheduled check rather than an unnamed person,
    /// which the screen says in as many words.
    pub detected_by_name: Option<String>,
    /// Who closed it, for a finding that is closed.
    pub resolved_by_name: Option<String>,
    /// Content addresses of the photographs offered in support of this finding.
    /// D140. Empty is the ordinary case: evidence is possible and never
    /// required.
    pub evidence: Vec<String>,
}

fn map_discrepancy_row(r: &tokio_postgres::Row) -> DiscrepancyRow {
    DiscrepancyRow {
        id: r.get(0),
        kind: r.get(1),
        state: r.get(2),
        item_id: r.get(3),
        item_code: r.get(4),
        holder_location_id: r.get(5),
        location_code: r.get(6),
        holder_package_id: r.get(7),
        package_barcode: r.get(8),
        expected_quantity: r.get(9),
        observed_quantity: r.get(10),
        variance: r.get(11),
        detail: r.get(12),
        stock_count_id: r.get(13),
        stock_movement_id: r.get(14),
        detected_at: r.get(15),
        detected_by_id: r.get(16),
        resolving_movement_id: r.get(17),
        resolved_at: r.get(18),
        resolved_by_id: r.get(19),
        resolution_reason: r.get(20),
        detected_by_name: r.get(21),
        resolved_by_name: r.get(22),
        evidence: r.get(23),
    }
}

async fn load_discrepancy_row(
    tx: &tokio_postgres::Transaction<'_>,
    id: Uuid,
) -> Result<DiscrepancyRow, ApiError> {
    let sql = format!("{FINDING_SELECT} WHERE d.id = $1");
    let row = tx.query_opt(&sql, &[&id]).await?;
    let Some(r) = row else {
        return Err(ApiError::NotFound);
    };
    Ok(map_discrepancy_row(&r))
}

#[derive(Deserialize, Debug, Default)]
pub struct DiscrepancyQuery {
    /// Comma-separated states. Default: `open` (floor queue).
    /// Use `open,investigating` for the active set; `resolved,accepted` for closed.
    pub state: Option<String>,
    /// Optional kind filter (e.g. `count_variance`).
    pub kind: Option<String>,
    /// Item code (exact).
    pub item: Option<String>,
    /// Item id.
    pub item_id: Option<Uuid>,
    /// Location code (exact) — holder bin.
    pub location: Option<String>,
    /// Holder location id.
    pub location_id: Option<Uuid>,
    /// Package barcode (exact).
    pub package: Option<String>,
    /// Holder package id.
    pub package_id: Option<Uuid>,
    /// Raised from this stock count.
    pub stock_count_id: Option<Uuid>,
    /// `detected_at >= since` (ISO-8601).
    pub since: Option<DateTime<Utc>>,
    /// `detected_at <= until` (ISO-8601).
    pub until: Option<DateTime<Utc>>,
    /// Only findings older than this many hours (`detected_at <= now() - N hours`).
    pub older_than_hours: Option<i64>,
    /// Max rows (default 100, cap 500).
    pub limit: Option<i64>,
}

/// Open (or filtered) findings for the tenant — the queue D8 raises into.
///
/// Default is `state=open`. Does not mutate. RLS scopes to the request tenant.
/// Filters: `kind`, `item`/`item_id`, `location`/`location_id`, `package`/
/// `package_id`, `stock_count_id`, `since`/`until`, `older_than_hours`.
#[get("/discrepancies")]
pub async fn discrepancies(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<DiscrepancyQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let states: Vec<String> = query
        .state
        .as_deref()
        .unwrap_or("open")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if states.is_empty() {
        return Err(ApiError::Rejected("state filter is empty".into()));
    }
    if let Some(h) = query.older_than_hours {
        if h < 0 {
            return Err(ApiError::Rejected(
                "older_than_hours must be non-negative".into(),
            ));
        }
    }
    if let (Some(since), Some(until)) = (query.since, query.until) {
        if since > until {
            return Err(ApiError::Rejected(
                "since must be at or before until".into(),
            ));
        }
    }

    let kind = query.kind.clone();
    let item_code = query.item.clone();
    let item_id = query.item_id;
    let location_code = query.location.clone();
    let location_id = query.location_id;
    // Avoid shadowing the `package` route unit struct from actix macros.
    let package_barcode = query.package.clone();
    let package_id = query.package_id;
    let stock_count_id = query.stock_count_id;
    let since = query.since;
    let until = query.until;
    let older_than_hours = query.older_than_hours;
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let rows = scope
        .run(|tx| {
            Box::pin(async move {
                let sql = format!(
                    "{FINDING_SELECT}
                      WHERE d.state::text = ANY ($1)
                        AND ($2::text IS NULL OR d.kind::text = $2)
                        AND ($3::text IS NULL OR i.code = $3)
                        AND ($4::uuid IS NULL OR d.item_id = $4)
                        AND ($5::text IS NULL OR l.code = $5)
                        AND ($6::uuid IS NULL OR d.holder_location_id = $6)
                        AND ($7::text IS NULL OR p.barcode = $7)
                        AND ($8::uuid IS NULL OR d.holder_package_id = $8)
                        AND ($9::uuid IS NULL OR d.stock_count_id = $9)
                        AND ($10::timestamptz IS NULL OR d.detected_at >= $10)
                        AND ($11::timestamptz IS NULL OR d.detected_at <= $11)
                        AND ($12::bigint IS NULL
                             OR d.detected_at <= now() - ($12::text || ' hours')::interval)
                      ORDER BY d.detected_at DESC, d.id DESC
                      LIMIT $13"
                );
                let rows = tx
                    .query(
                        &sql,
                        &[
                            &states,
                            &kind,
                            &item_code,
                            &item_id,
                            &location_code,
                            &location_id,
                            &package_barcode,
                            &package_id,
                            &stock_count_id,
                            &since,
                            &until,
                            &older_than_hours,
                            &limit,
                        ],
                    )
                    .await?;
                Ok(rows.iter().map(map_discrepancy_row).collect::<Vec<_>>())
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(rows))
}

/// One finding by id (any state).
#[get("/discrepancies/{id}")]
pub async fn discrepancy(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let id = path.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let row = scope
        .run(|tx| {
            Box::pin(async move { load_discrepancy_row(tx, id).await })
        })
        .await?;

    Ok(HttpResponse::Ok().json(row))
}

#[derive(Deserialize, Debug, Default)]
pub struct InvestigateDiscrepancyRequest {
    /// Optional free-text note (appended to detail when present).
    pub note: Option<String>,
}

/// Full finding after the transition, plus soft warnings.
#[derive(Serialize, Debug)]
pub struct InvestigateDiscrepancyResponse {
    #[serde(flatten)]
    pub finding: DiscrepancyRow,
    pub warnings: Vec<String>,
}

/// Mark a finding as under investigation (`open` → `investigating`).
///
/// Idempotent if already investigating (soft warning). Resolved/accepted findings
/// are refused. Response is the full finding row so the floor UI need not re-GET.
/// Resolution still goes through `/adjustments` (or later paths) with
/// `resolving_movement_id`.
#[post("/discrepancies/{id}/investigate")]
pub async fn investigate_discrepancy(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<InvestigateDiscrepancyRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let id = path.into_inner();
    let body = body.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let row = tx
                    .query_opt(
                        "SELECT state::text, detail FROM discrepancy WHERE id = $1",
                        &[&id],
                    )
                    .await?;
                let current = row.as_ref().map(|r| r.get::<_, String>(0));
                let detail: Option<String> =
                    row.as_ref().and_then(|r| r.get::<_, Option<String>>(1));

                let mut warnings = vec![];
                match crate::findings::check_investigate(current.as_deref()) {
                    Ok(()) => {}
                    Err(crate::findings::InvestigateProblem::NotFound) => {
                        return Err(ApiError::NotFound);
                    }
                    Err(e) if crate::findings::investigate_is_hard(&e) => {
                        return Err(ApiError::Rejected(e.to_string()));
                    }
                    Err(e) => {
                        // Soft: already investigating — still refresh detail note if given.
                        warnings.push(e.to_string());
                    }
                }

                let new_detail = match (body.note.as_ref(), detail) {
                    (Some(note), Some(d)) if !note.is_empty() => {
                        Some(format!("{d}\n[investigating] {note}"))
                    }
                    (Some(note), None) if !note.is_empty() => {
                        Some(format!("[investigating] {note}"))
                    }
                    (_, d) => d,
                };

                let n = tx
                    .execute(
                        "UPDATE discrepancy
                            SET state = 'investigating',
                                detail = COALESCE($2, detail)
                          WHERE id = $1
                            AND state IN ('open', 'investigating')",
                        &[&id, &new_detail],
                    )
                    .await?;
                if n == 0 && warnings.is_empty() {
                    // Race: became terminal between check and update.
                    return Err(ApiError::Rejected(
                        "the finding could not be moved to investigating".into(),
                    ));
                }

                let finding = load_discrepancy_row(tx, id).await?;
                Ok(InvestigateDiscrepancyResponse { finding, warnings })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

#[derive(Deserialize, Debug)]
pub struct AcceptDiscrepancyRequest {
    /// Who signs off (required). Stored as `resolved_by_id`.
    /// Why no ledger write (required). Stored as `resolution_reason`.
    pub reason: String,
    /// Optional free-text note (appended to detail when present).
    pub note: Option<String>,
    /// When accepted; defaults to now.
    pub accepted_at: Option<DateTime<Utc>>,
}

/// Full finding after accept, plus soft warnings.
#[derive(Serialize, Debug)]
pub struct AcceptDiscrepancyResponse {
    #[serde(flatten)]
    pub finding: DiscrepancyRow,
    pub warnings: Vec<String>,
}

/// Accept a finding without a stock movement (`open|investigating` → `accepted`).
///
/// Manager sign-off that the model stands — distinct from `/adjustments`, which
/// writes a world-event movement and sets `state = resolved` with
/// `resolving_movement_id`. Idempotent if already accepted (soft warning).
/// Ledger-resolved findings are refused. Response is the full finding row.
#[post("/discrepancies/{id}/accept")]
pub async fn accept_discrepancy(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<AcceptDiscrepancyRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let id = path.into_inner();
    let body = body.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let row = tx
                    .query_opt(
                        "SELECT state::text, detail FROM discrepancy WHERE id = $1",
                        &[&id],
                    )
                    .await?;
                let current = row.as_ref().map(|r| r.get::<_, String>(0));
                let detail: Option<String> =
                    row.as_ref().and_then(|r| r.get::<_, Option<String>>(1));

                let mut warnings = vec![];
                match crate::findings::check_accept(
                    current.as_deref(),
                    true,
                    Some(body.reason.as_str()),
                ) {
                    Ok(()) => {}
                    Err(crate::findings::AcceptProblem::NotFound) => {
                        return Err(ApiError::NotFound);
                    }
                    Err(e) if crate::findings::accept_is_hard(&e) => {
                        return Err(ApiError::Rejected(e.to_string()));
                    }
                    Err(e) => {
                        warnings.push(e.to_string());
                    }
                }

                // Person must exist (and be visible under RLS when tenant-scoped).
                let person_ok = tx
                    .query_opt("SELECT id FROM person WHERE id = $1", &[&who.person_id])
                    .await?
                    .is_some();
                if !person_ok {
                    return Err(ApiError::Rejected(
                        "recorded_by_id does not name a person".into(),
                    ));
                }

                let reason = body.reason.trim().to_string();
                let accepted_at = body.accepted_at.unwrap_or_else(Utc::now);

                let new_detail = match (body.note.as_ref(), detail) {
                    (Some(note), Some(d)) if !note.is_empty() => {
                        Some(format!("{d}\n[accepted] {note}"))
                    }
                    (Some(note), None) if !note.is_empty() => {
                        Some(format!("[accepted] {note}"))
                    }
                    (_, d) => d,
                };

                let n = tx
                    .execute(
                        "UPDATE discrepancy
                            SET state = 'accepted',
                                resolved_at = COALESCE(resolved_at, $2),
                                resolved_by_id = COALESCE(resolved_by_id, $3),
                                resolution_reason = COALESCE(resolution_reason, $4),
                                detail = COALESCE($5, detail)
                          WHERE id = $1
                            AND state IN ('open', 'investigating', 'accepted')",
                        &[
                            &id,
                            &accepted_at,
                            &who.person_id,
                            &reason,
                            &new_detail,
                        ],
                    )
                    .await?;
                if n == 0 && warnings.is_empty() {
                    return Err(ApiError::Rejected(
                        "the finding could not be moved to accepted".into(),
                    ));
                }

                let finding = load_discrepancy_row(tx, id).await?;
                Ok(AcceptDiscrepancyResponse { finding, warnings })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Fulfilment progress (the numbers D99–D103 fold and nothing had read)
// ---------------------------------------------------------------------------

#[derive(Serialize, Debug, PartialEq)]
pub struct FulfilmentLineProgress {
    pub id: Uuid,
    pub fulfilment_id: Uuid,
    pub fulfilment_state: String,
    pub order_id: Uuid,
    pub order_line_number: i32,
    pub item_code: String,
    /// Units this commitment is for.
    pub quantity: i64,
    /// Cache: folded columns on `fulfilment_line` (may lag the ledger).
    pub covered_quantity: i64,
    pub picked_quantity: i64,
    pub packed_quantity: i64,
    pub despatched_quantity: i64,
    pub uncovered_quantity: i64,
    /// When the fulfilment rebuild last ran for this tenant (null = never).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection_as_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Live fold from the ledger for this line only (D99/D103, O(line)).
    /// Present on single-line GET and on write responses; omitted on list endpoints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ledger: Option<Progress>,
}

/// Every line of a fulfilment, with the four quantities the floor and a dispute
/// both care about.
///
/// `covered` is an intention fold (`stock_allocation`); the other three are
/// ledger folds (D99/D100). They can disagree, and that disagreement is the
/// point rather than a bug — J56 reports it when it is pathological.
/// Attach a photograph to an observation.
///
/// **Raw bytes with a declared content type, not a JSON envelope.** A base64
/// field would inflate every upload by a third over a warehouse's wifi to save
/// the client a `fetch` option, and the face and the event are already in the
/// path where a cache and a log can see them.
///
/// The type is then read from the bytes rather than believed: a file stored as
/// one thing and served as another is how an image endpoint becomes an XSS, and
/// a client can be wrong or lying about `content-type`.
#[post("/observations/{id}/images/{face}")]
pub async fn record_observation_image(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<(Uuid, String)>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let (event_id, face) = path.into_inner();

    // **Checked here rather than by the constraint.** A CHECK is the backstop
    // and its violation arrives as an internal error with a constraint name in
    // it; the person holding the handheld wants to be told which seven words
    // are acceptable.
    if !crate::images::is_face(&face) {
        return Err(ApiError::Rejected(format!(
            "{face} is not a face; use one of {}",
            crate::images::FACES.join(", ")
        )));
    }
    if body.is_empty() {
        return Err(ApiError::Rejected("an empty upload is not a photograph".into()));
    }
    if body.len() > crate::images::MAX_BYTES {
        return Err(ApiError::Rejected(format!(
            "that image is {} bytes and the limit is {}",
            body.len(),
            crate::images::MAX_BYTES
        )));
    }

    let Some(mime) = crate::images::sniff(&body) else {
        return Err(ApiError::Rejected(format!(
            "those bytes are not one of {}",
            crate::images::ACCEPTED.join(", ")
        )));
    };

    // **Written to the store before the row, and that order is deliberate.** A
    // file with no row is unreferenced and the reaper's problem; a row with no
    // file is a photograph the interface promises and cannot show.
    let dir = crate::images::directory();
    let digest = crate::images::put(&dir, &body).await?;
    let (width, height) = match crate::images::dimensions(&body) {
        Some((w, h)) => (Some(w), Some(h)),
        None => (None, None),
    };
    let byte_count = i64::try_from(body.len()).unwrap_or(i64::MAX);

    let tenant = who.tenant_id;
    let written = digest.clone();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let stored = scope
        .run(move |tx| {
            Box::pin(async move {
                let event = tx
                    .query_opt(
                        "SELECT observed_at FROM observation_event WHERE id = $1",
                        &[&event_id],
                    )
                    .await?
                    .ok_or(ApiError::NotFound)?;
                let observed_at: DateTime<Utc> = event.get(0);

                // **Append, never replace.** A retake is a new row: this is a
                // fact table and a blurred first attempt is a thing that
                // happened. Which picture of a face is current is a fold —
                // the same winning-row rule `package_event` uses to say where
                // a carton is.
                let row = tx
                    .query_one(
                        "INSERT INTO observation_image
                             (tenant_id, observation_event_id, face, digest, mime,
                              byte_count, width_px, height_px, captured_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                         RETURNING id",
                        &[
                            &tenant,
                            &event_id,
                            &face,
                            &written,
                            &mime,
                            &byte_count,
                            &width,
                            &height,
                            &observed_at,
                        ],
                    )
                    .await?;
                Ok(row.get::<_, Uuid>(0))
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "image_id": stored,
        "digest": digest,
        "mime": mime,
        "byte_count": byte_count,
        "width_px": width,
        "height_px": height,
    })))
}

/// Serve a photograph by its content address.
///
/// Immutable by construction — the name *is* the bytes — so it caches for a
/// year. The digest is validated before it touches a path, because a path built
/// from unvalidated input is how `..` becomes an arbitrary read.
#[get("/images/{digest}")]
pub async fn read_image(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let digest = path.into_inner();
    if !crate::images::is_digest(&digest) {
        return Err(ApiError::Rejected("that is not a content address".into()));
    }

    // **The row is what authorises the bytes.** A content address is not a
    // secret and knowing one must not be enough: the lookup runs inside the
    // tenant scope, so a digest belonging to another tenant is a 404 here.
    let tenant = who.tenant_id;
    let wanted = digest.clone();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let mime = scope
        .run(move |tx| {
            Box::pin(async move {
                Ok(tx
                    .query_opt(
                        "SELECT mime FROM observation_image WHERE digest = $1 LIMIT 1",
                        &[&wanted],
                    )
                    .await?
                    .map(|r| r.get::<_, String>(0)))
            })
        })
        .await?
        .ok_or(ApiError::NotFound)?;

    match crate::images::get(&crate::images::directory(), &digest).await? {
        Some(bytes) => Ok(HttpResponse::Ok()
            .content_type(mime)
            .insert_header(("cache-control", "private, max-age=31536000, immutable"))
            // Belt and braces on a path that serves user-supplied bytes.
            .insert_header(("x-content-type-options", "nosniff"))
            .insert_header((
                "content-security-policy",
                "default-src 'none'; sandbox; frame-ancestors 'none'",
            ))
            .body(bytes)),
        // The row survives and the file does not, which is a real state: the
        // database and the volume are two things and restore separately.
        None => Err(ApiError::NotFound),
    }
}

/// The despatch bench, in one read.
///
/// Stages 6 to 9, which had no screen in any form: the pack bench ends at a
/// sealed carton and everything after it still happens in MachShip by eye.
/// Sealed-and-unconsigned grouped by job, what is booked and not yet gone,
/// what left today, and the carriers — the route as data rather than recall.
#[get("/sites/{id}/despatch")]
pub async fn site_despatch(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let screen = crate::despatch::screen(&state, &who, Some(path.into_inner())).await?;
    Ok(HttpResponse::Ok().json(screen))
}

#[derive(Deserialize, Debug)]
pub struct CaptureQuery {
    pub limit: Option<i64>,
}

/// Whether this deployment has anybody in it (D142).
///
/// Unauthenticated on purpose and safe to be: it answers a count, and the act
/// it precedes is gated by a token nobody outside the server's log or its
/// filesystem has.
#[get("/setup")]
pub async fn setup_status(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let conn = state.pool.get().await?;
    let required = crate::setup::deployment_is_empty(&conn).await?;
    Ok(HttpResponse::Ok().json(crate::setup::SetupStatus {
        required,
        token_ready: required && crate::setup::token_is_ready(),
    }))
}

/// Create the first administrator, once (D142).
#[post("/setup")]
pub async fn setup_deployment(
    state: web::Data<AppState>,
    body: web::Json<crate::setup::SetupRequest>,
) -> Result<HttpResponse, ApiError> {
    let done = crate::setup::create_first_administrator(&state.pool, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(done))
}

#[derive(Deserialize, Debug)]
pub struct ResolveQuery {
    /// Exactly what the scanner sent, including any symbology prefix and any
    /// FNC1 separators. **Not pre-cleaned by the client**: what was read is
    /// evidence, and a client that trims it has already made a decision the
    /// server cannot see.
    pub scan: String,
    /// `item`, `package`, `location` or absent. The narrowing step of D34's
    /// resolution function, taken from `expected_entity_kind`.
    pub expect: Option<String>,
}

/// What the thing in your hand is (D111, D34).
///
/// One input resolves any scannable identifier against the three surfaces —
/// `item_barcode`, the package's own codes and `location` — and answers with
/// one of D24's four outcome words. An item comes back with the capture
/// subjects it offers, which is the worklist's own enumeration rather than a
/// second one, so a scan cannot open a session the worklist would not list.
///
/// **A `GET`, and that is a claim.** D111 says a failed resolution is a record,
/// and D28 specifies it on `activity_event`, which does not exist. Nothing here
/// writes, so nothing here pretends to. When the record lands this becomes an
/// act and the verb changes with it. D136.
#[get("/resolve")]
pub async fn resolve_identifier(
    req: HttpRequest,
    state: web::Data<AppState>,
    q: web::Query<ResolveQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let q = q.into_inner();
    if q.scan.trim().is_empty() {
        return Err(ApiError::Rejected(
            "a scan of nothing is not a scan; send what the reader sent".into(),
        ));
    }
    let found = crate::locator::resolve(&state, &who, &q.scan, q.expect.as_deref()).await?;
    Ok(HttpResponse::Ok().json(found))
}

/// The capture worklist, in one read.
///
/// **What wants weighing, measuring or photographing**, which is wider than
/// `/revalidation`'s question and asked from the other end: that read starts
/// from `observation_current` and cannot see a thing nobody has ever observed,
/// which is exactly what a capture worklist is for.
///
/// Three lists, and a subject is on one of them or on none. Tenant-scoped
/// rather than site-scoped: what a kind of carton measures is not a fact about
/// a site, and an item nobody has ordered here still wants capturing. The site
/// comes back for the shell to name, and demand is what ranks.
#[get("/capture")]
pub async fn capture_worklist(
    req: HttpRequest,
    state: web::Data<AppState>,
    q: web::Query<CaptureQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let limit = q.into_inner().limit.unwrap_or(25).clamp(1, 200);
    let screen = crate::capture::screen(&state, &who, limit).await?;
    Ok(HttpResponse::Ok().json(screen))
}

/// Whether this is the one-meaning-at-a-time constraint and not some other
/// failure. Named rather than matched on the SQLSTATE alone: `23P01` is every
/// exclusion constraint, and turning an unrelated one into "that barcode is
/// taken" would be a sentence about the wrong thing.
fn is_one_meaning_violation(e: &tokio_postgres::Error) -> bool {
    e.as_db_error()
        .and_then(|d| d.constraint())
        .is_some_and(|c| c == "item_barcode_one_meaning_at_a_time")
}

/// What to tell somebody whose label already means something.
///
/// **The shared arm is a different sentence**, and it is the one with no way
/// out: a tenant may bind over nothing, and the shared catalogue is not this
/// tenant's to change. Saying "already bound" there would send an operator
/// looking for a screen that does not exist.
fn refusal(barcode: &str, code: &str, level: Option<&str>, shared: bool) -> String {
    let means = match level {
        Some(l) => format!("{code} at {l}"),
        None => format!("{code}, at a level nobody recorded"),
    };
    if shared {
        format!(
            "{barcode} means {means} in the shared catalogue, which is not this              company's to change from the floor"
        )
    } else {
        format!("{barcode} already means {means}; one identifier means one thing at a time")
    }
}

#[derive(Deserialize, Debug)]
pub struct BindBarcodeRequest {
    /// Exactly what the scanner or the keyboard produced.
    pub scan: String,
    /// One of the five `packaging_level` values. **Required**, and that is the
    /// point of D164: a barcode that does not say which box it is on is the
    /// gap migration 79 recorded and left open.
    pub packaging_level: String,
    /// Base units per scan, where the operator knows it.
    pub quantity: Option<i64>,
}

#[derive(Serialize, Debug)]
pub struct BoundBarcode {
    pub id: Uuid,
    pub item_id: Uuid,
    pub item_code: String,
    pub barcode: String,
    pub scheme: String,
    pub packaging_level: String,
    pub quantity: Option<i64>,
    pub bound_by_name: Option<String>,
    /// True where this binding already existed exactly as proposed. D5's shape
    /// for a reference write: sending it twice changes nothing and answers the
    /// same row, so a retry over a bad connection is not a second question.
    pub already: bool,
    pub warnings: Vec<String>,
}

/// Bind a scanned identifier to an item, at a packaging level (D164).
///
/// **The write path migration 79 said did not exist.** That file refuses a feed
/// writing here — *"a wrong barcode binding produces stock movements against
/// the wrong item"* — and closes by saying an assertion lands and *something
/// with a name promotes it*. A person at the shelf with the box in one hand and
/// the scanner in the other is that something, so the row records who they were
/// (D11) and the endpoint is behind a session rather than a machine token.
///
/// **One barcode means one thing at a time**, which is the table's own
/// exclusion constraint. Rebinding is therefore a refusal rather than an
/// overwrite, and the refusal says what the string currently means — because
/// the operator scanning it is holding something, and *that is already
/// something else* is the only answer that helps them.
#[post("/items/{id}/barcodes")]
pub async fn bind_barcode(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<BindBarcodeRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item_id = path.into_inner();
    let body = body.into_inner();

    let proposed = crate::binding::Proposed {
        raw: body.scan,
        packaging_level: body.packaging_level,
        quantity: body.quantity,
    };
    let decided = crate::binding::decide(&proposed).map_err(|e| ApiError::Rejected(e.to_string()))?;

    let person_id = who.person_id;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                // The item, and its base unit — `quantity` is base units per
                // scan, so the unit it counts in is the item's own.
                let item = tx
                    .query_opt(
                        "SELECT code, base_unit_id FROM item WHERE id = $1",
                        &[&item_id],
                    )
                    .await?
                    .ok_or(ApiError::NotFound)?;
                let item_code: String = item.get(0);
                let base_unit_id: Uuid = item.get(1);

                // **What this string means now, if anything.**
                //
                // Live rows only: `effective` is a range and a closed one is
                // the evidence for what a historical scan meant (D31), not a
                // claim about today.
                //
                // **`query` and not `query_opt`, and the difference is a 500.**
                // `item_barcode_shared_read` shows this tenant its own rows
                // *and* the shared catalogue's, and the exclusion constraint
                // COALESCEs the tenant so those two do not conflict with each
                // other — by design, since a barcode may mean one thing
                // globally and another here. So two live rows for one string
                // is a legal state, and `query_opt` answers it by erroring.
                //
                // Ordered the way `locator` orders the same read: the tenant's
                // own row is the one that wins a resolution, so it is the one
                // the operator is told about.
                let live = tx
                    .query(
                        "SELECT b.id, b.item_id, i.code, b.packaging_level::text,
                                b.quantity, p.display_name, b.tenant_id
                           FROM item_barcode b
                           JOIN item i ON i.id = b.item_id
                           LEFT JOIN person p ON p.id = b.bound_by_person_id
                          WHERE b.barcode = $1 AND b.effective @> CURRENT_DATE
                          ORDER BY b.tenant_id NULLS LAST",
                        &[&decided.barcode],
                    )
                    .await?;

                if let Some(row) = live.first() {
                    let bound_item: Uuid = row.get(1);
                    let bound_level: Option<String> = row.get(3);
                    let bound_code: String = row.get(2);
                    // Exactly this binding already: answer it rather than
                    // refuse it. Scanning the same label twice is a thing that
                    // happens with a trigger under a glove.
                    if bound_item == item_id && bound_level.as_deref() == Some(decided.packaging_level.as_str()) {
                        return Ok(BoundBarcode {
                            id: row.get(0),
                            item_id: bound_item,
                            item_code: bound_code,
                            barcode: decided.barcode.clone(),
                            scheme: decided.scheme.to_string(),
                            packaging_level: decided.packaging_level.clone(),
                            quantity: row.get(4),
                            bound_by_name: row.get(5),
                            already: true,
                            warnings: vec![],
                        });
                    }
                    let shared: Option<Uuid> = row.get(6);
                    return Err(ApiError::Rejected(refusal(
                        &decided.barcode,
                        &bound_code,
                        bound_level.as_deref(),
                        shared.is_none(),
                    )));
                }

                // **The read above is not the check, it is the courtesy.**
                // Two operators scanning one new label seconds apart both find
                // nothing live and both insert, and the second is refused by
                // `item_barcode_one_meaning_at_a_time` — which without this
                // arrives as a 500, the exact thing this endpoint exists to
                // stop happening to somebody holding a scanner. The constraint
                // is the check; this turns its refusal back into the sentence
                // the read would have produced.
                let inserted = tx
                    .query_one(
                        "INSERT INTO item_barcode
                             (tenant_id, item_id, barcode, scheme, unit_id, quantity,
                              packaging_level, bound_by_person_id)
                         VALUES (current_tenant(), $1, $2, $3, $4, $5, $6::text::packaging_level, $7)
                         RETURNING id",
                        &[
                            &item_id,
                            &decided.barcode,
                            &decided.scheme,
                            &base_unit_id,
                            &decided.quantity,
                            &decided.packaging_level,
                            &person_id,
                        ],
                    )
                    .await;
                let id: Uuid = match inserted {
                    Ok(row) => row.get(0),
                    Err(e) if is_one_meaning_violation(&e) => {
                        let now = tx
                            .query_opt(
                                "SELECT i.code, b.packaging_level::text, b.tenant_id
                                   FROM item_barcode b
                                   JOIN item i ON i.id = b.item_id
                                  WHERE b.barcode = $1 AND b.effective @> CURRENT_DATE
                                  ORDER BY b.tenant_id NULLS LAST",
                                &[&decided.barcode],
                            )
                            .await?;
                        return Err(ApiError::Rejected(match now {
                            Some(r) => refusal(
                                &decided.barcode,
                                &r.get::<_, String>(0),
                                r.get::<_, Option<String>>(1).as_deref(),
                                r.get::<_, Option<Uuid>>(2).is_none(),
                            ),
                            // Gone between the violation and the re-read. Rare
                            // and possible, and inventing a subject for the
                            // sentence would be worse than saying so.
                            None => format!(
                                "{} was bound by somebody else while this was being written",
                                decided.barcode
                            ),
                        }));
                    }
                    Err(e) => return Err(e.into()),
                };

                let bound_by_name: Option<String> = tx
                    .query_opt("SELECT display_name FROM person WHERE id = $1", &[&person_id])
                    .await?
                    .map(|r| r.get(0));

                Ok(BoundBarcode {
                    id,
                    item_id,
                    item_code,
                    barcode: decided.barcode.clone(),
                    scheme: decided.scheme.to_string(),
                    packaging_level: decided.packaging_level.clone(),
                    quantity: decided.quantity,
                    bound_by_name,
                    already: false,
                    warnings: decided.warnings.clone(),
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(out))
}

/// What an item answers to, newest binding first.
#[get("/items/{id}/barcodes")]
pub async fn item_barcodes(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item_id = path.into_inner();
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let rows = scope
        .run(move |tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        "SELECT b.id, b.item_id, i.code, b.barcode, b.scheme,
                                b.packaging_level::text, b.quantity, p.display_name
                           FROM item_barcode b
                           JOIN item i ON i.id = b.item_id
                           LEFT JOIN person p ON p.id = b.bound_by_person_id
                          WHERE b.item_id = $1 AND b.effective @> CURRENT_DATE
                          ORDER BY b.id DESC",
                        &[&item_id],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| BoundBarcode {
                        id: r.get(0),
                        item_id: r.get(1),
                        item_code: r.get(2),
                        barcode: r.get(3),
                        scheme: r.get(4),
                        packaging_level: r
                            .get::<_, Option<String>>(5)
                            .unwrap_or_else(|| "unrecorded".into()),
                        quantity: r.get(6),
                        bound_by_name: r.get(7),
                        already: true,
                        warnings: vec![],
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(rows))
}

/// The pack bench, in one read.
///
/// **The same figures the server-rendered page draws**, from
/// [`crate::bench`] rather than from SQL restated here. The maud page and this
/// endpoint are two renderings of one query set, which is the only arrangement
/// under which they cannot come to disagree about what is left to pack.
///
/// One request rather than five: D2's bar is *no page loads, sub-second*, and a
/// screen that opens by fetching a header, then lines, then cartons, then
/// contents, then presets has five chances to be slow.
#[get("/fulfilments/{id}/bench")]
pub async fn fulfilment_bench(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let screen = crate::bench::screen(&state, &who, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(screen))
}

#[get("/fulfilments/{id}/lines")]
pub async fn fulfilment_lines(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let fulfilment_id = path.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let rows = scope
        .run(|tx| {
            Box::pin(async move {
                // Confirm the fulfilment exists in this tenant (RLS alone would
                // return an empty list for a wrong id, which looks like "no
                // lines" rather than "not found").
                let found = tx
                    .query_opt("SELECT 1 FROM fulfilment WHERE id = $1", &[&fulfilment_id])
                    .await?;
                if found.is_none() {
                    return Err(ApiError::NotFound);
                }

                let rows = tx
                    .query(
                        "SELECT fl.id,
                                fl.fulfilment_id,
                                f.state::text,
                                f.order_id,
                                ol.line_number,
                                i.code,
                                fl.quantity,
                                fl.covered_quantity,
                                fl.picked_quantity,
                                fl.packed_quantity,
                                fl.despatched_quantity,
                                fl.uncovered_quantity
                           FROM fulfilment_line fl
                           JOIN fulfilment f ON f.id = fl.fulfilment_id
                           JOIN order_line ol ON ol.id = fl.order_line_id
                           JOIN item i ON i.id = ol.item_id
                          WHERE fl.fulfilment_id = $1
                          ORDER BY ol.line_number, fl.id",
                        &[&fulfilment_id],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| FulfilmentLineProgress {
                        id: r.get(0),
                        fulfilment_id: r.get(1),
                        fulfilment_state: r.get(2),
                        order_id: r.get(3),
                        order_line_number: r.get(4),
                        item_code: r.get(5),
                        quantity: r.get(6),
                        covered_quantity: r.get(7),
                        picked_quantity: r.get(8),
                        packed_quantity: r.get(9),
                        despatched_quantity: r.get(10),
                        uncovered_quantity: r.get(11),
                        projection_as_at: None,
                        ledger: None, // list stays cheap: projection only
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(rows))
}

#[derive(Deserialize, Debug, Default)]
pub struct LineQuery {
    /// `projection` (default) keeps cache fields primary; `ledger` still returns
    /// both, but clients that only care about live truth can key off `ledger`.
    pub view: Option<String>,
}

/// One fulfilment line by id — the unit a pick screen and a dispute both name.
///
/// Always includes `ledger` (live fold) and projection columns with
/// `projection_as_at`. The cache may lag; the ledger does not.
#[get("/fulfilment-lines/{id}")]
pub async fn fulfilment_line(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    query: web::Query<LineQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let line_id = path.into_inner();
    let _view = query.view.clone(); // reserved: both views always returned
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let row = scope
        .run(|tx| {
            Box::pin(async move {
                let row = tx
                    .query_opt(
                        "SELECT fl.id,
                                fl.fulfilment_id,
                                f.state::text,
                                f.order_id,
                                ol.line_number,
                                i.code,
                                fl.quantity,
                                fl.covered_quantity,
                                fl.picked_quantity,
                                fl.packed_quantity,
                                fl.despatched_quantity,
                                fl.uncovered_quantity
                           FROM fulfilment_line fl
                           JOIN fulfilment f ON f.id = fl.fulfilment_id
                           JOIN order_line ol ON ol.id = fl.order_line_id
                           JOIN item i ON i.id = ol.item_id
                          WHERE fl.id = $1",
                        &[&line_id],
                    )
                    .await?;
                let Some(r) = row else {
                    return Err(ApiError::NotFound);
                };
                let quantity: i64 = r.get(6);
                let covered: i64 = r.get(7);
                let picked: i64 = r.get(8);
                let packed: i64 = r.get(9);
                let despatched: i64 = r.get(10);
                let uncovered: i64 = r.get(11);
                let proj = ledger_views::line_progress_projection(
                    tx, covered, picked, packed, despatched, uncovered,
                )
                .await?;
                let ledger = ledger_views::line_progress_ledger(tx, line_id, quantity).await?;
                Ok(FulfilmentLineProgress {
                    id: r.get(0),
                    fulfilment_id: r.get(1),
                    fulfilment_state: r.get(2),
                    order_id: r.get(3),
                    order_line_number: r.get(4),
                    item_code: r.get(5),
                    quantity,
                    covered_quantity: covered,
                    picked_quantity: picked,
                    packed_quantity: packed,
                    despatched_quantity: despatched,
                    uncovered_quantity: uncovered,
                    projection_as_at: proj.as_at,
                    ledger: Some(ledger),
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(row))
}

// ---------------------------------------------------------------------------
// Package (carton) state — what packed_quantity reads
// ---------------------------------------------------------------------------

#[derive(Serialize, Debug, PartialEq)]
pub struct PackageView {
    pub id: Uuid,
    pub barcode: Option<String>,
    pub sscc: Option<String>,
    /// Fold of package_event (cache). Packed progress reads this after rebuild.
    pub status: Option<String>,
    /// Winning kind from the event log right now (may lead the fold).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_ledger: Option<String>,
    pub location_code: Option<String>,
    pub fulfilment_id: Option<Uuid>,
    pub depth: Option<i32>,
    /// Units currently held in this package (stock fold), if any.
    pub stock_quantity: i64,
}

#[get("/packages/{id}")]
pub async fn package(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let package_id = path.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let row = scope
        .run(|tx| {
            Box::pin(async move {
                let row = tx
                    .query_opt(
                        "SELECT p.id,
                                p.barcode,
                                p.sscc::text,
                                p.status,
                                l.code,
                                p.fulfilment_id,
                                p.depth,
                                coalesce((
                                    SELECT sum(s.quantity)::bigint
                                      FROM stock s
                                     WHERE s.holder_package_id = p.id
                                ), 0)
                           FROM package p
                           LEFT JOIN location l ON l.id = p.resolved_location_id
                          WHERE p.id = $1",
                        &[&package_id],
                    )
                    .await?;
                let Some(r) = row else {
                    return Err(ApiError::NotFound);
                };
                let status_ledger =
                    ledger_views::package_status_ledger(tx, package_id).await?;
                Ok(PackageView {
                    id: r.get(0),
                    barcode: r.get(1),
                    sscc: r.get(2),
                    status: r.get(3),
                    status_ledger,
                    location_code: r.get(4),
                    fulfilment_id: r.get(5),
                    depth: r.get(6),
                    stock_quantity: r.get(7),
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(row))
}

// ---------------------------------------------------------------------------
// Open work at a site
// ---------------------------------------------------------------------------

/// A line that still has something left to ship — the picking index.
#[derive(Serialize, Debug, PartialEq)]
pub struct OpenWorkLine {
    pub fulfilment_line_id: Uuid,
    pub fulfilment_id: Uuid,
    pub fulfilment_state: String,
    pub order_id: Uuid,
    pub order_line_number: i32,
    pub item_code: String,
    pub quantity: i64,
    pub covered_quantity: i64,
    pub picked_quantity: i64,
    pub packed_quantity: i64,
    pub despatched_quantity: i64,
    pub remaining_to_despatch: i64,
}

/// Fulfilment lines at a site that are not cancelled and not fully despatched.
#[get("/sites/{id}/open-lines")]
pub async fn open_lines(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let site_id = path.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let rows = scope
        .run(|tx| {
            Box::pin(async move {
                let found = tx
                    .query_opt("SELECT 1 FROM site WHERE id = $1", &[&site_id])
                    .await?;
                if found.is_none() {
                    return Err(ApiError::NotFound);
                }

                let rows = tx
                    .query(
                        "SELECT fl.id,
                                fl.fulfilment_id,
                                f.state::text,
                                f.order_id,
                                ol.line_number,
                                i.code,
                                fl.quantity,
                                fl.covered_quantity,
                                fl.picked_quantity,
                                fl.packed_quantity,
                                fl.despatched_quantity,
                                (fl.quantity - fl.despatched_quantity)
                           FROM fulfilment_line fl
                           JOIN fulfilment f ON f.id = fl.fulfilment_id
                           JOIN order_line ol ON ol.id = fl.order_line_id
                           JOIN item i ON i.id = ol.item_id
                          WHERE f.site_id = $1
                            AND f.state <> 'cancelled'
                            AND fl.despatched_quantity < fl.quantity
                          ORDER BY f.state, ol.line_number, fl.id",
                        &[&site_id],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| OpenWorkLine {
                        fulfilment_line_id: r.get(0),
                        fulfilment_id: r.get(1),
                        fulfilment_state: r.get(2),
                        order_id: r.get(3),
                        order_line_number: r.get(4),
                        item_code: r.get(5),
                        quantity: r.get(6),
                        covered_quantity: r.get(7),
                        picked_quantity: r.get(8),
                        packed_quantity: r.get(9),
                        despatched_quantity: r.get(10),
                        remaining_to_despatch: r.get(11),
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(rows))
}


// ---------------------------------------------------------------------------
// Create a package (skeleton + created event)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct CreatePackageRequest {
    /// Client-minted id (D5 offline). Generated server-side if omitted.
    pub id: Option<Uuid>,
    pub fulfilment_id: Option<Uuid>,
    pub sequence: Option<i32>,
    pub package_type_id: Option<Uuid>,
    /// Where the package comes into existence (D97: created asserts placement).
    pub location_id: Uuid,
    pub barcode: Option<String>,
    pub sscc: Option<String>,
    /// Defaults to operator_scan.
    pub source: Option<String>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct CreatePackageResponse {
    pub package_id: Uuid,
    pub event_id: Uuid,
}

/// Mint a package: INSERT the row the app may write, then a `created` event.
///
/// Barcode/sscc go on the event (not the package row — those columns are
/// projections). Placement and status appear after the package maintainers run.
#[post("/packages")]
pub async fn create_package(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<CreatePackageRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    let package_id = body.id.unwrap_or_else(Uuid::now_v7);
    let source = body
        .source
        .clone()
        .unwrap_or_else(|| "operator_scan".to_string());

    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let ful_row = if let Some(fid) = body.fulfilment_id {
                    tx.query_opt(
                        "SELECT id, tenant_id, (state = 'cancelled')
                           FROM fulfilment WHERE id = $1",
                        &[&fid],
                    )
                    .await?
                } else {
                    None
                };
                let loc_row = tx
                    .query_opt(
                        "SELECT id, tenant_id FROM location WHERE id = $1",
                        &[&body.location_id],
                    )
                    .await?;

                let fulfilment = ful_row.as_ref().map(|r| packages::FulfilmentRef {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    cancelled: r.get(2),
                });
                let location = loc_row.as_ref().map(|r| packages::LocationRef {
                    id: r.get(0),
                    tenant_id: r.get(1),
                });

                let proposed = ProposedPackage {
                    tenant_id: tenant,
                    package_id,
                    fulfilment_id: body.fulfilment_id,
                    sequence: body.sequence,
                    package_type_id: body.package_type_id,
                    location_id: body.location_id,
                    barcode: body.barcode.clone(),
                    sscc: body.sscc.clone(),
                    source: source.clone(),
                };
                let problems = packages::check(&proposed, fulfilment.as_ref(), location.as_ref());
                if !problems.is_empty() {
                    return Err(ApiError::Rejected(
                        problems
                            .iter()
                            .map(|p| p.to_string())
                            .collect::<Vec<_>>()
                            .join("; "),
                    ));
                }

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;

                if act.is_replay() {
                    let (event_id, prior_package_id, kind) =
                        client_events::require_one_package_event(tx, body.client_event_id)
                            .await?;
                    if kind != "created" {
                        return Err(ApiError::Rejected(format!(
                            "client_event already recorded package_event kind {kind}"
                        )));
                    }
                    return Ok(CreatePackageResponse {
                        package_id: prior_package_id,
                        event_id,
                    });
                }

                // Column-level INSERT only: no barcode/sscc/status (projections).
                tx.execute(
                    "INSERT INTO package (
                         id, tenant_id, fulfilment_id, package_type_id, sequence)
                     VALUES ($1, $2, $3, $4, $5)",
                    &[
                        &package_id,
                        &tenant,
                        &body.fulfilment_id,
                        &body.package_type_id,
                        &body.sequence,
                    ],
                )
                .await?;

                let event_id: Uuid = tx
                    .query_one(
                        "INSERT INTO package_event (
                             tenant_id, client_event_id, package_id, kind, source,
                             occurred_at, recorded_by_id, location_id, barcode, sscc)
                         VALUES (
                             $1, $2, $3, 'created', $4,
                             $5, $6, $7, $8, $9)
                         RETURNING id",
                        &[
                            &tenant,
                            &body.client_event_id,
                            &package_id,
                            &source,
                            &body.occurred_at,
                            &who.person_id,
                            &body.location_id,
                            &body.barcode,
                            &body.sscc,
                        ],
                    )
                    .await?
                    .get(0);

                tx.execute(
                    "SELECT projection_mark_dirty($1, 'package_create')",
                    &[&tenant],
                )
                .await?;

                Ok(CreatePackageResponse {
                    package_id,
                    event_id,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Place a package (relocate to a location)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct PlacePackageRequest {
    pub location_id: Uuid,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub source: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct PlacePackageResponse {
    pub event_id: Uuid,
    pub warnings: Vec<String>,
    /// Winning placement kind from the event log (includes this place).
    pub package_status_ledger: Option<String>,
}

/// Record a place: one `package_event` (`placed`) with a location.
///
/// Does not UPDATE `package.resolved_location_id` or status — those are folds
/// (J6 compare-and-set on device clock). Despatched/voided packages are refused.
#[post("/packages/{id}/place")]
pub async fn place_package(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<PlacePackageRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let package_id = path.into_inner();
    let body = body.into_inner();
    let source = body
        .source
        .clone()
        .unwrap_or_else(|| "operator_scan".to_string());

    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let pkg_row = tx
                    .query_opt(
                        "SELECT id, tenant_id, status, resolved_location_id, parent_package_id
                           FROM package WHERE id = $1",
                        &[&package_id],
                    )
                    .await?;
                let loc_row = tx
                    .query_opt(
                        "SELECT id, tenant_id FROM location WHERE id = $1",
                        &[&body.location_id],
                    )
                    .await?;

                let pkg_ref = pkg_row.as_ref().map(|r| packages::PackageRef {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    status: r.get(2),
                    resolved_location_id: r.get(3),
                    parent_package_id: r.get(4),
                });
                let location = loc_row.as_ref().map(|r| packages::LocationRef {
                    id: r.get(0),
                    tenant_id: r.get(1),
                });

                let proposed = ProposedPlace {
                    tenant_id: tenant,
                    package_id,
                    location_id: body.location_id,
                    source: source.clone(),
                };
                let (hard, soft) =
                    packages::check_place(&proposed, pkg_ref.as_ref(), location.as_ref());
                if !hard.is_empty() {
                    // Package/location missing → 404 when that is the only issue.
                    if hard.len() == 1
                        && matches!(
                            hard[0],
                            packages::PlaceProblem::PackageNotFound
                                | packages::PlaceProblem::LocationNotFound
                        )
                    {
                        return Err(ApiError::NotFound);
                    }
                    return Err(ApiError::Rejected(
                        hard.iter()
                            .map(|p| p.to_string())
                            .collect::<Vec<_>>()
                            .join("; "),
                    ));
                }
                let mut warnings: Vec<String> = soft.iter().map(|p| p.to_string()).collect();

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;

                let event_id = match act {
                    ActInsert::Replay => {
                        let (id, prior_pkg, kind) =
                            client_events::require_one_package_event(tx, body.client_event_id)
                                .await?;
                        if kind != "placed" || prior_pkg != package_id {
                            return Err(ApiError::Rejected(
                                "client_event already recorded a different package act".into(),
                            ));
                        }
                        warnings.push(client_events::REPLAY_WARNING.into());
                        id
                    }
                    ActInsert::Fresh => {
                        let id: Uuid = tx
                            .query_one(
                                "INSERT INTO package_event (
                                     tenant_id, client_event_id, package_id, kind, source,
                                     occurred_at, recorded_by_id, location_id)
                                 VALUES ($1, $2, $3, 'placed', $4, $5, $6, $7)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &body.client_event_id,
                                    &package_id,
                                    &source,
                                    &body.occurred_at,
                                    &who.person_id,
                                    &body.location_id,
                                ],
                            )
                            .await?
                            .get(0);
                        tx.execute(
                            "SELECT projection_mark_dirty($1, 'package_place')",
                            &[&tenant],
                        )
                        .await?;
                        id
                    }
                };

                let package_status_ledger =
                    ledger_views::package_status_ledger(tx, package_id).await?;

                Ok(PlacePackageResponse {
                    event_id,
                    warnings,
                    package_status_ledger,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Contain a package (nest under a parent)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct ContainPackageRequest {
    pub parent_package_id: Uuid,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub source: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ContainPackageResponse {
    pub event_id: Uuid,
    pub warnings: Vec<String>,
    pub package_status_ledger: Option<String>,
}

/// Record a contain: one `package_event` (`contained`) with `parent_package_id`.
///
/// Does not UPDATE `package.parent_package_id` — that is a fold (J6). Location
/// and parent are mutually exclusive holders on the event (CHECK). Despatched
/// or voided packages on either side are refused.
#[post("/packages/{id}/contain")]
pub async fn contain_package(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<ContainPackageRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let package_id = path.into_inner();
    let body = body.into_inner();
    let source = body
        .source
        .clone()
        .unwrap_or_else(|| "operator_scan".to_string());

    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let pkg_row = tx
                    .query_opt(
                        "SELECT id, tenant_id, status, resolved_location_id, parent_package_id
                           FROM package WHERE id = $1",
                        &[&package_id],
                    )
                    .await?;
                let parent_row = tx
                    .query_opt(
                        "SELECT id, tenant_id, status, resolved_location_id, parent_package_id
                           FROM package WHERE id = $1",
                        &[&body.parent_package_id],
                    )
                    .await?;

                let child_pkg = pkg_row.as_ref().map(|r| packages::PackageRef {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    status: r.get(2),
                    resolved_location_id: r.get(3),
                    parent_package_id: r.get(4),
                });
                let parent_pkg = parent_row.as_ref().map(|r| packages::PackageRef {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    status: r.get(2),
                    resolved_location_id: r.get(3),
                    parent_package_id: r.get(4),
                });

                let proposed = ProposedContain {
                    tenant_id: tenant,
                    package_id,
                    parent_package_id: body.parent_package_id,
                    source: source.clone(),
                };
                let (hard, soft) =
                    packages::check_contain(&proposed, child_pkg.as_ref(), parent_pkg.as_ref());
                if !hard.is_empty() {
                    if hard.len() == 1
                        && matches!(
                            hard[0],
                            packages::ContainProblem::PackageNotFound
                                | packages::ContainProblem::ParentNotFound
                        )
                    {
                        return Err(ApiError::NotFound);
                    }
                    return Err(ApiError::Rejected(
                        hard.iter()
                            .map(|p| p.to_string())
                            .collect::<Vec<_>>()
                            .join("; "),
                    ));
                }
                let mut warnings: Vec<String> = soft.iter().map(|p| p.to_string()).collect();

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;

                if act.is_replay() {
                    let (event_id, prior_pkg, kind) =
                        client_events::require_one_package_event(tx, body.client_event_id)
                            .await?;
                    if kind != "contained" || prior_pkg != package_id {
                        return Err(ApiError::Rejected(
                            "client_event already recorded a different package act".into(),
                        ));
                    }
                    warnings.push(client_events::REPLAY_WARNING.into());
                    let package_status_ledger =
                        ledger_views::package_status_ledger(tx, package_id).await?;
                    return Ok(ContainPackageResponse {
                        event_id,
                        warnings,
                        package_status_ledger,
                    });
                }

                // parent_package_id set, location_id null — holder CHECK.
                let event_id: Uuid = tx
                    .query_one(
                        "INSERT INTO package_event (
                             tenant_id, client_event_id, package_id, kind, source,
                             occurred_at, recorded_by_id, parent_package_id)
                         VALUES ($1, $2, $3, 'contained', $4, $5, $6, $7)
                         RETURNING id",
                        &[
                            &tenant,
                            &body.client_event_id,
                            &package_id,
                            &source,
                            &body.occurred_at,
                            &who.person_id,
                            &body.parent_package_id,
                        ],
                    )
                    .await?
                    .get(0);

                let package_status_ledger =
                    ledger_views::package_status_ledger(tx, package_id).await?;

                tx.execute(
                    "SELECT projection_mark_dirty($1, 'package_contain')",
                    &[&tenant],
                )
                .await?;

                Ok(ContainPackageResponse {
                    event_id,
                    warnings,
                    package_status_ledger,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Seal a package
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct SealPackageRequest {
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub source: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct SealPackageResponse {
    pub event_id: Uuid,
    pub warnings: Vec<String>,
    /// Winning package status from the event log (includes this seal).
    pub package_status_ledger: Option<String>,
}

/// Record a seal: one `package_event` (`sealed`) and stamp `sealed_at`.
///
/// Does not UPDATE `package.status` — that is a projection. Packed progress
/// reads status after the next rebuild (D100).
#[post("/packages/{id}/seal")]
pub async fn seal_package(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SealPackageRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let package_id = path.into_inner();
    let body = body.into_inner();
    let source = body
        .source
        .clone()
        .unwrap_or_else(|| "operator_scan".to_string());

    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let pkg_row = tx
                    .query_opt(
                        "SELECT id, tenant_id, status FROM package WHERE id = $1",
                        &[&package_id],
                    )
                    .await?;
                let pkg_state = pkg_row.as_ref().map(|r| despatching::PackageState {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    status: r.get(2),
                });

                let proposed = ProposedSeal {
                    tenant_id: tenant,
                    package_id,
                    source: source.clone(),
                };
                let problems = despatching::check_seal(&proposed, pkg_state.as_ref());
                let hard: Vec<String> = problems
                    .iter()
                    .filter(|p| despatching::seal_is_hard(p))
                    .map(|p| p.to_string())
                    .collect();
                if !hard.is_empty() {
                    return Err(ApiError::Rejected(hard.join("; ")));
                }
                let mut warnings: Vec<String> = problems
                    .iter()
                    .filter(|p| !despatching::seal_is_hard(p))
                    .map(|p| p.to_string())
                    .collect();

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;

                let event_id = match act {
                    ActInsert::Replay => {
                        let (id, prior_pkg, kind) =
                            client_events::require_one_package_event(tx, body.client_event_id)
                                .await?;
                        if kind != "sealed" || prior_pkg != package_id {
                            return Err(ApiError::Rejected(
                                "client_event already recorded a different package act".into(),
                            ));
                        }
                        warnings.push(client_events::REPLAY_WARNING.into());
                        id
                    }
                    ActInsert::Fresh => {
                        let id: Uuid = tx
                            .query_one(
                                "INSERT INTO package_event (
                                     tenant_id, client_event_id, package_id, kind, source,
                                     occurred_at, recorded_by_id)
                                 VALUES ($1, $2, $3, 'sealed', $4, $5, $6)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &body.client_event_id,
                                    &package_id,
                                    &source,
                                    &body.occurred_at,
                                    &who.person_id,
                                ],
                            )
                            .await?
                            .get(0);
                        // Freeze-time column the app may UPDATE (migration 9).
                        tx.execute(
                            "UPDATE package SET sealed_at = $1 WHERE id = $2 AND sealed_at IS NULL",
                            &[&body.occurred_at, &package_id],
                        )
                        .await?;
                        tx.execute(
                            "SELECT projection_mark_dirty($1, 'seal')",
                            &[&tenant],
                        )
                        .await?;
                        id
                    }
                };

                let package_status_ledger =
                    ledger_views::package_status_ledger(tx, package_id).await?;

                Ok(SealPackageResponse {
                    event_id,
                    warnings,
                    package_status_ledger,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Open a package (unseal for rework)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct OpenPackageRequest {
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub source: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct OpenPackageResponse {
    pub event_id: Uuid,
    pub warnings: Vec<String>,
    /// Winning package status from the event log (includes this open).
    pub package_status_ledger: Option<String>,
}

/// Record an open: one `package_event` (`opened`) and clear `sealed_at`.
///
/// Does not UPDATE `package.status` — that is a projection. Despatched/voided
/// packages are refused; opening something not projected as sealed is soft (D5).
#[post("/packages/{id}/open")]
pub async fn open_package(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<OpenPackageRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let package_id = path.into_inner();
    let body = body.into_inner();
    let source = body
        .source
        .clone()
        .unwrap_or_else(|| "operator_scan".to_string());

    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let pkg_row = tx
                    .query_opt(
                        "SELECT id, tenant_id, status FROM package WHERE id = $1",
                        &[&package_id],
                    )
                    .await?;
                let pkg_state = pkg_row.as_ref().map(|r| despatching::PackageState {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    status: r.get(2),
                });

                let proposed = ProposedOpen {
                    tenant_id: tenant,
                    package_id,
                    source: source.clone(),
                };
                let problems = despatching::check_open(&proposed, pkg_state.as_ref());
                let hard: Vec<String> = problems
                    .iter()
                    .filter(|p| despatching::open_is_hard(p))
                    .map(|p| p.to_string())
                    .collect();
                if !hard.is_empty() {
                    if hard.len() == 1
                        && matches!(
                            problems
                                .iter()
                                .find(|p| despatching::open_is_hard(p)),
                            Some(despatching::OpenProblem::PackageNotFound)
                        )
                    {
                        return Err(ApiError::NotFound);
                    }
                    return Err(ApiError::Rejected(hard.join("; ")));
                }
                let mut warnings: Vec<String> = problems
                    .iter()
                    .filter(|p| !despatching::open_is_hard(p))
                    .map(|p| p.to_string())
                    .collect();

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;

                let event_id = match act {
                    ActInsert::Replay => {
                        let (id, prior_pkg, kind) =
                            client_events::require_one_package_event(tx, body.client_event_id)
                                .await?;
                        if kind != "opened" || prior_pkg != package_id {
                            return Err(ApiError::Rejected(
                                "client_event already recorded a different package act".into(),
                            ));
                        }
                        warnings.push(client_events::REPLAY_WARNING.into());
                        id
                    }
                    ActInsert::Fresh => {
                        let id: Uuid = tx
                            .query_one(
                                "INSERT INTO package_event (
                                     tenant_id, client_event_id, package_id, kind, source,
                                     occurred_at, recorded_by_id)
                                 VALUES ($1, $2, $3, 'opened', $4, $5, $6)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &body.client_event_id,
                                    &package_id,
                                    &source,
                                    &body.occurred_at,
                                    &who.person_id,
                                ],
                            )
                            .await?
                            .get(0);
                        // Clear freeze-time stamp; status itself is a projection.
                        tx.execute(
                            "UPDATE package SET sealed_at = NULL WHERE id = $1",
                            &[&package_id],
                        )
                        .await?;
                        tx.execute(
                            "SELECT projection_mark_dirty($1, 'package_open')",
                            &[&tenant],
                        )
                        .await?;
                        id
                    }
                };

                let package_status_ledger =
                    ledger_views::package_status_ledger(tx, package_id).await?;

                Ok(OpenPackageResponse {
                    event_id,
                    warnings,
                    package_status_ledger,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Despatch a package (event + stock leaving)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct DespatchPackageRequest {
    pub fulfilment_line_id: Uuid,
    pub quantity: i64,
    /// Which cell in the package; if omitted, the sole cell for the line's item.
    pub from_stock_id: Option<Uuid>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub source: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct DespatchPackageResponse {
    pub event_id: Uuid,
    pub movement_id: Uuid,
    pub warnings: Vec<String>,
    /// Live progress for the line just despatched.
    pub ledger: Progress,
    pub projection: ProjectionProgress,
    /// Winning package status from the event log (includes this despatch).
    pub package_status_ledger: Option<String>,
}

/// Despatch: `package_event` despatched + `stock_movement` out of the package
/// with no `to` side (D99), naming the line. Never UPDATEs progress columns.
#[post("/packages/{id}/despatch")]
pub async fn despatch_package(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<DespatchPackageRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let package_id = path.into_inner();
    let body = body.into_inner();
    let source = body
        .source
        .clone()
        .unwrap_or_else(|| "operator_scan".to_string());

    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let pkg_row = tx
                    .query_opt(
                        "SELECT id, tenant_id, status FROM package WHERE id = $1",
                        &[&package_id],
                    )
                    .await?;
                let line_row = tx
                    .query_opt(
                        "SELECT fl.id, fl.tenant_id, ol.item_id, fl.quantity,
                                fl.despatched_quantity, (f.state = 'cancelled')
                           FROM fulfilment_line fl
                           JOIN fulfilment f ON f.id = fl.fulfilment_id
                           JOIN order_line ol ON ol.id = fl.order_line_id
                          WHERE fl.id = $1",
                        &[&body.fulfilment_line_id],
                    )
                    .await?;

                let pkg_state = pkg_row.as_ref().map(|r| despatching::PackageState {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    status: r.get(2),
                });
                let line = line_row.as_ref().map(|r| despatching::FulfilmentLine {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    item_id: r.get(2),
                    quantity: r.get(3),
                    despatched_quantity: r.get(4),
                    fulfilment_cancelled: r.get(5),
                });

                let item_id = line.as_ref().map(|l| l.item_id);
                let stock_row = if let Some(sid) = body.from_stock_id {
                    tx.query_opt(
                        "SELECT id, tenant_id, item_id, holder_package_id, lot_id,
                                status_id, owner_id, quantity
                           FROM stock WHERE id = $1",
                        &[&sid],
                    )
                    .await?
                } else if let Some(item_id) = item_id {
                    tx.query_opt(
                        "SELECT id, tenant_id, item_id, holder_package_id, lot_id,
                                status_id, owner_id, quantity
                           FROM stock
                          WHERE holder_package_id = $1 AND item_id = $2 AND quantity > 0
                          ORDER BY id
                          LIMIT 1",
                        &[&package_id, &item_id],
                    )
                    .await?
                } else {
                    None
                };

                let stock = stock_row.as_ref().map(|r| despatching::PackageStock {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    item_id: r.get(2),
                    holder_package_id: r.get(3),
                    lot_id: r.get(4),
                    status_id: r.get(5),
                    owner_id: r.get(6),
                    quantity: r.get(7),
                });

                let proposed = ProposedDespatch {
                    tenant_id: tenant,
                    package_id,
                    fulfilment_line_id: body.fulfilment_line_id,
                    quantity: body.quantity,
                    source: source.clone(),
                };
                let problems = despatching::check_despatch(
                    &proposed,
                    pkg_state.as_ref(),
                    line.as_ref(),
                    stock.as_ref(),
                );
                let hard: Vec<String> = problems
                    .iter()
                    .filter(|p| despatching::despatch_is_hard(p))
                    .map(|p| p.to_string())
                    .collect();
                if !hard.is_empty() {
                    return Err(ApiError::Rejected(hard.join("; ")));
                }
                let mut warnings: Vec<String> = problems
                    .iter()
                    .filter(|p| !despatching::despatch_is_hard(p))
                    .map(|p| p.to_string())
                    .collect();

                let stock = stock.expect("hard checks require stock");
                let line = line.expect("hard checks require a line");

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;

                let (event_id, movement_id) = match act {
                    ActInsert::Replay => {
                        let (eid, mid) =
                            client_events::require_despatch_facts(tx, body.client_event_id)
                                .await?;
                        let (_, qty) =
                            client_events::require_one_movement(tx, body.client_event_id)
                                .await?;
                        client_events::reject_quantity_mismatch(qty, body.quantity)?;
                        warnings.push(client_events::REPLAY_WARNING.into());
                        (eid, mid)
                    }
                    ActInsert::Fresh => {
                        let event_id: Uuid = tx
                            .query_one(
                                "INSERT INTO package_event (
                                     tenant_id, client_event_id, package_id, kind, source,
                                     occurred_at, recorded_by_id)
                                 VALUES ($1, $2, $3, 'despatched', $4, $5, $6)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &body.client_event_id,
                                    &package_id,
                                    &source,
                                    &body.occurred_at,
                                    &who.person_id,
                                ],
                            )
                            .await?
                            .get(0);

                        // From package, no to side — D99 despatch shape.
                        let movement_id: Uuid = tx
                            .query_one(
                                "INSERT INTO stock_movement (
                                     tenant_id, client_event_id, item_id, quantity,
                                     from_package_id, from_lot_id, from_status_id, from_owner_id,
                                     reason, occurred_at, recorded_by_id, fulfilment_line_id)
                                 VALUES (
                                     $1, $2, $3, $4,
                                     $5, $6, $7, $8,
                                     'despatch', $9, $10, $11)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &body.client_event_id,
                                    &stock.item_id,
                                    &body.quantity,
                                    &package_id,
                                    &stock.lot_id,
                                    &stock.status_id,
                                    &stock.owner_id,
                                    &body.occurred_at,
                                    &who.person_id,
                                    &body.fulfilment_line_id,
                                ],
                            )
                            .await?
                            .get(0);
                        tx.execute(
                            "SELECT projection_mark_dirty($1, 'despatch')",
                            &[&tenant],
                        )
                        .await?;
                        (event_id, movement_id)
                    }
                };
                let ledger = ledger_views::line_progress_ledger(
                    tx,
                    body.fulfilment_line_id,
                    line.quantity,
                )
                .await?;
                let proj_row = tx
                    .query_one(
                        "SELECT covered_quantity, picked_quantity, packed_quantity,
                                despatched_quantity, uncovered_quantity
                           FROM fulfilment_line WHERE id = $1",
                        &[&body.fulfilment_line_id],
                    )
                    .await?;
                let projection = ledger_views::line_progress_projection(
                    tx,
                    proj_row.get(0),
                    proj_row.get(1),
                    proj_row.get(2),
                    proj_row.get(3),
                    proj_row.get(4),
                )
                .await?;
                let package_status_ledger =
                    ledger_views::package_status_ledger(tx, package_id).await?;

                Ok(DespatchPackageResponse {
                    event_id,
                    movement_id,
                    warnings,
                    ledger,
                    projection,
                    package_status_ledger,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Allocate stock to a fulfilment line (intention)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct RecordAllocationRequest {
    /// Client-minted claim id (D5 offline, Q174). Generated server-side if
    /// omitted — but a caller that may retry should mint one, because it is the
    /// only thing that makes the retry a replay rather than a second claim.
    pub id: Option<Uuid>,
    pub fulfilment_line_id: Uuid,
    /// Location-held stock cell to claim.
    pub stock_id: Uuid,
    pub quantity: i64,
    /// When true the re-allocator may not steal this claim (D24). Default false.
    pub firm: Option<bool>,
    pub bound_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug)]
pub struct RecordAllocationResponse {
    pub allocation_id: Uuid,
    /// True when this request found the claim already there (Q174 replay).
    pub replayed: bool,
    pub warnings: Vec<String>,
    /// Live coverage after this claim (includes the new row; O(line)).
    pub ledger: Progress,
    /// Cached fulfilment_line columns (lag until maintainers run).
    pub projection: ProjectionProgress,
}

/// Claim location-held stock for a fulfilment line as one `stock_allocation`.
///
/// Does not UPDATE progress columns and does not move stock. Coverage is a fold
/// of allocations (J31); the scheduler rebuilds `covered_quantity` after dirty
/// mark. Directed only — the caller names the cell (question 26's "who" is still
/// open; this is the write once the decision is made).
///
/// **Retry-safe on a client-minted id (Q174).** This was the one write path with
/// no protection: a handheld that timed out and resubmitted wrote a second claim
/// and committed the cell twice, which drives `uncovered_quantity` negative and
/// then vanishes from the partial index that finds under-covered lines — right
/// for the index and exactly wrong as a way to find out. It takes no
/// `client_event`, because an allocation is an Intention and S19 asks facts for
/// one; what makes the retry safe is the identifier itself, which is the half of
/// D5 that was always doing the work.
#[post("/allocations")]
pub async fn record_allocation(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RecordAllocationRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    let firm = body.firm.unwrap_or(false);
    let bound_at = body.bound_at.unwrap_or_else(Utc::now);
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let line_row = tx
                    .query_opt(
                        // Covering set = allocating::COVERING / J31.
                        "SELECT fl.id, fl.tenant_id, ol.item_id, fl.quantity,
                                (f.state = 'cancelled'),
                                coalesce((
                                    SELECT sum(a.quantity)::bigint
                                      FROM stock_allocation a
                                     WHERE a.fulfilment_line_id = fl.id
                                       AND a.state IN (
                                           'allocated','picking','picked','packed','fulfilled')
                                ), 0)
                           FROM fulfilment_line fl
                           JOIN fulfilment f ON f.id = fl.fulfilment_id
                           JOIN order_line ol ON ol.id = fl.order_line_id
                          WHERE fl.id = $1",
                        &[&body.fulfilment_line_id],
                    )
                    .await?;
                let cell_row = tx
                    .query_opt(
                        "SELECT id, tenant_id, item_id, holder_location_id, holder_package_id,
                                quantity, available_quantity
                           FROM stock WHERE id = $1",
                        &[&body.stock_id],
                    )
                    .await?;

                let line = line_row.as_ref().map(|r| allocating::FulfilmentLine {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    item_id: r.get(2),
                    quantity: r.get(3),
                    fulfilment_cancelled: r.get(4),
                    covered_quantity: r.get(5),
                });
                let cell = cell_row.as_ref().map(|r| allocating::StockCell {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    item_id: r.get(2),
                    holder_location_id: r.get(3),
                    holder_package_id: r.get(4),
                    quantity: r.get(5),
                    available_quantity: r.get(6),
                });

                // **Replay is settled before the checks, not after.** The coverage
                // read above already counts the claim a retry is retrying, so
                // running `allocating::check` on the second submission compares
                // the request against a total that includes it and refuses the
                // line as over-covered. That is the receipt path's lesson one
                // table over: a validation that the first write already moved the
                // inputs of cannot also guard the replay.
                if let Some(id) = body.id {
                    if let Some(prior) =
                        client_events::prior_allocation(tx, tenant, id).await?
                    {
                        client_events::reject_allocation_mismatch(
                            &prior,
                            body.stock_id,
                            body.fulfilment_line_id,
                            body.quantity,
                        )?;
                        let Some(line) = line.as_ref() else {
                            return Err(ApiError::NotFound);
                        };
                        let ledger = ledger_views::line_progress_ledger(
                            tx,
                            body.fulfilment_line_id,
                            line.quantity,
                        )
                        .await?;
                        let proj_row = tx
                            .query_one(
                                "SELECT covered_quantity, picked_quantity, packed_quantity,
                                        despatched_quantity, uncovered_quantity
                                   FROM fulfilment_line WHERE id = $1",
                                &[&body.fulfilment_line_id],
                            )
                            .await?;
                        let projection = ledger_views::line_progress_projection(
                            tx,
                            proj_row.get(0),
                            proj_row.get(1),
                            proj_row.get(2),
                            proj_row.get(3),
                            proj_row.get(4),
                        )
                        .await?;
                        return Ok(RecordAllocationResponse {
                            allocation_id: prior.id,
                            replayed: true,
                            warnings: vec![
                                "replay of an existing allocation id; no second claim was \
                                 written"
                                    .into(),
                            ],
                            ledger,
                            projection,
                        });
                    }
                }

                let proposed = ProposedAllocation {
                    tenant_id: tenant,
                    fulfilment_line_id: body.fulfilment_line_id,
                    stock_id: body.stock_id,
                    quantity: body.quantity,
                    firm,
                    bound_at,
                };
                let problems = allocating::check(&proposed, line.as_ref(), cell.as_ref());
                let hard: Vec<String> = problems
                    .iter()
                    .filter(|p| allocating::is_hard(p))
                    .map(|p| p.to_string())
                    .collect();
                if !hard.is_empty() {
                    return Err(ApiError::Rejected(hard.join("; ")));
                }
                let warnings: Vec<String> = problems
                    .iter()
                    .filter(|p| !allocating::is_hard(p))
                    .map(|p| p.to_string())
                    .collect();

                let line = line.expect("hard checks require a line");

                let mut warnings = warnings;
                // Claim the id. Rows-affected decides replay from first write, so
                // two submissions of one claim serialize and the loser reads what
                // the winner wrote — `claim_act`'s mechanism without `claim_act`'s
                // envelope, which an Intention has no business carrying.
                let (allocation_id, replayed) = match body.id {
                    Some(id) => {
                        let inserted = tx
                            .execute(
                                "INSERT INTO stock_allocation (
                                     id, tenant_id, stock_id, fulfilment_line_id,
                                     quantity, state, firm, bound_at)
                                 VALUES ($1, $2, $3, $4, $5, 'allocated', $6, $7)
                                 ON CONFLICT (id) DO NOTHING",
                                &[
                                    &id,
                                    &tenant,
                                    &body.stock_id,
                                    &body.fulfilment_line_id,
                                    &body.quantity,
                                    &firm,
                                    &bound_at,
                                ],
                            )
                            .await?;
                        if inserted == 1 {
                            (id, false)
                        } else {
                            let Some(prior) =
                                client_events::prior_allocation(tx, tenant, id).await?
                            else {
                                return Err(ApiError::Rejected(
                                    "that allocation id is already in use and is not this                                      tenant's"
                                        .into(),
                                ));
                            };
                            client_events::reject_allocation_mismatch(
                                &prior,
                                body.stock_id,
                                body.fulfilment_line_id,
                                body.quantity,
                            )?;
                            warnings.push(
                                "replay of an existing allocation id; no second claim was                                  written"
                                    .into(),
                            );
                            (prior.id, true)
                        }
                    }
                    None => {
                        let id: Uuid = tx
                            .query_one(
                                "INSERT INTO stock_allocation (
                                     tenant_id, stock_id, fulfilment_line_id, quantity,
                                     state, firm, bound_at)
                                 VALUES ($1, $2, $3, $4, 'allocated', $5, $6)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &body.stock_id,
                                    &body.fulfilment_line_id,
                                    &body.quantity,
                                    &firm,
                                    &bound_at,
                                ],
                            )
                            .await?
                            .get(0);
                        (id, false)
                    }
                };

                let ledger = ledger_views::line_progress_ledger(
                    tx,
                    body.fulfilment_line_id,
                    line.quantity,
                )
                .await?;
                let proj_row = tx
                    .query_one(
                        "SELECT covered_quantity, picked_quantity, packed_quantity,
                                despatched_quantity, uncovered_quantity
                           FROM fulfilment_line WHERE id = $1",
                        &[&body.fulfilment_line_id],
                    )
                    .await?;
                let projection = ledger_views::line_progress_projection(
                    tx,
                    proj_row.get(0),
                    proj_row.get(1),
                    proj_row.get(2),
                    proj_row.get(3),
                    proj_row.get(4),
                )
                .await?;

                tx.execute(
                    "SELECT projection_mark_dirty($1, 'allocation')",
                    &[&tenant],
                )
                .await?;

                Ok(RecordAllocationResponse {
                    allocation_id,
                    replayed,
                    warnings,
                    ledger,
                    projection,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Release an allocation (withdraw a still-allocated claim)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Default)]
pub struct ReleaseAllocationRequest {
    /// Required when the claim is firm (D24).
    pub force: Option<bool>,
}

#[derive(Serialize, Debug)]
pub struct ReleaseAllocationResponse {
    pub allocation_id: Uuid,
    pub previous_state: String,
    pub warnings: Vec<String>,
    /// Live coverage after release (when the claim named a line).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ledger: Option<Progress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection: Option<ProjectionProgress>,
}

/// Withdraw a claim still in state `allocated` by setting `state = released`.
///
/// Does not DELETE the row (intention history). Only `allocated` is releasable
/// here; packed/picking/fulfilled need a different act. Firm claims require
/// `force: true`. Marks dirty so covered/available folds catch up.
#[post("/allocations/{id}/release")]
pub async fn release_allocation(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<ReleaseAllocationRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let allocation_id = path.into_inner();
    let force = body.force.unwrap_or(false);
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let row = tx
                    .query_opt(
                        "SELECT id, tenant_id, state, firm, fulfilment_line_id, quantity
                           FROM stock_allocation WHERE id = $1",
                        &[&allocation_id],
                    )
                    .await?;

                let allocation = row.as_ref().map(|r| allocating::Allocation {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    state: r.get(2),
                    firm: r.get(3),
                    fulfilment_line_id: r.get(4),
                    quantity: r.get(5),
                });

                let proposed = ProposedRelease {
                    tenant_id: tenant,
                    allocation_id,
                    force,
                };
                let (problems, soft) = allocating::check_release(&proposed, allocation.as_ref());
                if !problems.is_empty() {
                    // NotFound is 404; the rest are rejections.
                    if problems.len() == 1 && problems[0] == allocating::ReleaseProblem::NotFound {
                        return Err(ApiError::NotFound);
                    }
                    let detail = problems
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join("; ");
                    return Err(ApiError::Rejected(detail));
                }

                let allocation = allocation.expect("problems empty implies present");
                let previous_state = allocation.state.clone();

                let n = tx
                    .execute(
                        "UPDATE stock_allocation SET state = 'released' WHERE id = $1",
                        &[&allocation_id],
                    )
                    .await?;
                if n != 1 {
                    return Err(ApiError::NotFound);
                }

                let warnings: Vec<String> = soft.iter().map(|w| w.to_string()).collect();

                let (ledger, projection) = if let Some(line_id) = allocation.fulfilment_line_id {
                    let qty: i64 = tx
                        .query_one(
                            "SELECT quantity FROM fulfilment_line WHERE id = $1",
                            &[&line_id],
                        )
                        .await?
                        .get(0);
                    let ledger =
                        ledger_views::line_progress_ledger(tx, line_id, qty).await?;
                    let proj_row = tx
                        .query_one(
                            "SELECT covered_quantity, picked_quantity, packed_quantity,
                                    despatched_quantity, uncovered_quantity
                               FROM fulfilment_line WHERE id = $1",
                            &[&line_id],
                        )
                        .await?;
                    let projection = ledger_views::line_progress_projection(
                        tx,
                        proj_row.get(0),
                        proj_row.get(1),
                        proj_row.get(2),
                        proj_row.get(3),
                        proj_row.get(4),
                    )
                    .await?;
                    (Some(ledger), Some(projection))
                } else {
                    (None, None)
                };

                tx.execute(
                    "SELECT projection_mark_dirty($1, 'allocation_release')",
                    &[&tenant],
                )
                .await?;

                Ok(ReleaseAllocationResponse {
                    allocation_id,
                    previous_state,
                    warnings,
                    ledger,
                    projection,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Stock count (assertion; variance → finding, not ledger)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct RecordCountRequest {
    pub stock_id: Uuid,
    /// Absolute quantity counted in the cell (base units).
    pub counted_quantity: i64,
    pub client_event_id: Uuid,
    pub counted_at: DateTime<Utc>,
    pub blind: Option<bool>,
    /// D9: capture was challenged against system/last-move context.
    pub challenged: Option<bool>,
    pub challenge_context: Option<String>,
    /// Operator stood by the number after challenge.
    pub confirmed: Option<bool>,
}

#[derive(Serialize, Debug)]
pub struct RecordCountResponse {
    pub stock_count_id: Uuid,
    pub system_quantity: i64,
    pub counted_quantity: i64,
    pub variance: i64,
    /// Set when counted ≠ system (`count_variance` finding).
    pub discrepancy_id: Option<Uuid>,
}

/// Record a cycle count: INSERT `stock_count` only.
///
/// Never writes `stock_movement`. On variance, INSERT `discrepancy` kind
/// `count_variance` sourced by `stock_count_id` (D8). Resolve later with
/// `/adjustments` if the floor decides the ledger should move.
#[post("/counts")]
pub async fn record_count(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RecordCountRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    let blind = body.blind.unwrap_or(false);
    let challenged = body.challenged.unwrap_or(false);
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let cell_row = tx
                    .query_opt(
                        "SELECT id, tenant_id, item_id, holder_location_id, holder_package_id,
                                lot_id, status_id, owner_id, quantity
                           FROM stock WHERE id = $1",
                        &[&body.stock_id],
                    )
                    .await?;
                let cell = cell_row.as_ref().map(|r| counting::StockCell {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    item_id: r.get(2),
                    holder_location_id: r.get(3),
                    holder_package_id: r.get(4),
                    lot_id: r.get(5),
                    status_id: r.get(6),
                    owner_id: r.get(7),
                    quantity: r.get(8),
                });

                let proposed = ProposedCount {
                    tenant_id: tenant,
                    stock_id: body.stock_id,
                    counted_quantity: body.counted_quantity,
                    blind,
                    challenged,
                    challenge_context: body.challenge_context.clone(),
                    confirmed: body.confirmed,
                };
                let plan = match counting::check(&proposed, cell.as_ref()) {
                    Ok(p) => p,
                    Err(problems) => {
                        if problems.len() == 1
                            && matches!(problems[0], counting::Problem::CellNotFound)
                        {
                            return Err(ApiError::NotFound);
                        }
                        return Err(ApiError::Rejected(
                            problems
                                .iter()
                                .map(|p| p.to_string())
                                .collect::<Vec<_>>()
                                .join("; "),
                        ));
                    }
                };
                let cell = cell.expect("plan implies cell");

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.counted_at,
                    },
                )
                .await?;

                if act.is_replay() {
                    let (stock_count_id, system_quantity, counted_quantity, discrepancy_id) =
                        client_events::require_count_facts(tx, body.client_event_id).await?;
                    client_events::reject_quantity_mismatch(
                        counted_quantity,
                        body.counted_quantity,
                    )?;
                    return Ok(RecordCountResponse {
                        stock_count_id,
                        system_quantity,
                        counted_quantity,
                        variance: counted_quantity - system_quantity,
                        discrepancy_id,
                    });
                }

                // Bind via locals so Option/bool types match the working insert path.
                let stock_id = body.stock_id;
                let item_id = cell.item_id;
                let holder_location_id = cell.holder_location_id;
                let holder_package_id = cell.holder_package_id;
                let lot_id = cell.lot_id;
                let status_id = cell.status_id;
                let owner_id = cell.owner_id;
                let counted_quantity = plan.counted_quantity;
                let system_quantity = plan.system_quantity;
                let counted_at = body.counted_at;
                let client_event_id = body.client_event_id;
                let recorded_by_id = who.person_id;
                let challenge_context = body.challenge_context.clone();
                let confirmed = body.confirmed;

                let stock_count_id: Uuid = tx
                    .query_one(
                        "INSERT INTO stock_count (
                             tenant_id, stock_id, item_id,
                             holder_location_id, holder_package_id, lot_id,
                             status_id, owner_id,
                             counted_quantity, system_quantity,
                             counted_at, client_event_id, recorded_by_id,
                             blind, challenged, challenge_context, confirmed)
                         VALUES (
                             $1, $2, $3,
                             $4, $5, $6, $7, $8,
                             $9, $10,
                             $11, $12, $13,
                             $14, $15, $16, $17)
                         RETURNING id",
                        &[
                            &tenant,
                            &stock_id,
                            &item_id,
                            &holder_location_id,
                            &holder_package_id,
                            &lot_id,
                            &status_id,
                            &owner_id,
                            &counted_quantity,
                            &system_quantity,
                            &counted_at,
                            &client_event_id,
                            &recorded_by_id,
                            &blind,
                            &challenged,
                            &challenge_context,
                            &confirmed,
                        ],
                    )
                    .await
                    .map_err(|e| {
                        tracing::error!(error = %e, "stock_count insert failed");
                        e
                    })?
                    .get(0);

                let discrepancy_id = if plan.raise_variance {
                    let detail = format!(
                        "count variance: system {} counted {} (delta {})",
                        system_quantity, counted_quantity, plan.variance
                    );
                    // Quantities are i64 from our plan — embed in SQL as numeric
                    // literals so we do not depend on a NUMERIC ToSql impl.
                    let sql = format!(
                        "INSERT INTO discrepancy (
                             tenant_id, kind,
                             item_id, holder_location_id, holder_package_id,
                             lot_id, status_id, owner_id,
                             expected_quantity, observed_quantity,
                             stock_count_id, detail,
                             detected_at, detected_by_id, state)
                         VALUES (
                             $1, 'count_variance',
                             $2, $3, $4, $5, $6, $7,
                             {system_quantity}::numeric, {counted_quantity}::numeric,
                             $8, $9,
                             $10, $11, 'open')
                         RETURNING id"
                    );
                    let id: Uuid = tx
                        .query_one(
                            &sql,
                            &[
                                &tenant,
                                &item_id,
                                &holder_location_id,
                                &holder_package_id,
                                &lot_id,
                                &status_id,
                                &owner_id,
                                &stock_count_id,
                                &detail,
                                &counted_at,
                                &recorded_by_id,
                            ],
                        )
                        .await
                        .map_err(|e| {
                            tracing::error!(error = %e, "discrepancy insert failed");
                            e
                        })?
                        .get(0);
                    Some(id)
                } else {
                    None
                };

                Ok(RecordCountResponse {
                    stock_count_id,
                    system_quantity: plan.system_quantity,
                    counted_quantity: plan.counted_quantity,
                    variance: plan.variance,
                    discrepancy_id,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Inventory adjust from a cycle count (world-event movement)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct RecordAdjustmentRequest {
    /// Stock cell being counted / adjusted.
    pub stock_id: Uuid,
    /// Absolute quantity the operator counted in the cell (base units).
    pub counted_quantity: i64,
    /// Must be class `world_event` (found, damaged, …). Record errors use /corrections.
    pub adjustment_reason_id: Uuid,
    /// Open `count_variance` (or other) finding this movement resolves (D8).
    pub discrepancy_id: Option<Uuid>,
    /// Alternate to `discrepancy_id`: resolve the open variance for this count.
    pub stock_count_id: Option<Uuid>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct RecordAdjustmentResponse {
    /// Set when a movement was written; null when count matched system (soft no-op).
    pub movement_id: Option<Uuid>,
    pub system_quantity: i64,
    pub counted_quantity: i64,
    /// counted − system (positive = stock found).
    pub delta: i64,
    /// Finding closed with this movement, when one was linked.
    pub resolved_discrepancy_id: Option<Uuid>,
    pub warnings: Vec<String>,
}

/// Cycle-count style adjust: absolute count on a cell → signed `adjustment`
/// movement for the variance.
///
/// Does not reverse a prior movement (that is `/corrections`). Does not UPDATE
/// `stock.quantity` — the fold does after rebuild. Requires a `world_event`
/// `adjustment_reason`. Prefer `/counts` first (D8); pass `discrepancy_id` (or
/// `stock_count_id`) so the finding gets `resolving_movement_id` and state
/// `resolved`.
#[post("/adjustments")]
pub async fn record_adjustment(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RecordAdjustmentRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let cell_row = tx
                    .query_opt(
                        "SELECT id, tenant_id, item_id, holder_location_id, holder_package_id,
                                lot_id, status_id, owner_id, quantity
                           FROM stock WHERE id = $1",
                        &[&body.stock_id],
                    )
                    .await?;
                let reason_row = tx
                    .query_opt(
                        "SELECT id, class::text, code FROM adjustment_reason WHERE id = $1",
                        &[&body.adjustment_reason_id],
                    )
                    .await?;

                // Resolve which finding to close: explicit id, or open variance on stock_count.
                let finding_row = if let Some(did) = body.discrepancy_id {
                    tx.query_opt(
                        "SELECT d.id, d.tenant_id, d.kind::text, d.state::text, d.stock_count_id,
                                sc.stock_id
                           FROM discrepancy d
                           LEFT JOIN stock_count sc ON sc.id = d.stock_count_id
                          WHERE d.id = $1",
                        &[&did],
                    )
                    .await?
                } else if let Some(scid) = body.stock_count_id {
                    tx.query_opt(
                        "SELECT d.id, d.tenant_id, d.kind::text, d.state::text, d.stock_count_id,
                                sc.stock_id
                           FROM discrepancy d
                           JOIN stock_count sc ON sc.id = d.stock_count_id
                          WHERE d.stock_count_id = $1
                            AND d.kind = 'count_variance'
                            AND d.state IN ('open', 'investigating')
                          ORDER BY d.detected_at DESC
                          LIMIT 1",
                        &[&scid],
                    )
                    .await?
                } else {
                    None
                };

                let cell = cell_row.as_ref().map(|r| adjusting::StockCell {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    item_id: r.get(2),
                    holder_location_id: r.get(3),
                    holder_package_id: r.get(4),
                    lot_id: r.get(5),
                    status_id: r.get(6),
                    owner_id: r.get(7),
                    quantity: r.get(8),
                });
                let Some(reason_r) = reason_row.as_ref() else {
                    return Err(ApiError::Rejected(
                        adjusting::Problem::MissingReason.to_string(),
                    ));
                };
                let reason = adjusting::AdjustmentReason {
                    id: reason_r.get(0),
                    class: reason_r.get(1),
                    code: reason_r.get(2),
                };

                let finding = finding_row.as_ref().map(|r| adjusting::Finding {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    kind: r.get(2),
                    state: r.get(3),
                    stock_count_id: r.get(4),
                    count_stock_id: r.get(5),
                });

                let discrepancy_id = body
                    .discrepancy_id
                    .or_else(|| finding.as_ref().map(|f| f.id));

                let proposed = ProposedAdjustment {
                    tenant_id: tenant,
                    stock_id: body.stock_id,
                    counted_quantity: body.counted_quantity,
                    reason,
                    discrepancy_id,
                };
                let (problems, direction) =
                    adjusting::check(&proposed, cell.as_ref(), finding.as_ref());
                let hard: Vec<String> = problems
                    .iter()
                    .filter(|p| adjusting::is_hard(p))
                    .map(|p| p.to_string())
                    .collect();
                if !hard.is_empty() {
                    if hard.len() == 1
                        && problems.iter().any(|p| {
                            matches!(
                                p,
                                adjusting::Problem::CellNotFound
                                    | adjusting::Problem::FindingNotFound
                            )
                        })
                    {
                        return Err(ApiError::NotFound);
                    }
                    return Err(ApiError::Rejected(hard.join("; ")));
                }
                let warnings: Vec<String> = problems
                    .iter()
                    .filter(|p| !adjusting::is_hard(p))
                    .map(|p| p.to_string())
                    .collect();

                let cell = cell.expect("hard checks require a cell");
                let system_quantity = cell.quantity;
                let delta = body.counted_quantity - system_quantity;

                // Zero-variance soft path: no envelope, because a count that
                // agrees with the system records nothing durable. Non-zero:
                // claim the act before writing a movement.
                //
                // **The two paths are not independent, and the first HTTP test
                // of this endpoint is what showed it.** An adjustment that
                // lands changes the very quantity the delta is computed from,
                // so the *same act sent twice* arrives the second time with
                // `counted == system` and takes the zero-variance path — which
                // never reaches the claim, and so never reaches the replay
                // branch written for exactly this case. The endpoint answered
                // `movement_id: null` and *"no adjustment movement"* about an
                // act that had moved the ledger a second earlier.
                //
                // Nothing was corrupted by it: the second attempt writes
                // nothing, which is the outcome that matters. What was wrong
                // was the answer, and the answer is what a retrying client
                // reads — newly so, because until `domain/acts.ts` a retry
                // carried a fresh id and was never a replay at all.
                let mut warnings = warnings;
                let movement_id = if let Some(dir) = direction {
                    let act = client_events::claim_act(
                        tx,
                        &NewClientEvent {
                            tenant_id: tenant,
                            client_event_id: body.client_event_id,
                            site_id: who.site_id,
                            recorded_by_id: who.person_id,
                            submitted_at: body.occurred_at,
                        },
                    )
                    .await?;

                    if act.is_replay() {
                        let prior = client_events::optional_one_movement(tx, body.client_event_id)
                            .await?;
                        warnings.push(client_events::REPLAY_WARNING.into());
                        let movement_id = prior.map(|(id, _)| id);
                        // Finding already resolved on first success — do not re-resolve.
                        let resolved_discrepancy_id = if let Some(f) = finding.as_ref() {
                            tx.query_opt(
                                "SELECT id FROM discrepancy
                                  WHERE id = $1 AND resolving_movement_id IS NOT NULL",
                                &[&f.id],
                            )
                            .await?
                            .map(|r| r.get(0))
                        } else {
                            None
                        };
                        return Ok(RecordAdjustmentResponse {
                            movement_id,
                            system_quantity,
                            counted_quantity: body.counted_quantity,
                            delta,
                            resolved_discrepancy_id,
                            warnings,
                        });
                    }

                    let id: Uuid = match dir {
                        Direction::Increase { quantity } => tx
                            .query_one(
                                "INSERT INTO stock_movement (
                                     tenant_id, client_event_id, item_id, quantity,
                                     to_location_id, to_package_id, to_lot_id,
                                     to_status_id, to_owner_id,
                                     reason, occurred_at, recorded_by_id,
                                     adjustment_reason_id)
                                 VALUES (
                                     $1, $2, $3, $4,
                                     $5, $6, $7, $8, $9,
                                     'adjustment', $10, $11, $12)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &body.client_event_id,
                                    &cell.item_id,
                                    &quantity,
                                    &cell.holder_location_id,
                                    &cell.holder_package_id,
                                    &cell.lot_id,
                                    &cell.status_id,
                                    &cell.owner_id,
                                    &body.occurred_at,
                                    &who.person_id,
                                    &body.adjustment_reason_id,
                                ],
                            )
                            .await?
                            .get(0),
                        Direction::Decrease { quantity } => tx
                            .query_one(
                                "INSERT INTO stock_movement (
                                     tenant_id, client_event_id, item_id, quantity,
                                     from_location_id, from_package_id, from_lot_id,
                                     from_status_id, from_owner_id,
                                     reason, occurred_at, recorded_by_id,
                                     adjustment_reason_id)
                                 VALUES (
                                     $1, $2, $3, $4,
                                     $5, $6, $7, $8, $9,
                                     'adjustment', $10, $11, $12)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &body.client_event_id,
                                    &cell.item_id,
                                    &quantity,
                                    &cell.holder_location_id,
                                    &cell.holder_package_id,
                                    &cell.lot_id,
                                    &cell.status_id,
                                    &cell.owner_id,
                                    &body.occurred_at,
                                    &who.person_id,
                                    &body.adjustment_reason_id,
                                ],
                            )
                            .await?
                            .get(0),
                    };

                    tx.execute(
                        "SELECT projection_mark_dirty($1, 'adjustment')",
                        &[&tenant],
                    )
                    .await?;
                    Some(id)
                } else {
                    // **A zero variance can still be a replay**, and it is
                    // the commonest one there is: the act that made the
                    // variance zero was this one. Read before answering.
                    match client_events::optional_one_movement(tx, body.client_event_id).await? {
                        Some((id, _)) => {
                            warnings.push(client_events::REPLAY_WARNING.into());
                            Some(id)
                        }
                        None => None,
                    }
                };

                // Close the finding only when a movement was written.
                let resolved_discrepancy_id =
                    if let (Some(mid), Some(f)) = (movement_id, finding.as_ref()) {
                        tx.execute(
                            "UPDATE discrepancy
                                SET state = 'resolved',
                                    resolved_at = $1,
                                    resolved_by_id = $2,
                                    resolving_movement_id = $3,
                                    resolution_reason = $4
                              WHERE id = $5
                                AND state IN ('open', 'investigating')",
                            &[
                                &body.occurred_at,
                                &who.person_id,
                                &mid,
                                &format!(
                                    "inventory adjust delta {} (counted {} vs system {})",
                                    delta, body.counted_quantity, system_quantity
                                ),
                                &f.id,
                            ],
                        )
                        .await?;
                        Some(f.id)
                    } else {
                        None
                    };

                Ok(RecordAdjustmentResponse {
                    movement_id,
                    system_quantity,
                    counted_quantity: body.counted_quantity,
                    delta,
                    resolved_discrepancy_id,
                    warnings,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Record a correction (reverse a prior movement)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct RecordCorrectionRequest {
    /// Movement being reversed.
    pub reverses_movement_id: Uuid,
    /// Units to reverse (must not exceed the target).
    pub quantity: i64,
    /// Platform or tenant adjustment_reason (must be class record_error).
    pub adjustment_reason_id: Uuid,
    pub client_event_id: Uuid,
}

#[derive(Serialize, Debug)]
pub struct RecordCorrectionResponse {
    pub movement_id: Uuid,
    pub warnings: Vec<String>,
}

/// Insert a `record_error` correction: one `stock_movement` that mirrors and
/// reverses a prior row. Never UPDATEs the target. Discovery time is
/// `recorded_at` (default now); `occurred_at` is taken from the target (J50).
#[post("/corrections")]
pub async fn record_correction(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RecordCorrectionRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let target_row = tx
                    .query_opt(
                        "SELECT id, tenant_id, item_id, quantity, occurred_at,
                                from_location_id, from_package_id, from_lot_id,
                                from_status_id, from_owner_id,
                                to_location_id, to_package_id, to_lot_id,
                                to_status_id, to_owner_id
                           FROM stock_movement WHERE id = $1",
                        &[&body.reverses_movement_id],
                    )
                    .await?;

                let reason_row = tx
                    .query_opt(
                        // `class::text`: it is a `revision_class` enum, and
                        // reading one as a String panics the handler. The
                        // adjustments path a few hundred lines up casts; this one
                        // did not, so `POST /corrections` failed on every call it
                        // ever received — which was none until the bench needed
                        // to take units back out of a carton.
                        "SELECT class::text FROM adjustment_reason WHERE id = $1",
                        &[&body.adjustment_reason_id],
                    )
                    .await?;

                let target = target_row.as_ref().map(|r| correction::Movement {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    item_id: r.get(2),
                    quantity: r.get(3),
                    occurred_at: r.get(4),
                    from: Side {
                        location_id: r.get(5),
                        package_id: r.get(6),
                        lot_id: r.get(7),
                        status_id: r.get(8),
                        owner_id: r.get(9),
                    },
                    to: Side {
                        location_id: r.get(10),
                        package_id: r.get(11),
                        lot_id: r.get(12),
                        status_id: r.get(13),
                        owner_id: r.get(14),
                    },
                });

                let reason_class = reason_row.as_ref().and_then(|r| {
                    let class: String = r.get(0);
                    match class.as_str() {
                        "record_error" => Some(RevisionClass::RecordError),
                        "world_event" => Some(RevisionClass::WorldEvent),
                        _ => None,
                    }
                });

                let Some(target) = target.as_ref() else {
                    return Err(ApiError::NotFound);
                };

                // Mirror: reverse runs the other way.
                let proposed = ProposedCorrection {
                    tenant_id: tenant,
                    item_id: target.item_id,
                    quantity: body.quantity,
                    occurred_at: target.occurred_at,
                    from: target.to,
                    to: target.from,
                    reverses_movement_id: Some(body.reverses_movement_id),
                    reason_class,
                };
                let problems = correction::check(&proposed, Some(target));
                if !problems.is_empty() {
                    let detail = problems
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join("; ");
                    return Err(ApiError::Rejected(detail));
                }

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: Utc::now(),
                    },
                )
                .await?;

                if act.is_replay() {
                    let (movement_id, qty) =
                        client_events::require_one_movement(tx, body.client_event_id).await?;
                    client_events::reject_quantity_mismatch(qty, body.quantity)?;
                    return Ok(RecordCorrectionResponse {
                        movement_id,
                        warnings: vec![client_events::REPLAY_WARNING.into()],
                    });
                }

                let movement_id: Uuid = tx
                    .query_one(
                        "INSERT INTO stock_movement (
                             tenant_id, client_event_id, item_id, quantity,
                             from_location_id, from_package_id, from_lot_id,
                             from_status_id, from_owner_id,
                             to_location_id, to_package_id, to_lot_id,
                             to_status_id, to_owner_id,
                             reason, occurred_at, recorded_by_id,
                             reverses_movement_id, adjustment_reason_id)
                         VALUES (
                             $1, $2, $3, $4,
                             $5, $6, $7, $8, $9,
                             $10, $11, $12, $13, $14,
                             'adjustment', $15, $16, $17, $18)
                         RETURNING id",
                        &[
                            &tenant,
                            &body.client_event_id,
                            &proposed.item_id,
                            &body.quantity,
                            &proposed.from.location_id,
                            &proposed.from.package_id,
                            &proposed.from.lot_id,
                            &proposed.from.status_id,
                            &proposed.from.owner_id,
                            &proposed.to.location_id,
                            &proposed.to.package_id,
                            &proposed.to.lot_id,
                            &proposed.to.status_id,
                            &proposed.to.owner_id,
                            &proposed.occurred_at,
                            &who.person_id,
                            &body.reverses_movement_id,
                            &body.adjustment_reason_id,
                        ],
                    )
                    .await?
                    .get(0);

                tx.execute(
                    "SELECT projection_mark_dirty($1, 'correction')",
                    &[&tenant],
                )
                .await?;

                Ok(RecordCorrectionResponse {
                    movement_id,
                    warnings: vec![],
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Record a receipt against an expected supply promise
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct RecordReceiptRequest {
    pub expected_supply_id: Uuid,
    /// Dock or bin the goods land in.
    pub to_location_id: Uuid,
    /// Base units (eaches). Omit when counting cartons/inners via `entered_*`.
    pub quantity: Option<i64>,
    /// Count as entered at the dock (Q173 / D92). With level (+ config when not
    /// `each`) converts to base `quantity`.
    pub entered_quantity: Option<i64>,
    /// `each` | `inner` | `carton` | `layer` | `pallet`.
    pub entered_packaging_level: Option<String>,
    /// Required when level is not `each`. Must be for this item and already
    /// effective (J57).
    pub item_packing_config_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    /// **The lot as it is written on the carton**, when the receiver has the
    /// label and not a `lot_id` — which is every receiver, because nothing
    /// creates a `lot` row and no screen could hand one over.
    ///
    /// Found or created against `UNIQUE (tenant_id, item_id, code)`, which is
    /// what makes a retry safe: the second attempt finds what the first made.
    /// Ignored when `lot_id` is given.
    pub lot_code: Option<String>,
    /// The expiry off the same label. A GS1-128 carries it beside the lot, so
    /// one scan answers both and neither is typed.
    ///
    /// **It fills a blank and never overwrites.** A stated date that disagrees
    /// with the one on file is a disagreement, and D8 says those surface rather
    /// than being resolved by whoever wrote last — so the receipt is recorded,
    /// the held date stands, and the conflict comes back in `warnings`.
    pub lot_expiry: Option<chrono::NaiveDate>,
    /// Defaults to the promise's status / available if omitted.
    pub status_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub purchase_order_id: Option<Uuid>,
    /// When true, close the promise after this receipt (short if under-delivered).
    pub close_promise: Option<bool>,
    /// Delivery header (D43 / Q172). Omit to open a new one-line receipt.
    /// Pass a client-minted id on the first line of a multi-line truck, then the
    /// same id on later lines so they share one `goods_receipt`.
    pub goods_receipt_id: Option<Uuid>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct RecordReceiptResponse {
    pub goods_receipt_id: Uuid,
    pub goods_receipt_line_id: Uuid,
    /// True when this act inserted the header (first line of the delivery).
    pub header_created: bool,
    /// Canonical base units written (after packaging conversion).
    pub quantity: i64,
    pub entered_quantity: i64,
    pub entered_packaging_level: String,
    pub item_packing_config_id: Option<Uuid>,
    /// Null when the line was refused (e.g. `lot_missing`).
    pub movement_id: Option<Uuid>,
    pub stock_id: Option<Uuid>,
    pub repointed_allocation_ids: Vec<Uuid>,
    /// Version that governed the disposition (J63).
    pub receiving_policy_id: Option<Uuid>,
    pub accepted: bool,
    /// Finding raised by disposition (`over_receipt`, `lot_missing`, …).
    pub discrepancy_id: Option<Uuid>,
    pub warnings: Vec<String>,
}

/// Find the lot this code names, or make it.
///
/// **Found or made, never edited.** `UNIQUE (tenant_id, item_id, code)` is what
/// makes both halves safe: a replayed receipt finds what the first attempt made
/// rather than raising, and two receivers working one truck cannot make two rows
/// for one lot.
///
/// The expiry is the interesting half. A blank on file is filled — the first
/// receipt to carry a date is better than no date, and refusing to fill it would
/// leave a lot nobody can apply a shelf-life rule to. A date that disagrees is
/// **not** overwritten: the held date may be the one the recall will be run
/// against, and letting whoever received last decide is exactly the silent
/// resolution D8 built the findings queue to prevent. It comes back as a warning
/// instead, which is honest and visible and does not stop the goods coming in.
async fn find_or_make_lot(
    tx: &tokio_postgres::Transaction<'_>,
    tenant: Uuid,
    item_id: Uuid,
    code: &str,
    expiry: Option<chrono::NaiveDate>,
    warnings: &mut Vec<String>,
) -> Result<Uuid, ApiError> {
    if let Some(row) = tx
        .query_opt(
            "SELECT id, expiry_date FROM lot
              WHERE tenant_id = $1 AND item_id = $2 AND code = $3",
            &[&tenant, &item_id, &code.to_string()],
        )
        .await?
    {
        let id: Uuid = row.get(0);
        let held: Option<chrono::NaiveDate> = row.get(1);
        match (held, expiry) {
            (None, Some(stated)) => {
                tx.execute(
                    "UPDATE lot SET expiry_date = $2 WHERE id = $1",
                    &[&id, &stated],
                )
                .await?;
            }
            (Some(held), Some(stated)) if held != stated => {
                warnings.push(format!(
                    "lot {code} is on file with expiry {held}; this delivery says \
                     {stated}. The held date stands."
                ));
            }
            _ => {}
        }
        return Ok(id);
    }

    let id = Uuid::now_v7();
    tx.execute(
        "INSERT INTO lot (id, tenant_id, item_id, code, expiry_date)
         VALUES ($1, $2, $3, $4, $5)",
        &[&id, &tenant, &item_id, &code.to_string(), &expiry],
    )
    .await?;
    Ok(id)
}

/// Record one line of a goods receipt (D43: one header per delivery per demand
/// document).
///
/// Flow: claim act → (replay or) resolve policy → disposition → ensure header →
/// line → [`goods_receipt_line_dispose`] → optional movement → claim handover.
///
/// Does **not** stamp `accepted_at` directly (mediated write, D89). Pass
/// [`RecordReceiptRequest::goods_receipt_id`] to join lines on one delivery.
#[post("/receipts")]
pub async fn record_receipt(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RecordReceiptRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let supply_row = tx
                    .query_opt(
                        "SELECT es.id, es.tenant_id, es.item_id, es.site_id, es.owner_id,
                                es.status_id, es.quantity_expected, es.quantity_received,
                                es.closed_at, es.purchase_order_line_id,
                                po.supplier_party_id
                           FROM expected_supply es
                           JOIN purchase_order_line pol ON pol.id = es.purchase_order_line_id
                           JOIN purchase_order po ON po.id = pol.purchase_order_id
                          WHERE es.id = $1",
                        &[&body.expected_supply_id],
                    )
                    .await?;
                let Some(supply) = supply_row.as_ref() else {
                    return Err(ApiError::NotFound);
                };
                let supply_tenant: Uuid = supply.get(1);
                if supply_tenant != tenant {
                    return Err(ApiError::NotFound);
                }
                let item_id: Uuid = supply.get(2);
                let supply_site: Option<Uuid> = supply.get(3);
                let supply_owner: Option<Uuid> = supply.get(4);
                let supply_status: Option<Uuid> = supply.get(5);
                let closed_at: Option<DateTime<Utc>> = supply.get(8);
                let supplier_party_id: Option<Uuid> = supply.get(10);

                // Load packing config when named (Q173); conversion is pure.
                let packing_cfg = if let Some(cid) = body.item_packing_config_id {
                    let row = tx
                        .query_opt(
                            "SELECT id, item_id, units_per_inner, inners_per_carton,
                                    cartons_per_layer, layers_per_pallet, effective_from
                               FROM item_packing_config WHERE id = $1",
                            &[&cid],
                        )
                        .await?;
                    let Some(r) = row else {
                        return Err(ApiError::Rejected(
                            "item_packing_config_id was not found".into(),
                        ));
                    };
                    let effective_from: chrono::NaiveDate = r.get(6);
                    let act_day = body.occurred_at.date_naive();
                    if effective_from > act_day {
                        return Err(ApiError::Rejected(
                            "item_packing_config is not yet effective at the act time".into(),
                        ));
                    }
                    Some(receiving::PackingConfig {
                        id: r.get(0),
                        item_id: r.get(1),
                        units_per_inner: r.get(2),
                        inners_per_carton: r.get(3),
                        cartons_per_layer: r.get(4),
                        layers_per_pallet: r.get(5),
                    })
                } else {
                    None
                };

                let (base_quantity, entered) = receiving::resolve_count(
                    body.quantity,
                    body.entered_quantity,
                    body.entered_packaging_level.as_deref(),
                    packing_cfg.as_ref(),
                    item_id,
                )
                .map_err(|e| ApiError::Rejected(e.to_string()))?;
                if base_quantity <= 0 {
                    return Err(ApiError::Rejected(
                        receiving::Problem::NothingArrived.to_string(),
                    ));
                }
                let entered_level = entered.level.as_str().to_string();
                let entered_qty = entered.entered_quantity;
                let packing_config_id = entered.config_id;

                let loc_row = tx
                    .query_opt(
                        "SELECT id, tenant_id FROM location WHERE id = $1",
                        &[&body.to_location_id],
                    )
                    .await?;
                let Some(loc) = loc_row.as_ref() else {
                    return Err(ApiError::Rejected("destination location not found".into()));
                };
                let loc_tenant: Uuid = loc.get(1);
                if loc_tenant != tenant {
                    return Err(ApiError::Rejected("destination location not found".into()));
                }

                // Status/owner: request, then promise defaults, then platform available + owner from supply.
                let status_id: Uuid = if let Some(s) = body.status_id {
                    s
                } else if let Some(s) = supply_status {
                    s
                } else {
                    tx.query_one(
                        "SELECT id FROM inventory_status WHERE code = 'available' AND tenant_id IS NULL",
                        &[],
                    )
                    .await?
                    .get(0)
                };
                let owner_id: Uuid = if let Some(o) = body.owner_id {
                    o
                } else if let Some(o) = supply_owner {
                    o
                } else {
                    return Err(ApiError::Rejected(
                        "owner_id is required when the promise names none".into(),
                    ));
                };

                let po_id: Option<Uuid> = if body.purchase_order_id.is_some() {
                    body.purchase_order_id
                } else {
                    let pol: Uuid = supply.get(9);
                    tx.query_opt(
                        "SELECT purchase_order_id FROM purchase_order_line WHERE id = $1",
                        &[&pol],
                    )
                    .await?
                    .map(|r| r.get(0))
                };

                let expected_qty: i64 = supply.get(6);
                let site_for_policy = supply_site.or(who.site_id);

                // **Claim the act before resolving anything.** The resolution is
                // discarded on a replay — the response reads the governing
                // version back off the stored line — but its failure paths were
                // not discarded, so a binding retired since the original act
                // turned an idempotent retry into a 400. D5's retry is a device
                // on a bad network, and it may not depend on configuration
                // holding still in between.
                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;

                if act.is_replay() {
                    let (
                        goods_receipt_id,
                        goods_receipt_line_id,
                        movement_id,
                        movement_quantity,
                        receiving_policy_id,
                        accepted,
                    ) = client_events::require_receipt_facts(tx, body.client_event_id).await?;
                    if let Some(qty) = movement_quantity {
                        client_events::reject_quantity_mismatch(qty, base_quantity)?;
                    }
                    let stock_id: Option<Uuid> = tx
                        .query_opt(
                            "SELECT id FROM stock
                              WHERE holder_location_id = $1 AND item_id = $2
                              ORDER BY id LIMIT 1",
                            &[&body.to_location_id, &item_id],
                        )
                        .await?
                        .map(|r| r.get(0));
                    let discrepancy_id: Option<Uuid> = tx
                        .query_opt(
                            "SELECT id FROM discrepancy
                              WHERE goods_receipt_line_id = $1
                              ORDER BY detected_at, id LIMIT 1",
                            &[&goods_receipt_line_id],
                        )
                        .await?
                        .map(|r| r.get(0));
                    let line_form = tx
                        .query_one(
                            "SELECT quantity, entered_quantity, entered_packaging_level::text,
                                    item_packing_config_id
                               FROM goods_receipt_line WHERE id = $1",
                            &[&goods_receipt_line_id],
                        )
                        .await?;
                    let qty: i64 = line_form.get(0);
                    let ent: i64 = line_form.get(1);
                    let lvl: String = line_form.get(2);
                    let cfg: Option<Uuid> = line_form.get(3);
                    // Answer for the act being replayed, not for this call. A
                    // retry of the line that opened the delivery still opened it,
                    // and an idempotent endpoint that changes its answer on the
                    // second ask is not one.
                    let header_created: bool = tx
                        .query_one(
                            "SELECT client_event_id = $2 FROM goods_receipt WHERE id = $1",
                            &[&goods_receipt_id, &body.client_event_id],
                        )
                        .await?
                        .get(0);
                    return Ok(RecordReceiptResponse {
                        goods_receipt_id,
                        goods_receipt_line_id,
                        header_created,
                        quantity: qty,
                        entered_quantity: ent,
                        entered_packaging_level: lvl,
                        item_packing_config_id: cfg,
                        movement_id,
                        stock_id,
                        repointed_allocation_ids: vec![],
                        receiving_policy_id,
                        accepted,
                        discrepancy_id,
                        warnings: vec![client_events::REPLAY_WARNING.into()],
                    });
                }

                // **The lot, before the disposition needs to know there is
                // one.** `disposition` refuses a line whose policy requires a
                // lot and has none, so resolving the code into a row has to
                // happen before that question is asked — and it is a fact about
                // the goods rather than a decision about them, which is why it
                // sits ahead of D89's "policy before facts" line rather than
                // breaking it.
                let mut lot_warnings: Vec<String> = Vec::new();
                let lot_id = match body.lot_id {
                    Some(id) => Some(id),
                    None => match body.lot_code.as_deref().map(str::trim) {
                        None | Some("") => None,
                        Some(code) => Some(
                            find_or_make_lot(
                                tx,
                                tenant,
                                item_id,
                                code,
                                body.lot_expiry,
                                &mut lot_warnings,
                            )
                            .await?,
                        ),
                    },
                };

                // Policy → disposition before any fact write (D89).
                let resolved = receiving::resolve_receiving_policy(
                    tx,
                    tenant,
                    item_id,
                    supplier_party_id,
                    site_for_policy,
                    body.occurred_at,
                )
                .await?;
                let counted = receiving::CountedLine {
                    expected_quantity: expected_qty,
                    quantity: base_quantity,
                    has_lot: lot_id.is_some(),
                };
                let disp = receiving::disposition(&resolved.policy, &counted);

                // D43 / Q172: one header per delivery; lines join by named id.
                let (goods_receipt_id, header_created) = client_events::ensure_goods_receipt(
                    tx,
                    &client_events::NewGoodsReceipt {
                        tenant_id: tenant,
                        goods_receipt_id: body.goods_receipt_id,
                        site_id: who.site_id,
                        purchase_order_id: po_id,
                        received_at: body.occurred_at,
                        client_event_id: body.client_event_id,
                        recorded_by_id: who.person_id,
                    },
                )
                .await?;

                // Principle 5: store base quantity + entered form (D92 / Q173).
                //
                // The level is **bound and cast**, not formatted into the
                // statement. It is one of five `&'static str` enum labels and so
                // was never injectable, but this file has four thousand lines of
                // parameters and no other string-built SQL, and `tenancy.rs`
                // records why: string building is exactly where this goes wrong.
                // The double cast is load-bearing: `$8::packaging_level` alone
                // makes Postgres infer the parameter as the enum, which has no
                // `&str` binding either, so it has to go through `text`. The
                // ToSql impl the format! existed to avoid was never needed.
                let goods_receipt_line_id: Uuid = tx
                    .query_one(
                        "INSERT INTO goods_receipt_line (
                             tenant_id, goods_receipt_id, item_id, expected_supply_id,
                             expected_quantity, quantity, entered_quantity,
                             entered_packaging_level, item_packing_config_id, lot_id,
                             client_event_id, recorded_by_id)
                         VALUES (
                             $1, $2, $3, $4, $5, $6, $7, $8::text::packaging_level,
                             $9, $10, $11, $12)
                         RETURNING id",
                        &[
                            &tenant,
                            &goods_receipt_id,
                            &item_id,
                            &body.expected_supply_id,
                            &expected_qty,
                            &base_quantity,
                            &entered_qty,
                            &entered_level,
                            &packing_config_id,
                            &lot_id,
                            &body.client_event_id,
                            &who.person_id,
                        ],
                    )
                    .await?
                    .get(0);

                let raise: Option<&str> = disp.raise;
                let discrepancy_id: Option<Uuid> = tx
                    .query_one(
                        "SELECT goods_receipt_line_dispose(
                             $1, $2, $3, $4, $5, $6)",
                        &[
                            &goods_receipt_line_id,
                            &disp.receiving_policy_id,
                            &disp.accept,
                            &raise,
                            &who.person_id,
                            &body.client_event_id,
                        ],
                    )
                    .await?
                    .get(0);

                // The lot's own, gathered before the policy ran because the
                // disposition needed to know whether there was a lot at all.
                let mut warnings: Vec<String> = lot_warnings;
                if let Some(kind) = disp.raise {
                    warnings.push(format!("disposition raised finding kind {kind}"));
                }

                // **An ambiguous resolution is a finding, not a warning.** D22
                // chose to raise it rather than pick between two equally specific
                // bindings, and J16 counts the rows. The floor is not stopped —
                // the lower binding id already won and the goods are being
                // received under it — but the tie now outlives the response.
                if let Some(text) = resolved.ambiguity {
                    tx.execute(
                        "INSERT INTO discrepancy (
                             tenant_id, kind, item_id, lot_id,
                             detected_at, detected_by_id, state,
                             goods_receipt_line_id, detail)
                         VALUES ($1, 'policy_ambiguous', $2, $3,
                                 now(), $4, 'open', $5, $6)",
                        &[
                            &tenant,
                            &item_id,
                            &lot_id,
                            &who.person_id,
                            &goods_receipt_line_id,
                            &text,
                        ],
                    )
                    .await?;
                    warnings.push(text);
                }

                let mut movement_id = None;
                let mut stock_id = None;
                let mut repointed = vec![];

                if disp.accept {
                    let mid: Uuid = tx
                        .query_one(
                            "INSERT INTO stock_movement (
                                 tenant_id, client_event_id, item_id, quantity,
                                 to_location_id, to_lot_id, to_status_id, to_owner_id,
                                 reason, occurred_at, recorded_by_id, goods_receipt_line_id)
                             VALUES (
                                 $1, $2, $3, $4,
                                 $5, $6, $7, $8,
                                 'receipt', $9, $10, $11)
                             RETURNING id",
                            &[
                                &tenant,
                                &body.client_event_id,
                                &item_id,
                                &base_quantity,
                                &body.to_location_id,
                                &lot_id,
                                &status_id,
                                &owner_id,
                                &body.occurred_at,
                                &who.person_id,
                                &goods_receipt_line_id,
                            ],
                        )
                        .await?
                        .get(0);
                    movement_id = Some(mid);

                    let refresh: Option<i64> = tx
                        .query_one("SELECT projection_refresh_tenant($1)", &[&tenant])
                        .await?
                        .get(0);
                    if refresh.is_none() {
                        warnings.push(
                            "projection refresh rate-limited; claim handover deferred until \
                             the scheduler rebuilds stock"
                                .into(),
                        );
                    }

                    stock_id = tx
                        .query_opt(
                            "SELECT id FROM stock
                              WHERE item_id = $1
                                AND holder_location_id = $2
                                AND lot_id IS NOT DISTINCT FROM $3
                                AND status_id = $4
                                AND owner_id = $5
                                AND holder_package_id IS NULL",
                            &[
                                &item_id,
                                &body.to_location_id,
                                &lot_id,
                                &status_id,
                                &owner_id,
                            ],
                        )
                        .await?
                        .map(|r| r.get(0));

                    if let Some(sid) = stock_id {
                        let claim_rows = tx
                            .query(
                                "SELECT id, tenant_id, expected_supply_id, quantity, state, bound_at
                                   FROM stock_allocation
                                  WHERE expected_supply_id = $1",
                                &[&body.expected_supply_id],
                            )
                            .await?;
                        let claims: Vec<Claim> = claim_rows
                            .iter()
                            .map(|r| Claim {
                                id: r.get(0),
                                tenant_id: r.get(1),
                                expected_supply_id: r.get(2),
                                quantity: r.get(3),
                                state: r.get(4),
                                bound_at: r.get(5),
                            })
                            .collect();

                        let close = body.close_promise.unwrap_or(false) || closed_at.is_some();
                        let arrival = Arrival {
                            tenant_id: tenant,
                            expected_supply_id: body.expected_supply_id,
                            stock_id: sid,
                            quantity_received: base_quantity,
                            occurred_at: body.occurred_at,
                            promise_closed: close,
                        };
                        let (repoints, problems) = receiving::plan(&arrival, &claims);
                        for p in problems {
                            warnings.push(p.to_string());
                        }
                        for r in &repoints {
                            tx.execute(
                                "UPDATE stock_allocation
                                    SET stock_id = $1,
                                        expected_supply_id = NULL,
                                        origin_expected_supply_id = $2,
                                        bound_at = $3
                                  WHERE id = $4",
                                &[
                                    &r.stock_id,
                                    &r.origin_expected_supply_id,
                                    &r.bound_at,
                                    &r.allocation_id,
                                ],
                            )
                            .await?;
                            repointed.push(r.allocation_id);
                        }
                    } else {
                        warnings.push(
                            "stock cell not found after receipt; claims remain on the promise \
                             until the stock projection runs"
                                .into(),
                        );
                    }

                    tx.execute(
                        "SELECT projection_mark_dirty($1, 'receipt')",
                        &[&tenant],
                    )
                    .await?;
                } else {
                    warnings.push(
                        "receipt line refused by receiving policy; no stock movement written"
                            .into(),
                    );
                }

                Ok(RecordReceiptResponse {
                    goods_receipt_id,
                    goods_receipt_line_id,
                    header_created,
                    quantity: base_quantity,
                    entered_quantity: entered_qty,
                    entered_packaging_level: entered_level,
                    item_packing_config_id: packing_config_id,
                    movement_id,
                    stock_id,
                    repointed_allocation_ids: repointed,
                    receiving_policy_id: Some(disp.receiving_policy_id),
                    accepted: disp.accept,
                    discrepancy_id,
                    warnings,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Move / putaway (location → location)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct RecordMoveRequest {
    pub from_stock_id: Uuid,
    pub to_location_id: Uuid,
    pub quantity: i64,
    /// `move` (default) or `putaway`.
    pub reason: Option<String>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct RecordMoveResponse {
    pub movement_id: Uuid,
    pub warnings: Vec<String>,
}

/// Record a bin-to-bin move or putaway as one `stock_movement`. Does not UPDATE
/// stock quantities — the fold does that after the scheduler runs.
#[post("/moves")]
pub async fn record_move(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RecordMoveRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    let reason = body.reason.clone().unwrap_or_else(|| "move".to_string());
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let cell_row = tx
                    .query_opt(
                        "SELECT id, tenant_id, item_id, holder_location_id, holder_package_id,
                                lot_id, status_id, owner_id, quantity, available_quantity
                           FROM stock WHERE id = $1",
                        &[&body.from_stock_id],
                    )
                    .await?;
                let loc_row = tx
                    .query_opt(
                        "SELECT id, tenant_id FROM location WHERE id = $1",
                        &[&body.to_location_id],
                    )
                    .await?;

                let cell = cell_row.as_ref().map(|r| moving::StockCell {
                    id: r.get(0),
                    tenant_id: r.get(1),
                    item_id: r.get(2),
                    holder_location_id: r.get(3),
                    holder_package_id: r.get(4),
                    lot_id: r.get(5),
                    status_id: r.get(6),
                    owner_id: r.get(7),
                    quantity: r.get(8),
                    available_quantity: r.get(9),
                });
                let location = loc_row.as_ref().map(|r| moving::Location {
                    id: r.get(0),
                    tenant_id: r.get(1),
                });

                let proposed = ProposedMove {
                    tenant_id: tenant,
                    quantity: body.quantity,
                    from_stock_id: body.from_stock_id,
                    to_location_id: body.to_location_id,
                    reason: reason.clone(),
                };
                let problems = moving::check(&proposed, cell.as_ref(), location.as_ref());
                let hard: Vec<String> = problems
                    .iter()
                    .filter(|p| moving::is_hard(p))
                    .map(|p| p.to_string())
                    .collect();
                if !hard.is_empty() {
                    return Err(ApiError::Rejected(hard.join("; ")));
                }
                let warnings: Vec<String> = problems
                    .iter()
                    .filter(|p| !moving::is_hard(p))
                    .map(|p| p.to_string())
                    .collect();

                let cell = cell.expect("hard checks require a cell");
                let sides = moving::sides(&cell, body.to_location_id);

                let mut warnings = warnings;
                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;

                if act.is_replay() {
                    let (movement_id, qty) =
                        client_events::require_one_movement(tx, body.client_event_id).await?;
                    client_events::reject_quantity_mismatch(qty, body.quantity)?;
                    warnings.push(client_events::REPLAY_WARNING.into());
                    return Ok(RecordMoveResponse {
                        movement_id,
                        warnings,
                    });
                }

                let movement_id: Uuid = tx
                    .query_one(
                        "INSERT INTO stock_movement (
                             tenant_id, client_event_id, item_id, quantity,
                             from_location_id, from_lot_id, from_status_id, from_owner_id,
                             to_location_id, to_lot_id, to_status_id, to_owner_id,
                             reason, occurred_at, recorded_by_id)
                         VALUES (
                             $1, $2, $3, $4,
                             $5, $6, $7, $8,
                             $9, $10, $11, $12,
                             $13, $14, $15)
                         RETURNING id",
                        &[
                            &tenant,
                            &body.client_event_id,
                            &sides.item_id,
                            &body.quantity,
                            &sides.from_location_id,
                            &sides.from_lot_id,
                            &sides.from_status_id,
                            &sides.from_owner_id,
                            &sides.to_location_id,
                            &sides.to_lot_id,
                            &sides.to_status_id,
                            &sides.to_owner_id,
                            &reason,
                            &body.occurred_at,
                            &who.person_id,
                        ],
                    )
                    .await?
                    .get(0);

                tx.execute(
                    "SELECT projection_mark_dirty($1, $2)",
                    &[&tenant, &reason],
                )
                .await?;

                Ok(RecordMoveResponse {
                    movement_id,
                    warnings,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Hand cartons to a carrier
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct RecordConsignmentRequest {
    /// Client-minted (Q174's lesson): a consignment is a grouping and carries no
    /// `client_event`, so the identifier is what makes a retry a replay.
    pub id: Option<Uuid>,
    pub package_ids: Vec<Uuid>,
    pub carrier_service_id: Option<Uuid>,
    pub carrier_id: Option<Uuid>,
    pub freight_provider_id: Option<Uuid>,
    pub despatch_at: Option<DateTime<Utc>>,
}

/// One line of what a carrier is actually being handed.
///
/// **This is the walkthrough's "quantity gap", answered by the model instead of
/// by an operator.** NetSuite holds one line per product and box type, MachShip
/// wants a count of packages, and *"the count is currently re-entered by hand in
/// MachShip and exists in neither system beforehand."* Here it is a fold.
#[derive(Serialize, Debug, PartialEq)]
pub struct CarrierLine {
    /// The preset, or "(untyped)" for a carton nobody classified.
    pub package_type: Option<String>,
    pub carrier_package_code: Option<String>,
    /// How many of this kind. The number that existed nowhere.
    pub package_count: i64,
    pub gross_weight_g: Option<i64>,
    pub length_mm: Option<i32>,
    pub width_mm: Option<i32>,
    pub height_mm: Option<i32>,
    /// True when every carton of this kind carries the same three dimensions, so
    /// a carrier can be given one line rather than a row per parcel.
    pub uniform: bool,
}

#[derive(Serialize, Debug)]
pub struct ConsignmentResponse {
    pub consignment_id: Uuid,
    pub carrier_name: Option<String>,
    pub carrier_service_name: Option<String>,
    /// The carrier's word, NULL until one has been asked. Not ours to invent:
    /// the application holds no INSERT on this column.
    pub status: Option<String>,
    pub despatch_at: Option<DateTime<Utc>>,
    pub package_count: i64,
    pub total_gross_weight_g: Option<i64>,
    /// What would be sent to a freight system, grouped the way one wants it.
    pub carrier_lines: Vec<CarrierLine>,
    pub replayed: bool,
    pub warnings: Vec<String>,
}

/// Read a consignment and the parcel summary a carrier would be given.
async fn consignment_view(
    tx: &tokio_postgres::Transaction<'_>,
    consignment_id: Uuid,
    replayed: bool,
    warnings: Vec<String>,
) -> Result<ConsignmentResponse, ApiError> {
    let head = tx
        .query_opt(
            "SELECT c.status, c.despatch_at, ca.name, cs.name
               FROM consignment c
               LEFT JOIN carrier ca ON ca.id = c.carrier_id
               LEFT JOIN carrier_service cs ON cs.id = c.carrier_service_id
              WHERE c.id = $1",
            &[&consignment_id],
        )
        .await?
        .ok_or(ApiError::NotFound)?;

    // Grouped by preset and by the dimensions actually recorded, so two pallets
    // of different heights are two lines rather than one wrong one.
    //
    // **The preset's footprint stands in where the carton has none of its own.**
    // The bench records a weight and a height, because those are what vary: a
    // small box is 320 by 240 whoever packs it, and how tall it ends up is the
    // only question. Reading the carton alone therefore handed a carrier a line
    // with no length and no width, which is not an honest absence — the box's
    // dimensions are known, they are simply held on the type rather than
    // restated on every carton of it. Height is never inherited: that is the one
    // a person actually measured, and an unmeasured carton has no height to
    // claim. `uniform` is computed over the same coalesced values, so a line
    // that inherits its footprint and disagrees on nothing is still uniform.
    let rows = tx
        .query(
            "SELECT pt.name, pt.carrier_package_code, count(*)::bigint,
                    sum(p.gross_weight_g)::bigint,
                    min(coalesce(p.length_mm, pt.length_mm)),
                    min(coalesce(p.width_mm, pt.width_mm)),
                    min(p.height_mm),
                    count(DISTINCT (coalesce(p.length_mm, pt.length_mm),
                                    coalesce(p.width_mm, pt.width_mm),
                                    p.height_mm)) = 1
               FROM consignment_package cp
               JOIN package p ON p.id = cp.package_id
               LEFT JOIN package_type pt ON pt.id = p.package_type_id
              WHERE cp.consignment_id = $1
              GROUP BY pt.name, pt.carrier_package_code
              ORDER BY pt.name NULLS LAST",
            &[&consignment_id],
        )
        .await?;

    let carrier_lines: Vec<CarrierLine> = rows
        .iter()
        .map(|r| CarrierLine {
            package_type: r.get(0),
            carrier_package_code: r.get(1),
            package_count: r.get(2),
            gross_weight_g: r.get(3),
            length_mm: r.get(4),
            width_mm: r.get(5),
            height_mm: r.get(6),
            uniform: r.get::<_, Option<bool>>(7).unwrap_or(true),
        })
        .collect();

    let mut warnings = warnings;
    for l in &carrier_lines {
        if !l.uniform {
            warnings.push(format!(
                "{} cartons of {} do not all carry the same dimensions; a carrier \
                 given one line for them would be quoted the smallest",
                l.package_count,
                l.package_type.as_deref().unwrap_or("no preset")
            ));
        }
        if l.gross_weight_g.is_none() {
            warnings.push(format!(
                "{} carton(s) of {} have no weight recorded; a carrier will weigh \
                 them and invoice for what they find",
                l.package_count,
                l.package_type.as_deref().unwrap_or("no preset")
            ));
        }
    }

    Ok(ConsignmentResponse {
        consignment_id,
        status: head.get(0),
        despatch_at: head.get(1),
        carrier_name: head.get(2),
        carrier_service_name: head.get(3),
        package_count: carrier_lines.iter().map(|l| l.package_count).sum(),
        total_gross_weight_g: carrier_lines.iter().map(|l| l.gross_weight_g).sum(),
        carrier_lines,
        replayed,
        warnings,
    })
}

/// Hand a set of sealed cartons to a carrier as one consignment.
///
/// Stage 6 of the recorded process, where today *"NetSuite sends the record
/// details to MachShip, creating a Pending Consignment"* and the operator then
/// finds it again by eye, because *"there is no shared key surfaced in this
/// step."* Here the consignment is a row with an identifier, and no carrier is
/// called: the response is the payload one would be given.
///
/// **Sealed only.** An open carton is still being packed, and consigning one
/// commits a count that is still moving. Despatched is refused for the opposite
/// reason: it has already left.
#[post("/consignments")]
pub async fn record_consignment(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RecordConsignmentRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                if body.package_ids.is_empty() {
                    return Err(ApiError::Rejected(
                        "a consignment of no cartons is not a consignment".into(),
                    ));
                }

                // Replay before anything else, on Q174's shape: the id is the
                // only thing that makes a retry a replay, and re-running the
                // checks below against a database that already holds the links
                // would refuse the second submission as double-consigned.
                if let Some(id) = body.id {
                    let existing = tx
                        .query_opt(
                            "SELECT id FROM consignment WHERE id = $1 AND tenant_id = $2",
                            &[&id, &tenant],
                        )
                        .await?;
                    if existing.is_some() {
                        return consignment_view(
                            tx,
                            id,
                            true,
                            vec!["replay of an existing consignment id; nothing was written"
                                .into()],
                        )
                        .await;
                    }
                }

                // Every carton, checked before any of them moves. A partial
                // consignment is worse than a refused one: the operator would
                // have to work out which half went.
                let rows = tx
                    .query(
                        "SELECT p.id, p.status, p.sealed_at IS NOT NULL,
                                cp.consignment_id
                           FROM package p
                           LEFT JOIN consignment_package cp ON cp.package_id = p.id
                          WHERE p.id = ANY($1)",
                        &[&body.package_ids],
                    )
                    .await?;
                if rows.len() != body.package_ids.len() {
                    return Err(ApiError::Rejected(
                        "some of those cartons are not this tenant's, or do not exist".into(),
                    ));
                }
                let mut problems = vec![];
                for r in &rows {
                    let id: Uuid = r.get(0);
                    let status: Option<String> = r.get(1);
                    let sealed: bool = r.get(2);
                    let already: Option<Uuid> = r.get(3);
                    if let Some(c) = already {
                        problems.push(format!("carton {id} is already on consignment {c}"));
                        continue;
                    }
                    match status.as_deref() {
                        Some("despatched") => {
                            problems.push(format!("carton {id} has already left"))
                        }
                        Some("voided") => problems.push(format!("carton {id} is voided")),
                        _ if !sealed => problems.push(format!(
                            "carton {id} is not sealed; consigning it commits a count that \
                             is still moving"
                        )),
                        _ => {}
                    }
                }
                if !problems.is_empty() {
                    return Err(ApiError::Rejected(problems.join("; ")));
                }

                // The carrier may be named directly or through the service, and
                // the service knows its carrier — architecture keeps them apart
                // so the same carrier booked two ways keeps one cost history.
                let carrier_id = match (body.carrier_id, body.carrier_service_id) {
                    (Some(c), _) => Some(c),
                    (None, Some(svc)) => tx
                        .query_opt("SELECT carrier_id FROM carrier_service WHERE id = $1", &[&svc])
                        .await?
                        .map(|r| r.get(0)),
                    (None, None) => None,
                };

                // **No `status`, and the grant list is why.** The application
                // holds INSERT on nine columns of this table and not on `status`,
                // `eta` or `price_minor`: those are the carrier's answers, and a
                // consignment that has not been sent anywhere has none of them.
                // Writing 'pending' here would be importing the freight system's
                // vocabulary into a column that exists to hold the freight
                // system's reply.
                let consignment_id: Uuid = tx
                    .query_one(
                        "INSERT INTO consignment (
                             id, tenant_id, carrier_id, carrier_service_id,
                             freight_provider_id, despatch_at)
                         VALUES (coalesce($1, uuidv7()), $2, $3, $4, $5, $6)
                         RETURNING id",
                        &[
                            &body.id,
                            &tenant,
                            &carrier_id,
                            &body.carrier_service_id,
                            &body.freight_provider_id,
                            &body.despatch_at,
                        ],
                    )
                    .await?
                    .get(0);

                for pid in &body.package_ids {
                    // Migration 69's unique index is what actually decides this;
                    // the check above is the message, not the guard.
                    tx.execute(
                        "INSERT INTO consignment_package (tenant_id, consignment_id, package_id)
                         VALUES ($1, $2, $3)",
                        &[&tenant, &consignment_id, pid],
                    )
                    .await
                    .map_err(|e| {
                        if e.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) {
                            ApiError::Rejected(format!(
                                "carton {pid} was consigned by someone else while this \
                                 request was running; a carton goes on one truck"
                            ))
                        } else {
                            ApiError::Database(e)
                        }
                    })?;
                }

                consignment_view(tx, consignment_id, false, vec![]).await
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

/// The parcel summary for a consignment already created.
#[get("/consignments/{id}")]
pub async fn consignment(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let consignment_id = path.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    let result = scope
        .run(|tx| Box::pin(async move { consignment_view(tx, consignment_id, false, vec![]).await }))
        .await?;
    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Find an order by what a person quotes
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct OrderSearch {
    /// A confirmation number or an external reference. Matched against both.
    ///
    /// **Optional, because a search box is not a way in.** Somebody who has just
    /// been handed a job does not know its number; they know it is one of the
    /// ones that came in recently. Absent, this answers the latest orders
    /// instead, which is the difference between a screen you can browse and a
    /// screen you have to already know the answer to use.
    pub reference: Option<String>,
}

/// How many orders "the latest" is.
///
/// A number rather than a page, because this is a way in rather than a report:
/// the operator who cannot find their job in the last twenty knows its number
/// and can type it.
const LATEST_ORDERS: i64 = 20;

#[derive(Serialize, Debug, PartialEq)]
pub struct FulfilmentSummary {
    pub fulfilment_id: Uuid,
    pub state: String,
    pub site_id: Option<Uuid>,
    pub site_code: Option<String>,
    pub line_count: i64,
    /// Committed across the lines, and how far the floor has got.
    pub committed_quantity: i64,
    pub picked_quantity: i64,
    pub packed_quantity: i64,
    pub despatched_quantity: i64,
    /// **The stage-2 gate, computed.** True when every line is fully picked.
    pub fully_picked: bool,
    /// When the projection behind those numbers last ran. NULL = never (D95).
    pub as_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug)]
pub struct OrderMatch {
    pub order_id: Uuid,
    pub confirmation_number: Option<String>,
    pub external_ref: Option<String>,
    /// The contact stage 1 says to confirm the number against.
    pub customer_name: Option<String>,
    pub state: String,
    pub placed_at: Option<DateTime<Utc>>,
    pub promised_to: Option<DateTime<Utc>>,
    /// D44: an externally-authoritative order is amended by cancel-and-reraise,
    /// so one reference can name a cancelled order and the one that replaced it.
    pub supersedes_order_id: Option<Uuid>,
    pub fulfilments: Vec<FulfilmentSummary>,
}

/// Find an order by the number a customer quotes, with its fulfilments.
///
/// **Stages 1 and 2 of the recorded process, in one request.** Today they are
/// four screens across two systems: search, confirm the contact, open Related
/// Records, find the Item Fulfilment, check its status and its warehouse.
///
/// The status check is the part that could not be copied across. NetSuite has an
/// Item Fulfilment status of `Picked`; this model does not, and S44 says why:
///
/// > No table in the fulfilment set carries a stored `progress`, completion or
/// > rollup status column … any label it could hold is a function of the four
/// > coverage quantities that can disagree with them.
///
/// So `fully_picked` is computed from `fulfilment_line`, not read from a column,
/// and the quantities come back beside it so a partly-picked fulfilment says how
/// far rather than merely failing a gate. `as_at` says how old the projection is
/// (D95), because a gate answered from a stale cache should say so.
#[get("/orders")]
pub async fn find_orders(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<OrderSearch>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let site = who.site_id;
    let reference = query
        .into_inner()
        .reference
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty());
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                // Both indexes migration 68 added, as alternatives: a locally
                // raised order carries a confirmation number and an EDI one an
                // external reference, and D39 puts exactly one system of record
                // on each.
                let orders = match &reference {
                    Some(r) => {
                        tx.query(
                            "SELECT o.id, o.confirmation_number, o.external_ref, p.name,
                                    o.state::text, o.placed_at, o.promised_to,
                                    o.supersedes_order_id
                               FROM \"order\" o
                               LEFT JOIN party p ON p.id = o.customer_party_id
                              WHERE o.confirmation_number = $1 OR o.external_ref = $1
                              ORDER BY o.placed_at DESC NULLS LAST, o.id",
                            &[r],
                        )
                        .await?
                    }
                    // The latest, at the site they signed on at. A person with no
                    // site sees the tenant's, which is the same rule the pack
                    // queue follows rather than a second one.
                    None => {
                        tx.query(
                            "SELECT o.id, o.confirmation_number, o.external_ref, p.name,
                                    o.state::text, o.placed_at, o.promised_to,
                                    o.supersedes_order_id
                               FROM \"order\" o
                               LEFT JOIN party p ON p.id = o.customer_party_id
                              WHERE ($1::uuid IS NULL OR o.site_id = $1)
                              ORDER BY o.placed_at DESC NULLS LAST, o.id
                              LIMIT $2",
                            &[&site, &LATEST_ORDERS],
                        )
                        .await?
                    }
                };

                let mut matches = vec![];
                for o in &orders {
                    let order_id: Uuid = o.get(0);
                    // One row per fulfilment, folding its lines. A fulfilment has
                    // few lines -- D25's own argument for computing this rather
                    // than storing it.
                    let fs = tx
                        .query(
                            "SELECT f.id, f.state::text, f.site_id, s.code,
                                    count(fl.id),
                                    coalesce(sum(fl.quantity), 0)::bigint,
                                    coalesce(sum(fl.picked_quantity), 0)::bigint,
                                    coalesce(sum(fl.packed_quantity), 0)::bigint,
                                    coalesce(sum(fl.despatched_quantity), 0)::bigint,
                                    bool_and(fl.picked_quantity >= fl.quantity),
                                    -- The same read `ledger_views` uses for
                                    -- `ProjectionProgress.as_at`, rather than a
                                    -- second way of asking how old the numbers are.
                                    (SELECT pf.last_run_at FROM projection_freshness pf
                                      WHERE pf.function_name = 'projection_fulfilment_rebuild'
                                        AND pf.tenant_id = f.tenant_id)
                               FROM fulfilment f
                               LEFT JOIN fulfilment_line fl ON fl.fulfilment_id = f.id
                               LEFT JOIN site s ON s.id = f.site_id
                              WHERE f.order_id = $1
                              GROUP BY f.id, f.state, f.site_id, s.code, f.tenant_id
                              ORDER BY f.id",
                            &[&order_id],
                        )
                        .await?;

                    matches.push(OrderMatch {
                        order_id,
                        confirmation_number: o.get(1),
                        external_ref: o.get(2),
                        customer_name: o.get(3),
                        state: o.get(4),
                        placed_at: o.get(5),
                        promised_to: o.get(6),
                        supersedes_order_id: o.get(7),
                        fulfilments: fs
                            .iter()
                            .map(|r| FulfilmentSummary {
                                fulfilment_id: r.get(0),
                                state: r.get(1),
                                site_id: r.get(2),
                                site_code: r.get(3),
                                line_count: r.get(4),
                                committed_quantity: r.get(5),
                                picked_quantity: r.get(6),
                                packed_quantity: r.get(7),
                                despatched_quantity: r.get(8),
                                // NULL when the fulfilment has no lines, which is
                                // not "fully picked" however the SQL reads.
                                fully_picked: r.get::<_, Option<bool>>(9).unwrap_or(false)
                                    && r.get::<_, i64>(4) > 0,
                                as_at: r.get(10),
                            })
                            .collect(),
                    });
                }
                Ok(matches)
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// What is physically in a carton (the packing list)
// ---------------------------------------------------------------------------

#[derive(Serialize, Debug, PartialEq)]
pub struct PackedLine {
    pub item_code: String,
    pub description: Option<String>,
    pub lot_code: Option<String>,
    pub expiry_date: Option<chrono::NaiveDate>,
    /// Units of this item and lot in this carton, serving this line.
    pub quantity: i64,
    pub fulfilment_line_id: Uuid,
    pub order_line_id: Uuid,
    /// What a person quotes on the phone.
    pub order_reference: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct PackedCell {
    pub item_code: String,
    pub lot_code: Option<String>,
    pub quantity: i64,
    pub catch_weight_g: Option<i64>,
}

#[derive(Serialize, Debug)]
pub struct PackingListResponse {
    pub package_id: Uuid,
    pub barcode: Option<String>,
    pub sscc: Option<String>,
    pub status: Option<String>,
    pub sealed: bool,
    pub package_type: Option<String>,
    pub length_mm: Option<i32>,
    pub width_mm: Option<i32>,
    pub height_mm: Option<i32>,
    pub gross_weight_g: Option<i64>,
    pub dimensions_source: Option<String>,
    /// **The packing list**: contents folded from the ledger and named back to
    /// the order line each unit serves.
    pub lines: Vec<PackedLine>,
    /// What the `stock` projection says is in the carton, per cell.
    pub cells: Vec<PackedCell>,
    pub warnings: Vec<String>,
}

/// What is physically in this carton, and which order line each unit serves.
///
/// First on architecture.md's list of what the system being replaced cannot do:
///
/// > It can say what is physically in a carton. Cartons are real records and
/// > their contents link back to order lines, so a packing list is per carton, a
/// > damage claim can name the carton the item was in, and a declared weight can
/// > be checked against its contents before a carrier charges for the difference.
///
/// **Two answers, deliberately.** `lines` folds `stock_movement` — the picks that
/// put stock in, netted through `stock_movement_effective` so a corrected pick
/// takes back what it took back (D103) — and reaches the order line through
/// `fulfilment_line_id`, which is the column D53 added for exactly this. `cells`
/// reads `package_content`, the view D24 left behind over `stock`, which is the
/// projection and says what is in the carton without saying who it is for.
///
/// They can disagree, and the difference is worth printing rather than hiding: a
/// consolidation tote holds stock no single line owns, and a cell with no pick
/// behind it is stock that arrived in the carton by some other path. The
/// warnings name it; neither number is silently preferred.
#[get("/packages/{id}/contents")]
pub async fn package_contents(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let package_id = path.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let head = tx
                    .query_opt(
                        "SELECT p.barcode, p.sscc::text, p.status, p.sealed_at IS NOT NULL,
                                pt.name, p.length_mm, p.width_mm, p.height_mm,
                                p.gross_weight_g, p.dimensions_source
                           FROM package p
                           LEFT JOIN package_type pt ON pt.id = p.package_type_id
                          WHERE p.id = $1",
                        &[&package_id],
                    )
                    .await?;
                let Some(h) = head else {
                    return Err(ApiError::NotFound);
                };

                // The picks that filled this carton, netted for corrections.
                // `stock_movement_effective` holds roots only and a correction is
                // a term in its target's sum, so joining it is what stops a
                // reversed pick still reading as packed.
                let line_rows = tx
                    .query(
                        "SELECT i.code, i.description, l.code, l.expiry_date,
                                sum(e.effective_quantity)::bigint,
                                m.fulfilment_line_id, fl.order_line_id, o.confirmation_number
                           FROM stock_movement m
                           JOIN stock_movement_effective e ON e.movement_id = m.id
                           JOIN item i ON i.id = m.item_id
                           JOIN fulfilment_line fl ON fl.id = m.fulfilment_line_id
                           JOIN order_line ol ON ol.id = fl.order_line_id
                           JOIN \"order\" o ON o.id = ol.order_id
                           LEFT JOIN lot l ON l.id = m.to_lot_id
                          WHERE m.to_package_id = $1
                            AND m.fulfilment_line_id IS NOT NULL
                          GROUP BY i.code, i.description, l.code, l.expiry_date,
                                   m.fulfilment_line_id, fl.order_line_id,
                                   o.confirmation_number
                         HAVING sum(e.effective_quantity) <> 0
                          ORDER BY i.code, l.code NULLS FIRST",
                        &[&package_id],
                    )
                    .await?;
                let lines: Vec<PackedLine> = line_rows
                    .iter()
                    .map(|r| PackedLine {
                        item_code: r.get(0),
                        description: r.get(1),
                        lot_code: r.get(2),
                        expiry_date: r.get(3),
                        quantity: r.get(4),
                        fulfilment_line_id: r.get(5),
                        order_line_id: r.get(6),
                        order_reference: r.get(7),
                    })
                    .collect();

                // What the projection says is in it. `package_content` is D24's
                // view over `stock`, so this lags the ledger by however long ago
                // the maintainer ran — which is D25 working, not a defect.
                let cell_rows = tx
                    .query(
                        "SELECT i.code, l.code, pc.quantity, pc.catch_weight_g
                           FROM package_content pc
                           JOIN item i ON i.id = pc.item_id
                           LEFT JOIN lot l ON l.id = pc.lot_id
                          WHERE pc.package_id = $1 AND pc.quantity <> 0
                          ORDER BY i.code, l.code NULLS FIRST",
                        &[&package_id],
                    )
                    .await?;
                let cells: Vec<PackedCell> = cell_rows
                    .iter()
                    .map(|r| PackedCell {
                        item_code: r.get(0),
                        lot_code: r.get(1),
                        quantity: r.get(2),
                        catch_weight_g: r.get(3),
                    })
                    .collect();

                let mut warnings = vec![];
                let from_lines: i64 = lines.iter().map(|l| l.quantity).sum();
                let from_cells: i64 = cells.iter().map(|c| c.quantity).sum();
                if from_lines != from_cells {
                    warnings.push(format!(
                        "the picks account for {from_lines} units and the stock projection \
                         holds {from_cells}; either the projection has not caught up, or \
                         stock reached this carton by a path that named no line"
                    ));
                }
                if lines.is_empty() && !cells.is_empty() {
                    warnings.push(
                        "nothing in this carton names an order line; a packing list cannot \
                         say who it is for"
                            .into(),
                    );
                }

                Ok(PackingListResponse {
                    package_id,
                    barcode: h.get(0),
                    sscc: h.get(1),
                    status: h.get(2),
                    sealed: h.get(3),
                    package_type: h.get(4),
                    length_mm: h.get(5),
                    width_mm: h.get(6),
                    height_mm: h.get(7),
                    gross_weight_g: h.get(8),
                    dimensions_source: h.get(9),
                    lines,
                    cells,
                    warnings,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Void a carton
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct VoidPackageRequest {
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub source: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct VoidPackageResponse {
    pub event_id: Uuid,
    pub status: Option<String>,
    pub warnings: Vec<String>,
}

/// Void a carton the packer no longer wants.
///
/// **`voided` has been a status the package fold understands since migration 4
/// and nothing has ever written one.** It is what an append-only model should do
/// with a carton somebody started and abandoned: the row and its history stay,
/// and the thing stops being a carton anyone can put stock in.
///
/// **Refused while it still holds stock.** A voided carton with units inside is
/// stock nobody can find — `stock.holder_package_id` would point at something no
/// screen lists, which is J7's shape (`quantity > 0` implies a resolved location)
/// arriving one container out. Empty it first, which on the bench means
/// correcting the picks back out.
///
/// Sealed and despatched cartons are refused for the opposite reason: one has
/// been committed to and the other has left.
#[post("/packages/{id}/void")]
pub async fn void_package(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<VoidPackageRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let package_id = path.into_inner();
    let body = body.into_inner();
    let source = body.source.clone().unwrap_or_else(|| "operator_scan".into());
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let pkg = tx
                    .query_opt(
                        // **Folded from the ledger, not read from `stock`.** The
                        // first version asked the projection, which lags the
                        // write that just happened — so a carton picked into a
                        // second earlier reported empty and voided cleanly,
                        // stranding the units. A guard that reads a cache is a
                        // guard that can be wrong, and this one has to be right
                        // at the moment it is asked.
                        "SELECT status, sealed_at IS NOT NULL,
                                coalesce((
                                    SELECT sum(CASE WHEN m.to_package_id = $1
                                                    THEN e.effective_quantity
                                                    ELSE -e.effective_quantity END)
                                      FROM stock_movement m
                                      JOIN stock_movement_effective e
                                        ON e.movement_id = m.id
                                     WHERE m.to_package_id = $1
                                        OR m.from_package_id = $1), 0)::bigint,
                                coalesce((SELECT count(*) FROM package
                                           WHERE parent_package_id = $1), 0)::bigint
                           FROM package WHERE id = $1",
                        &[&package_id],
                    )
                    .await?
                    .ok_or(ApiError::NotFound)?;
                let status: Option<String> = pkg.get(0);
                let held: i64 = pkg.get(2);
                let children: i64 = pkg.get(3);

                match status.as_deref() {
                    Some("despatched") => {
                        return Err(ApiError::Rejected("that carton has already left".into()))
                    }
                    Some("voided") => {
                        return Err(ApiError::Rejected("that carton is already void".into()))
                    }
                    Some("sealed") => {
                        return Err(ApiError::Rejected(
                            "that carton is sealed; open it before voiding it".into(),
                        ))
                    }
                    _ => {}
                }
                if held != 0 {
                    return Err(ApiError::Rejected(format!(
                        "that carton still holds {held}; take the contents out before \
                         voiding it, or the stock belongs to something no screen lists"
                    )));
                }
                if children != 0 {
                    return Err(ApiError::Rejected(format!(
                        "that carton still has {children} package(s) nested under it"
                    )));
                }

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;

                let mut warnings = vec![];
                let event_id = match act {
                    ActInsert::Replay => {
                        let (id, prior_pkg, kind) =
                            client_events::require_one_package_event(tx, body.client_event_id)
                                .await?;
                        if kind != "voided" || prior_pkg != package_id {
                            return Err(ApiError::Rejected(
                                "client_event already recorded a different package act".into(),
                            ));
                        }
                        warnings.push(client_events::REPLAY_WARNING.into());
                        id
                    }
                    ActInsert::Fresh => {
                        let id: Uuid = tx
                            .query_one(
                                "INSERT INTO package_event (
                                     tenant_id, client_event_id, package_id, kind, source,
                                     occurred_at, recorded_by_id)
                                 VALUES ($1, $2, $3, 'voided', $4, $5, $6)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &body.client_event_id,
                                    &package_id,
                                    &source,
                                    &body.occurred_at,
                                    &who.person_id,
                                ],
                            )
                            .await?
                            .get(0);
                        tx.execute(
                            "SELECT projection_mark_dirty($1, 'package_void')",
                            &[&tenant],
                        )
                        .await?;
                        id
                    }
                };

                // `package.status` is a fold of the log, so it lags until the
                // maintainer runs. The winning event is the answer now.
                let winning: Option<String> = tx
                    .query_opt(
                        "SELECT kind FROM package_event
                          WHERE package_id = $1
                            AND kind IN ('created','placed','contained','sealed',
                                         'opened','despatched','voided')
                          ORDER BY occurred_at DESC, recorded_at DESC, id DESC
                          LIMIT 1",
                        &[&package_id],
                    )
                    .await?
                    .map(|r| r.get(0));

                Ok(VoidPackageResponse {
                    event_id,
                    status: winning,
                    warnings,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Signing on (Q171 / D11)
// ---------------------------------------------------------------------------

/// A hash to verify against when no account matched.
///
/// **A timing defence, not theatre.** OWASP asks for the same answer and the
/// same latency whether an address exists or not; without this, a missing
/// account returns in microseconds and a real one in the tens of milliseconds
/// Argon2 costs, which enumerates the staff list.
fn absent_account_hash() -> &'static str {
    static H: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    H.get_or_init(|| {
        auth::hash_password("a password nobody has").expect("hashing its own constant")
    })
}

/// Whether this request can carry a `Secure`, `__Host-` cookie.
///
/// **Derived from the request rather than from configuration.** A browser
/// refuses a `__Host-` cookie without TLS, so plain-HTTP development needs the
/// plainer one — and an environment variable deciding that is a setting somebody
/// eventually gets wrong in the direction that matters. Reading the scheme means
/// a connection over TLS always gets the hardened cookie and one without it
/// never silently pretends to.
///
/// `x-forwarded-proto` is what a terminating proxy sets, which is how this looks
/// behind Cloudflare or any load balancer. Trusting it is safe in this
/// direction: a forged header can only make the server offer a *stronger* cookie
/// than the connection deserves, which the browser then refuses. The reverse —
/// trusting a header to weaken the cookie — is the one that would matter, and
/// nothing here does that.
fn secure_cookies(req: &HttpRequest) -> bool {
    if req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|p| p.eq_ignore_ascii_case("https"))
    {
        return true;
    }
    req.connection_info().scheme() == "https"
}

#[derive(Deserialize, Debug)]
pub struct SignOnRequest {
    pub email: String,
    pub password: String,
    /// Required when the person belongs to more than one tenant. D19 makes
    /// `person` global, so which tenant they are acting for is not inferable.
    pub tenant_id: Option<Uuid>,
    /// Where they are working. Every act they record names it.
    pub site_id: Option<Uuid>,
    /// D27's recording device, when the client knows it.
    pub device_id: Option<Uuid>,
}

#[derive(Serialize, Debug)]
pub struct SignOnResponse {
    pub person_id: Uuid,
    pub display_name: String,
    pub tenant_id: Uuid,
    pub site_id: Option<Uuid>,
    pub expires_at: DateTime<Utc>,
    /// Returned once, for clients that cannot hold a cookie — D5's handhelds.
    /// A browser ignores this and uses the cookie.
    pub token: String,
}

/// The tenants a person could sign on for, when the request did not say.
#[derive(Serialize, Debug)]
pub struct TenantChoice {
    pub tenant_id: Uuid,
    pub name: String,
}

/// Sign on: verify a password, open a session, and set the cookie.
///
/// **The one endpoint that answers before a tenant is known**, because which
/// tenant a person acts for is a property of the session rather than of the
/// request. Everything else reads its tenant from the session this creates.
///
/// One generic refusal covers a wrong password, an unknown address, a locked
/// account and a person marked inactive. OWASP is explicit about that, and the
/// reason is that any distinction between them is an oracle for whether somebody
/// works here.
#[post("/sessions")]
pub async fn sign_on(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<SignOnRequest>,
) -> Result<HttpResponse, ApiError> {
    let body = body.into_inner();
    let conn = state.pool.get().await?;
    crate::tenancy::ensure_app_role(&conn).await?;

    // The only query shape that reaches a credential; the table itself is
    // unreadable to this role.
    let row = conn
        .query_opt(
            "SELECT person_id, phc, locked_until, active FROM credential_for_login($1)",
            &[&body.email],
        )
        .await?;

    let (person_id, phc, locked_until, active) = match &row {
        Some(r) => (
            Some(r.get::<_, Uuid>(0)),
            r.get::<_, String>(1),
            r.get::<_, Option<DateTime<Utc>>>(2),
            r.get::<_, bool>(3),
        ),
        // Verify against a real hash anyway, so the answer takes as long.
        None => (None, absent_account_hash().to_string(), None, false),
    };

    let password_ok = auth::verify_password(&body.password, &phc);
    let locked = locked_until.is_some_and(|t| t > Utc::now());
    let ok = password_ok && active && !locked && person_id.is_some();

    if let Some(id) = person_id {
        // Recorded either way: a success clears the counter, a failure advances
        // it. Counted against the account, per OWASP, because an attacker has
        // more addresses than we have accounts.
        conn.execute(
            "SELECT credential_record_attempt($1, $2)",
            &[&id, &password_ok],
        )
        .await?;
    }

    let Some(person_id) = person_id.filter(|_| ok) else {
        return Err(ApiError::Unauthenticated);
    };

    issue_session(&conn, &req, person_id, body.tenant_id, body.site_id, body.device_id).await
}

/// Mint a session for a person who has just proved who they are.
///
/// **Extracted so there is one way to become signed in.** A passkey assertion
/// and a password both end here: the same membership resolution, the same
/// `session_open`, the same cookie. A second path that issued its own session
/// would be a second place for the tenant check to be forgotten, and that check
/// is the sentence deciding whose data the session sees.
async fn issue_session(
    conn: &deadpool_postgres::Client,
    req: &HttpRequest,
    person_id: Uuid,
    wanted_tenant: Option<Uuid>,
    site_id: Option<Uuid>,
    device_id: Option<Uuid>,
) -> Result<HttpResponse, ApiError> {
    // Which tenant. One membership needs no choice; several need the request to
    // say, and the answer lists them rather than picking.
    // Through the definer, because `person_tenant` is not readable to this role.
    // It was until migration 87, which is how the application could read every
    // company's membership list while holding a tenant of its own — and why the
    // repair is a function rather than a policy: this runs before there is a
    // tenant to scope one to.
    let memberships = conn
        .query("SELECT tenant_id, name FROM memberships_of($1)", &[&person_id])
        .await?;
    if memberships.is_empty() {
        return Err(ApiError::Unauthenticated);
    }
    let tenant_id = match wanted_tenant {
        Some(t) => {
            if !memberships.iter().any(|m| m.get::<_, Uuid>(0) == t) {
                return Err(ApiError::Unauthenticated);
            }
            t
        }
        None if memberships.len() == 1 => memberships[0].get(0),
        None => {
            let choices: Vec<TenantChoice> = memberships
                .iter()
                .map(|m| TenantChoice {
                    tenant_id: m.get(0),
                    name: m.get(1),
                })
                .collect();
            return Ok(HttpResponse::Conflict().json(serde_json::json!({
                "error": "choose a tenant",
                "detail": "this person works for more than one; name which in tenant_id",
                "tenants": choices
            })));
        }
    };

    let minted = auth::mint_token();
    // `make_interval` from a typed integer: `chrono::Duration` has no interval
    // binding, and building one by string would be the format!-into-SQL habit
    // this codebase does not have.
    let lifetime_hours = auth::ABSOLUTE_LIFETIME_HOURS as i32;
    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    // Membership is re-checked inside the function, which is where it belongs:
    // it is the sentence that decides whose data this session sees.
    conn.query_one(
        "SELECT session_open($1, $2, $3, $4, make_interval(hours => $5), $6, $7)",
        &[
            &person_id,
            &tenant_id,
            &site_id,
            &minted.sha256,
            &lifetime_hours,
            &device_id,
            &user_agent,
        ],
    )
    .await?;

    let display_name: String = conn
        .query_one("SELECT display_name FROM person WHERE id = $1", &[&person_id])
        .await?
        .get(0);

    Ok(HttpResponse::Ok()
        .insert_header((
            "set-cookie",
            auth::session_cookie(&minted.token, secure_cookies(req)),
        ))
        .json(SignOnResponse {
            person_id,
            display_name,
            tenant_id,
            site_id,
            expires_at: Utc::now() + Duration::hours(auth::ABSOLUTE_LIFETIME_HOURS),
            token: minted.token,
        }))
}

/// Who the request is, or 401.
///
/// Both timeouts are enforced by `session_resolve` rather than here: OWASP puts
/// them server-side and a check the application applies is one it can forget.
/// The program behind an import request, or a refusal.
///
/// **A session is refused here, and that is the point.** D158 scopes an import
/// token by kind rather than by permission, and a kind that a session can also
/// satisfy is not a scope — it is a suggestion. Requiring the prefix keeps the
/// sentence "the import endpoints are reached by an import token" true, which
/// is the whole of the authorisation story and the only thing anybody has to
/// remember about it.
///
/// It also means a leaked session cannot load a warehouse.
async fn machine(
    state: &web::Data<AppState>,
    req: &HttpRequest,
) -> Result<auth::Machine, ApiError> {
    let cookie = req.headers().get("cookie").and_then(|v| v.to_str().ok());
    let authz = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());
    let Some(token) = auth::token_from_headers(cookie, authz) else {
        return Err(ApiError::Unauthenticated);
    };
    if !auth::is_api_token(&token) {
        return Err(ApiError::Unauthenticated);
    }
    crate::tokens::machine(&state.pool, &token).await
}

pub async fn caller(
    state: &web::Data<AppState>,
    req: &HttpRequest,
) -> Result<auth::Caller, ApiError> {
    let cookie = req.headers().get("cookie").and_then(|v| v.to_str().ok());
    let authz = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());
    let Some(token) = auth::token_from_headers(cookie, authz) else {
        return Err(ApiError::Unauthenticated);
    };

    let conn = state.pool.get().await?;
    crate::tenancy::ensure_app_role(&conn).await?;
    let idle_minutes = auth::IDLE_TIMEOUT_MINUTES as i32;
    let row = conn
        .query_opt(
            "SELECT session_id, person_id, tenant_id, site_id
               FROM session_resolve($1, make_interval(mins => $2))",
            &[&auth::token_digest(&token), &idle_minutes],
        )
        .await?
        .ok_or(ApiError::Unauthenticated)?;

    Ok(auth::Caller {
        session_id: row.get(0),
        person_id: row.get(1),
        tenant_id: row.get(2),
        site_id: row.get(3),
    })
}

#[derive(Serialize, Debug)]
pub struct CurrentSession {
    pub person_id: Uuid,
    pub display_name: String,
    pub tenant_id: Uuid,
    pub tenant_name: String,
    pub site_id: Option<Uuid>,
    pub site_code: Option<String>,
}

/// Who am I. What a screen calls to decide whether to show a sign-on form.
#[get("/sessions/current")]
pub async fn current_session(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    // **Scoped to the tenant the session named.** `site` is under row-level
    // security, so reading it on a bare connection returns nothing and the
    // answer silently loses the site it signed on at — which is how this first
    // came back with a null site code and a passing handler.
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let row = scope
        .run(|tx| {
            Box::pin(async move {
                Ok(tx
                    .query_one(
                        "SELECT p.display_name, t.name, s.code
                           FROM person p
                           CROSS JOIN tenant t
                           LEFT JOIN site s ON s.id = $3
                          WHERE p.id = $1 AND t.id = $2",
                        &[&who.person_id, &who.tenant_id, &who.site_id],
                    )
                    .await?)
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(CurrentSession {
        person_id: who.person_id,
        display_name: row.get(0),
        tenant_id: who.tenant_id,
        tenant_name: row.get(1),
        site_id: who.site_id,
        site_code: row.get(2),
    }))
}

#[derive(Serialize, Debug)]
pub struct SiteRow {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    /// True for the one this session is already working at.
    pub current: bool,
}

/// The sites this caller could be working at.
///
/// **Read inside a `TenantScope`, and that is not a style choice.** `site` is
/// under row-level security, and `current_session` records what happens when it
/// is not: *"reading it on a bare connection returns nothing and the answer
/// silently loses the site it signed on at — which is how this first came back
/// with a null site code and a passing handler."* The same trap, one endpoint
/// along.
#[get("/sites")]
pub async fn sites(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let rows = scope
        .run(|tx| {
            Box::pin(async move {
                Ok(tx
                    .query(
                        "SELECT id, code, name FROM site ORDER BY code",
                        &[],
                    )
                    .await?)
            })
        })
        .await?;

    let sites: Vec<SiteRow> = rows
        .iter()
        .map(|r| {
            let id: Uuid = r.get(0);
            SiteRow {
                id,
                code: r.get(1),
                name: r.get(2),
                current: who.site_id == Some(id),
            }
        })
        .collect();
    Ok(HttpResponse::Ok().json(sites))
}

#[derive(Deserialize, Debug)]
pub struct PackingSearch {
    /// A reference, an order number or a customer name. Empty means everything.
    pub q: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ChooseSiteRequest {
    pub site_id: Uuid,
}

/// Say where you are working, and get a session that says so.
///
/// # Why this is a new session rather than an UPDATE
///
/// `session.site_id` is documented as *"where they signed on"*, and a row
/// mutated underneath a live token would quietly make that sentence false — the
/// same session would name two different places over its life, and nothing in
/// the record would say when it changed. So the old session is revoked and a
/// new one opened through [`issue_session`], which is the one way to become
/// signed in. That is the function's whole reason for existing: *"a second path
/// that issued its own session would be a second place for the tenant check to
/// be forgotten."*
///
/// # Why this is not part of signing in
///
/// It could have been, and the first draft of the plan assumed it had to be.
/// It does not: `client_events` copies `who.site_id` into each act at write
/// time and nothing ever re-reads the session's copy, so a site chosen a moment
/// *after* authenticating is exactly as sound for D11's non-repudiable floor as
/// one chosen during it. Keeping it separate is what lets the sign-in page —
/// which runs before a session exists and carries the passkey ceremony — stay
/// last in the migration while the thing it was blocking ships now.
///
/// The database refuses a site belonging to another tenant on its own:
/// `session_site_fk` is `(site_id, tenant_id) REFERENCES site(id, tenant_id)`.
/// That is a constraint rather than a check somebody remembers to write.
#[post("/sessions/site")]
pub async fn choose_site(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<ChooseSiteRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let wanted = body.into_inner().site_id;

    // That the site is this tenant's, checked where the tenant's rows are
    // visible. The FK would refuse it anyway; this is the difference between a
    // sentence and a constraint violation.
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let found = scope
        .run(move |tx| {
            Box::pin(async move {
                Ok(tx
                    .query_opt("SELECT 1 FROM site WHERE id = $1", &[&wanted])
                    .await?)
            })
        })
        .await?;
    if found.is_none() {
        return Err(ApiError::Rejected(
            "that is not a site this account can work at".into(),
        ));
    }

    let conn = state.pool.get().await?;
    crate::tenancy::ensure_app_role(&conn).await?;
    let response = issue_session(
        &conn,
        &req,
        who.person_id,
        Some(who.tenant_id),
        Some(wanted),
        None,
    )
    .await?;

    // **Revoked after the new one exists, not before.** If minting fails, the
    // caller keeps the session they had rather than being signed out by a
    // failed attempt to answer a question.
    let cookie = req.headers().get("cookie").and_then(|v| v.to_str().ok());
    let authz = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());
    if let Some(token) = auth::token_from_headers(cookie, authz) {
        conn.execute("SELECT session_revoke($1)", &[&auth::token_digest(&token)])
            .await?;
    }

    Ok(response)
}

/// What there is to pack, at the site the caller signed on at.
///
/// The four groups come back on each job rather than as four lists: the
/// grouping is `packing::stage`, one pure function that this and the
/// server-rendered page both call, so they cannot disagree about whether a
/// commitment is ready.
#[get("/packing")]
pub async fn packing_queue(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<PackingSearch>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let jobs = crate::packing::queue(&state, &who, query.into_inner().q.as_deref().unwrap_or("")).await?;
    Ok(HttpResponse::Ok().json(jobs))
}

/// What is waiting for you, at this site, now (D112).
///
/// The landing screen and the rail's badges both read this, so they cannot
/// disagree about the same number.
#[get("/work")]
pub async fn work_waiting(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let waiting = crate::work::waiting(&state, who.tenant_id, who.site_id).await?;
    Ok(HttpResponse::Ok().json(waiting))
}

/// Sign off. Revokes the record, so the token is dead everywhere at once —
/// which is the thing a server-side session buys over a signed claim.
#[delete("/sessions/current")]
pub async fn sign_off(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let cookie = req.headers().get("cookie").and_then(|v| v.to_str().ok());
    let authz = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());
    if let Some(token) = auth::token_from_headers(cookie, authz) {
        let conn = state.pool.get().await?;
        crate::tenancy::ensure_app_role(&conn).await?;
        conn.execute(
            "SELECT session_revoke($1)",
            &[&auth::token_digest(&token)],
        )
        .await?;
    }
    // Always the same answer, whether there was a session or not.
    Ok(HttpResponse::Ok()
        .insert_header(("set-cookie", auth::clearing_cookie(secure_cookies(&req))))
        .json(serde_json::json!({ "signed_off": true })))
}

/// Change your own password.
///
/// **`/credentials/password` rather than under `/sessions`**, because the thing
/// being changed belongs to the person and outlives the session. The session
/// only says whose password it is, and supplies the one token that survives the
/// change.
///
/// Nobody's but your own: the person is read from the resolved session and there
/// is no path here that takes one in the body. Resetting somebody else's is
/// administration, it needs the role vocabulary question 176 carries, and it is
/// deliberately not this.
#[post("/credentials/password")]
pub async fn change_password(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<crate::credentials::ChangePasswordRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    // The token this request arrived with, so the caller keeps the session they
    // are using while every other one is revoked. `caller` has already proved it
    // resolves; this reads it back rather than threading it out of there.
    let cookie = req.headers().get("cookie").and_then(|v| v.to_str().ok());
    let authz = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());
    let token = auth::token_from_headers(cookie, authz).ok_or(ApiError::Unauthenticated)?;

    let done =
        crate::credentials::change_password(&state.pool, who.person_id, &token, body.into_inner())
            .await?;
    Ok(HttpResponse::Ok().json(done))
}

// ---------------------------------------------------------------------------
// Import tokens, and the imports they authorise (D158)
// ---------------------------------------------------------------------------

/// Mint an import token. **The secret is in the answer and nowhere else.**
#[post("/tokens")]
pub async fn mint_api_token(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<crate::tokens::MintRequest>,
) -> Result<HttpResponse, ApiError> {
    // A session, deliberately: a token cannot mint another token. Otherwise the
    // ninety-day expiry is a formality, because the holder renews itself
    // forever and the credential outlives every reason it was issued for.
    let who = caller(&state, &req).await?;
    let mut scope = crate::tenancy::TenantScope::begin(&state.pool, who.tenant_id).await?;
    let minted = crate::tokens::mint(&mut scope, who.person_id, body.into_inner()).await?;
    Ok(HttpResponse::Created().json(minted))
}

/// Every token this tenant has, without the secrets.
#[get("/tokens")]
pub async fn list_api_tokens(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let mut scope = crate::tenancy::TenantScope::begin(&state.pool, who.tenant_id).await?;
    Ok(HttpResponse::Ok().json(crate::tokens::list(&mut scope).await?))
}

/// Withdraw one.
#[delete("/tokens/{id}")]
pub async fn revoke_api_token(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let mut scope = crate::tenancy::TenantScope::begin(&state.pool, who.tenant_id).await?;
    match crate::tokens::revoke(&mut scope, id.into_inner()).await? {
        true => Ok(HttpResponse::NoContent().finish()),
        false => Err(ApiError::NotFound),
    }
}

/// Store the file as it arrived, unless this is a dry run.
///
/// **A dry run records no arrival, and still answers whether there is one.** The
/// loader does its writes and rolls them back, so a receipt that outlived that
/// rollback would assert a file arrived which was never kept — but a dry run
/// that could not tell you the bytes have been seen before would be a report
/// that does not say what applying will do, and the whole point of doing the
/// real writes and undoing them is that the two agree.
async fn store(
    tx: &tokio_postgres::Transaction<'_>,
    tenant: Uuid,
    actor: crate::importing::received::Actor,
    filename: Option<&str>,
    payload: &[u8],
    apply: bool,
) -> Result<Option<crate::importing::received::Received>, ApiError> {
    if !apply {
        let seen = crate::importing::received::seen(tx, tenant, payload)
            .await
            .map_err(ApiError::Rejected)?;
        return Ok(seen.map(|party_message_id| crate::importing::received::Received {
            party_message_id,
            client_event_id: None,
        }));
    }
    crate::importing::received::record(tx, tenant, actor, filename, payload)
        .await
        .map(Some)
        .map_err(ApiError::Rejected)
}

/// Mark a stored arrival as one we declined to read in full.
///
/// `partial` is the honest status for a file whose row count disagreed with
/// what it was said to be: the bytes arrived and are kept, and we refused to
/// act on them. Nothing else writes this status, and bins left out for stating
/// no type deliberately do not — that is a policy refusal, not a parse outcome.
async fn read_partially(
    tx: &tokio_postgres::Transaction<'_>,
    arrival: Option<crate::importing::received::Received>,
) -> Result<(), ApiError> {
    let Some(a) = arrival else { return Ok(()) };
    if a.is_replay() {
        return Ok(());
    }
    crate::importing::received::partial(tx, a.party_message_id)
        .await
        .map_err(ApiError::Rejected)
}

/// Mark a stored arrival as read, once the loader has returned.
///
/// **A replay is skipped**, which covers two cases with one condition: a dry run
/// that found an arrival wrote nothing and must not now update it, and a repeat
/// apply is pointing at a row the first apply already marked.
async fn read_through(
    tx: &tokio_postgres::Transaction<'_>,
    arrival: Option<crate::importing::received::Received>,
) -> Result<(), ApiError> {
    let Some(a) = arrival else { return Ok(()) };
    if a.is_replay() {
        return Ok(());
    }
    crate::importing::received::parsed(tx, a.party_message_id)
        .await
        .map_err(ApiError::Rejected)
}

/// What an import may say about how to read the file.
#[derive(Deserialize, Debug)]
pub struct ImportQuery {
    /// **Absent means dry run**, which is the CLI importers' documented
    /// contract. An HTTP import that wrote by default would be the same
    /// importer with the safety removed, which is not the same importer.
    #[serde(default)]
    pub apply: bool,
    /// What to call bins that state no type. Without it they are left out and
    /// counted, because `location.kind` is NOT NULL and has no value meaning
    /// "nobody said".
    pub assume_kind: Option<String>,
    #[serde(default)]
    pub include_external: bool,
    /// What the file was called where it came from, kept as
    /// `party_message.transport_ref`. Optional because a request body has no
    /// name of its own and inventing one would be worse than leaving it null.
    pub filename: Option<String>,
    /// **When the report was taken.** Required by the stock import and never
    /// defaulted: migration 86 exists because a six-day-old sheet was read as
    /// current, and a number that cannot say its own age gets believed forever.
    pub as_at: Option<DateTime<Utc>>,
    /// Which feed this export is, not which file. A second load under the same
    /// source replaces the first, because a snapshot is not cumulative.
    pub source: Option<String>,
    /// How many rows the export was said to have, read off the search that
    /// produced it. Absent means unchecked, and the report says so.
    pub expect: Option<usize>,
}

/// The arrival, once it is on file.
///
/// Absent on a dry run, because a dry run keeps nothing. `replay` is true when
/// these exact bytes had already arrived, in which case `party_message_id` names
/// the arrival that is on file rather than a new one.
#[derive(Serialize, Debug)]
pub struct FileArrival {
    pub party_message_id: Uuid,
    pub replay: bool,
}

impl From<crate::importing::received::Received> for FileArrival {
    fn from(r: crate::importing::received::Received) -> Self {
        FileArrival { party_message_id: r.party_message_id, replay: r.is_replay() }
    }
}

#[derive(Serialize, Debug)]
pub struct ImportReport {
    pub survey: crate::importing::bins::Survey,
    pub loaded: crate::importing::bins::Loaded,
    pub arrival: Option<FileArrival>,
}

#[derive(Serialize, Debug)]
pub struct StockImportReport {
    pub survey: crate::importing::stock::StockSurvey,
    pub loaded: crate::importing::stock::StockLoaded,
    pub arrival: Option<FileArrival>,
    /// Set when the row count disagreed with `expect`, in which case nothing
    /// was loaded and the stored arrival is marked `partial`.
    pub refused: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ItemImportReport {
    pub survey: crate::importing::items::ItemSurvey,
    pub loaded: crate::importing::items::ItemsLoaded,
    pub arrival: Option<FileArrival>,
}

/// Load the item master, from the CSV the system of record exports.
///
/// Nine thousand codes and their descriptions. Separate from the prepack list,
/// which carries the measurements and the judgement — this is what gives the
/// capture worklist something to be about.
#[post("/import/items")]
pub async fn import_items(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<ImportQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let machine = machine(&state, &req).await?;

    let rows = crate::importing::items::read(body.as_ref()).map_err(ApiError::Rejected)?;
    if rows.is_empty() {
        return Err(ApiError::Rejected(
            "no rows with a `Code` — is this the item export?".into(),
        ));
    }
    let survey = crate::importing::items::survey(&rows);

    let mut scope = crate::tenancy::TenantScope::begin(&state.pool, machine.tenant_id).await?;
    let tenant = scope.tenant();
    let apply = query.apply;
    let actor = crate::importing::received::Actor::Token(machine.token_id);
    let filename = query.filename.clone();
    let (loaded, arrival) = scope
        .run(move |tx| {
            Box::pin(async move {
                let arrival = store(tx, tenant, actor, filename.as_deref(), &body, apply).await?;
                let loaded = crate::importing::items::load(tx, tenant, &rows, apply)
                    .await
                    .map_err(ApiError::Rejected)?;
                read_through(tx, arrival).await?;
                Ok((loaded, arrival))
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(ItemImportReport {
        survey,
        loaded,
        arrival: arrival.map(FileArrival::from),
    }))
}

/// Load the inventory balance export, into `reported_stock`.
///
/// **Not into `stock`**, which is folded from this system's own ledger and which
/// the application role cannot write. Migration 86 sets out why at length: these
/// rows are a report from somewhere else, believed because of where they came
/// from, and turning them into movements would have this system assert it holds
/// goods it has never recorded receiving.
#[post("/import/stock")]
pub async fn import_stock(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<ImportQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let machine = machine(&state, &req).await?;

    // Both required, and neither has a defensible default. `as_at` is migration
    // 86's whole third section; `source` is the identity two loads are compared
    // on, and guessing it would make one export silently replace another.
    let Some(as_at) = query.as_at else {
        return Err(ApiError::Rejected(
            "as_at is required: a report that cannot say when it was taken              gets read as current forever"
                .into(),
        ));
    };
    let Some(source) = query.source.clone().filter(|s| !s.trim().is_empty()) else {
        return Err(ApiError::Rejected(
            "source is required: it names the feed, and a second export under              the same source replaces the first"
                .into(),
        ));
    };

    let rows = crate::importing::stock::read(body.as_ref()).map_err(ApiError::Rejected)?;
    if rows.is_empty() {
        return Err(ApiError::Rejected(
            "no rows with an item and a quantity — is this the inventory balance export?".into(),
        ));
    }
    let survey = crate::importing::stock::survey(&rows);
    let refused = crate::importing::stock::shortfall(rows.len(), query.expect);

    let mut scope = crate::tenancy::TenantScope::begin(&state.pool, machine.tenant_id).await?;
    let tenant = scope.tenant();
    let apply = query.apply;
    let actor = crate::importing::received::Actor::Token(machine.token_id);
    let filename = query.filename.clone();
    let short = refused.clone();
    let loaded = scope
        .run(move |tx| {
            Box::pin(async move {
                let arrival = store(tx, tenant, actor, filename.as_deref(), &body, apply).await?;
                // **The file is kept and the load is not run.** A truncated
                // inventory export is the failure that reads as success: a bin
                // missing from it looks empty, and an empty bin is an
                // instruction to put something there.
                if short.is_some() {
                    read_partially(tx, arrival).await?;
                    return Ok((crate::importing::stock::StockLoaded::default(), survey, arrival));
                }
                let l = crate::importing::stock::load(
                    tx, tenant, &rows, as_at, &source, apply,
                )
                .await
                .map_err(ApiError::Rejected)?;
                read_through(tx, arrival).await?;
                Ok((l, survey, arrival))
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(StockImportReport {
        survey: loaded.1,
        loaded: loaded.0,
        arrival: loaded.2.map(FileArrival::from),
        refused,
    }))
}

/// Load the bin list, from the CSV the system of record exports.
///
/// **The body is the file, unaltered.** Re-shaping it into JSON first would put
/// a translation between NetSuite and this, and a translation is a place for
/// bugs that neither side can see — the export *is* the interface, and the
/// importer already knows how to read it.
#[post("/import/bins")]
pub async fn import_bins(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<ImportQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let machine = machine(&state, &req).await?;

    let rows = crate::importing::bins::read(body.as_ref()).map_err(ApiError::Rejected)?;
    if rows.is_empty() {
        return Err(ApiError::Rejected(
            "no rows with a `Bin Number` — is this the bin export?".into(),
        ));
    }
    let survey = crate::importing::bins::survey(&rows, query.include_external);

    let mut scope = crate::tenancy::TenantScope::begin(&state.pool, machine.tenant_id).await?;
    let tenant = scope.tenant();
    let assume = query.assume_kind.clone();
    let apply = query.apply;
    let actor = crate::importing::received::Actor::Token(machine.token_id);
    let filename = query.filename.clone();
    let loaded = scope
        .run(move |tx| {
            Box::pin(async move {
                let arrival = store(tx, tenant, actor, filename.as_deref(), &body, apply).await?;
                let l = crate::importing::bins::load(
                    tx, tenant, &rows, &survey, assume.as_deref(), apply,
                )
                .await
                .map_err(ApiError::Rejected)?;
                read_through(tx, arrival).await?;
                Ok((l, survey, arrival))
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(ImportReport {
        survey: loaded.1,
        loaded: loaded.0,
        arrival: loaded.2.map(FileArrival::from),
    }))
}

/// One item fulfilment, as the NetSuite page shows it.
///
/// **JSON rather than a file**, which the other imports are not, because there
/// is no file: this is sent by a userscript from the item fulfilment page while
/// somebody has it open, and the page is the only export NetSuite gives us
/// without API access. The body is still stored exactly as it arrived, like an
/// export is.
#[derive(Deserialize, Debug)]
pub struct FulfilmentIntake {
    /// The sales order's document number. `Sales Order #SO1234` is read as
    /// `SO1234`, because that is how the page's "Created From" renders it.
    pub order: String,
    #[serde(default)]
    pub customer: String,
    #[serde(default)]
    pub po_ref: String,
    /// Day-first, as NetSuite renders it here. Blank means now.
    #[serde(default)]
    pub date: String,
    pub lines: Vec<FulfilmentIntakeLine>,
}

#[derive(Deserialize, Debug)]
pub struct FulfilmentIntakeLine {
    /// The sales order line this fulfils (`orderline` on the page).
    pub line: i32,
    /// As the page shows it; `Parent : Child` is read as the child.
    pub item: String,
    #[serde(default)]
    pub description: Option<String>,
    /// The warehouse: `Melbourne Warehouse`.
    pub location: String,
    /// What this fulfilment will pick.
    pub quantity: f64,
    /// What the sales order line still has outstanding, when the page shows
    /// it. The page never shows what was *ordered*, so the order line records
    /// the most we know was outstanding: this, or failing it, `quantity`.
    #[serde(default)]
    pub remaining: Option<f64>,
}

#[derive(Serialize, Debug)]
pub struct FulfilmentIntakeReport {
    pub order: String,
    pub lines: usize,
    pub loaded: crate::importing::orders::OrdersLoaded,
    pub arrival: Option<FileArrival>,
}

/// `Sales Order #SO1234` -> `SO1234`. Anything else, trimmed.
fn sales_order_number(raw: &str) -> String {
    let t = raw.trim();
    t.rsplit_once('#').map(|(_, n)| n.trim()).unwrap_or(t).to_string()
}

/// Load one item fulfilment's lines as work to pick (phase 1 of the Spork plan).
///
/// **Idempotent per order**, because the sender will resend: the page can be
/// opened twice, and a device resends after a failover (D170). The same order
/// twice writes nothing the second time, and a line whose quantities moved is
/// reported in `differs` and left alone. Dry run unless `?apply=true`, like
/// every import.
///
/// Items the catalogue lacks are created from the page's code and description:
/// the page is as much evidence an item exists as an order is that a warehouse
/// does. A warehouse with no clock on file is still refused, by line.
#[post("/import/fulfilment")]
pub async fn import_fulfilment(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<ImportQuery>,
    body: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let machine = machine(&state, &req).await?;

    let intake: FulfilmentIntake = serde_json::from_slice(&body)
        .map_err(|e| ApiError::Rejected(format!("not a fulfilment: {e}")))?;
    let order = sales_order_number(&intake.order);
    if order.is_empty() {
        return Err(ApiError::Rejected(
            "order is required: it is the key a resend is recognised by".into(),
        ));
    }
    if intake.lines.is_empty() {
        return Err(ApiError::Rejected("a fulfilment with no lines has nothing to pick".into()));
    }
    let customer = crate::orders::customer_name(&intake.customer);
    let rows: Vec<crate::importing::orders::Line> = intake
        .lines
        .iter()
        .map(|l| {
            let quantity = l.quantity.round() as i64;
            crate::importing::orders::Line {
                doc: order.clone(),
                line_no: l.line,
                item: crate::orders::item_code(&l.item),
                description: l.description.clone(),
                ordered: l.remaining.map(|r| r.round() as i64).unwrap_or(quantity).max(quantity),
                outstanding: quantity,
                customer: customer.clone(),
                po_ref: intake.po_ref.trim().to_string(),
                location: l.location.trim().to_string(),
                date: intake.date.clone(),
            }
        })
        .collect();

    let mut scope = crate::tenancy::TenantScope::begin(&state.pool, machine.tenant_id).await?;
    let tenant = scope.tenant();
    let apply = query.apply;
    let actor = crate::importing::received::Actor::Token(machine.token_id);
    let filename = query.filename.clone();
    let options = crate::importing::orders::Options { create_items: true };
    let (loaded, arrival) = scope
        .run(move |tx| {
            Box::pin(async move {
                let arrival = store(tx, tenant, actor, filename.as_deref(), &body, apply).await?;
                let loaded = crate::importing::orders::load(tx, tenant, &rows, options, apply)
                    .await
                    .map_err(ApiError::Rejected)?;
                read_through(tx, arrival).await?;
                Ok((loaded, arrival))
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(FulfilmentIntakeReport {
        order,
        lines: intake.lines.len(),
        loaded,
        arrival: arrival.map(FileArrival::from),
    }))
}

// ---------------------------------------------------------------------------
// Packaging presets
// ---------------------------------------------------------------------------

#[derive(Serialize, Debug, PartialEq)]
pub struct PackageTypeRow {
    pub id: Uuid,
    pub name: String,
    /// What a carrier calls it — PAL, CTN, SKD.
    pub carrier_package_code: Option<String>,
    /// True when the size is the preset's rather than a question for the
    /// operator. False is a pallet: length and width are standard, the stack
    /// height is not.
    pub dimensions_fixed: bool,
    pub length_mm: Option<i32>,
    pub width_mm: Option<i32>,
    pub height_mm: Option<i32>,
    pub tare_weight_g: Option<i64>,
    pub reusable: bool,
    pub max_payload_g: Option<i64>,
    /// True for a preset this tenant owns, false for one the platform ships.
    pub tenant_owned: bool,
}

/// The packaging presets, shipped and tenant-owned together.
///
/// The walkthrough calls this *"the useful domain data"* and says where it
/// should not be: *"buried in a NetSuite tab."* It is what turns the pack
/// screen's only genuinely variable stage into two fields instead of six —
/// *"for pallets, the fields typically changed are weight and height"* — because
/// everything else comes from the preset.
///
/// Reads shared and tenant rows in one list, which is D55's read policy doing
/// exactly what it is for. `tenant_owned` distinguishes them so a screen can say
/// which ones this business may edit; the write policy already enforces it.
#[derive(Serialize, Debug)]
pub struct ItemMeasurements {
    pub packaging_level: String,
    /// `own` when every number here was measured against this code, `style`
    /// when they were all inherited, `mixed` when some of each. A screen that
    /// cannot tell those apart reports a number nobody took against this code
    /// as though somebody had.
    pub source: String,
    /// The style it came from, when it was inherited.
    pub style_code: Option<String>,
    /// Absent when no case pack was in force, which only happens at `each`.
    pub item_packing_config_id: Option<Uuid>,
    /// Canonical throughout: millimetres and grams, per Principle 5. What was
    /// typed is on the observation.
    pub length_mm: Option<i64>,
    pub width_mm: Option<i64>,
    pub height_mm: Option<i64>,
    pub gross_weight_g: Option<i64>,
    pub net_weight_g: Option<i64>,
    pub tare_weight_g: Option<i64>,
    /// How the newest of these was come by: transcribed from a supplier's list,
    /// read off an instrument, asserted. A cube from a prepack sheet and a cube
    /// from a cubing scanner are both facts and are not the same fact.
    pub method: Option<String>,
    pub observed_at: Option<DateTime<Utc>>,
}

/// What we believe a thing of this kind measures, per packaging level.
///
/// **The kind, not the object.** `GET /packages/{id}` answers what the carton in
/// front of you measures; this answers what a carton of these measures. Both are
/// `observation` rows against an `observable`, which is the whole reason that
/// table has an item arm with a packaging level on it.
///
/// One row per level that anything has ever been recorded against. A level with
/// no observations is absent rather than a row of nulls: nobody has said, and a
/// screen that cannot tell "nobody said" from "it measures nothing" will show
/// zero and be believed.
#[get("/items/{id}/measurements")]
pub async fn item_measurements(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let item_id = path.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let rows = scope
        .run(move |tx| {
            Box::pin(async move {
                // `observation_current` is the fold that already answers "the
                // winning value per subject per metric", so this pivots it
                // rather than re-deciding which observation wins. D25: one
                // answer, one place that computes it.
                // **Most specific wins, per fact rather than per subject.**
                // D22's rule one level down, applied to each metric on its own:
                // a code weighed here but never measured still inherits its
                // style's dimensions. Resolving by subject instead would let one
                // weight against the SKU hide every other number the style has,
                // which is worse than not having recorded the weight.
                //
                // `DISTINCT ON (level, metric) ORDER BY … rank` is the whole
                // resolution. A style is exactly one hop, so there is no closure
                // table here and no walk.
                let rows = tx
                    .query(
                        "WITH subject AS (
                             SELECT o.id, o.packaging_level, o.item_packing_config_id,
                                    'own'::text AS src, NULL::text AS style_code, 0 AS rank
                               FROM observable o
                              WHERE o.item_id = $1
                             UNION ALL
                             SELECT o.id, o.packaging_level, o.item_packing_config_id,
                                    'style', s.code, 1
                               FROM observable o
                               JOIN item_style s ON s.id = o.item_style_id
                               JOIN item i ON i.style_id = s.id
                              WHERE i.id = $1
                         ),
                         resolved AS (
                             SELECT DISTINCT ON (s.packaging_level, oc.metric_id)
                                    s.packaging_level, s.item_packing_config_id,
                                    s.src, s.style_code, oc.metric_id,
                                    oc.value_numeric, oc.method, oc.observed_at
                               FROM subject s
                               JOIN observation_current oc ON oc.observable_id = s.id
                              ORDER BY s.packaging_level, oc.metric_id, s.rank
                         )
                         SELECT r.packaging_level::text,
                                (array_agg(r.item_packing_config_id
                                    ORDER BY CASE WHEN r.src = 'own' THEN 0 ELSE 1 END))[1],
                                (max(r.value_numeric)
                                     FILTER (WHERE m.code = 'length'))::bigint,
                                (max(r.value_numeric)
                                     FILTER (WHERE m.code = 'width'))::bigint,
                                (max(r.value_numeric)
                                     FILTER (WHERE m.code = 'height'))::bigint,
                                (max(r.value_numeric)
                                     FILTER (WHERE m.code = 'gross_weight'))::bigint,
                                (max(r.value_numeric)
                                     FILTER (WHERE m.code = 'net_weight'))::bigint,
                                (max(r.value_numeric)
                                     FILTER (WHERE m.code = 'tare_weight'))::bigint,
                                (array_agg(r.method ORDER BY r.observed_at DESC))[1],
                                max(r.observed_at),
                                CASE WHEN bool_and(r.src = 'own') THEN 'own'
                                     WHEN bool_and(r.src = 'style') THEN 'style'
                                     ELSE 'mixed' END,
                                max(r.style_code)
                           FROM resolved r
                           JOIN metric m ON m.id = r.metric_id
                          GROUP BY r.packaging_level
                          ORDER BY r.packaging_level",
                        &[&item_id],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| {
                        // Cast to bigint in the query rather than decoded as
                        // a decimal here: `value_numeric` is numeric because a
                        // temperature is not whole, and a length or a mass in
                        // canonical units always is.
                        ItemMeasurements {
                            packaging_level: r.get(0),
                            item_packing_config_id: r.get(1),
                            length_mm: r.get(2),
                            width_mm: r.get(3),
                            height_mm: r.get(4),
                            gross_weight_g: r.get(5),
                            net_weight_g: r.get(6),
                            tare_weight_g: r.get(7),
                            method: r.get(8),
                            observed_at: r.get(9),
                            source: r.get(10),
                            style_code: r.get(11),
                        }
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(rows))
}

#[get("/package-types")]
pub async fn package_types(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let rows = scope
        .run(|tx| {
            Box::pin(async move {
                // No WHERE tenant_id: row-level security applies the predicate,
                // and adding one would be worse than redundant.
                let rows = tx
                    .query(
                        "SELECT id, name, carrier_package_code, dimensions_fixed,
                                length_mm, width_mm, height_mm, tare_weight_g,
                                reusable, max_payload_g, tenant_id IS NOT NULL
                           FROM package_type
                          WHERE effective_from <= CURRENT_DATE
                          ORDER BY tenant_id IS NULL, name",
                        &[],
                    )
                    .await?;
                Ok(rows
                    .iter()
                    .map(|r| PackageTypeRow {
                        id: r.get(0),
                        name: r.get(1),
                        carrier_package_code: r.get(2),
                        dimensions_fixed: r.get(3),
                        length_mm: r.get(4),
                        width_mm: r.get(5),
                        height_mm: r.get(6),
                        tare_weight_g: r.get(7),
                        reusable: r.get(8),
                        max_payload_g: r.get(9),
                        tenant_owned: r.get(10),
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(rows))
}



/// What to pick at a site, in walking order, with a picture of each thing.
///
/// What is expected here and has not all arrived.
///
/// A read. `POST /receipts` has existed since migration 21 and nothing has ever
/// called it from a screen, because nothing said what to call it about.
#[get("/sites/{id}/receiving")]
pub async fn receiving_list(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    q: web::Query<PickingQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let screen =
        crate::receiving_list::screen(&state, &who, path.into_inner(), q.limit.unwrap_or(200))
            .await?;
    Ok(HttpResponse::Ok().json(screen))
}

/// What is on the dock with nowhere to live yet.
///
/// A read. The put-away itself is `POST /moves` with `reason = "putaway"`, which
/// migration 59 put in the CHECK on purpose and which nothing had ever called.
#[get("/sites/{id}/putaway")]
pub async fn putaway_list(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    q: web::Query<PickingQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let screen =
        crate::putaway_list::screen(&state, &who, path.into_inner(), q.limit.unwrap_or(200)).await?;
    Ok(HttpResponse::Ok().json(screen))
}

/// A read. `crate::picking_list` says why recording the pick is not here.
#[get("/sites/{id}/picking")]
pub async fn picking_list(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    q: web::Query<PickingQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let screen =
        crate::picking_list::screen(&state, &who, path.into_inner(), q.limit.unwrap_or(200)).await?;
    Ok(HttpResponse::Ok().json(screen))
}

#[derive(Deserialize)]
pub struct PickingQuery {
    pub limit: Option<i64>,
}

// ---------------------------------------------------------------------------
// Evidence: a look offered in support of a record
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct RecordEvidenceRequest {
    // ── the record this supports. Exactly one. ──────────────────────────
    pub discrepancy_id: Option<Uuid>,
    pub goods_receipt_line_id: Option<Uuid>,
    pub stock_count_id: Option<Uuid>,
    pub stock_movement_id: Option<Uuid>,
    pub package_event_id: Option<Uuid>,

    // ── the thing that was looked at. Exactly one, and not inferred from
    //    the record: a finding names an item, a location and a package
    //    without those being exclusive, so which one the operator held up
    //    to the camera is theirs to say. ──────────────────────────────────
    pub package_id: Option<Uuid>,
    pub location_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub item_id: Option<Uuid>,
    pub item_style_id: Option<Uuid>,
    pub item_part_id: Option<Uuid>,
    /// Required with `item_id` or `item_style_id`, refused with anything else.
    pub packaging_level: Option<String>,

    /// Why this is being offered, in the words of whoever offered it.
    pub note: Option<String>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct RecordEvidenceResponse {
    pub evidence_id: Uuid,
    /// **What the photographs hang off.** The same shape a capture session
    /// answers with: `POST /observations/{id}/images/{face}` next.
    pub observation_event_id: Uuid,
    pub observable_id: Uuid,
}

/// Offer a look in support of a record, and get back the event to photograph.
///
/// **One route, five records, and none of them require it.** Findings,
/// receiving, counts, adjustments and despatch can each carry evidence and none
/// of them gains an obligation — no write path changed, nothing became NOT
/// NULL, and a record with no evidence is the ordinary case. Adding the sixth
/// is a column on `evidence` and a line here.
///
/// # Why this is not `POST /observations` with no measurements
///
/// The obvious build relaxes the observation writer to accept an empty
/// `measurements` array, and it breaks idempotency in a way that loses data.
/// An accidentally empty request would **claim the `client_event_id`**; the
/// client retries with the real figures, hits D25's replay branch, and is
/// handed back the empty act's nothing. *"An observation of nothing is not an
/// observation"* is protecting that, not tidiness.
///
/// So a look that produces a picture instead of a figure is its own act with
/// its own route, which is the shape `POST /weighings` already has over the
/// same two tables.
///
/// # One act
///
/// The event and the link are written in one transaction under one
/// `client_event_id`, so there is no window in which a look exists that nothing
/// points at. The link is the act's fact.
#[post("/evidence")]
pub async fn record_evidence(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RecordEvidenceRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let records: [(&str, Option<Uuid>); 5] = [
                    ("discrepancy_id", body.discrepancy_id),
                    ("goods_receipt_line_id", body.goods_receipt_line_id),
                    ("stock_count_id", body.stock_count_id),
                    ("stock_movement_id", body.stock_movement_id),
                    ("package_event_id", body.package_event_id),
                ];
                let named: Vec<&(&str, Option<Uuid>)> =
                    records.iter().filter(|(_, id)| id.is_some()).collect();
                if named.len() != 1 {
                    return Err(ApiError::Rejected(
                        "name exactly one record this supports: discrepancy_id, \
                         goods_receipt_line_id, stock_count_id, stock_movement_id or \
                         package_event_id"
                            .into(),
                    ));
                }
                let (record_column, record_id) = (named[0].0, named[0].1.unwrap());

                let subject_ref = SubjectRef {
                    package_id: body.package_id,
                    location_id: body.location_id,
                    lot_id: body.lot_id,
                    item_id: body.item_id,
                    item_style_id: body.item_style_id,
                    item_part_id: body.item_part_id,
                    packaging_level: body.packaging_level.clone(),
                };
                if subject_ref.named() != 1 {
                    return Err(ApiError::Rejected(
                        "name exactly one thing that was looked at: package_id, \
                         location_id, lot_id, item_id, item_style_id or item_part_id"
                            .into(),
                    ));
                }
                if subject_ref.catalogue().is_some() == body.packaging_level.is_none() {
                    return Err(ApiError::Rejected(
                        "an item or style subject names a packaging_level and nothing \
                         else does"
                            .into(),
                    ));
                }

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;

                // **A replay answers with what the act produced.** The link and
                // the event are both found by the client event that made them,
                // so a retry after a timeout returns the same ids and the
                // photographs still have somewhere to go.
                if act.is_replay() {
                    let row = tx
                        .query_opt(
                            "SELECT ev.id, ev.observation_event_id, e.observable_id
                               FROM evidence ev
                               JOIN observation_event e ON e.id = ev.observation_event_id
                              WHERE e.client_event_id = $1",
                            &[&body.client_event_id],
                        )
                        .await?
                        .ok_or_else(|| {
                            ApiError::Rejected(
                                "that client_event_id belongs to a different act".into(),
                            )
                        })?;
                    return Ok(RecordEvidenceResponse {
                        evidence_id: row.get(0),
                        observation_event_id: row.get(1),
                        observable_id: row.get(2),
                    });
                }

                let (observable_id, _kind) =
                    observable_for(tx, tenant, &subject_ref, body.occurred_at).await?;

                // `photographed` rather than `instrument`. A camera is not a
                // measuring instrument, and saying it is would eventually let
                // revalidation read a photograph as evidence that something had
                // been weighed. Migration 83.
                let observation_event_id: Uuid = tx
                    .query_one(
                        "INSERT INTO observation_event (
                             tenant_id, client_event_id, observable_id, observed_at,
                             recorded_by_id, method, ingestion_channel)
                         VALUES ($1, $2, $3, $4, $5, 'photographed', 'api')
                         RETURNING id",
                        &[
                            &tenant,
                            &body.client_event_id,
                            &observable_id,
                            &body.occurred_at,
                            &who.person_id,
                        ],
                    )
                    .await?
                    .get(0);

                let evidence_id: Uuid = tx
                    .query_one(
                        &format!(
                            "INSERT INTO evidence
                                 (tenant_id, observation_event_id, {record_column}, note)
                             VALUES ($1, $2, $3, $4)
                             RETURNING id"
                        ),
                        &[&tenant, &observation_event_id, &record_id, &body.note],
                    )
                    .await?
                    .get(0);

                Ok(RecordEvidenceResponse {
                    evidence_id,
                    observation_event_id,
                    observable_id,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// The subject registry, get-or-create
// ---------------------------------------------------------------------------

/// What a look was taken of, as a caller names it.
///
/// **One resolution, two writers.** `POST /observations` and `POST /evidence`
/// both need a row in `observable` for the thing in front of the operator, and
/// the rules for getting one are not small — the case pack in force at the
/// moment named on the subject, `each` deliberately carrying none, a part
/// carrying no level at all. A second copy of that would be a second answer to
/// *which subject is this*, and the two would disagree the first time a case
/// pack was corrected.
///
/// The same argument `capture::classified_subjects` makes about the worklist
/// and the scan path sharing one enumeration, one level down.
pub struct SubjectRef {
    pub package_id: Option<Uuid>,
    pub location_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub item_id: Option<Uuid>,
    pub item_style_id: Option<Uuid>,
    pub item_part_id: Option<Uuid>,
    /// Required with `item_id` or `item_style_id`, refused with anything else.
    pub packaging_level: Option<String>,
}

impl SubjectRef {
    /// How many arms are named. Exactly one is the only legal answer, and the
    /// callers say so in their own words because the sentence differs.
    pub fn named(&self) -> usize {
        [
            self.package_id,
            self.location_id,
            self.lot_id,
            self.item_id,
            self.item_style_id,
            self.item_part_id,
        ]
        .iter()
        .filter(|s| s.is_some())
        .count()
    }

    /// The catalogue arms, which are the two that take a packaging level.
    pub fn catalogue(&self) -> Option<Uuid> {
        self.item_id.or(self.item_style_id)
    }
}

/// Find or create the registry row for a subject.
///
/// `observable` is one row per thing under a partial unique index per arm, so
/// the second observer of a pallet finds the first one's row rather than
/// minting a rival.
pub async fn observable_for(
    tx: &tokio_postgres::Transaction<'_>,
    tenant: Uuid,
    subject: &SubjectRef,
    occurred_at: DateTime<Utc>,
) -> Result<(Uuid, &'static str), ApiError> {
    if let Some(catalogue_id) = subject.catalogue() {
        let styled = subject.item_style_id.is_some();
        let level = subject.packaging_level.clone().unwrap_or_default();
        // **The case pack in force when this was true, named on the subject.**
        // A carton is only a definite physical object relative to a case pack,
        // so a corrected case pack must not silently rewrite what a carton
        // measured last year — which is why `observable` carries the config id
        // rather than resolving it at read time. D23, D58.
        //
        // `each` needs none: a pair of boots is a definite thing on its own,
        // and `observable_item_config_ck` requires the column to be absent
        // there rather than merely allowing it.
        let config_id: Option<Uuid> = if level == "each" {
            None
        } else {
            // A style's case pack is whatever its members agree on; taking the
            // newest in force is the same rule as for one item, applied over
            // the style's variants.
            let sql = if styled {
                // **`$2::timestamptz::date`, never `$2::date`.** One cast makes
                // Postgres infer the parameter as that type, and the driver
                // then refuses to send a timestamptz as a date. Third time this
                // shape has bitten: `$8::numeric` and `$8::packaging_level`
                // were the first two.
                "SELECT c.id FROM item_packing_config c
                   JOIN item i ON i.id = c.item_id
                  WHERE i.style_id = $1
                    AND c.effective_from <= $2::timestamptz::date
                  ORDER BY c.effective_from DESC, c.id DESC LIMIT 1"
            } else {
                "SELECT id FROM item_packing_config
                  WHERE item_id = $1
                    AND effective_from <= $2::timestamptz::date
                  ORDER BY effective_from DESC, id DESC LIMIT 1"
            };
            let found = tx
                .query_opt(sql, &[&catalogue_id, &occurred_at])
                .await?
                .map(|r| r.get(0));
            if found.is_none() {
                return Err(ApiError::Rejected(format!(
                    "no item_packing_config in force for that {} at {}, so a \
                     {level} of it is not yet a definite thing to measure",
                    if styled { "style" } else { "item" },
                    occurred_at.date_naive()
                )));
            }
            found
        };

        let column = if styled { "item_style_id" } else { "item_id" };
        tx.execute(
            &format!(
                "INSERT INTO observable
                     (tenant_id, {column}, packaging_level, item_packing_config_id)
                 VALUES ($1, $2, $3::text::packaging_level, $4)
                 ON CONFLICT (tenant_id, {column}, packaging_level,
                              item_packing_config_id)
                      WHERE {column} IS NOT NULL
                 DO NOTHING"
            ),
            &[&tenant, &catalogue_id, &level, &config_id],
        )
        .await?;
        let id: Uuid = tx
            .query_one(
                &format!(
                    "SELECT id FROM observable
                      WHERE tenant_id = $1 AND {column} = $2
                        AND packaging_level = $3::text::packaging_level
                        AND item_packing_config_id IS NOT DISTINCT FROM $4"
                ),
                &[&tenant, &catalogue_id, &level, &config_id],
            )
            .await?
            .get(0);
        Ok((id, if styled { "item_style" } else { "item" }))
    } else {
        let (subject_col, subject_id, subject_kind) = match (
            subject.package_id,
            subject.location_id,
            subject.lot_id,
            subject.item_part_id,
        ) {
            (Some(id), _, _, _) => ("package_id", id, "package"),
            (_, Some(id), _, _) => ("location_id", id, "location"),
            (_, _, Some(id), _) => ("lot_id", id, "lot"),
            // A part carries no level, so it registers like a package rather
            // than like the item it belongs to.
            (_, _, _, Some(id)) => ("item_part_id", id, "item_part"),
            _ => {
                return Err(ApiError::Rejected(
                    "an observation of nothing has no subject to register".into(),
                ))
            }
        };
        let upsert = format!(
            "INSERT INTO observable (tenant_id, {subject_col})
             VALUES ($1, $2)
             ON CONFLICT (tenant_id, {subject_col}) WHERE {subject_col} IS NOT NULL
             DO NOTHING"
        );
        tx.execute(upsert.as_str(), &[&tenant, &subject_id]).await?;
        let select =
            format!("SELECT id FROM observable WHERE tenant_id = $1 AND {subject_col} = $2");
        let id: Uuid = tx
            .query_one(select.as_str(), &[&tenant, &subject_id])
            .await?
            .get(0);
        Ok((id, subject_kind))
    }
}

// ---------------------------------------------------------------------------
// Record a measurement (the second spine table)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
pub struct Measurement {
    /// `metric.code`: gross_weight, height, length, width, tare_weight,
    /// net_weight, temperature.
    pub metric: String,
    /// **A string, not a number.** This is the only record of what the operator
    /// typed and a JSON float cannot hold 12.1. See [`observing`].
    ///
    /// Absent exactly when `absent_reason` is set.
    pub entered_value: Option<String>,
    /// `unit.code` — kg, g, mm, m, in. Must measure the metric's dimension.
    ///
    /// Absent exactly when `absent_reason` is set: a thing that has no height
    /// has no height in millimetres either.
    pub unit: Option<String>,
    /// **An answer, not a gap.** `not_applicable` says this subject has no such
    /// measurement — a two-part set has no bounding box, and the operator who
    /// looked at it knows that. `unreadable` says there was one and it could not
    /// be got. D138, and [`observing::check_absent_reason`] for the two words
    /// the column holds that a capture may not write.
    pub absent_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct RecordObservationRequest {
    /// Exactly one subject.
    pub package_id: Option<Uuid>,
    pub location_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    /// **The catalogue arm.** What a carton of this item measures, as against
    /// what the carton in front of you measures — `package_id` is a particular
    /// physical object and this is the kind of object. A prepack list is
    /// thousands of these.
    ///
    /// Requires `packaging_level`, because a pair of boots and a carton of them
    /// are different subjects with different sizes.
    pub item_id: Option<Uuid>,
    /// **The style arm.** What a carton of `SKU-0180` measures, standing for
    /// every size in it. Migration 73: writing the style's numbers onto each
    /// variant would claim four measurements where one carton was weighed.
    pub item_style_id: Option<Uuid>,
    /// **The part arm.** A pan and its 1200mm handle under one sellable code,
    /// measured separately because the set has no bounding box of its own.
    /// Migration 82. Takes no `packaging_level`: there is one handle, and until
    /// handles arrive in a carton of handles there is no such object.
    pub item_part_id: Option<Uuid>,
    /// `packaging_level`: each, inner, carton, layer, pallet. Only with
    /// `item_id` or `item_style_id`, and required with either.
    ///
    /// Above `each` the writer resolves the `item_packing_config` in force at
    /// `occurred_at` and names it on the subject, so a case pack corrected next
    /// year cannot retroactively change the dimensions of the carton this
    /// described. D23 and D58.
    pub packaging_level: Option<String>,
    pub measurements: Vec<Measurement>,
    /// **What state the subject was in.** `presentation.code`: as_supplied,
    /// assembled, knocked_down, folded, rolled, flat, compressed.
    ///
    /// A property of the act rather than of any one figure, which is why it sits
    /// beside `method` and not on the measurements — `observation_event` names
    /// one subject, so one look is one arrangement. Required for a length at
    /// `each`, where arranging the thing is part of measuring it. D138.
    pub presentation: Option<String>,
    /// `observation_method`: instrument, scan, keyed, derived, estimated,
    /// transcribed, asserted. Defaults to `keyed`.
    pub method: Option<String>,
    /// `ingestion_channel`: scale, scanner, keyed, api, … Defaults to `keyed`.
    pub ingestion_channel: Option<String>,
    /// The scale or measuring device, when there was one.
    pub instrument_device_id: Option<Uuid>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct RecordObservationResponse {
    pub observable_id: Uuid,
    pub observation_event_id: Uuid,
    pub observation_ids: Vec<Uuid>,
    /// Package dimension columns this act set, so J12 keeps holding.
    pub package_columns_written: Vec<String>,
    pub warnings: Vec<String>,
}

/// Record what was measured, in the unit it was measured in.
///
/// **The second of the two tables that carry everything, and the last to get a
/// write path.** Dimensions, weights, temperatures and quality grades all land
/// here, in one shape, which is what makes integration tractable: the same fact
/// arriving from a dock scale, a handheld or a supplier's message produces
/// identical rows apart from `ingestion_channel`.
///
/// One act measures one subject at one moment and any number of metrics, because
/// that is `observation_event`'s shape — weighing and measuring a pallet is one
/// act, not four. Conversion to base units happens in the writer per Principle 5
/// ([`observing::to_canonical`]), and the entered pair is stored beside the
/// canonical value so what the operator typed survives the rounding.
///
/// Act-idempotent on `client_event_id`.
#[post("/observations")]
pub async fn record_observation(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RecordObservationRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let subject_ref = SubjectRef {
                    package_id: body.package_id,
                    location_id: body.location_id,
                    lot_id: body.lot_id,
                    item_id: body.item_id,
                    item_style_id: body.item_style_id,
                    item_part_id: body.item_part_id,
                    packaging_level: body.packaging_level.clone(),
                };
                if subject_ref.named() != 1 {
                    return Err(ApiError::Rejected(
                        "name exactly one subject: package_id, location_id, lot_id, \
                         item_id, item_style_id or item_part_id"
                            .into(),
                    ));
                }
                let catalogue = subject_ref.catalogue();
                if catalogue.is_some() == body.packaging_level.is_none() {
                    return Err(ApiError::Rejected(
                        "an item or style subject names a packaging_level and nothing else \
                         does: a pair of boots and a carton of them are different sizes"
                            .into(),
                    ));
                }
                if body.measurements.is_empty() {
                    return Err(ApiError::Rejected(
                        "an observation of nothing is not an observation".into(),
                    ));
                }

                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;

                if act.is_replay() {
                    let (observation_event_id, observation_ids) =
                        client_events::require_observation_facts(tx, body.client_event_id)
                            .await?;
                    let observable_id: Uuid = tx
                        .query_one(
                            "SELECT observable_id FROM observation_event WHERE id = $1",
                            &[&observation_event_id],
                        )
                        .await?
                        .get(0);
                    return Ok(RecordObservationResponse {
                        observable_id,
                        observation_event_id,
                        observation_ids,
                        package_columns_written: vec![],
                        warnings: vec![client_events::REPLAY_WARNING.into()],
                    });
                }

                // The subject registry is get-or-create: `observable` is one row
                // per thing, and a partial unique index per arm makes the second
                // observer of a pallet find the first one's row rather than mint
                // a rival.
                let (observable_id, subject_kind) =
                    observable_for(tx, tenant, &subject_ref, body.occurred_at).await?;

                let method = body.method.clone().unwrap_or_else(|| "keyed".into());
                let channel = body
                    .ingestion_channel
                    .clone()
                    .unwrap_or_else(|| "keyed".into());

                // The vocabulary is a table, like `metric`'s and for the same
                // reason: a word the database has never heard of is a rejection
                // rather than a row nobody can interpret.
                let presentation_id: Option<Uuid> = match &body.presentation {
                    Some(code) => Some(
                        tx.query_opt("SELECT id FROM presentation WHERE code = $1", &[code])
                            .await?
                            .map(|r| r.get(0))
                            .ok_or_else(|| {
                                ApiError::Rejected(format!(
                                    "no presentation named {code}; the arrangements are \
                                     as_supplied, assembled, knocked_down, folded, rolled, \
                                     flat and compressed"
                                ))
                            })?,
                    ),
                    None => None,
                };

                // **Before the event is written, not after the figures are.**
                // D138 requires the arrangement for a length at `each`, and a
                // rejection that arrives after four observations have been
                // inserted is a rejection that has already half-happened.
                if presentation_id.is_none() {
                    if let Some(m) = body.measurements.iter().find(|m| {
                        m.absent_reason.is_none()
                            && observing::presentation_required(
                                &m.metric,
                                body.packaging_level.as_deref(),
                            )
                    }) {
                        return Err(ApiError::Rejected(
                            observing::ValueProblem::ArrangementUnstated {
                                metric: m.metric.clone(),
                            }
                            .to_string(),
                        ));
                    }
                }

                let observation_event_id: Uuid = tx
                    .query_one(
                        "INSERT INTO observation_event (
                             tenant_id, client_event_id, observable_id, observed_at,
                             instrument_device_id, recorded_by_id, method, ingestion_channel,
                             presentation_id)
                         VALUES (
                             $1, $2, $3, $4, $5, $6,
                             $7::text::observation_method, $8::text::ingestion_channel, $9)
                         RETURNING id",
                        &[
                            &tenant,
                            &body.client_event_id,
                            &observable_id,
                            &body.occurred_at,
                            &body.instrument_device_id,
                            &who.person_id,
                            &method,
                            &channel,
                            &presentation_id,
                        ],
                    )
                    .await?
                    .get(0);

                let mut observation_ids = vec![];
                let mut package_columns_written = vec![];
                let mut warnings = vec![];

                // Whether the package cache may move. J12 scopes itself to
                // unsealed packages, and so does this: a sealed carton's
                // dimensions are what it was despatched as, and a later
                // measurement is evidence about it rather than a correction of it.
                let package_open = match body.package_id {
                    Some(pid) => {
                        let sealed: Option<DateTime<Utc>> = tx
                            .query_opt("SELECT sealed_at FROM package WHERE id = $1", &[&pid])
                            .await?
                            .ok_or(ApiError::NotFound)?
                            .get(0);
                        if sealed.is_some() {
                            warnings.push(
                                "package is sealed; the measurement is recorded and its \
                                 dimension columns are left as despatched"
                                    .into(),
                            );
                        }
                        sealed.is_none()
                    }
                    None => false,
                };

                for m in &body.measurements {
                    let metric_row = tx
                        .query_opt(
                            "SELECT id, code, dimension_id, applies_to
                               FROM metric
                              WHERE code = $1 AND (tenant_id = $2 OR tenant_id IS NULL)
                              ORDER BY tenant_id NULLS LAST
                              LIMIT 1",
                            &[&m.metric, &tenant],
                        )
                        .await?;
                    let Some(mr) = metric_row else {
                        return Err(ApiError::Rejected(format!(
                            "no metric named {}; the vocabulary is a table, not a string",
                            m.metric
                        )));
                    };
                    let metric = observing::MetricSpec {
                        id: mr.get(0),
                        code: mr.get(1),
                        dimension_id: mr.get(2),
                        applies_to: mr.get::<_, Option<Vec<String>>>(3).unwrap_or_default(),
                    };

                    // **An absence is a result, and it takes the same route.**
                    // Same event, same metric, same subject — what it does not
                    // have is a number, which is the whole content of it. D138.
                    if let Some(reason) = &m.absent_reason {
                        if m.entered_value.is_some() || m.unit.is_some() {
                            return Err(ApiError::Rejected(format!(
                                "{} carries both a value and an absence; a thing that has \
                                 no height does not have one in millimetres either",
                                m.metric
                            )));
                        }
                        observing::check_absent_reason(reason)
                            .map_err(|e| ApiError::Rejected(e.to_string()))?;
                        observing::check_subject(&metric, subject_kind)
                            .map_err(|e| ApiError::Rejected(e.to_string()))?;

                        // The dimension comes off the metric rather than off a
                        // unit, because there is no unit. `length` still has a
                        // dimension when nothing was measured in it, and the
                        // composite FK to `metric(id, dimension_id)` requires
                        // the pair to agree.
                        let id: Uuid = tx
                            .query_one(
                                "INSERT INTO observation (
                                     tenant_id, observation_event_id, observable_id,
                                     observed_at, client_event_id, metric_id, result_kind,
                                     dimension_id, absent_reason)
                                 VALUES ($1, $2, $3, $4, $5, $6, 'quantity', $7, $8)
                                 RETURNING id",
                                &[
                                    &tenant,
                                    &observation_event_id,
                                    &observable_id,
                                    &body.occurred_at,
                                    &body.client_event_id,
                                    &metric.id,
                                    &metric.dimension_id,
                                    reason,
                                ],
                            )
                            .await?
                            .get(0);
                        observation_ids.push(id);

                        // **J12 compares the cache with `IS DISTINCT FROM`**, so
                        // a package whose height is now declared absent while
                        // the column still holds last week's number is a
                        // finding. Clearing it keeps the cache saying what the
                        // observations say, which is the only thing it is for.
                        if package_open {
                            if let Some(col) = observing::package_dimension_column(&metric.code) {
                                let sql = format!(
                                    "UPDATE package SET {} = NULL WHERE id = $1",
                                    col.name
                                );
                                tx.execute(sql.as_str(), &[&body.package_id.unwrap()]).await?;
                                package_columns_written.push(col.name.to_string());
                            }
                        }
                        continue;
                    }

                    let (Some(entered_value), Some(unit_code)) =
                        (m.entered_value.as_ref(), m.unit.as_ref())
                    else {
                        return Err(ApiError::Rejected(format!(
                            "{} names neither a value with its unit nor an absent_reason; \
                             an empty measurement records nothing and says nothing",
                            m.metric
                        )));
                    };

                    let unit_row = tx
                        .query_opt(
                            "SELECT id, dimension_id, factor_num, factor_den
                               FROM unit WHERE code = $1",
                            &[unit_code],
                        )
                        .await?;
                    let Some(ur) = unit_row else {
                        return Err(ApiError::Rejected(format!("no unit named {unit_code}")));
                    };
                    let unit_id: Uuid = ur.get(0);
                    let unit_dimension: Uuid = ur.get(1);
                    let factor = observing::Factor {
                        num: ur.get::<_, i64>(2),
                        den: ur.get::<_, i64>(3),
                    };

                    observing::check_applicable(&metric, subject_kind, unit_dimension, unit_code)
                        .map_err(|e| ApiError::Rejected(e.to_string()))?;
                    let entered = observing::parse_entered(entered_value)
                        .map_err(|e| ApiError::Rejected(e.to_string()))?;
                    let canonical = observing::to_canonical(entered, factor)
                        .map_err(|e| ApiError::Rejected(e.to_string()))?;

                    let id: Uuid = tx
                        .query_one(
                            "INSERT INTO observation (
                                 tenant_id, observation_event_id, observable_id, observed_at,
                                 client_event_id, metric_id, result_kind, dimension_id,
                                 value_numeric, entered_value, entered_unit_id)
                             VALUES (
                                 $1, $2, $3, $4, $5, $6, 'quantity', $7, $8,
                                 $9::text::numeric, $10)
                             RETURNING id",
                            &[
                                &tenant,
                                &observation_event_id,
                                &observable_id,
                                &body.occurred_at,
                                &body.client_event_id,
                                &metric.id,
                                &unit_dimension,
                                &canonical,
                                entered_value,
                                &unit_id,
                            ],
                        )
                        .await?
                        .get(0);
                    observation_ids.push(id);

                    // The cache J12 compares against. Written in the same
                    // transaction as the observation, because J12 uses
                    // `IS DISTINCT FROM`: a package carrying NULL against a
                    // recorded observation is a finding exactly as loudly as one
                    // carrying the wrong number.
                    if package_open {
                        if let Some(col) = observing::package_dimension_column(&metric.code) {
                            if !col.fits(canonical) {
                                return Err(ApiError::Rejected(format!(
                                    "{} in base units does not fit package.{}; check the \
                                     unit that was entered",
                                    canonical, col.name
                                )));
                            }
                            // `$1::bigint` because the three length columns are
                            // `integer` while the observation is `bigint`; the
                            // assignment cast narrows and `fits` has already
                            // guaranteed it can.
                            let sql = format!(
                                "UPDATE package
                                    SET {} = $1::bigint, dimensions_source = 'confirmed'
                                  WHERE id = $2",
                                col.name
                            );
                            tx.execute(sql.as_str(), &[&canonical, &body.package_id.unwrap()])
                                .await?;
                            package_columns_written.push(col.name.to_string());
                        }
                    }
                }

                tx.execute("SELECT projection_mark_dirty($1, 'observation')", &[&tenant])
                    .await?;

                Ok(RecordObservationResponse {
                    observable_id,
                    observation_event_id,
                    observation_ids,
                    package_columns_written,
                    warnings,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Phase C: on-demand rebuild for the current tenant
// ---------------------------------------------------------------------------

#[derive(Serialize, Debug)]
pub struct RefreshResponse {
    /// Rows touched by the rebuild. Zero on a warm accepted rebuild (D68), not
    /// on rate-limit — see `rate_limited`.
    pub rows_touched: i64,
    /// True when the 5-second on-demand gap has not elapsed (function returned NULL).
    pub rate_limited: bool,
}

/// Request a full-tenant projection rebuild for the caller's tenant only.
///
/// Rate-limited to one accepted rebuild per 5 seconds (D107). Prefer `ledger`
/// on write responses for floor UX; use this when the whole cache must converge.
#[post("/projections/refresh")]
pub async fn refresh_projections(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                // NULL = rate-limited; Some(n) = accepted (n may be 0 on a warm fold).
                let n: Option<i64> = tx
                    .query_one("SELECT projection_refresh_tenant($1)", &[&tenant])
                    .await?
                    .get(0);
                Ok(RefreshResponse {
                    rows_touched: n.unwrap_or(0),
                    rate_limited: n.is_none(),
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Every route this server serves, in one place.
///
/// **Shared with the tests deliberately.** The list used to live in `main.rs`,
/// where a test that built its own `App` would exercise a copy of it — so a
/// handler added here and forgotten there would pass the suite and 404 in
/// production. The binary and the harness now register from the same function,

// ---------------------------------------------------------------------------
// Passkeys
// ---------------------------------------------------------------------------
//
// Two ceremonies, two halves each. The server issues a challenge and remembers
// it; the authenticator signs it; the server checks the signature against a
// public key it holds. **The state between the halves lives in the database**
// (migration 75), because handing it to the browser and taking it back means
// trusting the party being authenticated to remember what they were asked.

#[derive(Deserialize, Debug)]
pub struct BeginRegistration {
    /// What the person calls this key. "The blue YubiKey."
    pub label: Option<String>,
}

#[derive(Serialize)]
pub struct Ceremony<T> {
    /// Names the server-side half. Opaque, single use, and expires.
    pub ceremony_id: Uuid,
    pub options: T,
}

/// Offer to enrol a key for whoever is signed in.
///
/// Authenticated: enrolling a credential is adding a way to become you, so it
/// requires already being you. Excludes the keys already held, so an
/// authenticator that is registered says so rather than silently making a
/// second credential.
#[post("/passkeys/registration/begin")]
pub async fn passkey_registration_begin(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<BeginRegistration>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let _ = body.into_inner();
    let relying = passkeys::shared().map_err(|e| ApiError::Configuration(e.to_string()))?;

    let conn = state.pool.get().await?;
    crate::tenancy::ensure_app_role(&conn).await?;

    let person = conn
        .query_one(
            "SELECT display_name, email FROM person WHERE id = $1",
            &[&who.person_id],
        )
        .await?;
    let display_name: String = person.get(0);
    let email: Option<String> = person.get(1);

    let already: Vec<CredentialID> = conn
        .query("SELECT id, verifier_state FROM passkeys_for_person($1)", &[&who.person_id])
        .await?
        .iter()
        .filter_map(|r| serde_json::from_str::<Passkey>(&r.get::<_, String>(1)).ok())
        .map(|p| p.cred_id().clone())
        .collect();

    let (mut options, registration) = relying
        .webauthn
        .start_passkey_registration(
            who.person_id,
            email.as_deref().unwrap_or(&display_name),
            &display_name,
            Some(already),
        )
        .map_err(|e| ApiError::Configuration(format!("start registration: {e}")))?;

    // **Ask for a key that knows who it is.**
    //
    // `start_passkey_registration` emits `residentKey: discouraged`, which is a
    // deliberate conservatism in the library and exactly wrong here: a
    // non-discoverable credential cannot be offered until the server already
    // knows whose it is, so tap-to-sign-in on a shared handheld is impossible
    // with one. This was found by reading the options the endpoint actually
    // returns rather than by assuming, and the key enrolled before it was found
    // is not discoverable.
    //
    // Overriding is sound because `residentKey` is a request to the
    // authenticator, not part of what `finish_passkey_registration` verifies —
    // it checks the challenge, the origin and the attestation, none of which
    // this touches.
    //
    // `required` rather than `preferred`: preferred fails silently, leaving a
    // key that enrols happily and then never appears at sign-in. Required fails
    // in the browser, in front of the person, while they can still do something
    // about it.
    if let Some(sel) = options.public_key.authenticator_selection.as_mut() {
        sel.resident_key = Some(webauthn_rs_proto::options::ResidentKeyRequirement::Required);
        sel.require_resident_key = true;
    }

    let ceremony_id = open_ceremony(&conn, Some(who.person_id), "registration", &registration).await?;
    Ok(HttpResponse::Ok().json(Ceremony { ceremony_id, options }))
}

#[derive(Deserialize, Debug)]
pub struct FinishRegistration {
    pub ceremony_id: Uuid,
    pub label: Option<String>,
    pub credential: RegisterPublicKeyCredential,
}

#[post("/passkeys/registration/finish")]
pub async fn passkey_registration_finish(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<FinishRegistration>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let body = body.into_inner();
    let relying = passkeys::shared().map_err(|e| ApiError::Configuration(e.to_string()))?;

    let conn = state.pool.get().await?;
    crate::tenancy::ensure_app_role(&conn).await?;

    let (owner, state_json) = claim_ceremony(&conn, body.ceremony_id, "registration").await?;
    // The ceremony was opened for somebody. Finishing it as anybody else would
    // enrol a key against the wrong person.
    if owner != Some(who.person_id) {
        return Err(ApiError::Unauthenticated);
    }
    let registration: PasskeyRegistration = serde_json::from_str(&state_json)
        .map_err(|e| ApiError::Configuration(format!("ceremony state: {e}")))?;

    let passkey = relying
        .webauthn
        .finish_passkey_registration(&body.credential, &registration)
        .map_err(|e| ApiError::Rejected(format!("that key was not accepted: {e}")))?;

    let described = passkeys::describe(&passkey)
        .map_err(|e| ApiError::Configuration(format!("serialise credential: {e}")))?;

    let id: Uuid = conn
        .query_one(
            "SELECT passkey_register($1, $2, $3, NULL, 0, NULL, NULL, NULL, $4)",
            &[
                &who.person_id,
                &described.credential_id,
                &body.label,
                &described.verifier_state,
            ],
        )
        .await?
        .get(0);

    Ok(HttpResponse::Ok().json(serde_json::json!({ "passkey_id": id })))
}

#[derive(Deserialize, Debug)]
pub struct BeginAuthentication {
    /// **Optional, and absent is the better path.** Given, the server offers the
    /// keys that person holds. Omitted, it asks for any key this site has on
    /// this authenticator and learns who it is talking to from the answer —
    /// which is one fewer field to type on a touchscreen with gloves on.
    pub email: Option<String>,
}

/// Offer to sign in with a key.
///
/// **Answers the same shape whether or not the account exists**, for the reason
/// the password path verifies against a dummy hash: an endpoint that says "no
/// such person" is an endpoint that enumerates people. An unknown email gets a
/// ceremony with an empty allow-list, which the browser fails in its own time.
#[post("/passkeys/authentication/begin")]
pub async fn passkey_authentication_begin(
    state: web::Data<AppState>,
    body: web::Json<BeginAuthentication>,
) -> Result<HttpResponse, ApiError> {
    let body = body.into_inner();
    let relying = passkeys::shared().map_err(|e| ApiError::Configuration(e.to_string()))?;

    let conn = state.pool.get().await?;
    crate::tenancy::ensure_app_role(&conn).await?;

    let email = body.email.map(|e| e.trim().to_string()).filter(|e| !e.is_empty());

    // **No email, no allow-list, no idea who this is yet.** The authenticator
    // offers what it holds for this origin and the assertion says whose it is.
    let Some(email) = email else {
        let (options, authentication) = relying
            .webauthn
            .start_discoverable_authentication()
            .map_err(|e| ApiError::Configuration(format!("start discoverable: {e}")))?;
        let ceremony_id = open_ceremony(&conn, None, "discoverable", &authentication).await?;
        return Ok(HttpResponse::Ok().json(Ceremony { ceremony_id, options }));
    };

    let person: Option<Uuid> = conn
        .query_opt("SELECT person_id FROM credential_for_login($1)", &[&email])
        .await?
        .map(|r| r.get(0));

    let keys: Vec<Passkey> = match person {
        Some(id) => conn
            .query("SELECT id, verifier_state FROM passkeys_for_person($1)", &[&id])
            .await?
            .iter()
            .filter_map(|r| serde_json::from_str(&r.get::<_, String>(1)).ok())
            .collect(),
        None => vec![],
    };

    let (options, authentication) = relying
        .webauthn
        .start_passkey_authentication(&keys)
        .map_err(|e| ApiError::Configuration(format!("start authentication: {e}")))?;

    let ceremony_id = open_ceremony(&conn, person, "authentication", &authentication).await?;
    Ok(HttpResponse::Ok().json(Ceremony { ceremony_id, options }))
}

#[derive(Deserialize, Debug)]
pub struct FinishAuthentication {
    pub ceremony_id: Uuid,
    pub credential: PublicKeyCredential,
    pub tenant_id: Option<Uuid>,
    pub site_id: Option<Uuid>,
    pub device_id: Option<Uuid>,
}

#[post("/passkeys/authentication/finish")]
pub async fn passkey_authentication_finish(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<FinishAuthentication>,
) -> Result<HttpResponse, ApiError> {
    let body = body.into_inner();
    let relying = passkeys::shared().map_err(|e| ApiError::Configuration(e.to_string()))?;

    let conn = state.pool.get().await?;
    crate::tenancy::ensure_app_role(&conn).await?;

    // Which ceremony this was is a fact on the row, not a guess from a null
    // person: an unknown email also yields a null person, so inferring would
    // have meant two things by one absence. Migration 76.
    let (kind, owner, state_json) = claim_either(&conn, body.ceremony_id).await?;

    let result = if kind == "discoverable" {
        // **The credential says whose it is.** `identify` reads the user handle
        // the authenticator returned; the key is then fetched and the assertion
        // verified against it. A handle naming somebody with no live key is
        // simply a failed sign-in.
        let (handle, _) = relying
            .webauthn
            .identify_discoverable_authentication(&body.credential)
            .map_err(|_| ApiError::Unauthenticated)?;
        let keys: Vec<DiscoverableKey> = conn
            .query("SELECT id, verifier_state FROM passkeys_for_person($1)", &[&handle])
            .await?
            .iter()
            .filter_map(|r| serde_json::from_str::<Passkey>(&r.get::<_, String>(1)).ok())
            .map(|p| DiscoverableKey::from(&p))
            .collect();
        if keys.is_empty() {
            return Err(ApiError::Unauthenticated);
        }
        let authentication: DiscoverableAuthentication = serde_json::from_str(&state_json)
            .map_err(|e| ApiError::Configuration(format!("ceremony state: {e}")))?;
        relying
            .webauthn
            .finish_discoverable_authentication(&body.credential, authentication, &keys)
            .map_err(|_| ApiError::Unauthenticated)?
    } else {
        let authentication: PasskeyAuthentication = serde_json::from_str(&state_json)
            .map_err(|e| ApiError::Configuration(format!("ceremony state: {e}")))?;
        relying
            .webauthn
            .finish_passkey_authentication(&body.credential, &authentication)
            .map_err(|_| ApiError::Unauthenticated)?
    };

    // Whose key signed. Looked up by credential rather than taken from the
    // ceremony, so the row that records the use is the row that was verified.
    let held = conn
        .query_opt(
            "SELECT id, person_id, verifier_state FROM passkey_by_credential($1)",
            &[&result.cred_id().as_ref().to_vec()],
        )
        .await?
        .ok_or(ApiError::Unauthenticated)?;
    let passkey_id: Uuid = held.get(0);
    let person_id: Uuid = held.get(1);

    // The ceremony named a person, so the assertion had better be theirs.
    if owner.is_some_and(|o| o != person_id) {
        return Err(ApiError::Unauthenticated);
    }

    // **The counter is the clone detector.** A signature arriving at or below
    // the stored count means two devices hold one key. Some authenticators never
    // implement it and report zero forever, so this is evidence rather than a
    // refusal — but it is evidence that gets recorded.
    let went_backwards: bool = conn
        .query_one(
            "SELECT passkey_record_use($1, $2, $3)",
            &[
                &passkey_id,
                &(result.counter() as i64),
                &result.backup_state(),
            ],
        )
        .await?
        .get(0);
    if went_backwards {
        tracing::warn!(
            %passkey_id, %person_id,
            "passkey signature counter did not advance: the key may be cloned"
        );
    }

    issue_session(&conn, &req, person_id, body.tenant_id, body.site_id, body.device_id).await
}

#[derive(Serialize)]
pub struct PasskeyRow {
    pub id: Uuid,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub sign_count: i64,
    pub backup_state: Option<bool>,
}

/// The keys that can become you, so you can see them and take one away.
#[get("/passkeys")]
pub async fn passkeys_list(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let conn = state.pool.get().await?;
    crate::tenancy::ensure_app_role(&conn).await?;
    // Through the definer, then re-read for the display columns: the listing
    // function returns what a ceremony needs, and this needs what a person reads.
    let rows = conn
        .query(
            "SELECT p.id, p.label, p.created_at, p.last_used_at, p.sign_count, p.backup_state
               FROM passkeys_for_person($1) k
               JOIN person_passkey p ON p.id = k.id
              ORDER BY p.created_at",
            &[&who.person_id],
        )
        .await?;
    let out: Vec<PasskeyRow> = rows
        .iter()
        .map(|r| PasskeyRow {
            id: r.get(0),
            label: r.get(1),
            created_at: r.get(2),
            last_used_at: r.get(3),
            sign_count: r.get(4),
            backup_state: r.get(5),
        })
        .collect();
    Ok(HttpResponse::Ok().json(out))
}

/// Revoke a key. Disabled, never deleted: a key that signed movements is part of
/// how those movements are attributed, and deleting it makes an answered
/// question unanswerable. D11.
#[actix_web::delete("/passkeys/{id}")]
pub async fn passkey_revoke(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let id = path.into_inner();
    let conn = state.pool.get().await?;
    crate::tenancy::ensure_app_role(&conn).await?;
    // The function checks ownership itself, so one person cannot revoke
    // another's key by knowing its id.
    let done: Option<bool> = conn
        .query_opt("SELECT passkey_disable($1, $2)", &[&id, &who.person_id])
        .await?
        .and_then(|r| r.get(0));
    match done {
        Some(true) => Ok(HttpResponse::NoContent().finish()),
        _ => Err(ApiError::NotFound),
    }
}

async fn open_ceremony<T: Serialize>(
    conn: &deadpool_postgres::Client,
    person_id: Option<Uuid>,
    kind: &str,
    state: &T,
) -> Result<Uuid, ApiError> {
    let json = serde_json::to_string(state)
        .map_err(|e| ApiError::Configuration(format!("ceremony state: {e}")))?;
    let ttl = passkeys::CEREMONY_TTL_SECONDS as i32;
    Ok(conn
        .query_one(
            "SELECT webauthn_challenge_open($1, $2, $3, make_interval(secs => $4))",
            &[&person_id, &kind, &json, &(ttl as f64)],
        )
        .await?
        .get(0))
}

/// Claim the server's half, exactly once.
///
/// A miss is `Unauthenticated` rather than `NotFound`, because "that ceremony is
/// spent" and "that ceremony never existed" are the same answer to anyone who
/// should not have it.
/// Claim an authentication ceremony of either kind, and say which it was.
async fn claim_either(
    conn: &deadpool_postgres::Client,
    id: Uuid,
) -> Result<(String, Option<Uuid>, String), ApiError> {
    for kind in ["authentication", "discoverable"] {
        if let Some(row) = conn
            .query_opt(
                "SELECT person_id, state FROM webauthn_challenge_claim($1, $2)",
                &[&id, &kind],
            )
            .await?
        {
            return Ok((kind.to_string(), row.get(0), row.get(1)));
        }
    }
    Err(ApiError::Unauthenticated)
}

async fn claim_ceremony(
    conn: &deadpool_postgres::Client,
    id: Uuid,
    kind: &str,
) -> Result<(Option<Uuid>, String), ApiError> {
    let row = conn
        .query_opt("SELECT person_id, state FROM webauthn_challenge_claim($1, $2)", &[&id, &kind])
        .await?
        .ok_or(ApiError::Unauthenticated)?;
    Ok((row.get(0), row.get(1)))
}


// ---------------------------------------------------------------------------
// Weighing
// ---------------------------------------------------------------------------
//
// A weight typed off a supplier's sheet and a weight read off a dock scale are
// the same number in NetSuite and different facts here. What was missing was a
// way to turn the first into the second, one item at a time.

#[derive(Serialize)]
pub struct ToWeigh {
    pub item_id: Option<Uuid>,
    pub item_style_id: Option<Uuid>,
    pub code: String,
    pub description: Option<String>,
    pub packaging_level: String,
    /// What is held now, in grams, and how it was come by.
    pub held_g: Option<i64>,
    pub held_method: Option<String>,
    pub held_at: Option<DateTime<Utc>>,
    /// How many open order lines name it. The reason to walk there first.
    pub demand: i64,
    /// `never` — nobody has measured it. `overdue` — somebody did, long ago.
    pub because: String,
}

#[derive(Deserialize, Debug)]
pub struct WorklistQuery {
    pub limit: Option<i64>,
}

/// What to put on the scale, in the order worth doing it.
///
/// **Two lists, because they are two jobs.** A value nobody measured needs a
/// first weighing whatever its apparent age; a value somebody measured needs
/// re-weighing when it gets old. An imported figure carries the date of the
/// import rather than of the observation, so its age is unknown rather than
/// small, and one interval covering both reported an empty worklist over a
/// hundred unverified numbers.
///
/// Derived on read. Nothing stores "needs confirming": it is a function of
/// `observation_current`'s `observed_at` and `method`, and a third column could
/// disagree with the two behind it.
#[get("/revalidation")]
pub async fn revalidation_worklist(
    req: HttpRequest,
    state: web::Data<AppState>,
    q: web::Query<WorklistQuery>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let limit = q.into_inner().limit.unwrap_or(25).clamp(1, 200);
    let rows = weighing_worklist(&state, who.tenant_id, limit).await?;
    Ok(HttpResponse::Ok().json(rows))
}

pub async fn weighing_worklist(
    state: &web::Data<AppState>,
    tenant: Uuid,
    limit: i64,
) -> Result<Vec<ToWeigh>, ApiError> {
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;
    scope
        .run(move |tx| {
            Box::pin(async move {
                let rows = tx
                    .query(
                        "SELECT o.item_id, o.item_style_id,
                                coalesce(i.code, s.code),
                                coalesce(i.description, s.description),
                                o.packaging_level::text,
                                oc.value_numeric::bigint,
                                oc.method,
                                oc.observed_at,
                                (SELECT count(*) FROM order_line ol
                                  WHERE ol.item_id = o.item_id)
                           FROM observation_current oc
                           JOIN observable o ON o.id = oc.observable_id
                           JOIN metric m ON m.id = oc.metric_id
                           LEFT JOIN item i ON i.id = o.item_id
                           LEFT JOIN item_style s ON s.id = o.item_style_id
                          WHERE m.code = 'gross_weight'
                            AND (o.item_id IS NOT NULL OR o.item_style_id IS NOT NULL)",
                        &[],
                    )
                    .await?;

                let now = Utc::now();
                let mut out: Vec<ToWeigh> = rows
                    .iter()
                    .map(|r| {
                        let method: Option<String> = r.get(6);
                        let m = method.clone().unwrap_or_default();
                        let observed_at: DateTime<Utc> = r.get(7);
                        ToWeigh {
                            item_id: r.get(0),
                            item_style_id: r.get(1),
                            code: r.get(2),
                            description: r.get(3),
                            packaging_level: r.get(4),
                            held_g: r.get(5),
                            held_method: method,
                            held_at: Some(observed_at),
                            demand: r.get::<_, Option<i64>>(8).unwrap_or(0),
                            because: if revalidation::ever_measured(&m) {
                                "overdue".into()
                            } else {
                                "never".into()
                            },
                        }
                    })
                    .filter(|t| {
                        t.because == "never"
                            || revalidation::staleness(
                                t.held_at.unwrap_or(now),
                                t.held_method.as_deref().unwrap_or(""),
                                now,
                            ) >= 1.0
                    })
                    .collect();

                // Never-measured first — an absent measurement is a worse state
                // than an old one — then by how often the thing is actually sold.
                out.sort_by(|a, b| {
                    (a.because != "never")
                        .cmp(&(b.because != "never"))
                        .then(b.demand.cmp(&a.demand))
                        .then(a.code.cmp(&b.code))
                });
                out.truncate(limit as usize);
                Ok(out)
            })
        })
        .await
}

#[derive(Deserialize, Debug)]
pub struct RecordWeighing {
    pub item_id: Option<Uuid>,
    pub item_style_id: Option<Uuid>,
    pub packaging_level: String,
    /// What the scale said, as a string. The only record of what was read.
    pub entered_value: String,
    /// `kg` or `g`.
    pub unit: String,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct WeighingRecorded {
    pub observation_id: Uuid,
    pub recorded_g: i64,
    pub previous_g: Option<i64>,
    pub previous_method: Option<String>,
    /// True when the reading and what it replaced are far enough apart to be
    /// worth somebody's attention.
    pub disagreed: bool,
    pub discrepancy_id: Option<Uuid>,
}

/// Put a thing on the scale and record what it said.
///
/// **This is the act that makes the interval mean something.** Until a weight
/// has been measured once, its age is the age of the import that carried it, and
/// no schedule can be built on that.
///
/// Always `instrument` off a `scale`. A weighing that was actually a person
/// typing a number they remembered is a different fact, and it goes through
/// `/observations` where it can say so.
#[post("/weighings")]
pub async fn record_weighing(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RecordWeighing>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let tenant = who.tenant_id;
    let body = body.into_inner();
    if body.item_id.is_some() == body.item_style_id.is_some() {
        return Err(ApiError::Rejected(
            "name exactly one of item_id or item_style_id".into(),
        ));
    }
    let mut scope = TenantScope::begin(&state.pool, tenant).await?;

    let result = scope
        .run(|tx| {
            Box::pin(async move {
                let act = client_events::claim_act(
                    tx,
                    &NewClientEvent {
                        tenant_id: tenant,
                        client_event_id: body.client_event_id,
                        site_id: who.site_id,
                        recorded_by_id: who.person_id,
                        submitted_at: body.occurred_at,
                    },
                )
                .await?;
                if act.is_replay() {
                    return Err(ApiError::Rejected(
                        "that weighing has already been recorded".into(),
                    ));
                }

                let (column, subject) = match (body.item_id, body.item_style_id) {
                    (Some(id), _) => ("item_id", id),
                    (_, Some(id)) => ("item_style_id", id),
                    _ => unreachable!("checked above"),
                };

                // The carton level needs a case pack to be a definite object;
                // `each` does not. Same rule the observations handler applies.
                let config: Option<Uuid> = if body.packaging_level == "each" {
                    None
                } else {
                    tx.query_opt(
                        &format!(
                            "SELECT c.id FROM item_packing_config c JOIN item i ON i.id = c.item_id
                              WHERE i.{} = $1 AND c.effective_from <= $2::timestamptz::date
                              ORDER BY c.effective_from DESC, c.id DESC LIMIT 1",
                            if column == "item_id" { "id" } else { "style_id" }
                        ),
                        &[&subject, &body.occurred_at],
                    )
                    .await?
                    .map(|r| r.get(0))
                };

                tx.execute(
                    &format!(
                        "INSERT INTO observable (tenant_id, {column}, packaging_level,
                             item_packing_config_id)
                         VALUES ($1, $2, $3::text::packaging_level, $4)
                         ON CONFLICT (tenant_id, {column}, packaging_level,
                                      item_packing_config_id)
                              WHERE {column} IS NOT NULL DO NOTHING"
                    ),
                    &[&tenant, &subject, &body.packaging_level, &config],
                )
                .await?;
                let observable: Uuid = tx
                    .query_one(
                        &format!(
                            "SELECT id FROM observable WHERE tenant_id = $1 AND {column} = $2
                              AND packaging_level = $3::text::packaging_level
                              AND item_packing_config_id IS NOT DISTINCT FROM $4"
                        ),
                        &[&tenant, &subject, &body.packaging_level, &config],
                    )
                    .await?
                    .get(0);

                // **Read what is held before writing over it.** The comparison
                // is the whole point, and after the insert there is nothing to
                // compare against.
                let held = tx
                    .query_opt(
                        "SELECT oc.value_numeric::bigint, oc.method
                           FROM observation_current oc
                           JOIN metric m ON m.id = oc.metric_id
                          WHERE oc.observable_id = $1 AND m.code = 'gross_weight'",
                        &[&observable],
                    )
                    .await?;
                let previous_g: Option<i64> = held.as_ref().and_then(|r| r.get(0));
                let previous_method: Option<String> = held.as_ref().and_then(|r| r.get(1));

                let unit = tx
                    .query_opt(
                        "SELECT factor_num, factor_den FROM unit WHERE code = $1",
                        &[&body.unit],
                    )
                    .await?
                    .ok_or_else(|| ApiError::Rejected(format!("no unit named {}", body.unit)))?;
                let factor = observing::Factor {
                    num: unit.get(0),
                    den: unit.get(1),
                };
                let entered = observing::parse_entered(&body.entered_value)
                    .map_err(|e| ApiError::Rejected(e.to_string()))?;
                let recorded_g = observing::to_canonical(entered, factor)
                    .map_err(|e| ApiError::Rejected(e.to_string()))?;

                let event: Uuid = tx
                    .query_one(
                        "INSERT INTO observation_event (tenant_id, client_event_id,
                             observable_id, observed_at, recorded_by_id, method,
                             ingestion_channel)
                         VALUES ($1, $2, $3, $4, $5, 'instrument', 'scale') RETURNING id",
                        &[
                            &tenant,
                            &body.client_event_id,
                            &observable,
                            &body.occurred_at,
                            &who.person_id,
                        ],
                    )
                    .await?
                    .get(0);

                let observation_id: Uuid = tx
                    .query_one(
                        "INSERT INTO observation (tenant_id, observation_event_id,
                             observable_id, observed_at, client_event_id, metric_id,
                             result_kind, dimension_id, value_numeric, entered_value,
                             entered_unit_id)
                         SELECT $1, $2, $3, $4, $5, m.id, 'quantity', m.dimension_id, $6,
                                $7::text::numeric, u.id
                           FROM metric m, unit u
                          WHERE m.code = 'gross_weight' AND m.tenant_id IS NULL
                            AND u.code = $8
                         RETURNING id",
                        &[
                            &tenant,
                            &event,
                            &observable,
                            &body.occurred_at,
                            &body.client_event_id,
                            &recorded_g,
                            &body.entered_value,
                            &body.unit,
                        ],
                    )
                    .await?
                    .get(0);

                // **A first weighing cannot contradict anything.** It can only
                // establish something, so a finding is raised only where there
                // was a value to disagree with.
                let disagreed = previous_g
                    .is_some_and(|p| revalidation::materially_differs(p, recorded_g));
                let mut discrepancy_id = None;
                if disagreed {
                    let p = previous_g.expect("checked");
                    let detail = format!(
                        "A weighing found {:.3} kg where {} held {:.3} kg. Both are on \
                         file; the scale reading is now current and the earlier value is \
                         still there to compare against.",
                        recorded_g as f64 / 1000.0,
                        previous_method.as_deref().unwrap_or("an earlier record"),
                        p as f64 / 1000.0
                    );
                    let id: Uuid = tx
                        .query_one(
                            // **`variance` is generated**, so Postgres computes
                            // observed minus expected and refuses a supplied
                            // value. Which is right: a difference that could be
                            // written independently of the two numbers it comes
                            // from is a third fact able to disagree with them.
                            "INSERT INTO discrepancy (tenant_id, kind, item_id, detail,
                                 detected_at, detected_by_id, state, expected_quantity,
                                 observed_quantity)
                             VALUES ($1, 'identity_mismatch', $2, $3, now(), $4, 'open',
                                     $5::text::numeric, $6::text::numeric)
                             RETURNING id",
                            &[
                                &tenant,
                                &body.item_id,
                                &detail,
                                &who.person_id,
                                &rust_decimal_from(p),
                                &rust_decimal_from(recorded_g),
                            ],
                        )
                        .await?
                        .get(0);
                    discrepancy_id = Some(id);
                }

                Ok(WeighingRecorded {
                    observation_id,
                    recorded_g,
                    previous_g,
                    previous_method,
                    disagreed,
                    discrepancy_id,
                })
            })
        })
        .await?;

    Ok(HttpResponse::Ok().json(result))
}

/// `numeric` columns take a decimal; the values here are whole grams, so this
/// hands Postgres the text and lets it parse rather than pulling in a decimal
/// crate for three fields.
fn rust_decimal_from(grams: i64) -> String {
    grams.to_string()
}

/// which makes "is this endpoint wired up" a thing the tests can answer.
///
/// **The API only, and mounted at whatever the caller mounts it at.** [`mount`]
/// is what a running server uses; this is what the suite uses, at the root,
/// because a test is about a handler rather than about a prefix. The maud pages
/// used to be registered from the tail of this function and are now in `mount`
/// beside the scope, since they are not API and must not move under it.
pub fn configure(cfg: &mut web::ServiceConfig) {
    // **The transfer ceiling, and it has to be said out loud.**
    //
    // `web::Bytes` takes its limit from `PayloadConfig`, and the default is
    // 262,144 bytes. Nothing set one, so for as long as photographs have
    // existed the server refused every one above 256kB with a bare 413 — while
    // `images::MAX_BYTES` said twelve megabytes and the check enforcing it sat
    // downstream of a body that never arrived. Every test uploaded a dozen
    // bytes, so the suite agreed with the comment rather than with the server.
    //
    // Set from the same constant the handler checks against, so the two cannot
    // drift apart again: the framework's ceiling and the endpoint's rule are
    // now one number with one reason.
    cfg.app_data(web::PayloadConfig::new(crate::images::MAX_BYTES))
        // D163. Three paths, one deep handler and one cheap one; the module
        // says why the compose healthcheck stays on the deep one.
        .service(crate::health::health)
        .service(crate::health::readiness)
        .service(crate::health::liveness)
        .service(stock_on_hand)
        .service(discrepancies)
        .service(discrepancy)
        .service(investigate_discrepancy)
        .service(accept_discrepancy)
        .service(record_observation_image)
        .service(record_evidence)
        .service(picking_list)
        .service(putaway_list)
        .service(receiving_list)
        .service(read_image)
        .service(site_despatch)
        .service(fulfilment_bench)
        .service(fulfilment_lines)
        .service(fulfilment_line)
        .service(package)
        .service(create_package)
        .service(place_package)
        .service(contain_package)
        .service(seal_package)
        .service(open_package)
        .service(despatch_package)
        .service(open_lines)
        .service(crate::picking::record::record_pick)
        .service(crate::workspace::workspace)
        .service(record_correction)
        .service(record_adjustment)
        .service(record_count)
        .service(record_receipt)
        .service(record_move)
        .service(record_observation)
        .service(passkey_registration_begin)
        .service(passkey_registration_finish)
        .service(passkey_authentication_begin)
        .service(passkey_authentication_finish)
        .service(passkeys_list)
        .service(passkey_revoke)
        .service(revalidation_worklist)
        .service(capture_worklist)
        .service(resolve_identifier)
        .service(setup_status)
        .service(setup_deployment)
        .service(record_weighing)
        .service(item_measurements)
        .service(package_types)
        .service(package_contents)
        .service(find_orders)
        .service(record_consignment)
        .service(consignment)
        .service(record_allocation)
        .service(release_allocation)
        .service(refresh_projections)
        .service(sign_on)
        .service(current_session)
        .service(sign_off)
        .service(sites)
        .service(choose_site)
        .service(work_waiting)
        .service(packing_queue)
        .service(change_password)
        .service(mint_api_token)
        .service(list_api_tokens)
        .service(revoke_api_token)
        .service(import_bins)
        .service(import_items)
        .service(import_stock)
        .service(import_fulfilment)
        .service(void_package)
        .service(bind_barcode)
        .service(item_barcodes);
}

/// The API under `/api`, and the server-rendered documents beside it.
///
/// # Why the API has a prefix at all
///
/// So the pages can have the root. Every screen wants a name a person could
/// read down a phone — `/pack`, `/findings`, `/capture` — and two of those were
/// already taken by endpoints. A rule that new endpoints must avoid the page
/// names is a rule somebody eventually forgets; a prefix makes the collision
/// impossible to write.
///
/// # The default service is the load-bearing line
///
/// A single-page application needs a catch-all returning `index.html`, and at
/// the root that catch-all swallows every unmatched path. `assets.rs` records
/// what that costs: a route missing from a stale build answered 404 with an
/// empty body, and *"the emptiness is what identified it"* — where a page would
/// have been returned as 200 and the diagnosis would have taken far longer.
///
/// Terminating unmatched `/api/*` here is what makes a root-mounted client safe.
/// A request for an endpoint that does not exist gets an empty 404 and stays
/// diagnosable, whatever is mounted underneath it.
pub fn mount(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .configure(configure)
            .default_service(web::to(|| async { HttpResponse::NotFound().finish() })),
    );
    // Not under the scope: these are documents, not API (D113), and they keep
    // their own place until the last of them narrows to `/print/*`.
    crate::web::configure(cfg);
}
