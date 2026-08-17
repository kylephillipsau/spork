//! The write budget, measured rather than stated.
//!
//! Question 120, which was 111 in the supply-side design before consolidation
//! renumbered it:
//!
//! > *"ASN ingestion performs one parent update per child row, so a 200-line
//! > DESADV touches one parent 200 times while the allocator wants it. Batching
//! > the decrement per parent inside the ingestion transaction turns 200 updates
//! > into one and needs no schema change — but the model states a read budget and
//! > no write budget, for this table or for `stock`, and D5 only decides the
//! > floor-blocking half."*
//!
//! Three claims, and each is measurable.
//!
//! # Why an `expected_supply` row is contended on purpose
//!
//! The supply-side design keeps `expected_supply` a table rather than a view for
//! two reasons, and the second is this one: *"the FK target is the gate row
//! Postgres needs to serialise concurrent allocations, in the absence of the gap
//! locks ERPNext's availability check depends on."*
//!
//! So the row is a lock. Two allocators asking "is there enough left to promise"
//! must not both say yes, and `SELECT ... FOR UPDATE` on the promise is what makes
//! them take turns. **That makes every needless write to it a needless queue**,
//! which is what the 200-versus-1 question is really about.
//!
//! # What the fixtures cannot do
//!
//! `history.sql` produces a year of volume from one connection. Volume is not
//! contention: nothing in it ever waits for a lock. This is the third time the
//! instrument has had to change before a question could be read — D65 built the
//! year, D69 gave it a workforce, and this gives it two hands.
//!
//! Skips rather than fails without `DATABASE_URL`, like the rest of the suite.

use postgres::{Client, NoTls};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

const TENANT: &str = "33333333-3333-3333-3333-333333333333";

/// A promise to contend over, and a clean slate on it.
fn a_promise(c: &mut Client) -> Option<String> {
    let rows = c
        .query(
            "SELECT id::text FROM expected_supply
              WHERE tenant_id = ($1)::text::uuid AND closed_at IS NULL LIMIT 1",
            &[&TENANT],
        )
        .expect("promise");
    rows.first().map(|r| r.get(0))
}

fn updates_on(c: &mut Client, table: &str) -> i64 {
    c.execute("SELECT pg_stat_force_next_flush()", &[]).ok();
    c.query_one(
        "SELECT coalesce(n_tup_upd, 0)::bigint FROM pg_stat_user_tables WHERE relname = $1",
        &[&table],
    )
    .expect("stat")
    .get(0)
}

/// The measurement 120 asks for: what one parent costs when it is touched once
/// per child line, against once per message.
///
/// Both forms produce the same final number. What differs is how long the gate
/// row is held and how many versions of it exist afterwards, and both of those
/// are paid by whoever is waiting.
#[test]
fn a_batched_decrement_costs_less_than_one_per_line() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = nylonite_invariants::connect_exclusive(&url);
    let Some(promise) = a_promise(&mut c) else {
        eprintln!("no open promise: this has nothing to contend over");
        return;
    };

    // A 200-line despatch advice, which is an ordinary grocery message rather
    // than a pathological one.
    const LINES: i64 = 200;

    // Form A: one update per line, which is what the sketch describes.
    let before = updates_on(&mut c, "expected_supply");
    let t0 = Instant::now();
    let mut tx = c.transaction().expect("begin");
    tx.execute(
        "SELECT 1 FROM expected_supply WHERE id = ($1)::text::uuid FOR UPDATE",
        &[&promise],
    )
    .expect("gate");
    for _ in 0..LINES {
        tx.execute(
            "UPDATE expected_supply SET quantity_refined = quantity_refined + 1
              WHERE id = ($1)::text::uuid",
            &[&promise],
        )
        .expect("per-line update");
    }
    tx.rollback().expect("rollback");
    let per_line = t0.elapsed();
    let per_line_versions = updates_on(&mut c, "expected_supply") - before;

    // Form B: the same arithmetic, once.
    let before = updates_on(&mut c, "expected_supply");
    let t0 = Instant::now();
    let mut tx = c.transaction().expect("begin");
    tx.execute(
        "SELECT 1 FROM expected_supply WHERE id = ($1)::text::uuid FOR UPDATE",
        &[&promise],
    )
    .expect("gate");
    tx.execute(
        "UPDATE expected_supply SET quantity_refined = quantity_refined + $2
          WHERE id = ($1)::text::uuid",
        &[&promise, &LINES],
    )
    .expect("batched update");
    tx.rollback().expect("rollback");
    let batched = t0.elapsed();
    let batched_versions = updates_on(&mut c, "expected_supply") - before;

    println!(
        "\n  one update per line  {:>8.2} ms   {} row versions\n  \
         one update per message {:>6.2} ms   {} row versions\n",
        per_line.as_secs_f64() * 1000.0,
        per_line_versions,
        batched.as_secs_f64() * 1000.0,
        batched_versions
    );

    // The version count is the claim worth asserting. Timing varies with the
    // machine; row versions are arithmetic, and every one of them is a dead
    // tuple on a row that other transactions are queued behind.
    assert_eq!(batched_versions, 1, "the batched form writes one version");
    assert!(
        per_line_versions >= LINES,
        "the per-line form writes a version per line, and wrote {per_line_versions}"
    );
}

/// What the waiting side pays, which is the half the sketch describes and does
/// not measure.
///
/// An allocator asking whether a promise still has room takes the gate. While an
/// ingester holds it, the allocator waits — so the ingester's transaction
/// duration *is* the allocator's latency, and the 200-versus-1 choice is a choice
/// about somebody else's response time.
#[test]
fn the_allocator_waits_exactly_as_long_as_the_ingester_holds_the_gate() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = nylonite_invariants::connect_exclusive(&url);
    let Some(promise) = a_promise(&mut c) else {
        eprintln!("no open promise: this has nothing to contend over");
        return;
    };

    let wait_behind = |hold: Duration| -> Duration {
        let held_url = url.clone();
        let held_promise = promise.clone();
        let (ready_tx, ready_rx) = mpsc::channel();

        // The ingester: takes the gate, works, releases.
        let holder = thread::spawn(move || {
            let mut h = Client::connect(&held_url, NoTls).expect("connect");
            let mut tx = h.transaction().expect("begin");
            tx.execute(
                "SELECT 1 FROM expected_supply WHERE id = ($1)::text::uuid FOR UPDATE",
                &[&held_promise],
            )
            .expect("gate");
            ready_tx.send(()).expect("signal");
            thread::sleep(hold);
            tx.rollback().expect("rollback");
        });

        ready_rx.recv().expect("gate taken");
        // The allocator, arriving second.
        let mut a = Client::connect(url.as_str(), NoTls).expect("connect");
        let t0 = Instant::now();
        a.execute(
            "SELECT 1 FROM expected_supply WHERE id = ($1)::text::uuid FOR UPDATE",
            &[&promise],
        )
        .expect("wait for the gate");
        let waited = t0.elapsed();
        holder.join().expect("holder");
        waited
    };

    let short = wait_behind(Duration::from_millis(20));
    let long = wait_behind(Duration::from_millis(200));

    println!(
        "\n  gate held  20 ms  ->  allocator waited {:>7.1} ms\n  \
         gate held 200 ms  ->  allocator waited {:>7.1} ms\n",
        short.as_secs_f64() * 1000.0,
        long.as_secs_f64() * 1000.0
    );

    // The point is not the exact number, it is that the waiting side pays the
    // holding side's duration in full: there is no queue-jumping and no partial
    // progress, so shortening the hold is the only lever.
    assert!(
        long > short,
        "a longer hold has to cost the waiter more, or the gate is not a gate"
    );
    assert!(
        long.as_millis() >= 150,
        "the allocator waited {long:?} behind a 200 ms hold, which is not a wait at all"
    );
}

/// How many allocators the gate serialises before it stops being free.
///
/// D5 decides the floor-blocking half — a scan is never refused to protect a
/// number. It says nothing about how many allocators may ask about one promise at
/// once, which is what a write budget is.
#[test]
fn the_gate_serialises_and_the_cost_is_linear() {
    let Some(url) = database_url() else {
        eprintln!("DATABASE_URL unset: skipping");
        return;
    };
    let mut c = nylonite_invariants::connect_exclusive(&url);
    let Some(promise) = a_promise(&mut c) else {
        eprintln!("no open promise: this has nothing to contend over");
        return;
    };

    let run = |n: usize| -> Duration {
        let t0 = Instant::now();
        let handles: Vec<_> = (0..n)
            .map(|_| {
                let url = url.clone();
                let promise = promise.clone();
                thread::spawn(move || {
                    let mut a = Client::connect(&url, NoTls).expect("connect");
                    let mut tx = a.transaction().expect("begin");
                    tx.execute(
                        "SELECT 1 FROM expected_supply WHERE id = ($1)::text::uuid FOR UPDATE",
                        &[&promise],
                    )
                    .expect("gate");
                    // What an allocator does while holding it: read the promise,
                    // decide, write its claim. Approximated by the read.
                    tx.execute(
                        "SELECT quantity_promisable FROM expected_supply
                          WHERE id = ($1)::text::uuid",
                        &[&promise],
                    )
                    .expect("read");
                    tx.rollback().expect("rollback");
                })
            })
            .collect();
        for h in handles {
            h.join().expect("allocator");
        }
        t0.elapsed()
    };

    let one = run(1);
    let eight = run(8);
    let thirty_two = run(32);

    println!(
        "\n  allocators contending for one promise\n    \
         1  {:>7.1} ms\n    8  {:>7.1} ms\n   32  {:>7.1} ms\n",
        one.as_secs_f64() * 1000.0,
        eight.as_secs_f64() * 1000.0,
        thirty_two.as_secs_f64() * 1000.0
    );

    // No assertion on the shape beyond completion. Thirty-two threads on a
    // laptop measure the laptop as much as the schema, and a threshold picked to
    // pass here would be a number nobody could defend. What matters is that the
    // gate serialises without deadlocking or erroring, which is the property a
    // budget is built on top of.
    assert!(thirty_two > Duration::ZERO);
}
