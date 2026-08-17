//! The file, stored as it arrived, before anything reads it.
//!
//! D21's definition is "stored exactly as exchanged". `party_message` was built
//! for that in migration 34 and **nothing has written to it since**. The
//! importers go straight to `site`, `location` and `item`, so what the system of
//! record actually sent stops existing the moment it is parsed, and a second run
//! cannot tell a file it has already seen from a new one.
//!
//! # The transport is a column, which is the whole argument
//!
//! `message_channel` reads `('edi', 'portal', 'csv', 'email', 'api', 'webhook',
//! 'print')`. An export downloaded by hand and a response fetched from an API
//! are the same message arriving two ways. Storing the file here now is what
//! makes the API, when there is one, a change of one enum value rather than a
//! second integration written beside this one.
//!
//! # Only on apply
//!
//! A dry run records no arrival. [`super::bins::load`] does the writes and rolls
//! them back, and a receipt that survived that rollback would assert a file
//! arrived which was never kept.
//!
//! # A second copy of the same bytes is not a second arrival
//!
//! [`record`] asks before it writes. Identical bytes under the same tenant
//! return the message already on file and write nothing, because the export
//! arrived once however many times it is read. The load still runs: a bin list
//! is reference data and re-importing it is a correction, which is the position
//! [`super::bins`] already takes and the reason migration 74's `pick_sequence`
//! could be filled by re-importing at all.
//!
//! # What is not filed yet
//!
//! **Only this module's callers.** `import_orders` and `import_prepack` are
//! separate examples that write to `order` and `item` directly rather than
//! through [`super`], so the exports they read still stop existing when they are
//! parsed. Bringing them across is the same three lines each and a decision
//! about who their actor is; it has not been made.

use sha2::{Digest, Sha256};
use tokio_postgres::Transaction;
use uuid::Uuid;

/// Who is loading the file.
///
/// `client_event_actor_ck` is `num_nonnulls(recorded_by_id, automation_key) = 1`,
/// so this is an exclusive or too, and for the same reason D11 gives: "who did
/// this" has to have an answer.
#[derive(Clone, Copy, Debug)]
pub enum Actor {
    /// Somebody at a terminal, running an importer by hand.
    Person(Uuid),
    /// An import token (D158).
    ///
    /// What gets recorded is the token's id and not its label. Question 105 asks
    /// what an `automation_key` *is* and D27 only narrowed it by contrast, so a
    /// word like `import` would be vocabulary invented here to answer it. A
    /// pointer at a row needs no vocabulary: whoever reads it joins.
    Token(Uuid),
}

impl Actor {
    fn columns(self) -> (Option<Uuid>, Option<String>) {
        match self {
            Actor::Person(id) => (Some(id), None),
            Actor::Token(id) => (None, Some(format!("api_token:{id}"))),
        }
    }
}

/// What was recorded about the arrival.
#[derive(Clone, Copy, Debug)]
pub struct Received {
    pub party_message_id: Uuid,
    /// `None` when these bytes were already on file, in which case nothing was
    /// written and `party_message_id` names the arrival that was.
    pub client_event_id: Option<Uuid>,
}

impl Received {
    /// True when these exact bytes had arrived before.
    pub fn is_replay(&self) -> bool {
        self.client_event_id.is_none()
    }
}

/// SHA-256 of the payload, which is what `party_message.content_hash` holds.
pub fn digest(payload: &[u8]) -> Vec<u8> {
    Sha256::digest(payload).to_vec()
}

/// Whether these exact bytes are already on file, without writing anything.
///
/// Split out so a dry run can answer the replay question too. A dry run that
/// could not would be a report that does not say what applying will do, which is
/// the one thing [`super::bins::load`]'s design is built to avoid.
pub async fn seen(
    tx: &Transaction<'_>,
    tenant: Uuid,
    payload: &[u8],
) -> Result<Option<Uuid>, String> {
    tx.query_opt(
        "SELECT id FROM party_message
          WHERE tenant_id = $1 AND direction = 'inbound' AND content_hash = $2
          ORDER BY recorded_at
          LIMIT 1",
        &[&tenant, &digest(payload)],
    )
    .await
    .map_err(|e| e.to_string())
    .map(|r| r.map(|r| r.get(0)))
}

/// Record that this file arrived, or find that it already had.
///
/// Runs on the caller's transaction and inside no savepoint of its own: the
/// arrival is kept exactly when the load is, because the caller only calls this
/// when applying.
///
/// **`tenant` must be the transaction's own tenant.** The lookup runs under
/// row-level security and the insert does not, so a caller that passed a
/// different one would be told nothing is on file and would then file a
/// duplicate. Both callers take it from the same place the scope did.
pub async fn record(
    tx: &Transaction<'_>,
    tenant: Uuid,
    actor: Actor,
    transport_ref: Option<&str>,
    payload: &[u8],
) -> Result<Received, String> {
    let hash = digest(payload);

    if let Some(id) = seen(tx, tenant, payload).await? {
        return Ok(Received { party_message_id: id, client_event_id: None });
    }

    let client_event_id = Uuid::now_v7();
    let (person, automation) = actor.columns();
    tx.execute(
        "INSERT INTO client_event
             (tenant_id, client_event_id, recorded_by_id, automation_key,
              app_version, submitted_at, received_at)
         VALUES ($1, $2, $3, $4, $5, now(), now())",
        &[
            &tenant,
            &client_event_id,
            &person,
            &automation,
            &env!("CARGO_PKG_VERSION"),
        ],
    )
    .await
    .map_err(|e| format!("recording the act behind the import: {e}"))?;

    // **`occurred_at` is when it reached us, and that is a weaker claim than it
    // looks.** The column is NOT NULL and the export states nowhere when the
    // system of record produced it. Filling it with a production time nobody
    // told us would turn an absent fact into a stated one, which is the same
    // refusal `bins::load` makes about a bin with no type.
    let party_message_id: Uuid = tx
        .query_one(
            "INSERT INTO party_message
                 (tenant_id, party_id, direction, channel, transport_ref,
                  content_type, payload, byte_count, content_hash,
                  occurred_at, client_event_id, parse_status)
             VALUES ($1, NULL, 'inbound', 'csv', $2,
                     'text/csv', $3, $4, $5,
                     now(), $6, 'pending')
             RETURNING id",
            &[
                &tenant,
                &transport_ref,
                &payload,
                &i32::try_from(payload.len()).map_err(|_| "the file does not fit in byte_count")?,
                &hash,
                &client_event_id,
            ],
        )
        .await
        .map_err(|e| format!("storing the file as it arrived: {e}"))?
        .get(0);

    Ok(Received { party_message_id, client_event_id: Some(client_event_id) })
}

/// Mark the stored file as one we declined to read in full.
///
/// The bytes are kept and nothing was acted on. The first caller is the row-count
/// refusal on the stock import, which is what this status was reserved for.
pub async fn partial(tx: &Transaction<'_>, party_message_id: Uuid) -> Result<(), String> {
    tx.execute(
        "UPDATE party_message
            SET parse_status = 'partial', parser_version = $2
          WHERE id = $1",
        &[&party_message_id, &env!("CARGO_PKG_VERSION")],
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Mark the stored file as read.
///
/// Called after the loader returns, so a row left at `pending` is one whose load
/// did not finish. `partial` is deliberately not written here: bins left out for
/// stating no type are a policy refusal rather than a parse outcome, and
/// conflating the two would make `parse_status` mean two things.
pub async fn parsed(tx: &Transaction<'_>, party_message_id: Uuid) -> Result<(), String> {
    tx.execute(
        "UPDATE party_message
            SET parse_status = 'parsed', parser_version = $2
          WHERE id = $1",
        &[&party_message_id, &env!("CARGO_PKG_VERSION")],
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}
