//! The item list, exported (D216), over HTTP.
//!
//! An item in a family whose carton is measured, its own each measured, and
//! its box drawn. Its exported row says what its page says, level by level;
//! the CSV reads as text a spreadsheet opens; the workbook holds its picture.

use std::io::{Cursor, Read};

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};
use uuid::Uuid;

use super::common;
use common::{pool, url};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

/// A real PNG, which a workbook's picture has to be.
fn png() -> Vec<u8> {
    let mut picture = image::RgbaImage::new(24, 16);
    for (x, _, px) in picture.enumerate_pixels_mut() {
        *px = image::Rgba([(x * 10) as u8, 120, 200, 255]);
    }
    let mut out = Cursor::new(vec![]);
    picture
        .write_to(&mut out, image::ImageFormat::Png)
        .expect("a png");
    out.into_inner()
}

#[actix_web::test]
async fn the_list_exported_says_what_each_item_page_says() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let state = web::Data::new(AppState { pool: pool(&u) });
    let db = state.pool.get().await.expect("a connection");
    db.execute(
        "SELECT set_config('spork.tenant_id', $1, false)",
        &[&TENANT],
    )
    .await
    .expect("the tenant scope");
    let n = Uuid::new_v4().simple().to_string()[..8].to_uppercase();
    let code = format!("EXP-{n}");
    let style: Uuid = db
        .query_one(
            "INSERT INTO item_style (tenant_id, code, description) VALUES (current_tenant(), $1, 'Dish brush family')
             RETURNING id",
            &[&format!("EXPFAM-{n}")],
        )
        .await
        .expect("the family")
        .get(0);
    let item: Uuid = db
        .query_one(
            "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking, style_id)
             SELECT current_tenant(), $1, 'Dish brush, × 2, ‘soft’', u.id, 'none', $2 FROM unit u WHERE u.code = 'ea'
             RETURNING id",
            &[&code, &style],
        )
        .await
        .expect("the item")
        .get(0);

    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(routes::configure),
    )
    .await;
    let auth = ("authorization", common::bearer(&app).await);
    let call = |req: test::TestRequest| {
        let auth = auth.clone();
        let app = &app;
        async move {
            let r = test::call_service(app, req.insert_header(auth).to_request()).await;
            let status = r.status().as_u16();
            let kind = r
                .headers()
                .get("content-type")
                .map(|v| v.to_str().unwrap_or("").to_string())
                .unwrap_or_default();
            let disposition = r
                .headers()
                .get("content-disposition")
                .map(|v| v.to_str().unwrap_or("").to_string())
                .unwrap_or_default();
            (status, kind, disposition, test::read_body(r).await.to_vec())
        }
    };
    let json_of = |b: &[u8]| {
        serde_json::from_slice::<Value>(b).unwrap_or_else(|_| json!(String::from_utf8_lossy(b)))
    };
    let now = "2026-10-05T03:00:00Z";
    let observe = |body: Value| {
        call(
            test::TestRequest::post()
                .uri("/observations")
                .set_json(body),
        )
    };

    // ── a carton of 1,000 in packs of 50: what makes the family's a carton ─
    let (status, _, _, said) = call(
        test::TestRequest::post()
            .uri(&format!("/items/{item}/carton"))
            .set_json(json!({
                "holds": 20, "per": 50, "client_event_id": Uuid::new_v4(), "occurred_at": now,
            })),
    )
    .await;
    assert!(status < 300, "{status}: {}", json_of(&said));

    // ── the family's carton, and the item's own each ────────────────────
    let (status, _, _, said) = observe(json!({
        "item_style_id": style, "packaging_level": "carton",
        "measurements": [
            { "metric": "gross_weight", "entered_value": "6.4", "unit": "kg" },
            { "metric": "length", "entered_value": "41", "unit": "cm" },
            { "metric": "width",  "entered_value": "31", "unit": "cm" },
            { "metric": "height", "entered_value": "22.5", "unit": "cm" },
        ],
        "method": "instrument", "client_event_id": Uuid::new_v4(), "occurred_at": now,
    }))
    .await;
    assert_eq!(status, 200, "{}", json_of(&said));
    let (status, _, _, said) = observe(json!({
        "item_id": item, "packaging_level": "each",
        "measurements": [
            { "metric": "gross_weight", "entered_value": "0.52", "unit": "kg" },
            { "metric": "length", "entered_value": "24", "unit": "cm" },
            { "metric": "width",  "entered_value": "6", "unit": "cm" },
            { "metric": "height", "entered_value": "4.5", "unit": "cm" },
        ],
        "presentation": "as_supplied",
        "method": "instrument", "client_event_id": Uuid::new_v4(), "occurred_at": now,
    }))
    .await;
    assert_eq!(status, 200, "{}", json_of(&said));
    db.execute(
        "SELECT projection_observation_current_rebuild(current_tenant())",
        &[],
    )
    .await
    .expect("the fold");

    // ── its box, drawn ──────────────────────────────────────────────────
    let (status, _, _, stored) = call(
        test::TestRequest::post()
            .uri("/images")
            .insert_header(("content-type", "image/png"))
            .set_payload(png()),
    )
    .await;
    assert_eq!(status, 200, "{}", json_of(&stored));
    let drawing = json_of(&stored)["digest"].as_str().unwrap().to_string();
    let (status, _, _, drawn) = call(
        test::TestRequest::post()
            .uri(&format!("/items/{item}/box-picture"))
            .set_json(json!({
                "digest": drawing, "made_from": [drawing, drawing, drawing],
                "client_event_id": Uuid::new_v4(), "occurred_at": now,
            })),
    )
    .await;
    assert!(status < 300, "{status}: {}", json_of(&drawn));

    // ── its row says what its page says ─────────────────────────────────
    let (status, _, _, page) = call(test::TestRequest::get().uri(&format!("/items/{item}"))).await;
    assert_eq!(status, 200);
    let page = json_of(&page);
    let (status, _, _, rows) =
        call(test::TestRequest::get().uri(&format!("/items/export?format=json&q={code}"))).await;
    assert_eq!(status, 200, "{}", json_of(&rows));
    let rows = json_of(&rows);
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 1, "the list's own search: {rows:?}");
    let row = &rows[0];
    for level in ["each", "carton"] {
        let card = page["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| {
                s["item_id"] == item.to_string()
                    && s["packaging_level"] == level
                    && s["lot_id"].is_null()
            })
            .unwrap_or_else(|| panic!("its {level} card: {page}"));
        for (ours, theirs) in [
            ("weight_g", "gross_weight_g"),
            ("length_mm", "length_mm"),
            ("width_mm", "width_mm"),
            ("height_mm", "height_mm"),
            ("source", "source"),
            ("photos", "faces"),
        ] {
            assert_eq!(
                row[level][ours], card[theirs],
                "{level} {ours}: the export and the page agree"
            );
        }
    }
    assert_eq!(
        row["carton"]["source"], "style",
        "its carton is its family's, and says so"
    );
    assert_eq!(row["carton"]["weight_g"], 6400, "6.4 kg, its family's");
    assert_eq!(row["each"]["arranged"], "As supplied");
    assert!(row["each"]["by"].is_string(), "who recorded it: {row}");
    assert!(
        row["carton"]["by"].is_string(),
        "who recorded its family's: {row}"
    );
    assert_eq!(row["family"], format!("EXPFAM-{n}"));
    assert_eq!(
        (
            row["carton_holds"].as_i64(),
            row["packs_per_carton"].as_i64(),
            row["each_per_pack"].as_i64()
        ),
        (Some(1000), Some(20), Some(50)),
        "1,000 in packs of 50"
    );
    assert_eq!(row["picture"]["kind"], "drawing");
    assert_eq!(row["picture"]["digest"], drawing.as_str());

    // The whole list, nothing asked: every item, as the list counts them.
    let (_, _, _, all) = call(test::TestRequest::get().uri("/items/export?format=json")).await;
    let (_, _, _, listed) = call(test::TestRequest::get().uri("/items")).await;
    assert_eq!(
        json_of(&all).as_array().unwrap().len() as i64,
        json_of(&listed)["total"].as_i64().unwrap()
    );

    // ── CSV: text a spreadsheet opens as written ────────────────────────
    let (status, kind, disposition, csv) =
        call(test::TestRequest::get().uri(&format!("/items/export?q={code}"))).await;
    assert_eq!(status, 200);
    assert!(kind.starts_with("text/csv"), "{kind}");
    assert!(
        disposition.contains("attachment") && disposition.contains(".csv"),
        "{disposition}"
    );
    assert!(
        csv.starts_with(b"\xEF\xBB\xBF"),
        "a byte-order mark, so `×` reads as itself"
    );
    let text = String::from_utf8(csv[3..].to_vec()).expect("UTF-8");
    let mut lines = text.lines();
    let head = lines.next().unwrap();
    assert!(
        head.contains("Each weight (kg)") && head.contains("Carton length (cm)"),
        "{head}"
    );
    let line = lines.next().expect("its row");
    assert!(
        line.contains(&code)
            && line.contains("0.520")
            && line.contains("24.0")
            && line.contains("6.400"),
        "{line}"
    );
    assert!(
        line.contains("× 2, ‘soft’"),
        "its description, as typed: {line}"
    );
    assert!(
        line.contains(&format!("/images/{drawing}")),
        "a link to its picture: {line}"
    );

    // ── the workbook holds its picture ──────────────────────────────────
    let (status, kind, disposition, book) =
        call(test::TestRequest::get().uri(&format!("/items/export?format=xlsx&q={code}"))).await;
    assert_eq!(status, 200);
    assert!(kind.contains("spreadsheetml"), "{kind}");
    assert!(disposition.contains(".xlsx"), "{disposition}");
    let mut zip = zip::ZipArchive::new(Cursor::new(book)).expect("a workbook is a zip");
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("xl/media/") && n.ends_with(".png")),
        "its picture: {names:?}"
    );
    let mut strings = String::new();
    zip.by_name("xl/sharedStrings.xml")
        .expect("its text")
        .read_to_string(&mut strings)
        .unwrap();
    assert!(
        strings.contains(&code) && strings.contains("Each weight (kg)"),
        "{strings}"
    );

    // ── refused, in words ───────────────────────────────────────────────
    let (status, _, _, _) = call(test::TestRequest::get().uri("/items/export?format=pdf")).await;
    assert_eq!(status, 400);
}
