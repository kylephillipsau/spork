//! Load the item master and the prepack list.
//!
//! ```sh
//! cargo run -p spork-server --example import_prepack -- \
//!     --items ~/Downloads/items.csv \
//!     --prepack ~/Downloads/prepack.csv
//! # then, once the report reads right:
//! cargo run -p spork-server --example import_prepack -- ... --apply
//! ```
//!
//! **Dry run by default.** It prints what it would write and writes nothing.
//! Loading 186 rows under a wrong assumption is worse than not loading them, and
//! the assumption most likely to be wrong is which rows are styles.
//!
//! # What it writes, and what it refuses to invent
//!
//! Items, styles, the `item.style_id` links, case packs where the name stated
//! one, package types for the boxes, and observations for every measurement —
//! entered in the units the sheet uses, canonical beside them, per Principle 5.
//!
//! Three things it will not do:
//!
//! **It does not guess a case pack.** A carton is only a definite object
//! relative to one, so a style with no stated count still needs an
//! `item_packing_config` row; it gets one with every count null. That row says
//! "there is a carton and nobody has recorded what is in it", which is true, and
//! is a thing the schema can hold because the count columns are nullable.
//!
//! **It does not pick a winner between rows that disagree.** `SKU-0890` and
//! `SKU-0240` each appear twice, as two distinct NetSuite records with different
//! weights — and `SKU-0240` twice with the same three numbers in a different
//! order. The file carries no timestamps, so `observation_current` would resolve
//! them arbitrarily. Both are loaded and a `discrepancy` is raised.
//!
//! **It does not call a stated weight a measurement.** Every weight here is
//! `transcribed` off a `csv`. A container's weight is `estimated`: the arithmetic
//! says none of them is an empty box — `Small Box` states 3 kg where the board
//! would weigh 0.4–0.7 kg — so they are somebody's typical packed load, not a
//! tare and not a measurement of any particular carton.
//!
//! # Idempotent by content
//!
//! Every act's `client_event_id` is a v5 uuid over (file, row, metric), so
//! re-running the same file is a replay rather than a second load. A changed row
//! produces a new act, which is what a changed row should do.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use spork_server::observing;
use spork_server::prepack::{decide, Catalogue, Subject};
use tokio_postgres::{Client, NoTls};
use uuid::Uuid;

/// Namespace for the deterministic act identifiers. Fixed forever: changing it
/// would make every previous load look like a different one.
const ACTS: Uuid = Uuid::from_u128(0x6e796c6f_6e69_7465_7072_657061636b21);

struct Args {
    items: PathBuf,
    prepack: PathBuf,
    tenant: Uuid,
    person: Uuid,
    site: Uuid,
    apply: bool,
    url: String,
}

fn args() -> Result<Args, String> {
    let mut items = None;
    let mut prepack = None;
    let mut apply = false;
    let mut tenant = "11111111-1111-1111-1111-111111111111".to_string();
    let mut person = "77770000-0000-0000-0000-000000000001".to_string();
    let mut site = "a5170000-0000-0000-0000-000000000001".to_string();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--items" => items = it.next().map(PathBuf::from),
            "--prepack" => prepack = it.next().map(PathBuf::from),
            "--tenant" => tenant = it.next().unwrap_or_default(),
            "--person" => person = it.next().unwrap_or_default(),
            "--site" => site = it.next().unwrap_or_default(),
            "--apply" => apply = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        items: items.ok_or("--items is required")?,
        prepack: prepack.ok_or("--prepack is required")?,
        tenant: tenant.parse().map_err(|_| "bad --tenant")?,
        person: person.parse().map_err(|_| "bad --person")?,
        site: site.parse().map_err(|_| "bad --site")?,
        apply,
        url: std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set")?,
    })
}

#[derive(Debug, Clone)]
struct Item {
    code: String,
    description: String,
}

#[derive(Debug, Clone)]
struct Row {
    line: usize,
    name: String,
    weight: Option<String>,
    length: Option<String>,
    width: Option<String>,
    height: Option<String>,
    weight_unit: String,
    size_unit: String,
    package_type: String,
}

fn read_items(path: &PathBuf) -> Result<Vec<Item>, String> {
    let mut rdr = csv::Reader::from_path(path).map_err(|e| e.to_string())?;
    let mut out = vec![];
    for rec in rdr.deserialize::<HashMap<String, String>>() {
        let r = rec.map_err(|e| e.to_string())?;
        let code = r.get("Code").map(|s| s.trim()).unwrap_or("").to_string();
        if code.is_empty() {
            continue;
        }
        let description = r
            .get("Description")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or(&code)
            .to_string();
        out.push(Item { code, description });
    }
    Ok(out)
}

fn read_prepack(path: &PathBuf) -> Result<Vec<Row>, String> {
    let mut rdr = csv::Reader::from_path(path).map_err(|e| e.to_string())?;
    let mut out = vec![];
    for (i, rec) in rdr.deserialize::<HashMap<String, String>>().enumerate() {
        let r = rec.map_err(|e| e.to_string())?;
        let get = |k: &str| r.get(k).map(|s| s.trim().to_string()).unwrap_or_default();
        let some = |k: &str| {
            let v = get(k);
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        };
        let name = get("Name");
        if name.is_empty() {
            continue;
        }
        out.push(Row {
            line: i + 2, // header is line 1
            name,
            weight: some("Weight"),
            length: some("Length"),
            width: some("Width"),
            height: some("Height"),
            weight_unit: some("Weight Unit").unwrap_or_else(|| "kg".into()),
            size_unit: some("Size Unit").unwrap_or_else(|| "cm".into()),
            package_type: get("Package Type"),
        });
    }
    Ok(out)
}

/// One measurement, as it will be written.
struct Measure {
    metric: &'static str,
    entered: String,
    unit: String,
}

fn measures(row: &Row) -> Vec<Measure> {
    let mut m = vec![];
    let size = |v: &Option<String>, metric: &'static str| {
        v.as_ref().map(|v| Measure {
            metric,
            entered: v.clone(),
            unit: row.size_unit.clone(),
        })
    };
    m.extend(size(&row.length, "length"));
    m.extend(size(&row.width, "width"));
    m.extend(size(&row.height, "height"));
    if let Some(w) = &row.weight {
        m.push(Measure {
            metric: "gross_weight",
            entered: w.clone(),
            unit: row.weight_unit.clone(),
        });
    }
    m
}

fn act_for(file: &str, line: usize, what: &str) -> Uuid {
    Uuid::new_v5(&ACTS, format!("{file}#{line}#{what}").as_bytes())
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\nusage: --items <csv> --prepack <csv> [--apply]");
            std::process::exit(2);
        }
    };

    let items = read_items(&args.items)?;
    let rows = read_prepack(&args.prepack)?;
    let catalogue = Catalogue::new(items.iter().map(|i| i.code.clone()));
    let by_code: HashMap<&str, &Item> = items.iter().map(|i| (i.code.as_str(), i)).collect();

    println!("\n  {} items, {} prepack rows\n", items.len(), rows.len());

    // ---- classify, and find the rows that disagree with each other ----
    let mut plans: Vec<(Row, spork_server::prepack::Decision)> = vec![];
    for row in rows {
        let d = decide(&row.name, &catalogue);
        plans.push((row, d));
    }

    let mut seen: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, (_, d)) in plans.iter().enumerate() {
        let key = match &d.subject {
            Subject::Item { code } => format!("item:{code}"),
            Subject::Style { code, .. } => format!("style:{code}"),
            Subject::Container { name } => format!("box:{name}"),
            Subject::Unresolved { name } => format!("unknown:{name}"),
        };
        seen.entry(key).or_default().push(i);
    }
    let contested: Vec<(&String, &Vec<usize>)> =
        seen.iter().filter(|(_, v)| v.len() > 1).collect();

    let mut n_item = 0;
    let mut n_style = 0;
    let mut n_box = 0;
    let mut covered = 0usize;
    let mut unresolved: Vec<&str> = vec![];
    for (_, d) in &plans {
        match &d.subject {
            Subject::Item { .. } => n_item += 1,
            Subject::Style { variants, .. } => {
                n_style += 1;
                covered += variants;
            }
            Subject::Container { .. } => n_box += 1,
            Subject::Unresolved { name } => unresolved.push(name),
        }
    }

    println!("  measurements attach to");
    println!("    {n_item:>4} stock codes");
    println!("    {n_style:>4} styles, covering {covered} codes");
    println!("    {n_box:>4} containers -> package_type");
    println!("    {:>4} unresolved, written nowhere", unresolved.len());
    println!(
        "    {:>4} rows state a case pack",
        plans.iter().filter(|(_, d)| d.per_carton.is_some()).count()
    );

    if !unresolved.is_empty() {
        println!(
            "\n  {} rows name something shaped like a stock code that the item\n               export does not contain, in any form. Nothing is written for these —\n               the reading is that the export is incomplete, not that the warehouse\n               packs into a box called {}:",
            unresolved.len(),
            unresolved[0]
        );
        for chunk in unresolved.chunks(8) {
            println!("      {}", chunk.join("  "));
        }
    }

    if !contested.is_empty() {
        println!("\n  two rows describe the same subject; both load, neither wins:");
        for (key, idx) in &contested {
            println!("    {key}");
            for i in *idx {
                let r = &plans[*i].0;
                println!(
                    "      line {:<4} {:>7} {} x {} x {} {}",
                    r.line,
                    r.weight.clone().unwrap_or_else(|| "-".into()),
                    r.length.clone().unwrap_or_else(|| "-".into()),
                    r.width.clone().unwrap_or_else(|| "-".into()),
                    r.height.clone().unwrap_or_else(|| "-".into()),
                    r.size_unit
                );
            }
        }
    }

    // ---- connect ----
    let (client, connection) = tokio_postgres::connect(&args.url, NoTls)
        .await
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
        .batch_execute("SET ROLE spork_app")
        .await
        .map_err(|e| format!("SET ROLE: {e}"))?;
    client
        .execute(
            "SELECT set_config('spork.tenant_id', $1::text, false)",
            &[&args.tenant.to_string()],
        )
        .await
        .map_err(|e| e.to_string())?;

    let units = unit_factors(&client).await?;
    for m in plans.iter().flat_map(|(r, _)| measures(r)) {
        if !units.contains_key(&m.unit) {
            return Err(format!(
                "the sheet quotes {:?}, which is not a unit in this database",
                m.unit
            ));
        }
    }

    if !args.apply {
        println!(
            "\n  Dry run. Nothing was written. Add --apply once the above reads right.\n"
        );
        return Ok(());
    }

    // ---- write ----
    let mut client = client;
    let tx = client.transaction().await.map_err(|e| e.to_string())?;

    let mut wrote_items = 0;
    for i in &items {
        let n = tx
            .execute(
                "INSERT INTO item (tenant_id, code, description, base_unit_id, tracking)
                 SELECT $1, $2, $3, u.id, 'none' FROM unit u WHERE u.code = 'ea'
                 ON CONFLICT (tenant_id, code) DO NOTHING",
                &[&args.tenant, &i.code, &i.description],
            )
            .await
            .map_err(|e| format!("item {}: {e}", i.code))?;
        wrote_items += n;
    }

    let mut style_ids: HashMap<String, Uuid> = HashMap::new();
    for (_, d) in &plans {
        if let Subject::Style { code, .. } = &d.subject {
            let description = by_code
                .keys()
                .find(|c| c.starts_with(code.as_str()))
                .and_then(|c| by_code.get(c))
                .map(|i| i.description.clone());
            tx.execute(
                "INSERT INTO item_style (tenant_id, code, description) VALUES ($1, $2, $3)
                 ON CONFLICT (tenant_id, code) DO NOTHING",
                &[&args.tenant, code, &description],
            )
            .await
            .map_err(|e| format!("style {code}: {e}"))?;
            let id: Uuid = tx
                .query_one(
                    "SELECT id FROM item_style WHERE tenant_id = $1 AND code = $2",
                    &[&args.tenant, code],
                )
                .await
                .map_err(|e| e.to_string())?
                .get(0);
            style_ids.insert(code.clone(), id);
        }
    }

    // **Every style in the database, not only the ones this run created.** The
    // first version linked only its own, so a style already on file — the
    // fixture's `STY-7720` — watched thirteen of its own variants arrive and
    // stayed empty. A style is a fact about the catalogue, not about which file
    // happened to mention it.
    //
    // The prefix rule is `prepack::style_of` rather than a SQL `LIKE`, so there
    // is one definition of what a variant is and it is the one with tests.
    for r in tx
        .query("SELECT code, id FROM item_style WHERE tenant_id = $1", &[&args.tenant])
        .await
        .map_err(|e| e.to_string())?
    {
        style_ids.insert(r.get(0), r.get(1));
    }

    let mut codes: Vec<String> = vec![];
    let mut styles: Vec<Uuid> = vec![];
    for i in &items {
        if let Some(style) = spork_server::prepack::style_of(&i.code) {
            if let Some(id) = style_ids.get(&style) {
                codes.push(i.code.clone());
                styles.push(*id);
            }
        }
    }
    let linked = tx
        .execute(
            "UPDATE item i SET style_id = s.style
               FROM unnest($2::text[], $3::uuid[]) AS s(code, style)
              WHERE i.tenant_id = $1 AND i.code = s.code AND i.style_id IS NULL",
            &[&args.tenant, &codes, &styles],
        )
        .await
        .map_err(|e| format!("linking styles: {e}"))?;

    // A carton needs a case pack to be a definite object, so every measured
    // subject gets one — with the count when the name said, and every count null
    // when it did not. A row of nulls is "there is a carton and nobody has
    // recorded what is in it", which is exactly the state of these thirteen.
    let mut configs = 0;
    for (_, d) in &plans {
        let (target, per) = match (&d.subject, d.per_carton) {
            (Subject::Item { code }, p) => (format!("code = '{}'", esc(code)), p),
            (Subject::Style { code, .. }, p) => {
                (format!("style_id = '{}'", style_ids[code]), p)
            }
            (Subject::Container { .. } | Subject::Unresolved { .. }, _) => continue,
        };
        let sql = format!(
            "INSERT INTO item_packing_config
                 (tenant_id, item_id, units_per_inner, inners_per_carton, effective_from)
             SELECT $1, i.id, CASE WHEN $2::int IS NULL THEN NULL ELSE 1 END, $2::int,
                    CURRENT_DATE
               FROM item i
              WHERE i.tenant_id = $1 AND i.{target}
                AND NOT EXISTS (SELECT 1 FROM item_packing_config c
                                 WHERE c.item_id = i.id)"
        );
        configs += tx
            .execute(sql.as_str(), &[&args.tenant, &per.map(|p| p as i32)])
            .await
            .map_err(|e| format!("packing config: {e}"))?;
    }

    let mut boxes = 0;
    for (row, d) in &plans {
        let Subject::Container { name } = &d.subject else {
            continue;
        };
        let mm = |v: &Option<String>| -> Result<Option<i32>, String> {
            let Some(v) = v else { return Ok(None) };
            let e = observing::parse_entered(v).map_err(|e| e.to_string())?;
            let f = units[&row.size_unit];
            Ok(Some(
                observing::to_canonical(e, f).map_err(|e| e.to_string())? as i32,
            ))
        };
        let (l, w, h) = (mm(&row.length)?, mm(&row.width)?, mm(&row.height)?);
        boxes += tx
            .execute(
                "INSERT INTO package_type
                     (tenant_id, name, carrier_package_code, dimensions_fixed,
                      length_mm, width_mm, height_mm)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (tenant_id, name) DO NOTHING",
                &[
                    &args.tenant,
                    name,
                    &Some(row.package_type.clone()).filter(|s| !s.is_empty()),
                    &(l.is_some() && w.is_some() && h.is_some()),
                    &l,
                    &w,
                    &h,
                ],
            )
            .await
            .map_err(|e| format!("package_type {name}: {e}"))?;
    }

    // ---- the measurements ----
    let file = args
        .prepack
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut observations = 0;
    let mut acts = 0;
    for (row, d) in &plans {
        let ms = measures(row);
        if ms.is_empty() {
            continue;
        }
        // A container's dimensions are columns on `package_type`, already written.
        let (column, id) = match &d.subject {
            Subject::Item { code } => (
                "item_id",
                tx.query_opt(
                    "SELECT id FROM item WHERE tenant_id = $1 AND code = $2",
                    &[&args.tenant, code],
                )
                .await
                .map_err(|e| e.to_string())?
                .map(|r| r.get::<_, Uuid>(0)),
            ),
            Subject::Style { code, .. } => ("item_style_id", style_ids.get(code).copied()),
            Subject::Container { .. } | Subject::Unresolved { .. } => continue,
        };
        let Some(subject_id) = id else { continue };

        let config: Option<Uuid> = tx
            .query_opt(
                &format!(
                    "SELECT c.id FROM item_packing_config c JOIN item i ON i.id = c.item_id
                      WHERE i.tenant_id = $1 AND i.{}
                      ORDER BY c.effective_from DESC, c.id DESC LIMIT 1",
                    match &d.subject {
                        Subject::Item { code } => format!("code = '{}'", esc(code)),
                        Subject::Style { code, .. } =>
                            format!("style_id = '{}'", style_ids[code]),
                        Subject::Container { .. } | Subject::Unresolved { .. } =>
                            unreachable!("skipped above"),
                    }
                ),
                &[&args.tenant],
            )
            .await
            .map_err(|e| e.to_string())?
            .map(|r| r.get(0));

        let observable: Uuid = {
            tx.execute(
                &format!(
                    "INSERT INTO observable
                         (tenant_id, {column}, packaging_level, item_packing_config_id)
                     VALUES ($1, $2, 'carton', $3)
                     ON CONFLICT (tenant_id, {column}, packaging_level,
                                  item_packing_config_id)
                          WHERE {column} IS NOT NULL DO NOTHING"
                ),
                &[&args.tenant, &subject_id, &config],
            )
            .await
            .map_err(|e| format!("observable for {}: {e}", row.name))?;
            tx.query_one(
                &format!(
                    "SELECT id FROM observable WHERE tenant_id = $1 AND {column} = $2
                      AND packaging_level = 'carton'
                      AND item_packing_config_id IS NOT DISTINCT FROM $3"
                ),
                &[&args.tenant, &subject_id, &config],
            )
            .await
            .map_err(|e| e.to_string())?
            .get(0)
        };

        let act = act_for(&file, row.line, "measure");
        let already = tx
            .execute(
                "INSERT INTO client_event
                     (tenant_id, client_event_id, site_id, recorded_by_id,
                      submitted_at, received_at)
                 VALUES ($1, $2, $3, $4, now(), now())
                 ON CONFLICT DO NOTHING",
                &[&args.tenant, &act, &args.site, &args.person],
            )
            .await
            .map_err(|e| e.to_string())?;
        if already == 0 {
            continue; // this row has been loaded before, from this same file
        }
        acts += 1;

        let event: Uuid = tx
            .query_one(
                "INSERT INTO observation_event
                     (tenant_id, client_event_id, observable_id, observed_at,
                      recorded_by_id, method, ingestion_channel)
                 VALUES ($1, $2, $3, now(), $4, 'transcribed', 'csv') RETURNING id",
                &[&args.tenant, &act, &observable, &args.person],
            )
            .await
            .map_err(|e| format!("observation_event: {e}"))?
            .get(0);

        for m in ms {
            let entered = observing::parse_entered(&m.entered).map_err(|e| e.to_string())?;
            let canonical = observing::to_canonical(entered, units[&m.unit])
                .map_err(|e| e.to_string())?;
            observations += tx
                .execute(
                    "INSERT INTO observation
                         (tenant_id, observation_event_id, observable_id, observed_at,
                          client_event_id, metric_id, result_kind, dimension_id,
                          value_numeric, entered_value, entered_unit_id)
                     SELECT $1, $2, $3, now(), $4, m.id, 'quantity', m.dimension_id,
                            $5, $6::text::numeric, u.id
                       FROM metric m, unit u
                      WHERE m.code = $7 AND m.tenant_id IS NULL AND u.code = $8",
                    &[
                        &args.tenant,
                        &event,
                        &observable,
                        &act,
                        &canonical,
                        &m.entered,
                        &m.metric,
                        &m.unit,
                    ],
                )
                .await
                .map_err(|e| format!("observation {} {}: {e}", row.name, m.metric))?;
        }
    }

    // ---- the disagreements ----
    //
    // **Both rows are loaded above and neither is preferred here.** The file has
    // no timestamps, so `observation_current` would resolve them by whichever
    // insert happened to land last, which is not a resolution. `identity_mismatch`
    // is the kind: two records claim to describe the same thing and say different
    // things about it.
    //
    // `detected_by_id` is null and `automation_key` is set, because
    // `discrepancy_actor_ck` insists exactly one of them is — an import is a
    // machine noticing, not a person. The key is derived from the file and the
    // subject, so re-running raises nothing new.
    let mut findings = 0;
    for (key, idx) in &contested {
        let (item_id, what) = match &plans[idx[0]].1.subject {
            Subject::Item { code } => (
                tx.query_opt(
                    "SELECT id FROM item WHERE tenant_id = $1 AND code = $2",
                    &[&args.tenant, code],
                )
                .await
                .map_err(|e| e.to_string())?
                .map(|r| r.get::<_, Uuid>(0)),
                format!("item {code}"),
            ),
            // **A finding about a style has nowhere to point.** `discrepancy`
            // predates D108 and carries no `item_style_id`, so the style is named
            // in the detail rather than misfiled against one arbitrary variant.
            // That is a gap, and it is recorded in the report rather than papered
            // over by picking a size.
            Subject::Style { code, .. } => (None, format!("style {code}")),
            _ => continue,
        };
        let lines: Vec<String> = idx
            .iter()
            .map(|i| {
                let r = &plans[*i].0;
                format!(
                    "line {} says {} {} at {}x{}x{} {}",
                    r.line,
                    r.weight.clone().unwrap_or_else(|| "?".into()),
                    r.weight_unit,
                    r.length.clone().unwrap_or_else(|| "?".into()),
                    r.width.clone().unwrap_or_else(|| "?".into()),
                    r.height.clone().unwrap_or_else(|| "?".into()),
                    r.size_unit
                )
            })
            .collect();
        let detail = format!(
            "The prepack list describes {what} twice and disagrees with itself: {}. \
             Both are recorded; neither is preferred, because the file carries no \
             times to resolve them by.",
            lines.join("; ")
        );
        findings += tx
            .execute(
                "INSERT INTO discrepancy
                     (tenant_id, kind, item_id, detail, detected_at, automation_key, state)
                 SELECT $1, 'identity_mismatch', $2, $3, now(), $4, 'open'
                  WHERE NOT EXISTS (SELECT 1 FROM discrepancy d
                                     WHERE d.tenant_id = $1 AND d.automation_key = $4)",
                &[
                    &args.tenant,
                    &item_id,
                    &detail,
                    &format!("prepack:{file}:{key}"),
                ],
            )
            .await
            .map_err(|e| format!("finding for {key}: {e}"))?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    println!("\n  Applied.");
    println!("    {wrote_items:>5} items created");
    println!("    {:>5} styles", style_ids.len());
    println!("    {linked:>5} items linked to a style");
    println!("    {configs:>5} packing configs");
    println!("    {boxes:>5} package types");
    println!("    {acts:>5} acts, {observations} observations");
    println!("    {findings:>5} findings raised for rows that disagree");
    println!(
        "\n  Re-run without --apply to confirm it is now a no-op; the acts are \
         derived from the file, so a second load of the same file writes nothing.\n"
    );
    Ok(())
}

/// Single quotes are the only thing that could break out of the identifiers
/// interpolated above, and all of them come from the item master rather than
/// from a request. Doubling them keeps that true regardless.
fn esc(s: &str) -> String {
    s.replace('\'', "''")
}

async fn unit_factors(client: &Client) -> Result<HashMap<String, observing::Factor>, String> {
    let rows = client
        .query("SELECT code, factor_num, factor_den FROM unit", &[])
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .map(|r| {
            (
                r.get::<_, String>(0),
                observing::Factor {
                    num: r.get(1),
                    den: r.get(2),
                },
            )
        })
        .collect())
}
