//! Load the bin list: sites and the shelves inside them.
//!
//! ```sh
//! cargo run -p spork-server --example import_bins -- --bins bins.csv
//! cargo run -p spork-server --example import_bins -- --bins bins.csv --apply
//! ```
//!
//! Dry run by default, like [the prepack importer](import_prepack), and for the
//! same reason.
//!
//! # This is a front end, and the importer is in the crate
//!
//! The reading, the survey and the writing all live in
//! [`spork_server::importing`], because `POST /api/import/bins` loads the
//! same file and D158 would otherwise have shipped a second implementation of
//! this one. What is left here is argument parsing, a connection, and printing
//! — the parts a terminal needs and an HTTP request does not.
//!
//! # What it does
//!
//! A warehouse becomes a `site`, matched to one already on file by name before
//! a new one is made — `Melbourne Warehouse` is the `MEL` the fixture already
//! has, not a second Melbourne. A bin becomes a `location`, with its code broken
//! into `aisle`, `bay` and `level`, columns migration 1 created and nothing has
//! filled since.
//!
//! # What it refuses
//!
//! **A bin with no stated type.** 421 of them, 420 being the whole of Perth.
//! `location.kind` is NOT NULL and its vocabulary has no value meaning "nobody
//! said", so guessing would turn an absent fact into a stated one. Pass
//! `--assume-kind bulk` to say what they are; without it they are reported and
//! left out.
//!
//! **A warehouse that is probably not yours.** `Partner Warehouse` holds eleven
//! bins, one of them called `3PL`. Creating a site asserts this business has a
//! building there. `--include-external` overrides.
//!
//! **A site whose clock nobody knows.** `site.timezone` is NOT NULL, and
//! defaulting a new warehouse to Melbourne's clock is wrong in Brisbane twice a
//! year. An unknown site is reported and skipped.
//!
//! # The walking order
//!
//! `WMS Picking Order` becomes `location.pick_sequence`, with **0 read as
//! absent**: the export uses it 150 times to mean unsequenced — including `3PL`
//! and `ASSEMBLY-BIN`, which are not stops on a route — and loaded literally
//! every one of them sorts ahead of the first real bin.
//!
//! The file carries a second ordering, `WMS Bin Sequence`, and the two disagree
//! on 4,759 of the 7,882 bins that have both. Only one can be the order somebody
//! walks, so only one is stored and the disagreement is reported rather than
//! averaged or silently preferred.
//!
//! # Idempotent
//!
//! Sites match on code and bins on (site, code), the second correcting what it
//! finds, so a second run writes only what changed. Neither is a fact table: a
//! bin list is reference data, and re-importing it is a correction rather than
//! an event.

use std::path::PathBuf;

use spork_server::{bins, importing};
use tokio_postgres::NoTls;
use uuid::Uuid;

struct Args {
    bins: PathBuf,
    tenant: Uuid,
    recorded_by: Option<Uuid>,
    apply: bool,
    assume_kind: Option<String>,
    include_external: bool,
    url: String,
}

fn args() -> Result<Args, String> {
    let mut bins_path = None;
    let mut apply = false;
    let mut assume_kind = None;
    let mut include_external = false;
    let mut tenant = "11111111-1111-1111-1111-111111111111".to_string();
    let mut recorded_by: Option<String> = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--bins" => bins_path = it.next().map(PathBuf::from),
            "--tenant" => tenant = it.next().unwrap_or_default(),
            "--recorded-by" => recorded_by = it.next(),
            "--assume-kind" => assume_kind = it.next(),
            "--include-external" => include_external = true,
            "--apply" => apply = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if let Some(k) = &assume_kind {
        if bins::kind_of(k).is_none() && !matches!(k.as_str(),
            "pick_face" | "bulk" | "staging" | "dock" | "overflow") {
            return Err(format!(
                "--assume-kind {k} is not one of pick_face, bulk, staging, dock, overflow"
            ));
        }
    }
    // **`--recorded-by` is required to apply, and only to apply.** A dry run
    // keeps nothing, so it owes nobody a name; an apply files the export as a
    // `party_message`, and `client_event_actor_ck` insists every act names a
    // person or an automation. This importer is a person at a terminal, so it
    // is the person arm, and defaulting it to somebody would be inventing the
    // answer to D11's question rather than asking it.
    let recorded_by = match recorded_by {
        Some(p) => Some(p.parse::<Uuid>().map_err(|_| "bad --recorded-by")?),
        None if apply => {
            return Err(
                "--recorded-by <person uuid> is required with --apply: the arrival is \
                 recorded as an act and an act names who performed it"
                    .into(),
            )
        }
        None => None,
    };
    Ok(Args {
        bins: bins_path.ok_or("--bins is required")?,
        tenant: tenant.parse().map_err(|_| "bad --tenant")?,
        recorded_by,
        apply,
        assume_kind,
        include_external,
        url: std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set")?,
    })
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\nusage: --bins <csv> [--apply --recorded-by <person>] \
                       [--assume-kind <kind>] [--include-external]");
            std::process::exit(2);
        }
    };

    // **The bytes are read once and both parsed and stored**, so the export
    // filed as `party_message.payload` cannot be a different file from the one
    // the loader read.
    let payload = std::fs::read(&args.bins)
        .map_err(|e| format!("{}: {e}", args.bins.display()))?;
    let rows = importing::bins::read(payload.as_slice())?;
    let survey = importing::bins::survey(&rows, args.include_external);

    println!("\n  {} bins\n", survey.bins);
    println!("  {:<22}{:>7}{:>8}{:>10}  clock", "warehouse", "bins", "typed", "untyped");
    for s in &survey.sites {
        println!(
            "  {:<22}{:>7}{:>8}{:>10}  {}",
            s.warehouse, s.bins, s.typed, s.untyped, s.note
        );
    }

    println!("\n  walking order");
    println!("    {:>5} bins carry a position", survey.sequenced);
    println!(
        "    {:>5} say 0, which the export means as unsequenced, stored as absent",
        survey.zeroed
    );
    println!(
        "    {:>5} have a second, different `WMS Bin Sequence`; only the picking\n             \
         order is stored, because only one of them can be the route somebody walks",
        survey.disagree
    );

    if survey.untyped > 0 {
        match &args.assume_kind {
            Some(k) => println!(
                "\n  {} bins state no type; --assume-kind {k} will bring them in as {k}.",
                survey.untyped
            ),
            None => println!(
                "\n  {} bins state no type and are left out. `location.kind` is NOT NULL\n  \
                 and has no value meaning \"nobody said\", so this will not choose one for you.\n  \
                 Pass --assume-kind bulk to say what they are.",
                survey.untyped
            ),
        }
    }

    // **The tenant is set on the connection and the role is assumed**, so the
    // importer writes under row-level security rather than around it. That is
    // the same footing `POST /api/import/bins` runs on, which is what makes the
    // shared [`importing::bins::load`] a true shared definition rather than one that
    // happens to compile in two places.
    let (client, connection) = tokio_postgres::connect(&args.url, NoTls)
        .await
        .map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client.batch_execute("SET ROLE spork_app").await.map_err(|e| e.to_string())?;
    client
        .execute(
            "SELECT set_config('spork.tenant_id', $1::text, false)",
            &[&args.tenant.to_string()],
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut client = client;
    let tx = client.transaction().await.map_err(|e| e.to_string())?;
    let arrival = match args.recorded_by {
        Some(person) => Some(
            importing::received::record(
                &tx,
                args.tenant,
                importing::received::Actor::Person(person),
                args.bins.file_name().and_then(|n| n.to_str()),
                &payload,
            )
            .await?,
        ),
        None => None,
    };
    let loaded = importing::bins::load(
        &tx,
        args.tenant,
        &rows,
        &survey,
        args.assume_kind.as_deref(),
        args.apply,
    )
    .await?;
    if let Some(a) = &arrival {
        importing::received::parsed(&tx, a.party_message_id).await?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;

    // **The dry run reports the same numbers**, because it did the same writes
    // and rolled them back. There is no second code path to be wrong.
    println!("\n  {}", if loaded.applied { "Applied." } else { "Dry run — nothing was kept." });
    println!(
        "    {:>5} sites created, {} matched to one already on file",
        loaded.sites_created, loaded.sites_matched
    );
    println!("    {:>5} bins created", loaded.bins_created);
    println!("    {:>5} bins corrected", loaded.bins_corrected);
    println!("    {:>5} bins left out", loaded.bins_left_out);
    if let Some(a) = &arrival {
        println!(
            "\n  Filed as party_message {}{}",
            a.party_message_id,
            if a.is_replay() { " — these exact bytes had already arrived" } else { "" }
        );
    }
    if !loaded.applied {
        println!("\n  Add --apply once the above reads right.");
    }
    println!(
        "\n  `WMS Picking Order` is stored as `location.pick_sequence`, which is the\n  \
         order a picker walks the aisles in — and the order a capture worklist\n  \
         should be read in, because measuring a warehouse is a walk too.\n"
    );
    Ok(())
}
