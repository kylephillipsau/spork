//! Boxes cut to set heights, and box types photographed, over HTTP (D252,
//! migration 141).
//!
//! A box of the test's own, so nothing another test reads changes. Heights are
//! added, and the wrong ones refused; a box on file is linked by its name; the
//! full box is photographed, and a carton of a cut is drawn with its
//! photographs, weighs its empty weight and is booked at its height; and a
//! height is taken away, unless something was packed in it.
//!
//! Names are random, so this runs against a database it has already written to.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

/// PACK-1, where the fixture's Melbourne packs (migration 97).
const PACK: &str = "10c00000-0000-0000-0000-000000000004";
/// The fixture's small box preset, for the workspace it belongs to.
const SMALL_BOX: &str = "9a7e0000-0000-0000-0000-0000000000b1";
/// The shared PALLET preset (migration 67): the platform's, not the workspace's.
const PALLET: &str = "9a7e0000-0000-0000-0000-000000000001";

async fn call<S>(app: &S, req: test::TestRequest) -> (u16, Value)
where
    S: actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
{
    let r = test::call_service(app, req.to_request()).await;
    let status = r.status().as_u16();
    let bytes = test::read_body(r).await;
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, v)
}

#[actix_web::test]
async fn a_box_is_cut_to_set_heights_and_drawn_from_its_photographs() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let session = common::bearer(&app).await;
    let auth = ("authorization", session.clone());
    let post = |uri: String, body: Value| test::TestRequest::post().uri(&uri).insert_header(auth.clone()).set_json(body);
    let now = chrono::Utc::now().to_rfc3339();

    let (db, connection) = tokio_postgres::connect(&u, tokio_postgres::NoTls).await.expect("connect");
    tokio::spawn(async move { let _ = connection.await; });
    let run = &Uuid::new_v4().simple().to_string()[..8];
    let (full_name, half_name, odd_name) = (format!("Test Box {run}"), format!("Half {run}"), format!("Odd {run}"));
    let mut made = vec![];
    for (name, height) in [(&full_name, 300), (&half_name, 150), (&odd_name, 120)] {
        let id: Uuid = db
            .query_one(
                "INSERT INTO package_type (tenant_id, name, carrier_package_code, length_mm, width_mm, height_mm)
                 SELECT tenant_id, $1, 'CTN', 400, 300, $2 FROM package_type WHERE id = $3::text::uuid
                 RETURNING id",
                &[name, &height, &SMALL_BOX],
            )
            .await
            .expect("a box of the test's own")
            .get(0);
        made.push(id);
    }
    let (full, half) = (made[0], made[1]);
    let (status, body) = call(&app, post(format!("/package-types/{full}/empty-weight"), json!({ "tare_weight_g": 500 }))).await;
    assert_eq!(status, 204, "{body}");

    // ── heights added, and the wrong ones refused ───────────────────────
    let height = |id: Uuid, mm: i32, name: Option<&str>| post(format!("/package-types/{id}/heights"), json!({ "height_mm": mm, "name": name }));
    let (status, cut) = call(&app, height(full, 200, None)).await;
    assert_eq!(status, 200, "{cut}");
    assert_eq!(cut["name"], format!("{full_name}, cut to 20"), "{cut}");
    assert_eq!(cut["linked"], false);
    let cut_id: Uuid = cut["id"].as_str().unwrap().parse().unwrap();
    let (status, again) = call(&app, height(full, 200, None)).await;
    assert_eq!(status, 400, "one cut a height: {again}");
    let (status, tall) = call(&app, height(full, 300, None)).await;
    assert_eq!(status, 400, "a cut is lower than its box: {tall}");
    let (status, twice) = call(&app, height(cut_id, 100, None)).await;
    assert_eq!(status, 400, "a cut is not cut again: {twice}");
    assert!(twice.to_string().contains("full box"), "{twice}");
    let (status, theirs) = call(&app, height(PALLET.parse().unwrap(), 500, None)).await;
    assert_eq!(status, 404, "the platform's pallet is not the workspace's to cut: {theirs}");

    // ── a box on file, by its name ──────────────────────────────────────
    let (status, linked) = call(&app, height(full, 150, Some(&half_name))).await;
    assert_eq!(status, 200, "{linked}");
    assert_eq!(linked["linked"], true, "the half box on file becomes the cut: {linked}");
    assert_eq!(linked["id"], half.to_string());
    let (status, odd) = call(&app, height(full, 100, Some(&odd_name))).await;
    assert_eq!(status, 400, "a box of another height is not this cut: {odd}");

    // ── the full box photographed; the platform's is nobody's to ─────────
    let look = |subject: &str| {
        post("/observations".into(), json!({
            "package_type_id": subject, "measurements": [], "photographs": true,
            "client_event_id": Uuid::now_v7(), "occurred_at": now,
        }))
    };
    let (status, refused) = call(&app, look(PALLET)).await;
    assert_eq!(status, 404, "{refused}");
    let (status, looked) = call(&app, look(&full.to_string())).await;
    assert_eq!(status, 200, "{looked}");
    let (status, photo) = call(
        &app,
        test::TestRequest::post()
            .uri(&format!("/observations/{}/images/front", looked["observation_event_id"].as_str().unwrap()))
            .insert_header(auth.clone())
            .insert_header(("content-type", "image/png"))
            .set_payload(common::png(1200, 900)),
    )
    .await;
    assert_eq!(status, 200, "{photo}");
    let (status, face) = call(
        &app,
        post(
            format!("/observation-images/{}/cuts", photo["image_id"].as_str().unwrap()),
            json!({ "digest": photo["digest"], "corners": [0.1, 0.1, 0.9, 0.1, 0.9, 0.9, 0.1, 0.9],
                    "client_event_id": Uuid::now_v7(), "occurred_at": now }),
        ),
    )
    .await;
    assert_eq!(status, 200, "{face}");

    let (_, boxes) = call(&app, test::TestRequest::get().uri("/package-types").insert_header(auth.clone())).await;
    let row = |id: Uuid| boxes.as_array().unwrap().iter().find(|b| b["id"] == id.to_string()).cloned().expect("the box");
    let photos = &row(full)["photos"];
    assert_eq!(photos[0]["face"], "front", "{boxes}");
    assert_eq!(photos[0]["cut"]["digest"], photo["digest"], "{boxes}");
    assert_eq!(row(cut_id)["cut"]["from_name"], full_name.as_str(), "{boxes}");
    assert_eq!(row(cut_id)["cut"]["full_height_mm"], 300);
    assert!(row(cut_id)["tare_weight_g"].is_null(), "its own says nothing; the full box's stands in: {boxes}");

    // ── a carton of the cut on the bench ────────────────────────────────
    let (_, minted) = call(&app, post("/tokens".into(), json!({ "label": "boxes test" }))).await;
    let token = format!("Bearer {}", minted["token"].as_str().unwrap());
    let if_id = format!("if-bx-{run}");
    let (status, imported) = call(
        &app,
        test::TestRequest::post()
            .uri("/import/fulfilment?apply=true")
            .insert_header(("authorization", token))
            .set_payload(
                json!({
                    "order": format!("S-bx-{run}"),
                    "customer": format!("Boxes Test {run}"),
                    "fulfilment_id": if_id,
                    "fulfilment_number": "IF-BX",
                    "status": "Picked",
                    "observed_at": "2026-10-14T11:00:00Z",
                    "lines": [
                        { "line": 1, "item": format!("BX-{run}"), "description": "Boxes test item",
                          "location": "Melbourne Warehouse", "quantity": 3, "external_line": "1" },
                    ]
                })
                .to_string(),
            ),
    )
    .await;
    assert_eq!(status, 200, "{imported}");
    let order = db
        .query_one(
            "SELECT f.id, fl.id FROM fulfilment_line fl JOIN fulfilment f ON f.id = fl.fulfilment_id
              WHERE f.external_id = $1",
            &[&if_id],
        )
        .await
        .expect("the fulfilment");
    let (fulfilment, line): (Uuid, Uuid) = (order.get(0), order.get(1));
    let carton = Uuid::now_v7();
    let (status, body) = call(
        &app,
        post("/packages".into(), json!({
            "id": carton, "fulfilment_id": fulfilment, "package_type_id": cut_id, "location_id": PACK,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T11:05:00Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "{body}");

    let (status, bench) = call(&app, test::TestRequest::get().uri(&format!("/fulfilments/{fulfilment}/bench")).insert_header(auth.clone())).await;
    assert_eq!(status, 200, "{bench}");
    let preset = bench["presets"].as_array().unwrap().iter().find(|p| p["id"] == cut_id.to_string()).cloned().expect("the cut");
    assert_eq!(preset["cut"]["from_name"], full_name.as_str(), "the bench says what to cut: {preset}");
    assert_eq!(preset["tare_weight_g"], 500, "the full box's empty weight: {preset}");
    assert_eq!(preset["faces"]["front"], photo["digest"], "the full box's photographs: {preset}");
    let on = bench["cartons"].as_array().unwrap().iter().find(|c| c["id"] == carton.to_string()).cloned().expect("the carton");
    assert_eq!(on["cut"]["full_height_mm"], 300, "{on}");
    assert_eq!(on["faces"]["front"], photo["digest"], "{on}");
    assert_eq!(on["stated_size"]["height_mm"], 200, "{on}");

    // ── booked at its height, which the cut says ────────────────────────
    let (status, handed) = call(
        &app,
        post("/handovers".into(), json!({
            "fulfilment_line_id": line, "quantity": 3, "to_package_id": carton,
            "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T11:06:00Z"
        })),
    )
    .await;
    assert_eq!(status, 200, "{handed}");
    let (status, sealed) = call(
        &app,
        post(format!("/packages/{carton}/seal"), json!({ "client_event_id": Uuid::now_v7(), "occurred_at": "2026-10-14T11:07:00Z" })),
    )
    .await;
    assert_eq!(status, 200, "{sealed}");
    let (status, booked) = call(&app, post("/consignments".into(), json!({ "id": Uuid::now_v7(), "package_ids": [carton] }))).await;
    assert_eq!(status, 200, "{booked}");
    assert_eq!(booked["carrier_lines"][0]["height_mm"], 200, "never measured, and still its height: {booked}");

    // ── taken away, unless something was packed in it ───────────────────
    let remove = |id: Uuid| test::TestRequest::delete().uri(&format!("/package-types/{id}")).insert_header(auth.clone());
    let (status, kept) = call(&app, remove(cut_id)).await;
    assert_eq!(status, 400, "packed in, it stays: {kept}");
    assert!(kept.to_string().contains("untick Suggest"), "{kept}");
    let (status, full_kept) = call(&app, remove(full)).await;
    assert_eq!(status, 400, "a full box is the import's: {full_kept}");
    let (status, body) = call(&app, remove(half)).await;
    assert_eq!(status, 204, "{body}");
}
