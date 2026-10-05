//! An item not where NetSuite lists it, said on the floor (D215).
//!
//! NetSuite keeps the shelves (D212): which bins hold an item, and how many,
//! is its inventory balance, loaded as `reported_stock`. A person walking a
//! list with that balance in hand meets two differences, and says each from
//! the item's page:
//!
//! - **Not here.** NetSuite lists it in a bin, and none is there:
//!   `not_in_listed_bin`, expecting NetSuite's count and finding none.
//! - **Found here.** It is in a bin where NetSuite lists none:
//!   `found_in_unlisted_bin`, expecting none and finding the count, when it
//!   was counted.
//!
//! Each is a finding, never a change to anything: NetSuite is put right by a
//! person, who then accepts the finding with what they did. An adjustment
//! can't resolve one (`adjusting.rs`), because Spork holds no cell of these
//! to adjust.
//!
//! **A finding, not a count.** D212 had a picker record a count (D8). A count
//! asserts against a cell of Spork's own ledger, `stock_count.system_quantity`,
//! and at a site where NetSuite keeps the shelves there is no such cell. What
//! the person said and what NetSuite listed are both on the finding.
//!
//! **Said once.** Saying the same of the same bin while it is still open
//! returns the finding already there, and a retry of the act returns it too.

use actix_web::{post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::client_events::{self, NewClientEvent};
use crate::error::ApiError;
use crate::routes::caller;
use crate::tenancy::TenantScope;
use crate::AppState;

/// The two kinds, which only NetSuite can put right.
pub const KINDS: [&str; 2] = ["not_in_listed_bin", "found_in_unlisted_bin"];

/// Whether a finding is about NetSuite's report rather than Spork's ledger.
pub fn is_netsuites(kind: &str) -> bool {
    KINDS.contains(&kind)
}

#[derive(Deserialize, Debug)]
pub struct FlagRequest {
    /// `not_here` or `found_here`.
    pub said: String,
    /// The bin, by id (a row of NetSuite's balance), or by its code as typed,
    /// at the caller's site.
    pub location_id: Option<Uuid>,
    pub bin_code: Option<String>,
    /// How many were found, for `found_here`, when they were counted.
    pub quantity: Option<i64>,
    /// Anything else worth the fixer knowing.
    pub note: Option<String>,
    pub client_event_id: Uuid,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct BinFlagged {
    pub discrepancy_id: Uuid,
    /// Already said and still open: the finding is the one already there.
    pub already: bool,
    pub bin_code: String,
}

/// A bin, and what NetSuite's newest balance lists in it.
struct Bin {
    id: Uuid,
    code: String,
    site_id: Uuid,
}

async fn bin_of(tx: &Transaction<'_>, site: Option<Uuid>, body: &FlagRequest) -> Result<Bin, ApiError> {
    let row = match (body.location_id, body.bin_code.as_deref().map(str::trim)) {
        (Some(id), _) => tx.query_opt("SELECT id, code, site_id FROM location WHERE id = $1", &[&id]).await?,
        (None, Some(code)) if !code.is_empty() => {
            let site = site.ok_or_else(|| ApiError::Rejected("choose the warehouse you're working at first".into()))?;
            let found = tx
                .query_opt(
                    "SELECT id, code, site_id FROM location WHERE site_id = $1 AND lower(code) = lower($2)",
                    &[&site, &code],
                )
                .await?;
            if found.is_none() {
                return Err(ApiError::Rejected(format!("there's no bin {code} at this warehouse")));
            }
            found
        }
        _ => return Err(ApiError::Rejected("say which bin".into())),
    };
    let r = row.ok_or(ApiError::NotFound)?;
    Ok(Bin { id: r.get(0), code: r.get(1), site_id: r.get(2) })
}

/// Say an item isn't in a bin NetSuite lists it in, or is in one it doesn't.
#[post("/items/{id}/bin-flags")]
pub async fn flag_bin(
    req: HttpRequest,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<FlagRequest>,
) -> Result<HttpResponse, ApiError> {
    let who = caller(&state, &req).await?;
    let item = path.into_inner();
    let body = body.into_inner();
    let kind = match body.said.as_str() {
        "not_here" => KINDS[0],
        "found_here" => KINDS[1],
        _ => return Err(ApiError::Rejected("say not_here or found_here".into())),
    };
    if body.quantity.is_some_and(|q| q < 1) {
        return Err(ApiError::Rejected("how many were found is a whole number, 1 or more".into()));
    }
    if kind == KINDS[0] && body.quantity.is_some() {
        return Err(ApiError::Rejected("none was there, so there is no count to give".into()));
    }
    let ev = NewClientEvent {
        tenant_id: who.tenant_id,
        client_event_id: body.client_event_id,
        site_id: who.site_id,
        recorded_by_id: who.person_id,
        submitted_at: body.occurred_at,
    };
    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let out = scope
        .run(move |tx| {
            Box::pin(async move {
                let code: String = tx
                    .query_opt("SELECT code FROM item WHERE id = $1", &[&item])
                    .await?
                    .ok_or(ApiError::NotFound)?
                    .get(0);
                let bin = bin_of(tx, who.site_id, &body).await?;

                // The same said of the same bin, still open, is the finding there.
                let open = |states: &'static str| {
                    let sql = format!(
                        "SELECT id FROM discrepancy
                          WHERE kind = $1::text::discrepancy_kind AND item_id = $2 AND holder_location_id = $3
                            AND state::text IN ({states})
                          ORDER BY detected_at DESC LIMIT 1"
                    );
                    async move { tx.query_opt(&sql, &[&kind, &item, &bin.id]).await }
                };
                if client_events::claim_act(tx, &ev).await?.is_replay() {
                    let id: Uuid = open("'open', 'investigating', 'resolved', 'accepted'")
                        .await?
                        .ok_or_else(|| ApiError::Rejected("that act was recorded as something else".into()))?
                        .get(0);
                    return Ok(BinFlagged { discrepancy_id: id, already: true, bin_code: bin.code });
                }
                if let Some(r) = open("'open', 'investigating'").await? {
                    return Ok(BinFlagged { discrepancy_id: r.get(0), already: true, bin_code: bin.code });
                }

                // NetSuite's newest word on this bin, and where else it lists
                // the item. **The newest row, then whether it lists any**: a
                // stale feed's row saying 12 must not outrank a newer one
                // saying none.
                let listed = tx
                    .query_opt(
                        "SELECT on_hand_text, at FROM (
                             SELECT rs.on_hand, rs.on_hand::text AS on_hand_text,
                                    to_char(rs.as_at AT TIME ZONE s.timezone, 'FMDD Mon HH24:MI') AS at
                               FROM reported_stock rs JOIN site s ON s.id = rs.site_id
                              WHERE rs.item_id = $1 AND rs.location_id = $2
                              ORDER BY rs.as_at DESC LIMIT 1) newest
                          WHERE on_hand > 0",
                        &[&item, &bin.id],
                    )
                    .await?
                    .map(|r| (r.get::<_, String>(0), r.get::<_, String>(1)));
                let elsewhere: Vec<String> = tx
                    .query(
                        "SELECT said FROM (
                             SELECT DISTINCT ON (l.id) l.code || ' (' || rs.on_hand::text || ')' AS said,
                                    rs.on_hand, l.code
                               FROM reported_stock rs JOIN location l ON l.id = rs.location_id
                              WHERE rs.item_id = $1 AND rs.site_id = $2 AND rs.location_id <> $3
                              ORDER BY l.id, rs.as_at DESC) newest
                          WHERE on_hand > 0
                          ORDER BY code",
                        &[&item, &bin.site_id, &bin.id],
                    )
                    .await?
                    .iter()
                    .map(|r| r.get(0))
                    .collect();
                let others = if elsewhere.is_empty() {
                    "NetSuite lists it in no other bin here.".to_string()
                } else {
                    format!("NetSuite also lists it in {}.", elsewhere.join(", "))
                };

                let (expected, observed, said) = match (kind, &listed) {
                    ("not_in_listed_bin", None) => {
                        return Err(ApiError::Rejected(format!("NetSuite doesn't list {code} in {}", bin.code)));
                    }
                    ("not_in_listed_bin", Some((on_hand, as_at))) => (
                        on_hand.clone(),
                        Some("0".to_string()),
                        format!(
                            "NetSuite's inventory balance (as at {as_at}) lists {on_hand} of {code} in {}, and none \
                             was there. {others}",
                            bin.code
                        ),
                    ),
                    (_, Some((on_hand, _))) => {
                        return Err(ApiError::Rejected(format!(
                            "NetSuite already lists {on_hand} of {code} in {}",
                            bin.code
                        )));
                    }
                    (_, None) => (
                        "0".to_string(),
                        body.quantity.map(|q| q.to_string()),
                        format!(
                            "{code} was found in {}{}, where NetSuite's inventory balance lists none. {others}",
                            bin.code,
                            body.quantity.map(|q| format!(", {q} of it")).unwrap_or_default()
                        ),
                    ),
                };
                let note = body.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
                let detail = format!(
                    "{said}{} Put it right in NetSuite, then accept this with what was done.",
                    note.map(|n| format!(" They said: {n}")).unwrap_or_default()
                );
                let id: Uuid = tx
                    .query_one(
                        "INSERT INTO discrepancy (tenant_id, kind, item_id, holder_location_id, detail,
                             detected_at, detected_by_id, state, expected_quantity, observed_quantity)
                         VALUES ($1, $2::text::discrepancy_kind, $3, $4, $5, $6, $7, 'open',
                                 $8::text::numeric, $9::text::numeric)
                         RETURNING id",
                        &[
                            &ev.tenant_id,
                            &kind,
                            &item,
                            &bin.id,
                            &detail,
                            &body.occurred_at,
                            &ev.recorded_by_id,
                            &expected,
                            &observed,
                        ],
                    )
                    .await?
                    .get(0);
                Ok(BinFlagged { discrepancy_id: id, already: false, bin_code: bin.code })
            })
        })
        .await?;
    Ok(HttpResponse::Ok().json(out))
}
