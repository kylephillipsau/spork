//! Act-envelope idempotency (D5 / D25).
//!
//! One physical submission owns one `client_event` primary key. Facts carry a
//! plain FK to that row (multi-fact acts are allowed). The server must not
//! insert facts again when the envelope already exists — that is the defect
//! `ON CONFLICT DO NOTHING` alone produced when fact INSERTs always ran.
//!
//! Contract:
//! - first success: insert envelope + facts in one transaction
//! - replay same `(tenant_id, client_event_id)`: load prior facts, return 200
//! - envelope without facts (incomplete prior): hard reject

use chrono::{DateTime, Utc};
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::error::ApiError;

/// Soft warning appended when the act was already recorded.
pub const REPLAY_WARNING: &str =
    "replay of an existing client_event; no new facts were written";

/// Parameters for the act envelope insert.
#[derive(Clone, Debug)]
pub struct NewClientEvent {
    pub tenant_id: Uuid,
    pub client_event_id: Uuid,
    /// Where the act happened, from the session that recorded it. Optional
    /// because `client_event.site_id` is, and because a person can sign on
    /// without naming a dock.
    pub site_id: Option<Uuid>,
    /// **D11's non-repudiable floor.** From the authenticated session, never
    /// from the request: *"whatever else is claimed, we always know which
    /// person, on which device, recorded this."*
    pub recorded_by_id: Uuid,
    pub submitted_at: DateTime<Utc>,
}

/// Result of claiming the act primary key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActInsert {
    /// Envelope inserted; caller must write facts in this transaction.
    Fresh,
    /// Envelope already present; caller must load prior facts and not insert.
    Replay,
}

impl ActInsert {
    pub fn is_replay(self) -> bool {
        matches!(self, ActInsert::Replay)
    }
}

/// Insert the act envelope, or detect that it already exists.
///
/// Uses `ON CONFLICT DO NOTHING` and rows-affected so concurrent first-writers
/// serialize: the loser becomes [`ActInsert::Replay`] and must load facts.
pub async fn claim_act(
    tx: &Transaction<'_>,
    ev: &NewClientEvent,
) -> Result<ActInsert, ApiError> {
    let n = tx
        .execute(
            "INSERT INTO client_event
                 (tenant_id, client_event_id, site_id, recorded_by_id,
                  submitted_at, received_at)
             VALUES ($1, $2, $3, $4, $5, now())
             ON CONFLICT (tenant_id, client_event_id) DO NOTHING",
            &[
                &ev.tenant_id,
                &ev.client_event_id,
                &ev.site_id,
                &ev.recorded_by_id,
                &ev.submitted_at,
            ],
        )
        .await?;
    if n == 0 {
        Ok(ActInsert::Replay)
    } else {
        Ok(ActInsert::Fresh)
    }
}

/// One stock_movement for a 1:1 act, or hard error if missing / ambiguous.
pub async fn require_one_movement(
    tx: &Transaction<'_>,
    client_event_id: Uuid,
) -> Result<(Uuid, i64), ApiError> {
    let rows = tx
        .query(
            "SELECT id, quantity FROM stock_movement
              WHERE client_event_id = $1
              ORDER BY id",
            &[&client_event_id],
        )
        .await?;
    match rows.len() {
        0 => Err(ApiError::Rejected(
            "client_event exists but no stock_movement was recorded; incomplete act".into(),
        )),
        1 => Ok((rows[0].get(0), rows[0].get(1))),
        _ => Err(ApiError::Rejected(
            "client_event owns multiple stock_movement rows; ambiguous replay for this endpoint"
                .into(),
        )),
    }
}

/// Optional single movement (zero-variance adjust wrote no movement).
pub async fn optional_one_movement(
    tx: &Transaction<'_>,
    client_event_id: Uuid,
) -> Result<Option<(Uuid, i64)>, ApiError> {
    let rows = tx
        .query(
            "SELECT id, quantity FROM stock_movement
              WHERE client_event_id = $1
              ORDER BY id",
            &[&client_event_id],
        )
        .await?;
    match rows.len() {
        0 => Ok(None),
        1 => Ok(Some((rows[0].get(0), rows[0].get(1)))),
        _ => Err(ApiError::Rejected(
            "client_event owns multiple stock_movement rows; ambiguous replay for this endpoint"
                .into(),
        )),
    }
}

/// One package_event for a 1:1 package act.
pub async fn require_one_package_event(
    tx: &Transaction<'_>,
    client_event_id: Uuid,
) -> Result<(Uuid, Uuid, String), ApiError> {
    let rows = tx
        .query(
            "SELECT id, package_id, kind::text FROM package_event
              WHERE client_event_id = $1
              ORDER BY id",
            &[&client_event_id],
        )
        .await?;
    match rows.len() {
        0 => Err(ApiError::Rejected(
            "client_event exists but no package_event was recorded; incomplete act".into(),
        )),
        1 => Ok((rows[0].get(0), rows[0].get(1), rows[0].get(2))),
        _ => Err(ApiError::Rejected(
            "client_event owns multiple package_event rows; ambiguous replay for this endpoint"
                .into(),
        )),
    }
}

/// Despatch-style act: one package_event + one movement (or require both).
pub async fn require_despatch_facts(
    tx: &Transaction<'_>,
    client_event_id: Uuid,
) -> Result<(Uuid, Uuid), ApiError> {
    let (event_id, _package_id, kind) =
        require_one_package_event(tx, client_event_id).await?;
    if kind != "despatched" {
        return Err(ApiError::Rejected(format!(
            "client_event already recorded a package_event of kind {kind}, not despatched"
        )));
    }
    let (movement_id, _) = require_one_movement(tx, client_event_id).await?;
    Ok((event_id, movement_id))
}

/// Stock count + optional discrepancy for a count act.
pub async fn require_count_facts(
    tx: &Transaction<'_>,
    client_event_id: Uuid,
) -> Result<(Uuid, i64, i64, Option<Uuid>), ApiError> {
    let rows = tx
        .query(
            "SELECT id, system_quantity, counted_quantity FROM stock_count
              WHERE client_event_id = $1
              ORDER BY id",
            &[&client_event_id],
        )
        .await?;
    if rows.is_empty() {
        return Err(ApiError::Rejected(
            "client_event exists but no stock_count was recorded; incomplete act".into(),
        ));
    }
    if rows.len() > 1 {
        return Err(ApiError::Rejected(
            "client_event owns multiple stock_count rows; ambiguous replay".into(),
        ));
    }
    let stock_count_id: Uuid = rows[0].get(0);
    let system_quantity: i64 = rows[0].get(1);
    let counted_quantity: i64 = rows[0].get(2);
    let disc: Option<Uuid> = tx
        .query_opt(
            "SELECT id FROM discrepancy
              WHERE stock_count_id = $1
              ORDER BY detected_at DESC, id DESC
              LIMIT 1",
            &[&stock_count_id],
        )
        .await?
        .map(|r| r.get(0));
    Ok((stock_count_id, system_quantity, counted_quantity, disc))
}

/// Receipt facts under one act: the **line** this act wrote, its header, and the
/// movement if one was written.
///
/// Under Q172 a multi-line delivery shares one `goods_receipt` whose
/// `client_event_id` is only the act that *opened* the header. Later lines carry
/// their own acts. Replay therefore finds the line by `client_event_id` first and
/// reads the header from the line — never `goods_receipt.client_event_id = act`.
pub async fn require_receipt_facts(
    tx: &Transaction<'_>,
    client_event_id: Uuid,
) -> Result<(Uuid, Uuid, Option<Uuid>, Option<i64>, Option<Uuid>, bool), ApiError> {
    let line = tx
        .query(
            "SELECT id, goods_receipt_id, accepted_at IS NOT NULL, receiving_policy_id
               FROM goods_receipt_line
              WHERE client_event_id = $1
              ORDER BY id",
            &[&client_event_id],
        )
        .await?;
    match line.len() {
        0 => {
            return Err(ApiError::Rejected(
                "client_event exists but no goods_receipt_line was recorded; incomplete act"
                    .into(),
            ))
        }
        1 => {}
        _ => {
            return Err(ApiError::Rejected(
                "client_event owns multiple goods_receipt_line rows; ambiguous replay for \
                 this endpoint"
                    .into(),
            ))
        }
    }
    let goods_receipt_line_id: Uuid = line[0].get(0);
    let goods_receipt_id: Uuid = line[0].get(1);
    let accepted: bool = line[0].get(2);
    let receiving_policy_id: Option<Uuid> = line[0].get(3);
    let movement = optional_one_movement(tx, client_event_id).await?;
    Ok((
        goods_receipt_id,
        goods_receipt_line_id,
        movement.map(|(id, _)| id),
        movement.map(|(_, qty)| qty),
        receiving_policy_id,
        accepted,
    ))
}

/// The delivery a receipt line belongs to, as the caller states it.
///
/// A struct rather than seven positional arguments, on [`NewClientEvent`]'s
/// pattern: five of these are `Uuid` or `Option<Uuid>` and a caller that
/// transposes two of them compiles.
#[derive(Clone, Debug)]
pub struct NewGoodsReceipt {
    pub tenant_id: Uuid,
    /// Client-minted, so the first line of a multi-line truck names the header
    /// the rest will join. `None` opens a fresh one-line receipt.
    pub goods_receipt_id: Option<Uuid>,
    /// From the session. `goods_receipt.site_id` is nullable, so a person who
    /// signed on without naming a dock can still record a delivery.
    pub site_id: Option<Uuid>,
    pub purchase_order_id: Option<Uuid>,
    pub received_at: DateTime<Utc>,
    pub client_event_id: Uuid,
    pub recorded_by_id: Uuid,
}

/// Create or join a delivery header (D43 / Q172).
///
/// - `None` → mint a new header; a single-line receipt still works.
/// - existing → validate site and purchase-order demand, then join.
/// - named but absent → insert it under the client-minted id.
///
/// **The insert is `ON CONFLICT DO NOTHING` and the join re-reads**, which is
/// [`claim_act`]'s pattern and exists for [`claim_act`]'s reason. Check-then-
/// insert loses the race that Q172 is entirely about: two lines of one delivery
/// arriving together both see no header, both insert, and the second takes a
/// duplicate-key error — a 500 on the dock, on the exact traffic this endpoint
/// was widened to carry. Demonstrated against a live database before it was
/// changed, rather than argued.
///
/// Returns `(goods_receipt_id, created)`, where `created` is true only for the
/// act that actually inserted the row.
pub async fn ensure_goods_receipt(
    tx: &Transaction<'_>,
    gr: &NewGoodsReceipt,
) -> Result<(Uuid, bool), ApiError> {
    let Some(id) = gr.goods_receipt_id else {
        let id: Uuid = tx
            .query_one(
                "INSERT INTO goods_receipt (
                     tenant_id, site_id, purchase_order_id, received_at,
                     client_event_id, recorded_by_id)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 RETURNING id",
                &[
                    &gr.tenant_id,
                    &gr.site_id,
                    &gr.purchase_order_id,
                    &gr.received_at,
                    &gr.client_event_id,
                    &gr.recorded_by_id,
                ],
            )
            .await?
            .get(0);
        return Ok((id, true));
    };

    // Claim the header id. Rows-affected says whether this act opened the
    // delivery or joined one, so two concurrent first lines serialize and the
    // loser reads what the winner wrote instead of failing.
    let inserted = tx
        .execute(
            "INSERT INTO goods_receipt (
                 id, tenant_id, site_id, purchase_order_id, received_at,
                 client_event_id, recorded_by_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (id) DO NOTHING",
            &[
                &id,
                &gr.tenant_id,
                &gr.site_id,
                &gr.purchase_order_id,
                &gr.received_at,
                &gr.client_event_id,
                &gr.recorded_by_id,
            ],
        )
        .await?;
    if inserted == 1 {
        return Ok((id, true));
    }

    // Joined. **Read the header explicitly scoped to the tenant** rather than
    // relying on RLS alone: `ON CONFLICT` sees the unique index, which is not
    // filtered by any policy, so a header id belonging to another tenant
    // conflicts and then reads back as absent. Saying so beats a 500 that
    // confirms the id exists.
    let existing = tx
        .query_opt(
            "SELECT site_id, purchase_order_id
               FROM goods_receipt WHERE id = $1 AND tenant_id = $2",
            &[&id, &gr.tenant_id],
        )
        .await?;
    let Some(r) = existing else {
        return Err(ApiError::Rejected(
            "that goods_receipt id is already in use and is not this tenant's".into(),
        ));
    };
    let header_site: Option<Uuid> = r.get(0);
    let header_po: Option<Uuid> = r.get(1);

    // `site_id` is nullable on the header, so a header opened without one takes
    // lines from anywhere; only a stated site is a constraint.
    // Only a stated site on both sides is a constraint. A header opened without
    // one takes lines from anywhere, and a session that named no dock is not a
    // claim about where the goods are.
    if let (Some(hs), Some(ls)) = (header_site, gr.site_id) {
        if hs != ls {
            return Err(ApiError::Rejected(
                "goods_receipt belongs to a different site than this line".into(),
            ));
        }
    }
    // D43: one receipt per delivery per demand document. Only when both sides
    // name one — a header opened blind, or a line of free stock against a header
    // that has a purchase order, are both ordinary.
    if let (Some(hp), Some(lp)) = (header_po, gr.purchase_order_id) {
        if hp != lp {
            return Err(ApiError::Rejected(
                "goods_receipt is already for a different purchase order; \
                 one delivery per demand document (D43)"
                    .into(),
            ));
        }
    }
    Ok((id, false))
}

/// Observation facts under one act: the event, and the rows it recorded.
///
/// One act measures one subject at one moment, which is `observation_event`'s
/// own shape — it carries composite keys binding it to a single `observable`,
/// `observed_at` and `client_event`, so a second event under one act would be a
/// second subject and this endpoint does not do that. The observations beneath
/// it are one per metric, which is what makes weighing a pallet one act rather
/// than four.
pub async fn require_observation_facts(
    tx: &Transaction<'_>,
    client_event_id: Uuid,
) -> Result<(Uuid, Vec<Uuid>), ApiError> {
    let ev = tx
        .query(
            "SELECT id FROM observation_event WHERE client_event_id = $1 ORDER BY id",
            &[&client_event_id],
        )
        .await?;
    match ev.len() {
        0 => {
            return Err(ApiError::Rejected(
                "client_event exists but no observation_event was recorded; incomplete act"
                    .into(),
            ))
        }
        1 => {}
        _ => {
            return Err(ApiError::Rejected(
                "client_event owns multiple observation_event rows; ambiguous replay".into(),
            ))
        }
    }
    let event_id: Uuid = ev[0].get(0);
    let rows = tx
        .query(
            "SELECT id FROM observation WHERE observation_event_id = $1 ORDER BY id",
            &[&event_id],
        )
        .await?;
    if rows.is_empty() {
        return Err(ApiError::Rejected(
            "observation_event exists with no observations; incomplete act".into(),
        ));
    }
    Ok((event_id, rows.iter().map(|r| r.get(0)).collect()))
}

/// A claim the caller named, as it already stands in the database.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriorAllocation {
    pub id: Uuid,
    pub stock_id: Option<Uuid>,
    pub fulfilment_line_id: Option<Uuid>,
    pub quantity: i64,
    pub firm: bool,
}

/// Load a client-minted allocation id if this tenant already holds one (Q174).
///
/// **An allocation is an Intention, so it carries no `client_event`** — S19 asks
/// facts for one and D12 makes a claim a plan rather than an observation. What it
/// does carry is an identifier the caller may mint, which is the same property
/// D5 relies on for packages: *"every entry carries an identifier the handheld
/// generates. Sending the same entry twice changes nothing."* The envelope was
/// never the mechanism; the client-minted key was.
///
/// Scoped to the tenant explicitly rather than left to RLS, for
/// [`ensure_goods_receipt`]'s reason: the conflict is decided by a unique index
/// no policy filters, so an id belonging to somebody else has to be told apart
/// from an id belonging to nobody.
pub async fn prior_allocation(
    tx: &Transaction<'_>,
    tenant_id: Uuid,
    allocation_id: Uuid,
) -> Result<Option<PriorAllocation>, ApiError> {
    let row = tx
        .query_opt(
            "SELECT id, stock_id, fulfilment_line_id, quantity, firm
               FROM stock_allocation
              WHERE id = $1 AND tenant_id = $2",
            &[&allocation_id, &tenant_id],
        )
        .await?;
    Ok(row.map(|r| PriorAllocation {
        id: r.get(0),
        stock_id: r.get(1),
        fulfilment_line_id: r.get(2),
        quantity: r.get(3),
        firm: r.get(4),
    }))
}

/// Reject a replay whose body disagrees with the claim already stored.
///
/// **A retry is the same act arriving twice, not a second act wearing the first
/// one's name.** Returning the stored row for a request that asks for something
/// else would silently discard the difference — the same failure
/// [`reject_quantity_mismatch`] exists to stop on the ledger, one table over.
pub fn reject_allocation_mismatch(
    prior: &PriorAllocation,
    stock_id: Uuid,
    fulfilment_line_id: Uuid,
    quantity: i64,
) -> Result<(), ApiError> {
    let mut differs = vec![];
    if prior.stock_id != Some(stock_id) {
        differs.push(format!(
            "stock_id {:?} (this request has {stock_id})",
            prior.stock_id
        ));
    }
    if prior.fulfilment_line_id != Some(fulfilment_line_id) {
        differs.push(format!(
            "fulfilment_line_id {:?} (this request has {fulfilment_line_id})",
            prior.fulfilment_line_id
        ));
    }
    if prior.quantity != quantity {
        differs.push(format!(
            "quantity {} (this request has {quantity})",
            prior.quantity
        ));
    }
    if differs.is_empty() {
        return Ok(());
    }
    Err(ApiError::Rejected(format!(
        "allocation {} already exists with {}",
        prior.id,
        differs.join("; ")
    )))
}

/// Reject when a prior 1:1 movement quantity disagrees with this request.
pub fn reject_quantity_mismatch(prior: i64, requested: i64) -> Result<(), ApiError> {
    if prior != requested {
        return Err(ApiError::Rejected(format!(
            "client_event already recorded quantity {prior}; this request has {requested}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_flag() {
        assert!(!ActInsert::Fresh.is_replay());
        assert!(ActInsert::Replay.is_replay());
    }

    #[test]
    fn quantity_mismatch() {
        assert!(reject_quantity_mismatch(5, 5).is_ok());
        assert!(reject_quantity_mismatch(5, 6).is_err());
    }
}
