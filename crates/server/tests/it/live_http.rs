//! The live channel (D206): a device signed on at a site hears that something
//! changed there, and only there; and a change written to the database reaches
//! the hub through migration 118's triggers and the listening connection.

use std::pin::Pin;
use std::time::Duration;

use actix_web::body::MessageBody;
use actix_web::{test, web, App};
use spork_server::live::{Change, Live};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

const ALPHA: Uuid = Uuid::from_u128(0x11111111_1111_1111_1111_111111111111);
const SITE: Uuid = Uuid::from_u128(0xa5170000_0000_0000_0000_000000000001);
const FULFILMENT: &str = "f01f0000-0000-0000-0000-000000000001";

/// The next piece of a streamed body, or `None` if nothing came in time.
async fn next(body: &mut actix_web::body::BoxBody, within: Duration) -> Option<String> {
    let piece = tokio::time::timeout(within, futures_util::future::poll_fn(|cx| Pin::new(&mut *body).poll_next(cx)))
        .await
        .ok()??;
    Some(String::from_utf8_lossy(&piece.expect("a readable piece")).into_owned())
}

#[actix_web::test]
async fn a_device_hears_what_changed_at_its_own_site() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let live = web::Data::new(Live::quiet());
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { pool: pool(&u) }))
            .app_data(live.clone())
            .configure(routes::configure),
    )
    .await;

    let nobody = test::call_service(&app, test::TestRequest::get().uri("/changes").to_request()).await;
    assert_eq!(nobody.status().as_u16(), 401, "a stream is a signed-in device's");

    let auth = ("authorization", common::bearer(&app).await);
    let resp = test::call_service(&app, test::TestRequest::get().uri("/changes").insert_header(auth).to_request()).await;
    assert!(resp.status().is_success(), "{}", resp.status());
    assert_eq!(
        resp.headers().get("content-type").and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );
    let mut body = resp.into_body();
    let hello = next(&mut body, Duration::from_secs(2)).await.expect("it opens at once");
    assert!(hello.contains("event: hello"), "{hello}");

    // Another tenant, and another site of this one: not heard.
    live.send(Change::At { tenant: Uuid::new_v4(), site: Some(SITE) });
    live.send(Change::At { tenant: ALPHA, site: Some(Uuid::new_v4()) });
    // This site: heard.
    live.send(Change::At { tenant: ALPHA, site: Some(SITE) });
    let heard = next(&mut body, Duration::from_secs(2)).await.expect("a change here is heard");
    assert_eq!(heard, "event: changed\ndata: {}\n\n", "one frame, and only for this site");

    // The server stopping ends the stream rather than holding the drain.
    live.send(Change::Closing);
    assert_eq!(next(&mut body, Duration::from_secs(2)).await, None, "the stream ends");
}

#[actix_web::test]
async fn a_change_in_the_database_reaches_the_hub_at_commit() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let live = Live::listening(u.clone());
    let mut heard = live.subscribe();
    let db = pool(&u).get().await.expect("a connection");
    db.batch_execute("RESET ROLE").await.expect("as the owner");

    // A statement that changes nothing still fires the statement trigger, so
    // it is a harmless way to make one. Repeated until the listener, which
    // connects on its own time, has started listening.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let change = loop {
        assert!(tokio::time::Instant::now() < deadline, "no notification reached the hub");
        db.execute(
            "UPDATE fulfilment SET state = state WHERE id = $1::text::uuid",
            &[&FULFILMENT],
        )
        .await
        .expect("touch the order");
        match tokio::time::timeout(Duration::from_millis(500), heard.recv()).await {
            Ok(Ok(change @ Change::At { .. })) => break change,
            _ => continue,
        }
    };
    assert_eq!(change, Change::At { tenant: ALPHA, site: Some(SITE) }, "where, and nothing else");
}
