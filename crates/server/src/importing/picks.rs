//! Picks made elsewhere, as reported (D172).
//!
//! At Melbourne the goods are picked on the WMS handheld and NetSuite records
//! the item fulfilment as Picked; the packing happens here. This records what
//! NetSuite says was picked on each line, as `external_pick`: a level as at the
//! item fulfilment's last change, never a movement. The goods reach the ledger
//! when they are handed over, not here (migration 94 says why).
//!
//! D160's split: [`reportable`] decides from the sender's words alone, and
//! [`record`] persists. A report the same as one already on file (same line,
//! same moment) writes nothing, so a resend is harmless; a later report of a
//! different number is a new level, which is how an un-pick arrives.

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

/// One item fulfilment's picks, as the sender read them.
#[derive(Debug, Clone)]
pub struct Report {
    /// The item fulfilment's internal id: which fulfilment these lines are on.
    pub fulfilment_id: String,
    /// Its number, as a person quotes it.
    pub document: String,
    /// When the item fulfilment last changed, per the other system.
    pub observed_at: DateTime<Utc>,
    /// The picker, in the other system's words.
    pub picked_by: Option<String>,
    /// Each line's key and how many are picked on it.
    pub lines: Vec<(String, i64)>,
}

/// What recording did.
#[derive(Debug, Default, Serialize, Clone)]
pub struct PicksRecorded {
    /// New reports written.
    pub recorded: u64,
    /// Reports already on file for that line and moment.
    pub unchanged: u64,
    /// Line keys the fulfilment has no line for, which were not recorded.
    pub unmatched: Vec<String>,
}

/// Whether a status says the item fulfilment is picked, in any of the forms
/// NetSuite gives it: the label, the letter, or the qualified code.
pub fn is_picked(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "picked" | "a" | "itemship:a" | "item fulfillment : picked"
    )
}

/// The report a send makes, if it makes one.
///
/// Only an item fulfilment that says it is picked, that says which it is, and
/// that says when: a pick with no moment cannot be ordered against the next
/// report, and filling in "now" would state a time nobody told us.
pub fn reportable(
    status: Option<&str>,
    fulfilment_id: Option<&str>,
    document: Option<&str>,
    observed_at: Option<DateTime<Utc>>,
    picked_by: Option<&str>,
    lines: &[(Option<String>, i64)],
) -> Option<Report> {
    if !status.is_some_and(is_picked) {
        return None;
    }
    let fulfilment_id = fulfilment_id.map(str::trim).filter(|s| !s.is_empty())?;
    let observed_at = observed_at?;
    let lines: Vec<(String, i64)> = lines
        .iter()
        .filter_map(|(key, qty)| {
            let key = key.as_deref().map(str::trim).filter(|s| !s.is_empty())?;
            Some((key.to_string(), (*qty).max(0)))
        })
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(Report {
        fulfilment_id: fulfilment_id.to_string(),
        document: document.map(str::trim).unwrap_or_default().to_string(),
        observed_at,
        picked_by: picked_by.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string),
        lines,
    })
}

/// Write the report under `client_event`, the act that stored the send.
///
/// Runs after the fulfilment and its lines are loaded, on the same transaction,
/// so the lines it names exist. Marks projections dirty when anything was
/// written, so `external_picked_quantity` catches up.
pub async fn record(
    tx: &Transaction<'_>,
    tenant: Uuid,
    client_event: Uuid,
    report: &Report,
) -> Result<PicksRecorded, String> {
    let mut out = PicksRecorded::default();
    let channel: Uuid = tx
        .query_one(
            "SELECT id FROM source_channel WHERE tenant_id = $1 AND code = 'netsuite'",
            &[&tenant],
        )
        .await
        .map_err(|e| format!("the netsuite channel: {e}"))?
        .get(0);

    for (key, quantity) in &report.lines {
        let Some(line) = tx
            .query_opt(
                "SELECT fl.id FROM fulfilment f
                   JOIN fulfilment_line fl ON fl.fulfilment_id = f.id
                  WHERE f.tenant_id = $1 AND f.source_channel_id = $2
                    AND f.external_id = $3 AND fl.external_line = $4",
                &[&tenant, &channel, &report.fulfilment_id, key],
            )
            .await
            .map_err(|e| e.to_string())?
        else {
            out.unmatched.push(key.clone());
            continue;
        };
        let line: Uuid = line.get(0);
        let written = tx
            .execute(
                "INSERT INTO external_pick
                     (tenant_id, client_event_id, fulfilment_line_id, source_channel_id,
                      quantity, document, external_id, external_line, picked_by, observed_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                 ON CONFLICT (tenant_id, fulfilment_line_id, external_line, observed_at)
                 DO NOTHING",
                &[
                    &tenant,
                    &client_event,
                    &line,
                    &channel,
                    quantity,
                    &report.document,
                    &report.fulfilment_id,
                    key,
                    &report.picked_by,
                    &report.observed_at,
                ],
            )
            .await
            .map_err(|e| format!("external_pick {key}: {e}"))?;
        if written == 1 {
            out.recorded += 1;
        } else {
            out.unchanged += 1;
        }
    }

    if out.recorded > 0 {
        tx.execute("SELECT projection_mark_dirty($1, 'external_pick')", &[&tenant])
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> Option<DateTime<Utc>> {
        Some("2026-09-28T09:00:00Z".parse().unwrap())
    }

    #[test]
    fn picked_reads_every_form_netsuite_gives() {
        for s in ["Picked", "picked", "A", "ItemShip:A", "Item Fulfillment : Picked"] {
            assert!(is_picked(s), "{s}");
        }
        for s in ["Packed", "B", "ItemShip:C", "Shipped", ""] {
            assert!(!is_picked(s), "{s}");
        }
    }

    #[test]
    fn a_report_needs_a_picked_document_and_a_moment() {
        let lines = [(Some("1".to_string()), 3)];
        assert!(reportable(Some("Picked"), Some("10134954"), Some("IF270947"), at(), None, &lines).is_some());
        assert!(reportable(Some("Packed"), Some("10134954"), None, at(), None, &lines).is_none(), "not picked");
        assert!(reportable(None, Some("10134954"), None, at(), None, &lines).is_none(), "no status");
        assert!(reportable(Some("Picked"), None, None, at(), None, &lines).is_none(), "no document");
        assert!(reportable(Some("Picked"), Some("10134954"), None, None, None, &lines).is_none(), "no moment");
        assert!(
            reportable(Some("Picked"), Some("10134954"), None, at(), None, &[(None, 3)]).is_none(),
            "no line keys"
        );
    }

    #[test]
    fn a_negative_count_is_read_as_none_picked() {
        let r = reportable(Some("A"), Some("x"), None, at(), Some("  "), &[(Some("1".into()), -2)]).unwrap();
        assert_eq!(r.lines, vec![("1".to_string(), 0)]);
        assert_eq!(r.picked_by, None, "a blank name is no name");
    }
}
