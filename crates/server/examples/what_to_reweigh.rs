//! What should go on the scale, and why.
//!
//! ```sh
//! cargo run -p spork-server --example what_to_reweigh
//! ```
//!
//! **Reads only.** No migration, no new table, nothing written. Age and trust
//! are both functions of `observation_current.observed_at` and `.method`, so a
//! stored "needs confirming" flag would be a third column able to disagree with
//! the two behind it.
//!
//! The point of running this before building the loop is to find out whether the
//! intervals are the interesting part. If every value is overdue because almost
//! all of them came off a spreadsheet, then the answer is not a better schedule
//! — it is a week with a scale.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use spork_server::revalidation::{ever_measured, priority, staleness, trust_of, Trust};
use tokio_postgres::NoTls;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), String> {
    let url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set")?;
    let tenant: Uuid = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "11111111-1111-1111-1111-111111111111".into())
        .parse()
        .map_err(|_| "bad tenant uuid")?;

    let (client, connection) = tokio_postgres::connect(&url, NoTls)
        .await
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client.batch_execute("SET ROLE spork_app").await.map_err(|e| e.to_string())?;
    client
        .execute(
            "SELECT set_config('spork.tenant_id', $1::text, false)",
            &[&tenant.to_string()],
        )
        .await
        .map_err(|e| e.to_string())?;

    // Every current weight, whose it is, and how it was come by. The style arm
    // is included because D108 makes a style a subject in its own right, and a
    // style's carton weight goes stale exactly as an item's does.
    let rows = client
        .query(
            "SELECT coalesce(i.code, s.code) AS code,
                    i.code IS NULL AS is_style,
                    o.packaging_level::text,
                    oc.method,
                    oc.observed_at,
                    (SELECT count(*) FROM order_line ol
                      WHERE ol.item_id = i.id) AS demand
               FROM observation_current oc
               JOIN observable o ON o.id = oc.observable_id
               JOIN metric m ON m.id = oc.metric_id
               LEFT JOIN item i ON i.id = o.item_id
               LEFT JOIN item_style s ON s.id = o.item_style_id
              WHERE oc.tenant_id = $1
                AND m.code = 'gross_weight'
                AND (o.item_id IS NOT NULL OR o.item_style_id IS NOT NULL)",
            &[&tenant],
        )
        .await
        .map_err(|e| e.to_string())?;

    let now = Utc::now();
    struct Row {
        code: String,
        is_style: bool,
        level: String,
        method: String,
        observed_at: DateTime<Utc>,
        demand: i64,
        staleness: f64,
        priority: f64,
    }

    let all: Vec<Row> = rows
        .iter()
        .map(|r| {
            let method: String = r.get::<_, Option<String>>(3).unwrap_or_default();
            let observed_at: DateTime<Utc> = r.get(4);
            let demand: i64 = r.get(5);
            let s = staleness(observed_at, &method, now);
            Row {
                code: r.get(0),
                is_style: r.get(1),
                level: r.get(2),
                method,
                observed_at,
                demand,
                staleness: s,
                priority: priority(s, demand),
            }
        })
        .collect();

    println!("\n  {} current weights on file\n", all.len());

    // ---- where the numbers came from ----
    let mut by_method: HashMap<&str, (usize, usize)> = HashMap::new();
    for r in &all {
        let e = by_method.entry(r.method.as_str()).or_insert((0, 0));
        e.0 += 1;
        if r.staleness >= 1.0 {
            e.1 += 1;
        }
    }
    println!("  {:<14}{:>8}{:>10}{:>12}", "method", "held", "overdue", "trust");
    let mut methods: Vec<_> = by_method.into_iter().collect();
    methods.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
    for (m, (held, overdue)) in methods {
        println!(
            "  {:<14}{held:>8}{overdue:>10}{:>12}",
            if m.is_empty() { "(none)" } else { m },
            match trust_of(m) {
                Trust::Measured => "measured",
                Trust::Stated => "stated",
            }
        );
    }

    let overdue = all.iter().filter(|r| r.staleness >= 1.0).count();
    println!(
        "\n  {overdue} of {} are past their interval ({:.0}%)",
        all.len(),
        100.0 * overdue as f64 / all.len().max(1) as f64
    );

    let measured = all.iter().filter(|r| trust_of(&r.method) == Trust::Measured).count();
    println!(
        "  {measured} of {} were ever put on a scale",
        all.len()
    );

    // ---- two lists, because they are two different jobs ----
    //
    // **The clock cannot start on a value nobody measured.** An imported figure
    // carries the import date, not the day it was established, and neither the
    // spreadsheet nor NetSuite records the latter. So "never confirmed" is not a
    // stale measurement; it is the absence of one, and it wants a first weighing
    // rather than a re-weighing.
    let (mut never, mut stale): (Vec<&Row>, Vec<&Row>) =
        all.iter().partition(|r| !ever_measured(&r.method));
    never.sort_by(|a, b| b.demand.cmp(&a.demand).then(a.code.cmp(&b.code)));
    stale.retain(|r| r.staleness >= 1.0);
    stale.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));

    println!("\n  never confirmed by an instrument — a first weighing\n");
    if never.is_empty() {
        println!("  Nothing. Every weight on file was measured.");
    } else {
        println!("  {:<18}{:<9}{:<8}{:<14}{:>7}", "code", "level", "kind", "came from", "orders");
        for r in never.iter().take(10) {
            println!(
                "  {:<18}{:<9}{:<8}{:<14}{:>7}",
                r.code,
                r.level,
                if r.is_style { "style" } else { "item" },
                r.method,
                r.demand
            );
        }
        if never.len() > 10 {
            println!("  … and {} more", never.len() - 10);
        }
    }

    println!("\n  measured once and now overdue — a re-weighing\n");
    if stale.is_empty() {
        println!("  Nothing. Every measured weight is inside its interval.");
    } else {
        println!("  {:<18}{:<9}{:<14}{:>7}{:>9}", "code", "level", "measured", "orders", "overdue");
        for r in stale.iter().take(10) {
            println!(
                "  {:<18}{:<9}{:<14}{:>7}{:>8.1}×",
                r.code,
                r.level,
                r.observed_at.format("%Y-%m-%d").to_string(),
                r.demand,
                r.staleness
            );
        }
    }

    println!(
        "\n  Oldest reading: {}\n",
        all.iter()
            .map(|r| r.observed_at)
            .min()
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "none".into())
    );
    Ok(())
}
