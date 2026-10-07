//! What NetSuite has still to pick (D231), landing in `reported_order_line`.
//!
//! Its open sales orders' lines at a warehouse, one row a line, each carrying
//! its order's particulars. The Bridge sends the whole of it every few minutes,
//! as it sends the balance (D212): a report, replaced by each load of its
//! feed, held with its age.
//!
//! # Not orders, and not work
//!
//! [`super::orders`] loads an order as work: lines committed to a fulfilment,
//! on the picking walk, packed at the bench. A line here is none of that. It is
//! what NetSuite says is left to pick, so that a batch of picking tickets can
//! be looked up, routed, shared out and printed. The picks are recorded in
//! NetSuite, which keeps the shelves, and reach Spork as they always have: an
//! item fulfilment marked Picked (D172).
//!
//! # What it keeps that the balance would drop
//!
//! **A line whose item the catalogue lacks.** The balance leaves an unknown
//! code out, because a quantity on an unknown item is nothing to act on. A
//! line on an order is still to be picked whatever the catalogue knows, so it
//! is kept with its code and no item, counted, and printed without a bin.
//!
//! **An empty load,** when it was said to be empty. Nothing left to pick is a
//! real answer, and the one most worth hearing; [`super::stock::shortfall`]
//! is what tells it from a search that failed.

use std::collections::HashMap;

use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

/// One line of an open order, as sent.
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub order: String,
    pub order_id: Option<String>,
    /// Day first, as NetSuite writes it: `7/10/2026`.
    pub date: Option<String>,
    pub customer: Option<String>,
    pub ship_to: Option<String>,
    /// What the order says to the people picking it.
    pub picking_instructions: Option<String>,
    /// What the business notes of the customer for its own people.
    pub customer_notes: Option<String>,
    pub ship_via: Option<String>,
    pub po: Option<String>,
    pub status: Option<String>,
    pub line: String,
    pub line_no: Option<i32>,
    /// The code as NetSuite renders it; read through [`crate::orders::item_code`].
    pub item: String,
    pub description: Option<String>,
    /// The supplier's code printed on it, as the order line carries it.
    pub art_no: Option<String>,
    pub item_type: Option<String>,
    pub kit_line: Option<String>,
    /// The warehouse, as NetSuite names it.
    pub location: String,
    /// Quantities as text, cast in the statement: `numeric`, and no decimal
    /// dependency here, as the balance does it.
    pub ordered: String,
    pub committed: Option<String>,
    pub to_pick: String,
}

/// The columns, by the names the Bridge writes. Spork's words, not NetSuite's:
/// the Bridge maps NetSuite's fields to these, so a field that moves in
/// NetSuite is a change to the Bridge and nothing here.
const ORDER: &str = "Order";
const ORDER_ID: &str = "Order ID";
const DATE: &str = "Date";
const CUSTOMER: &str = "Customer";
const SHIP_TO: &str = "Ship To";
const PICKING_INSTRUCTIONS: &str = "Picking Instructions";
const CUSTOMER_NOTES: &str = "Customer Notes";
const SHIP_VIA: &str = "Ship Via";
const PO: &str = "PO";
const STATUS: &str = "Status";
const LINE: &str = "Line";
const LINE_NO: &str = "Line No";
const ITEM: &str = "Item";
const DESCRIPTION: &str = "Description";
const ART_NO: &str = "Art No";
const ITEM_TYPE: &str = "Item Type";
const KIT_LINE: &str = "Kit Line";
const LOCATION: &str = "Location";
const ORDERED: &str = "Ordered";
const COMMITTED: &str = "Committed";
const TO_PICK: &str = "To Pick";

/// Read the report from anything. By header name; a column it needs and can't
/// find is an error naming the columns it did find.
pub fn read<R: std::io::Read>(source: R) -> Result<Vec<Row>, String> {
    let mut rdr = csv::Reader::from_reader(source);
    let header: Vec<String> = rdr.headers().map_err(|e| e.to_string())?.iter().map(|s| s.trim().to_string()).collect();
    let find = |name: &str| header.iter().position(|h| h.eq_ignore_ascii_case(name));
    let need = |name: &str| find(name).ok_or_else(|| format!("no {name} column: the file has {}", header.join(", ")));
    let (i_order, i_line, i_item, i_location, i_ordered, i_to_pick) =
        (need(ORDER)?, need(LINE)?, need(ITEM)?, need(LOCATION)?, need(ORDERED)?, need(TO_PICK)?);
    let optional: HashMap<&str, Option<usize>> = [
        ORDER_ID, DATE, CUSTOMER, SHIP_TO, PICKING_INSTRUCTIONS, CUSTOMER_NOTES, SHIP_VIA, PO, STATUS, LINE_NO, DESCRIPTION, ART_NO, ITEM_TYPE, KIT_LINE,
        COMMITTED,
    ]
    .into_iter()
    .map(|n| (n, find(n)))
    .collect();

    let mut rows = vec![];
    for rec in rdr.records() {
        let r = rec.map_err(|e| e.to_string())?;
        let at = |i: usize| r.get(i).map(str::trim).unwrap_or("").to_string();
        let opt = |name: &str| optional[name].map(at).filter(|s| !s.is_empty());
        let row = Row {
            order: at(i_order),
            order_id: opt(ORDER_ID),
            date: opt(DATE),
            customer: opt(CUSTOMER).map(|c| crate::orders::customer_name(&c)),
            ship_to: opt(SHIP_TO),
            picking_instructions: opt(PICKING_INSTRUCTIONS),
            customer_notes: opt(CUSTOMER_NOTES),
            ship_via: opt(SHIP_VIA),
            po: opt(PO),
            status: opt(STATUS),
            line: at(i_line),
            line_no: opt(LINE_NO).and_then(|n| n.parse().ok()),
            item: crate::orders::item_code(&at(i_item)),
            description: opt(DESCRIPTION),
            art_no: opt(ART_NO),
            item_type: opt(ITEM_TYPE),
            kit_line: opt(KIT_LINE),
            location: at(i_location),
            ordered: at(i_ordered),
            committed: opt(COMMITTED),
            to_pick: at(i_to_pick),
        };
        // A row with no order, line or item is a spacer, not a line.
        if row.order.is_empty() || row.line.is_empty() || row.item.is_empty() {
            continue;
        }
        rows.push(row);
    }
    Ok(rows)
}

/// What the report holds, before any database is consulted.
#[derive(Debug, Serialize, PartialEq)]
pub struct OpenOrdersSurvey {
    pub lines: usize,
    pub orders: usize,
    /// Lines with something still to pick.
    pub to_pick: usize,
}

pub fn survey(rows: &[Row]) -> OpenOrdersSurvey {
    let mut orders: Vec<&str> = rows.iter().map(|r| r.order.as_str()).collect();
    orders.sort_unstable();
    orders.dedup();
    OpenOrdersSurvey {
        lines: rows.len(),
        orders: orders.len(),
        to_pick: rows.iter().filter(|r| r.to_pick.parse::<f64>().is_ok_and(|q| q > 0.0)).count(),
    }
}

/// What the database did, or would have done.
#[derive(Debug, Default, Serialize)]
pub struct OpenOrdersLoaded {
    pub lines_written: usize,
    /// Lines the previous load of this feed left, now cleared.
    pub lines_replaced: usize,
    /// Lines kept with their code and no item: the catalogue lacks the code.
    pub items_unknown: usize,
    /// Lines left out: their warehouse is no site here.
    pub warehouses_unknown: usize,
    /// Lines left out: the same order and line sent twice in one load.
    pub lines_repeated: usize,
    pub applied: bool,
}

/// Write the report, or find out what writing it would do. `source` names the
/// feed: a load replaces everything the last one of the same feed wrote.
pub async fn load(
    tx: &Transaction<'_>,
    tenant: Uuid,
    rows: &[Row],
    as_at: chrono::DateTime<chrono::Utc>,
    source: &str,
    apply: bool,
) -> Result<OpenOrdersLoaded, String> {
    tx.batch_execute("SAVEPOINT spork_open_orders").await.map_err(|e| e.to_string())?;
    let mut out = OpenOrdersLoaded { applied: apply, ..Default::default() };
    out.lines_replaced = tx
        .execute("DELETE FROM reported_order_line WHERE tenant_id = $1 AND source = $2", &[&tenant, &source])
        .await
        .map_err(|e| format!("clearing the previous {source}: {e}"))? as usize;

    let mut sites: HashMap<String, Option<Uuid>> = HashMap::new();
    let mut items: HashMap<String, Option<Uuid>> = HashMap::new();
    for r in rows {
        let site = match sites.get(&r.location) {
            Some(s) => *s,
            None => {
                // The matching every NetSuite feed uses, so one warehouse is one site.
                let found: Option<Uuid> = tx
                    .query_opt(
                        "SELECT id FROM site WHERE tenant_id = $1 AND (code = $2 OR name = $3)",
                        &[&tenant, &crate::bins::site_code(&r.location), &crate::bins::site_name(&r.location)],
                    )
                    .await
                    .map_err(|e| e.to_string())?
                    .map(|row| row.get(0));
                sites.insert(r.location.clone(), found);
                found
            }
        };
        let Some(site_id) = site else {
            out.warehouses_unknown += 1;
            continue;
        };
        let item = match items.get(&r.item) {
            Some(i) => *i,
            None => {
                let found: Option<Uuid> = tx
                    .query_opt("SELECT id FROM item WHERE tenant_id = $1 AND code = $2", &[&tenant, &r.item])
                    .await
                    .map_err(|e| e.to_string())?
                    .map(|row| row.get(0));
                items.insert(r.item.clone(), found);
                found
            }
        };
        if item.is_none() {
            out.items_unknown += 1;
        }
        let ordered_on = r.date.as_deref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%d/%m/%Y").ok());
        // Less than nothing left is nothing left: NetSuite can pick more than
        // a line asked for, and the report is of what is still to do. Nothing
        // said committed stays unsaid; `greatest` would make it nought.
        let n = tx
            .execute(
                "INSERT INTO reported_order_line
                     (tenant_id, site_id, order_number, order_external_id, ordered_on, customer, ship_to,
                      picking_instructions, customer_notes, ship_via, po_ref, status, line_key, line_no,
                      item_id, item_code, description, art_no, item_type, kit_line, ordered, committed, to_pick,
                      as_at, source)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                         greatest($21::text::numeric, 0),
                         CASE WHEN $22::text IS NOT NULL THEN greatest($22::text::numeric, 0) END,
                         greatest($23::text::numeric, 0), $24, $25)
                 ON CONFLICT ON CONSTRAINT reported_order_line_once DO NOTHING",
                &[
                    &tenant,
                    &site_id,
                    &r.order,
                    &r.order_id,
                    &ordered_on,
                    &r.customer,
                    &r.ship_to,
                    &r.picking_instructions,
                    &r.customer_notes,
                    &r.ship_via,
                    &r.po,
                    &r.status,
                    &r.line,
                    &r.line_no,
                    &item,
                    &r.item,
                    &r.description,
                    &r.art_no,
                    &r.item_type,
                    &r.kit_line,
                    &r.ordered,
                    &r.committed,
                    &r.to_pick,
                    &as_at,
                    &source,
                ],
            )
            .await
            .map_err(|e| format!("{} line {}: {e}", r.order, r.line))?;
        if n == 1 {
            out.lines_written += 1;
        } else {
            out.lines_repeated += 1;
        }
    }

    let end = if apply { "RELEASE SAVEPOINT spork_open_orders" } else { "ROLLBACK TO SAVEPOINT spork_open_orders" };
    tx.batch_execute(end).await.map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENT: &str = "\
Order,Order ID,Date,Customer,Ship To,Picking Instructions,Customer Notes,Ship Via,PO,Status,Line,Line No,Item,Description,Art No,Item Type,Kit Line,Location,Ordered,Committed,To Pick
S100001,501,7/10/2026,Parent : Cafe,\"Cafe\n1 Main St\nTOWN VIC 3000\",Paid - thankyou,No snakes,Courier,PO-1,Pending Fulfillment,11,1,STY : GLOVE-M,Gloves,311-2044,InvtPart,,Melbourne Warehouse,10,10,10
S100001,501,7/10/2026,Parent : Cafe,,,,,,,12,2,EAR-1,Earplugs,,InvtPart,,Melbourne Warehouse,4,,0
,,,,,,,,,,,,,,,,,,,,
";

    #[test]
    fn it_reads_the_bridge_s_columns() {
        let rows = read(SENT.as_bytes()).unwrap();
        assert_eq!(rows.len(), 2, "the spacer is no line");
        let r = &rows[0];
        assert_eq!((r.order.as_str(), r.line.as_str(), r.item.as_str()), ("S100001", "11", "GLOVE-M"));
        assert_eq!(r.customer.as_deref(), Some("Parent : Cafe"));
        assert_eq!(r.ship_to.as_deref(), Some("Cafe\n1 Main St\nTOWN VIC 3000"), "an address keeps its lines");
        assert_eq!((r.picking_instructions.as_deref(), r.customer_notes.as_deref()), (Some("Paid - thankyou"), Some("No snakes")));
        assert_eq!((r.art_no.as_deref(), rows[1].art_no.as_deref()), (Some("311-2044"), None));
        assert_eq!(rows[1].committed, None);
        assert_eq!(survey(&rows), OpenOrdersSurvey { lines: 2, orders: 1, to_pick: 1 });
    }

    #[test]
    fn a_missing_column_is_named() {
        let err = read("Order,Line,Item\nS1,1,X\n".as_bytes()).unwrap_err();
        assert!(err.contains("no Location column"), "{err}");
    }

    #[test]
    fn nothing_waiting_is_an_empty_report() {
        let rows = read("Order,Line,Item,Location,Ordered,To Pick\n".as_bytes()).unwrap();
        assert!(rows.is_empty());
    }
}
