//! Devices are told when something changes at their site. D206.
//!
//! A picker in an aisle and a packer at the bench look at the same orders.
//! When one of them records something, the other's screen should not wait for
//! them to touch it before it is right. Migration 118 has Postgres NOTIFY
//! `spork_live` with a tenant and a site whenever an act is recorded or an order
//! arrives or moves on. This process holds one connection that LISTENs, and
//! passes each notification to the devices signed on at that site over
//! `GET /changes`, a `text/event-stream`.
//!
//! **A change says where, never what.** A device told of one reads again what it
//! is showing, through the endpoints it always uses. So there is nothing to
//! replay: a device that was away, for a second or an hour, reads once when it
//! is back, which is what it would have done with every message it missed. No
//! event ids, no buffer, no `Last-Event-ID`.
//!
//! **A stream ends after a few minutes,** and the device opens another. That is
//! when the session is checked again, so somebody taken out of the workspace
//! (D205) stops hearing within minutes rather than at their next request.

use std::time::Duration;

use actix_web::{get, web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::broadcast;
use tokio_postgres::AsyncMessage;
use uuid::Uuid;

use crate::error::ApiError;
use crate::routes::caller;
use crate::AppState;

/// The channel migration 118 notifies on.
pub const CHANNEL: &str = "spork_live";

/// How long one stream lasts before the device opens another.
const STREAM_FOR: Duration = Duration::from_secs(5 * 60);

/// A comment line this often, so a proxy or a phone's radio does not decide
/// the connection is idle and drop it.
const KEEPALIVE: Duration = Duration::from_secs(20);

/// What travels from the listener to the streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Something changed for this tenant, at this site or at none in particular.
    At { tenant: Uuid, site: Option<Uuid> },
    /// The listener lost Postgres and is back: whatever happened in between
    /// went unheard, so every device reads again.
    Everywhere,
    /// The server is stopping. Streams end so a drain is not held open.
    Closing,
}

impl Change {
    /// Whether a device signed on as `tenant` at `site` hears this.
    pub fn reaches(&self, tenant: Uuid, site: Option<Uuid>) -> bool {
        match *self {
            Change::At { tenant: t, site: s } => t == tenant && (s.is_none() || site.is_none() || s == site),
            Change::Everywhere => true,
            Change::Closing => false,
        }
    }
}

/// The payload migration 118 sends.
#[derive(Deserialize)]
struct Payload {
    tenant: Uuid,
    site: Option<Uuid>,
}

/// Parse one notification. A payload that does not parse tells everybody,
/// because hearing too much costs a read and hearing too little costs a picker
/// a wasted walk.
pub fn parse(payload: &str) -> Change {
    match serde_json::from_str::<Payload>(payload) {
        Ok(p) => Change::At { tenant: p.tenant, site: p.site },
        Err(_) => Change::Everywhere,
    }
}

/// The hub every stream subscribes to. Registered as its own `web::Data`
/// rather than on `AppState`, which every test builds by hand.
pub struct Live {
    tx: broadcast::Sender<Change>,
}

impl Live {
    /// A hub that nothing feeds: for tests that send to it themselves.
    pub fn quiet() -> Self {
        let (tx, _) = broadcast::channel(256);
        Live { tx }
    }

    /// A hub fed by a connection that LISTENs at `url`, reconnecting when it
    /// is lost.
    pub fn listening(url: String) -> Self {
        let live = Live::quiet();
        let tx = live.tx.clone();
        tokio::spawn(listen(url, tx));
        live
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Change> {
        self.tx.subscribe()
    }

    /// Pass a change to every stream. Nobody listening is not an error.
    pub fn send(&self, change: Change) {
        let _ = self.tx.send(change);
    }
}

/// Hold one connection on `spork_live` for the life of the process.
async fn listen(url: String, tx: broadcast::Sender<Change>) {
    let mut wait = Duration::from_secs(1);
    let mut lost = false;
    loop {
        match listen_once(&url, &tx, lost).await {
            // Heard, then lost: try again at once, because a connection that
            // worked is likely to again.
            Ok(()) => wait = Duration::from_secs(1),
            Err(e) => tracing::warn!(error = %e, "the live channel could not listen; trying again"),
        }
        lost = true;
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(Duration::from_secs(30));
    }
}

/// One connection's worth. Returns when the connection ends.
async fn listen_once(url: &str, tx: &broadcast::Sender<Change>, lost: bool) -> Result<(), tokio_postgres::Error> {
    let (client, mut connection) = tokio_postgres::connect(url, tokio_postgres::NoTls).await?;
    // The connection has to be polled for the client to make progress, and
    // polling it this way is also how notifications arrive.
    let (heard_tx, mut heard) = tokio::sync::mpsc::unbounded_channel();
    let driver = tokio::spawn(async move {
        let mut messages = futures_util::stream::poll_fn(move |cx| connection.poll_message(cx));
        while let Some(message) = messages.next().await {
            match message {
                Ok(AsyncMessage::Notification(n)) => {
                    if heard_tx.send(parse(n.payload())).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    });
    client.batch_execute(&format!("LISTEN {CHANNEL}")).await?;
    tracing::info!("the live channel is listening");
    if lost {
        let _ = tx.send(Change::Everywhere);
    }
    while let Some(change) = heard.recv().await {
        let _ = tx.send(change);
    }
    drop(client);
    match driver.await {
        Ok(result) => result,
        Err(_) => Ok(()),
    }
}

/// One server-sent event, as the bytes on the wire.
fn frame(event: &str) -> web::Bytes {
    web::Bytes::from(format!("event: {event}\ndata: {{}}\n\n"))
}

/// `GET /changes`: a `text/event-stream` of `changed` events for the caller's
/// tenant and site. It opens with `hello`, so a device knows the stream is
/// really arriving rather than being held back by something in between, and
/// it ends after a few minutes or when the server stops.
#[get("/changes")]
pub async fn stream(
    req: HttpRequest,
    state: web::Data<AppState>,
    live: web::Data<Live>,
) -> Result<HttpResponse, ApiError> {
    // A stream opened while the server drains would miss `Closing` and hold
    // the stop open. The device tries again, and reaches the next server.
    if crate::health::is_draining() {
        return Ok(HttpResponse::ServiceUnavailable().finish());
    }
    let who = caller(&state, &req).await?;
    let (tenant, site) = (who.tenant_id, who.site_id);
    let rx = live.subscribe();
    let until = tokio::time::Instant::now() + STREAM_FOR;

    // `retry` is how long a browser's EventSource waits to reconnect; the
    // client here does its own, but says the same.
    let hello = web::Bytes::from("retry: 3000\nevent: hello\ndata: {}\n\n");
    let first = futures_util::stream::once(async move { Ok::<_, actix_web::Error>(hello) });
    let rest = futures_util::stream::unfold(Some(rx), move |rx| async move {
        let mut rx = rx?;
        loop {
            let now = tokio::time::Instant::now();
            if now >= until {
                return None;
            }
            let tick = KEEPALIVE.min(until - now);
            match tokio::time::timeout(tick, rx.recv()).await {
                Ok(Ok(Change::Closing)) | Ok(Err(broadcast::error::RecvError::Closed)) => return None,
                Ok(Ok(change)) if change.reaches(tenant, site) => {
                    return Some((Ok::<_, actix_web::Error>(frame("changed")), Some(rx)));
                }
                Ok(Ok(_)) => continue,
                // Too far behind to know what was missed: read again.
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                    return Some((Ok(frame("changed")), Some(rx)));
                }
                Err(_) => return Some((Ok(web::Bytes::from_static(b": keepalive\n\n")), Some(rx))),
            }
        }
    });

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("cache-control", "no-cache"))
        // nginx and its relatives hold a response back to buffer it otherwise.
        .insert_header(("x-accel-buffering", "no"))
        .streaming(first.chain(rest)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Uuid = Uuid::from_u128(1);
    const OTHER: Uuid = Uuid::from_u128(2);
    const SITE: Uuid = Uuid::from_u128(10);
    const ELSEWHERE: Uuid = Uuid::from_u128(11);

    #[test]
    fn a_change_reaches_its_own_tenant_and_site_only() {
        let here = Change::At { tenant: T, site: Some(SITE) };
        assert!(here.reaches(T, Some(SITE)));
        assert!(!here.reaches(T, Some(ELSEWHERE)), "another site of the same tenant");
        assert!(!here.reaches(OTHER, Some(SITE)), "another tenant, whatever its site");
        // A device signed on without a site hears its whole tenant.
        assert!(here.reaches(T, None));
        // A change at no site in particular reaches every site of its tenant.
        assert!(Change::At { tenant: T, site: None }.reaches(T, Some(ELSEWHERE)));
        assert!(Change::Everywhere.reaches(OTHER, None));
        assert!(!Change::Closing.reaches(T, Some(SITE)));
    }

    #[test]
    fn a_payload_is_read_as_migration_118_writes_it() {
        let payload = format!(r#"{{"tenant" : "{T}", "site" : "{SITE}"}}"#);
        assert_eq!(parse(&payload), Change::At { tenant: T, site: Some(SITE) });
        let nowhere = format!(r#"{{"tenant" : "{T}", "site" : null}}"#);
        assert_eq!(parse(&nowhere), Change::At { tenant: T, site: None });
        assert_eq!(parse("not json"), Change::Everywhere, "unreadable tells everybody");
    }
}
