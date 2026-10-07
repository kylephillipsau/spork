//! A batch of picking tickets, looked up (D231) and shared out (D230).
//!
//! NetSuite's open orders arrive as a report, as the Bridge will send them;
//! the batch is pasted as a row of a sheet; each order says where it stands,
//! each line where to take it from, a bin short sends the rest to the next,
//! and the batch is shared between pickers. An empty report, said to be
//! empty, clears the last.
//!
//! The item and the order numbers are this run's own, so the bins hold
//! exactly what this file says.

use actix_web::{test, web, App};
use serde_json::{json, Value};
use spork_server::{routes, AppState};

use super::common;
use common::{pool, url, SITE};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

async fn call<S>(app: &S, req: test::TestRequest) -> (u16, Value)
where
    S: actix_web::dev::Service<actix_http::Request, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error>,
{
    let r = test::call_service(app, req.to_request()).await;
    let status = r.status().as_u16();
    let bytes = test::read_body(r).await;
    (status, serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned())))
}

fn order<'a>(read: &'a Value, asked: &str) -> &'a Value {
    read["orders"].as_array().unwrap().iter().find(|o| o["asked"] == asked).unwrap_or_else(|| panic!("{asked} asked: {read}"))
}

fn takes(line: &Value) -> Vec<(String, f64)> {
    line["takes"].as_array().unwrap().iter().map(|t| (t["bin"].as_str().unwrap().to_string(), t["quantity"].as_f64().unwrap())).collect()
}

#[actix_web::test]
async fn a_batch_of_tickets_is_looked_up_and_shared_out() {
    let _file = common::file_gate(module_path!());
    let Some(u) = url() else {
        eprintln!("no DATABASE_URL: skipping");
        return;
    };
    let db = common::connect(&u, false).await;
    let state = web::Data::new(AppState { pool: pool(&u) });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let session = common::bearer(&app).await;
    let minted = common::ok_json(
        &app,
        test::TestRequest::post()
            .uri("/tokens")
            .insert_header(("authorization", common::bearer_without_site(&app).await))
            .set_json(json!({ "label": "open orders, from a test" }))
            .to_request(),
        "minting an import token",
    )
    .await;
    let machine = format!("Bearer {}", minted["token"].as_str().unwrap());

    let run = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() % 1_000_000;
    let code = format!("TICKET-{run}");
    db.execute(
        "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
         SELECT $1::uuid, $2, 'Gloves for a picking ticket', u.id, 'none' FROM unit u WHERE u.code = 'ea'",
        &[&uuid::Uuid::parse_str(TENANT).unwrap(), &code],
    )
    .await
    .expect("this run's item");

    // NetSuite's balance: 6 on the pick face, 20 in bulk.
    let (status, balance) = call(
        &app,
        test::TestRequest::post()
            .uri(&format!("/import/stock?as_at=2026-10-07T01:00:00Z&source=test-ticket-balance-{run}&apply=true"))
            .insert_header(("authorization", machine.clone()))
            .insert_header(("content-type", "text/csv"))
            .set_payload(format!(
                "Item,Location,Bin Number,On Hand\n{code},Melbourne Warehouse,A-01-1,6\n{code},Melbourne Warehouse,B-01-1,20\n"
            )),
    )
    .await;
    assert_eq!(status, 200, "{balance}");

    // What NetSuite has still to pick: three orders, one of a kit.
    let (a, b, c, d) = (format!("S7{run:06}1"), format!("S7{run:06}2"), format!("S7{run:06}3"), format!("S7{run:06}4"));
    let header = "Order,Order ID,Date,Customer,Ship To,Picking Instructions,Customer Notes,Ship Via,PO,Status,Line,Line No,Item,Description,Art No,Item Type,Kit Line,Location,Ordered,Committed,To Pick\n";
    let report = format!(
        "{header}\
{a},1,6/10/2026,Cafe,\"Cafe\n1 Main St\",Rear door,No snakes,Courier,PO-1,Pending Fulfillment,11,1,{code},Gloves,AN-1,InvtPart,,Melbourne Warehouse,10,10,10\n\
{b},2,7/10/2026,Deli,,,,,,Pending Fulfillment,21,1,{code},Gloves,,InvtPart,,Melbourne Warehouse,15,15,15\n\
{b},2,7/10/2026,Deli,,,,,,Pending Fulfillment,22,2,NOBODY-KNOWS,Something,,InvtPart,,Melbourne Warehouse,2,2,2\n\
{c},3,7/10/2026,Bakery,,,,,,Pending Fulfillment,31,1,KIT-1,A kit,,Kit,,Melbourne Warehouse,1,1,1\n\
{c},3,7/10/2026,Bakery,,,,,,Pending Fulfillment,32,2,{code},Gloves,,InvtPart,31,Melbourne Warehouse,2,2,2\n\
{d},4,7/10/2026,Florist,,,,,,Pending Fulfillment,41,1,KIT-2,A kit with no parts here,,Kit,,Melbourne Warehouse,1,1,1\n"
    );
    let source = format!("test-open-orders-{run}");
    let load = |body: String, expect: Option<usize>| {
        let mut uri = format!("/import/open-orders?as_at=2026-10-07T01:05:00Z&source={source}&apply=true");
        if let Some(n) = expect {
            uri.push_str(&format!("&expect={n}"));
        }
        test::TestRequest::post()
            .uri(&uri)
            .insert_header(("authorization", machine.clone()))
            .insert_header(("content-type", "text/csv"))
            .set_payload(body)
    };
    let (status, loaded) = call(&app, load(report, Some(6))).await;
    assert_eq!(status, 200, "{loaded}");
    assert_eq!(loaded["loaded"]["lines_written"], 6, "{loaded}");
    assert_eq!(loaded["loaded"]["items_unknown"], 3, "kept, with no item: {loaded}");
    let (status, short) = call(&app, load(format!("{header}{a},1,,,,,,,,,11,1,{code},,,,,Melbourne Warehouse,1,1,1\n"), Some(2))).await;
    assert_eq!((status, &short["loaded"]["lines_written"]), (200, &json!(0)), "a short report is kept, not loaded: {short}");

    // The row of the sheet, with an order typed without its S and one nobody has.
    let row = format!("{a}\t{}\tS0000001\t{c}\t{d}", &b[1..]);
    let get = |q: String| {
        test::TestRequest::get().uri(&format!("/sites/{SITE}/to-pick?orders={}{q}", urlencode(&row))).insert_header(("authorization", session.clone()))
    };
    let (status, read) = call(&app, get(String::new())).await;
    assert_eq!(status, 200, "{read}");
    assert_eq!(order(&read, &a)["state"], "waiting");
    assert_eq!((&order(&read, &a)["picking_instructions"], &order(&read, &a)["customer_notes"]), (&json!("Rear door"), &json!("No snakes")));
    assert_eq!(order(&read, &a)["ship_to"], "Cafe\n1 Main St");
    assert_eq!(order(&read, &b[1..])["number"], b.as_str(), "found by its digits");
    assert_eq!(order(&read, "S0000001")["state"], "unknown");

    // The pick face first, then bulk for the rest; the next order gets what
    // the first left.
    let first = &order(&read, &a)["lines"][0];
    assert_eq!(first["art_no"], "AN-1");
    assert_eq!(takes(first), vec![("A-01-1".into(), 6.0), ("B-01-1".into(), 4.0)], "{first}");
    let second = &order(&read, &b[1..])["lines"];
    assert_eq!(takes(&second[0]), vec![("B-01-1".into(), 15.0)], "{second}");
    assert_eq!(second[1]["short"], 2.0, "no bin for a code nobody knows: {second}");
    let kit = &order(&read, &c)["lines"];
    assert_eq!((kit[0]["kit"].as_bool(), kit[1]["part_of"].as_str()), (Some(true), Some("KIT-1")), "{kit}");
    assert_eq!(takes(&kit[1]), vec![("B-01-1".into(), 1.0)], "a part is picked, the kit isn't; one left in bulk: {kit}");
    assert_eq!(kit[1]["short"], 1.0);
    let alone = &order(&read, &d)["lines"][0];
    assert_eq!((alone["kit"].as_bool(), alone["short"].as_f64()), (Some(false), Some(1.0)), "a kit with no parts on the order is picked as it is: {alone}");

    // One picker: one trip with every waiting order.
    let trips = &read["plan"]["pickers"];
    assert_eq!(trips.as_array().unwrap().len(), 1, "{read}");
    assert_eq!(trips[0][0]["orders"].as_array().unwrap().len(), 4, "{trips}");
    // Two pickers, one order a trip: every order on a trip of its own, shared out.
    let (_, shared) = call(&app, get("&pickers=2&per_trip=1".into())).await;
    let plan = &shared["plan"]["pickers"];
    let each: Vec<usize> = plan.as_array().unwrap().iter().map(|p| p.as_array().unwrap().len()).collect();
    assert_eq!(each.iter().sum::<usize>(), 4, "{shared}");
    assert!(each.iter().all(|&n| n >= 1), "both pick: {shared}");
    // Gathered: the shelf every order wants is walked to by one trip.
    let (_, gathered) = call(&app, get("&pickers=2&per_trip=1&gather=true".into())).await;
    let bulk_trips = gathered["plan"]["pickers"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p.as_array().unwrap())
        .filter(|t| t["stops"].as_array().unwrap().iter().any(|s| s["bin"] == "B-01-1"))
        .count();
    assert_eq!(bulk_trips, 1, "{gathered}");

    // Nothing left to pick, said so: the report is cleared.
    let (status, refused) = call(&app, load(header.to_string(), None)).await;
    assert_eq!(status, 400, "an empty report not said to be empty: {refused}");
    let (status, cleared) = call(&app, load(header.to_string(), Some(0))).await;
    assert_eq!(status, 200, "{cleared}");
    let (_, after) = call(&app, get(String::new())).await;
    assert_eq!(order(&after, &a)["state"], "unknown", "{after}");
    assert_eq!(after["plan"], Value::Null);

    db.execute("DELETE FROM reported_stock WHERE source = $1", &[&format!("test-ticket-balance-{run}")]).await.unwrap();
}

/// Just enough of a URL's encoding for a pasted row: tabs and spaces.
fn urlencode(s: &str) -> String {
    s.replace('\t', "%09").replace(' ', "%20")
}
