//! The recorded answers, and how they are taken.
//!
//! A golden snapshot is only worth what its cases are worth. These are chosen so
//! that **each one would move if a different thing broke**, rather than to cover
//! rows:
//!
//! - a resolution decided on Counterparty, which moves if D81's `shelf_life`
//!   ordering is re-argued;
//! - the same candidate rows under a kind ordered the other way, which moves if
//!   the per-kind-ness of the ordering is lost;
//! - a winner overridden by a floor it did not set, which moves if a clamp
//!   direction flips or `apply_clamps` stops looking at the losers;
//! - three clamps at once against a platform ceiling, which moves if the
//!   ceiling half is broken while the floor half still works;
//! - a request before any tenant version was effective, which moves if the
//!   effective-range filter is dropped — the half D70 said no epoch can see;
//! - a request naming no counterparty, which moves if a NULL axis stops meaning
//!   "any".
//!
//! The format is one line per case, `case = answer`, sorted, with the version on
//! the first line. Text rather than JSON because the point is that a human reads
//! the diff: **the snapshot's whole value is in being reviewed when it changes.**

use nylonite_policy::{apply_clamps, resolve, Candidate, PolicyKind};
use postgres::Client;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct Snapshot {
    pub version: u32,
    pub answers: BTreeMap<String, String>,
}

fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/invariants sits two below the root")
        .join("fixtures/resolver-golden.txt")
}

pub fn read_snapshot() -> Option<Snapshot> {
    let text = std::fs::read_to_string(path()).ok()?;
    let mut version = 0;
    let mut answers = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(v) = line.strip_prefix("RESOLVER_VERSION ") {
            version = v.trim().parse().ok()?;
            continue;
        }
        let (case, answer) = line.split_once(" = ")?;
        answers.insert(case.trim().to_string(), answer.trim().to_string());
    }
    Some(Snapshot { version, answers })
}

/// The tenant and the rows the cases are written against.
const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const GLOVE: &str = "17e10000-0000-0000-0000-000000000001";
const GLOVECO: &str = "9a247000-0000-0000-0000-000000000002";
const SITE: &str = "a5170000-0000-0000-0000-000000000001";

/// name, kind, item, party, at
type Case = (&'static str, &'static str, Option<&'static str>, Option<&'static str>, &'static str);

const CASES: &[Case] = &[
    ("shelf_life/glove+gloveco", "shelf_life", Some(GLOVE), Some(GLOVECO), "2026-08-04T12:00:00Z"),
    ("shelf_life/glove+no-party", "shelf_life", Some(GLOVE), None, "2026-08-04T12:00:00Z"),
    ("shelf_life/glove+gloveco@february", "shelf_life", Some(GLOVE), Some(GLOVECO), "2026-02-01T00:00:00Z"),
    ("receiving/glove+gloveco", "receiving", Some(GLOVE), Some(GLOVECO), "2026-08-04T12:00:00Z"),
    ("receiving/no-item+no-party", "receiving", None, None, "2026-08-04T12:00:00Z"),
    ("allocation/glove+gloveco", "allocation", Some(GLOVE), Some(GLOVECO), "2026-08-04T12:00:00Z"),
    // D85. The kind that decides whose measurement is current, watched like the
    // rest -- it changes what a freight invoice is checked against.
    ("observation_precedence/tenant-wide", "observation_precedence", None, None, "2026-08-04T12:00:00Z"),
];

fn kind_of(name: &str) -> PolicyKind {
    match name {
        "shelf_life" => PolicyKind::ShelfLife,
        "observation_precedence" => PolicyKind::ObservationPrecedence,
        "receiving" => PolicyKind::Receiving,
        "allocation" => PolicyKind::Allocation,
        other => panic!("no registry entry for {other}"),
    }
}

fn as_uuid_text(id: u128) -> String {
    let h = format!("{id:032x}");
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// Resolve every case as the code stands now.
///
/// The answer is written as the winning binding's *note* rather than its id, and
/// deliberately: a note survives a reseeded fixture and says what the binding is
/// for, so a diff reads "the platform default now wins" instead of two uuids.
pub fn observe(c: &mut Client) -> Result<Vec<(String, String)>, postgres::Error> {
    let mut out = vec![];
    for (name, kind, item, party, at) in CASES {
        let rows = c.query(
            "SELECT c.policy_binding_id::text, b.note, c.tenancy, c.product,
                    c.counterparty, c.space, c.ownership, c.metric
               FROM policy_candidate(($1)::text::uuid, ($2)::text::policy_kind,
                                     ($3)::text::uuid, ($4)::text::uuid,
                                     ($5)::text::uuid, NULL, NULL, NULL,
                                     ($6)::text::timestamptz) c
               JOIN policy_binding b ON b.id = c.policy_binding_id",
            &[&TENANT, kind, item, party, &SITE, at],
        )?;

        let mut notes = BTreeMap::new();
        let candidates: Vec<Candidate> = rows
            .iter()
            .map(|r| {
                let id: String = r.get(0);
                let binding = u128::from_str_radix(&id.replace('-', ""), 16).expect("uuid");
                notes.insert(binding, r.get::<_, String>(1));
                Candidate {
                    binding,
                    tenancy: r.get(2),
                    product: r.get(3),
                    counterparty: r.get(4),
                    space: r.get(5),
                    ownership: r.get(6),
                    metric: r.get(7),
                }
            })
            .collect();

        let k = kind_of(kind);
        let Some(r) = resolve(k, &candidates) else {
            out.push((name.to_string(), "no policy applies".to_string()));
            continue;
        };

        let ids: Vec<String> = candidates.iter().map(|c| as_uuid_text(c.binding)).collect();
        let value_rows = c.query(
            "SELECT policy_binding_id::text, field, value::float8
               FROM policy_value(($1)::text::policy_kind, ($2::text[])::uuid[],
                                 ($3)::text::timestamptz)
              WHERE value IS NOT NULL",
            &[kind, &ids, at],
        )?;
        let owned: Vec<(u128, String, f64)> = value_rows
            .iter()
            .map(|r| {
                let id: String = r.get(0);
                (
                    u128::from_str_radix(&id.replace('-', ""), 16).expect("uuid"),
                    r.get(1),
                    r.get(2),
                )
            })
            .collect();
        let values: Vec<(u128, &str, f64)> =
            owned.iter().map(|(b, f, v)| (*b, f.as_str(), *v)).collect();

        let mut clamps: Vec<String> = apply_clamps(k, r.winner.binding, &values)
            .iter()
            .map(|c| format!("{}:{}->{}", c.field, c.winner_said, c.applied))
            .collect();
        clamps.sort();

        let decided = r.decided_on.map(|d| d.to_string()).unwrap_or_else(|| "sole".into());
        let answer = if clamps.is_empty() {
            format!("{} on {decided}", notes[&r.winner.binding])
        } else {
            format!("{} on {decided}, clamped {}", notes[&r.winner.binding], clamps.join(" "))
        };
        out.push((name.to_string(), answer));
    }
    Ok(out)
}

/// Render a snapshot from what the code answers now, for regenerating the file.
pub fn render(c: &mut Client) -> Result<String, postgres::Error> {
    let mut s = String::from(
        "# The answers this resolver gives, recorded so that a change to what ships\n\
         # cannot happen quietly. J22. Regenerate only with a RESOLVER_VERSION bump,\n\
         # and read the diff.\n",
    );
    s.push_str(&format!("RESOLVER_VERSION {}\n\n", nylonite_policy::RESOLVER_VERSION));
    let mut answers: Vec<(String, String)> = observe(c)?;
    answers.sort();
    for (case, answer) in answers {
        s.push_str(&format!("{case} = {answer}\n"));
    }
    Ok(s)
}
