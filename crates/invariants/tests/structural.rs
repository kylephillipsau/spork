//! Runs the structural suite against a live database and prints a report.
//!
//!     docker compose up -d
//!     psql "$DATABASE_URL" -f migrations/2026-08-04-000001_reference/up.sql
//!     DATABASE_URL=postgres://postgres:nylonite@localhost:55432/nylonite cargo test -- --nocapture
//!
//! Skips rather than fails when DATABASE_URL is unset, so `cargo test` on a
//! machine with no database still compiles and runs the checks that need none.

use nylonite_invariants::{reason, run, spec, Check, Verdict, ALL};

fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

#[test]
fn every_id_has_a_spec_and_they_are_unique() {
    // The enum makes a duplicate variant a compile error. This catches the other
    // half: an id present in the enum but missing from ALL, which would make it
    // silently unrunnable.
    let mut seen = std::collections::HashSet::new();
    for &id in ALL {
        assert!(seen.insert(id), "{id} appears twice in ALL");
        let inv = spec(id);
        assert_eq!(inv.id, id, "spec({id}) returned metadata for {}", inv.id);
        assert!(!inv.statement.is_empty(), "{id} has no statement");
        assert!(!inv.owners.is_empty(), "{id} names no owning decision");
    }
    assert_eq!(seen.len(), 56, "the register holds 56 structural invariants");
}

#[test]
fn pending_checks_say_why() {
    // A gap with no reason attached is indistinguishable from an oversight.
    for &id in ALL {
        if let Check::Pending(blockers) = spec(id).check {
            assert!(
                !blockers.is_empty(),
                "{id} is pending without naming what it is waiting for"
            );
        }
    }
}

#[test]
fn structural_invariants_hold() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping the checks that need a database");
        return;
    };
    let mut client = nylonite_invariants::connect_exclusive(&url);

    let (mut passed, mut vacuous, mut pending, mut failed) = (0, 0, 0, 0);
    let mut failures: Vec<String> = vec![];

    println!("\n  structural invariants\n");
    for &id in ALL {
        let inv = spec(id);
        let (verdict, outcome) = run(&mut client, id).expect("run check");
        match verdict {
            Verdict::Pass => {
                passed += 1;
                println!("  PASS     {id:<4} examined {:<4} {}", outcome.examined, inv.statement);
            }
            Verdict::Vacuous => {
                vacuous += 1;
                println!(
                    "  VACUOUS  {id:<4} examined 0    {} -- passes, proves nothing",
                    inv.statement
                );
            }
            Verdict::Pending => {
                pending += 1;
                if let Check::Pending(blockers) = inv.check {
                    println!("  PENDING  {id:<4}               {}", reason(blockers));
                }
            }
            Verdict::Fail => {
                failed += 1;
                println!("  FAIL     {id:<4} {}", inv.statement);
                for v in &outcome.violations {
                    println!("           {v}");
                    failures.push(format!("{id}: {v}"));
                }
            }
        }
    }

    println!(
        "\n  {passed} passed, {vacuous} vacuous, {pending} pending, {failed} failed of {}\n",
        ALL.len()
    );
    println!(
        "  A vacuous result is not a pass. It is a check whose population was\n  \
         empty, so it proves the absence of nothing.\n"
    );

    assert!(failures.is_empty(), "structural invariants violated:\n{}", failures.join("\n"));
}
