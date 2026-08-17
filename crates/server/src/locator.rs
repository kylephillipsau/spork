//! What the thing in your hand is, from whatever was printed on it.
//!
//! D111 makes one input the primary locator on every surface: *"One
//! always-present input resolves any scannable identifier to its subject and
//! navigates."* This is the resolving half. Nothing here writes.
//!
//! # One function, three surfaces, because the scanner does not know which
//!
//! D34 names the shape. There are three identifier surfaces and they stay three
//! tables, because they sit in different provenance categories: a package's
//! identity is something that happened at a place and time, an item's barcode is
//! a durable fact about a product class, and a location's code is reference. One
//! table for all three would put a projection and a reference row under one key.
//!
//! Resolution is nonetheless one function, in three steps:
//!
//! 1. **Parse** — [`crate::barcodes`] turns the string into whatever it carries.
//! 2. **Dispatch** — a GTIN goes to `item_barcode`, an SSCC to the package
//!    surface, and a raw string to all three.
//! 3. **Narrow** — the caller says what it expects, and the outcome is one of
//!    D24's four words.
//!
//! # The four outcomes are not "found" and "not found"
//!
//! `identifier_unknown` is a well-formed identifier nothing holds — a real GTIN
//! for a product we do not stock. `identifier_unrecognised` is a string that is
//! not an identifier at all. `identifier_ambiguous` is more than one subject,
//! which is a supplier code collision or a scope overlap and must never be
//! resolved by silent preference. Telling an operator "no such item" when the
//! truth is "that is two items" is how a scan writes stock against the wrong
//! one.
//!
//! # What is not here yet
//!
//! **The failure record.** D111 says a scan resolving to nothing writes one, and
//! D28 specifies it on `activity_event` with a `symbology` reference beside it.
//! Neither table exists. The outcome word is returned so the screen can say the
//! right thing; nothing is written down. Deferred deliberately — D136 — and it
//! is the reason this is a `GET`: while resolution is a pure read, a verb that
//! claims otherwise would be the lie. When the record lands, a failed resolution
//! becomes an act and the verb changes with it.

use actix_web::web;
use serde::Serialize;
use uuid::Uuid;

use crate::auth::Caller;
use crate::barcodes::{self, Scanned};
use crate::capture::CaptureSubject;
use crate::error::ApiError;
use crate::tenancy::TenantScope;
use crate::AppState;

/// What a scan turned out to be, and what to do about it.
#[derive(Serialize)]
pub struct Resolution {
    /// `resolved`, `identifier_unknown`, `identifier_unrecognised`,
    /// `identifier_ambiguous`. D24's four, reused unchanged.
    pub outcome: String,
    /// Exactly what was scanned, after the symbology prefix is stripped.
    pub scanned: String,
    /// The normalised GTIN, when the string carried one. Fourteen characters.
    pub gtin: Option<String>,
    pub sscc: Option<String>,
    /// A GS1-128 carton label carries the lot and the expiry beside the GTIN,
    /// so one scan answers three questions. Receiving stops keying batch
    /// numbers, which is most of the operational value of doing this properly.
    pub lot: Option<String>,
    pub expiry: Option<String>,
    /// Everything that matched, which is one row unless the outcome is
    /// ambiguous.
    pub subjects: Vec<Subject>,
}

/// One thing the identifier resolved to.
#[derive(Serialize)]
pub struct Subject {
    /// `item`, `package` or `location`.
    pub kind: String,
    pub id: Uuid,
    pub code: String,
    pub description: Option<String>,
    /// Which surface answered: `item_barcode`, `item_code`, `package_barcode`,
    /// `package_sscc` or `location_code`. Two arms agreeing on one subject is
    /// one hit, and this says which of them spoke.
    pub via: String,
    /// **The capture subjects this item offers**, which is the whole reason a
    /// scan can open a measurement session safely.
    ///
    /// Not derived here: [`crate::capture::subjects_for_item`] is the worklist's
    /// own enumeration with a filter on it. A scan that opened a session against
    /// the item at carton level would walk back into the trip D108 exists to
    /// prevent, because for a styled variant the worklist offers the *style's*
    /// carton instead — and the scan path would have won silently.
    ///
    /// **Always on the wire, empty or not.** This carried
    /// `skip_serializing_if = "Vec::is_empty"`, which omits the field entirely
    /// when there is nothing to offer — while `domain/types.ts` declares it
    /// required. The contract check compares names and cannot see a field that
    /// is sometimes absent, so this is exactly the hand-mirrored drift D113's
    /// check exists for, arriving through the one gap that check has. An empty
    /// array is the honest answer and costs two bytes.
    pub capture: Vec<CaptureSubject>,
}

/// Resolve a scanned string.
///
/// `expect` narrows: a screen that can only act on items says so, and a carton
/// scanned there reports `identifier_unknown` rather than navigating somewhere
/// the operator did not ask to go. D34 calls this the narrow step and takes the
/// value from `expected_entity_kind`, which is one of `package | location |
/// item | lot | none`.
pub async fn resolve(
    state: &web::Data<AppState>,
    who: &Caller,
    raw: &str,
    expect: Option<&str>,
) -> Result<Resolution, ApiError> {
    let scan: Scanned = barcodes::parse(raw);
    let expect = expect.unwrap_or("none").to_string();

    let mut scope = TenantScope::begin(&state.pool, who.tenant_id).await?;
    let scan_for_tx = scan.clone();
    let expect_for_tx = expect.clone();
    let site_id = who.site_id;

    let subjects = scope
        .run(move |tx| {
            let scan = scan_for_tx;
            let expect = expect_for_tx;
            Box::pin(async move {
                let mut found: Vec<Subject> = vec![];
                let wants = |kind: &str| expect == "none" || expect == kind;

                // ── the item surface ────────────────────────────────────
                if wants("item") {
                    // **Both spellings, because only one scheme is a GTIN.**
                    //
                    // The first draft consulted `item_barcode` only when the
                    // parser had found a GTIN, which quietly made the other two
                    // schemes unreachable: an `internal` code and a supplier's
                    // `supplier_reference` are not GTINs and never will be, so
                    // the table's own CHECK admits three schemes while the
                    // resolver could reach one. D34 is explicit that a raw
                    // string dispatches to all three surfaces.
                    let mut candidates: Vec<String> = vec![scan.raw.clone()];
                    if let Some(g) = &scan.gtin {
                        if g != &scan.raw {
                            candidates.push(g.clone());
                        }
                    }

                    // **`effective @> now()` and `tenant_id NULLS LAST`, and
                    // both are the decision rather than housekeeping.**
                    //
                    // Without the range predicate a closed binding keeps
                    // resolving and the whole argument for a daterange over an
                    // `active` boolean is decoration. Without the ordering, a
                    // tenant's own row and the shared catalogue's row are a
                    // coin toss — D19's amendment makes the tenant row win,
                    // deterministically.
                    let rows = tx
                        .query(
                            "SELECT i.id, i.code, i.description
                               FROM item_barcode b
                               JOIN item i ON i.id = b.item_id
                              WHERE b.barcode = ANY($1)
                                AND b.effective @> CURRENT_DATE
                              ORDER BY b.tenant_id NULLS LAST
                              LIMIT 1",
                            &[&candidates],
                        )
                        .await?;
                    for r in &rows {
                        found.push(Subject {
                            kind: "item".into(),
                            id: r.get(0),
                            code: r.get(1),
                            description: r.get(2),
                            via: "item_barcode".into(),
                            capture: vec![],
                        });
                    }

                    // **The code arm is exact, because this is a locator and
                    // not a search.** A prefix match would resolve `GLOVE` to
                    // whichever glove sorted first, which is the confident
                    // wrong answer this whole module is arranged against.
                    let rows = tx
                        .query(
                            "SELECT id, code, description FROM item WHERE code = $1",
                            &[&scan.raw],
                        )
                        .await?;
                    for r in &rows {
                        found.push(Subject {
                            kind: "item".into(),
                            id: r.get(0),
                            code: r.get(1),
                            description: r.get(2),
                            via: "item_code".into(),
                            capture: vec![],
                        });
                    }
                }

                // ── the package surface ─────────────────────────────────
                //
                // Direct against `package`, because D24's `package_identifier`
                // projection does not exist yet. Same two columns it would fold.
                if wants("package") {
                    let sscc = scan.sscc.clone().unwrap_or_else(|| scan.raw.clone());
                    // **`sscc` is `character(18)`, not text.** A fixed-width
                    // column pads on read, so it comes back as eighteen
                    // characters with trailing spaces where the value is
                    // shorter, and an unqualified comparison against a text
                    // parameter leaves the driver to infer a type it gets
                    // wrong. Cast both sides and the padding stops mattering.
                    let rows = tx
                        .query(
                            "SELECT id, coalesce(sequence::text, '—'),
                                    sscc::text, barcode
                               FROM package
                              WHERE sscc::text = $1::text OR barcode = $2::text",
                            &[&sscc, &scan.raw],
                        )
                        .await?;
                    for r in &rows {
                        let by_sscc: Option<String> = r.get(2);
                        found.push(Subject {
                            kind: "package".into(),
                            id: r.get(0),
                            code: r.get(1),
                            description: None,
                            via: if by_sscc.as_deref() == Some(sscc.as_str()) {
                                "package_sscc".into()
                            } else {
                                "package_barcode".into()
                            },
                            capture: vec![],
                        });
                    }
                }

                // ── the location surface ────────────────────────────────
                if wants("location") {
                    // `location` carries a code and no name: a bin is its code,
                    // and there is nothing else to show.
                    let rows = tx
                        .query(
                            "SELECT id, code FROM location WHERE code = $1",
                            &[&scan.raw],
                        )
                        .await?;
                    for r in &rows {
                        found.push(Subject {
                            kind: "location".into(),
                            id: r.get(0),
                            code: r.get(1),
                            description: None,
                            via: "location_code".into(),
                            capture: vec![],
                        });
                    }
                }

                // **Two arms agreeing on one subject is one hit.** A tenant that
                // prints the SKU as a Code 128 *and* registers it as an internal
                // barcode is the ordinary case, not an ambiguity — reporting it
                // as one would make the commonest correct setup unusable.
                found.dedup_by(|a, b| a.kind == b.kind && a.id == b.id);
                let mut seen: Vec<(String, Uuid)> = vec![];
                found.retain(|s| {
                    let key = (s.kind.clone(), s.id);
                    if seen.contains(&key) {
                        false
                    } else {
                        seen.push(key);
                        true
                    }
                });

                // The capture subjects, for the one item this resolved to.
                if found.len() == 1 && found[0].kind == "item" {
                    let id = found[0].id;
                    // The site, because a capture subject now carries the bin
                    // it is in and how much is there: a scan answers "what is
                    // this, and where does the shelf say it lives".
                    found[0].capture =
                        crate::capture::subjects_for_item(tx, site_id, id).await?;
                }

                Ok(found)
            })
        })
        .await?;

    let outcome = outcome_of(&scan, subjects.len());

    Ok(Resolution {
        outcome: outcome.to_string(),
        scanned: scan.raw,
        gtin: scan.gtin,
        sscc: scan.sscc,
        lot: scan.lot,
        expiry: scan.expiry,
        subjects,
    })
}

/// Which of D24's four words describes this result.
///
/// Separated from the query so it can be tested without a database, because the
/// distinction between "well-formed and unheld" and "not an identifier" is a
/// judgement about the *string* and nothing to do with what is in the table.
pub fn outcome_of(scan: &Scanned, hits: usize) -> &'static str {
    match hits {
        1 => "resolved",
        n if n > 1 => "identifier_ambiguous",
        // Nothing matched. Which report depends on whether the string was an
        // identifier at all: a valid GTIN nobody stocks is a different fact
        // about the world from a smudge, and an operator can act on the first.
        _ if scan.is_structured() || scan.gs1 => "identifier_unknown",
        _ if looks_like_a_code(&scan.raw) => "identifier_unknown",
        _ => "identifier_unrecognised",
    }
}

/// Whether a bare string is plausibly somebody's code rather than noise.
///
/// **The bar is deliberately low.** This warehouse's own item codes include
/// `2019 Calendar` and `$15.00 Freight`, so a rule tight enough to exclude a
/// misread would exclude half the catalogue. What it rejects is the empty
/// string and a lone control character — which is what a trigger-pull with
/// nothing in front of it produces.
fn looks_like_a_code(raw: &str) -> bool {
    let t = raw.trim();
    !t.is_empty() && t.chars().any(|c| c.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(s: &str) -> Scanned {
        barcodes::parse(s)
    }

    #[test]
    fn one_hit_resolves_and_two_are_never_resolved_by_preference() {
        assert_eq!(outcome_of(&scan("STY-7720-08"), 1), "resolved");
        // A supplier code collision, which is exactly what `issuer_party_id`
        // exists to scope and what must never be answered by picking one.
        assert_eq!(outcome_of(&scan("CTN-99"), 2), "identifier_ambiguous");
        assert_eq!(outcome_of(&scan("CTN-99"), 7), "identifier_ambiguous");
    }

    #[test]
    fn a_well_formed_identifier_nobody_holds_is_not_the_same_as_a_smudge() {
        // A real GTIN for a product we do not stock. The operator can act on
        // this: it is a thing, and it is not ours.
        assert_eq!(outcome_of(&scan("9312345678907"), 0), "identifier_unknown");
        // A GS1 element string that parsed and matched nothing.
        assert_eq!(outcome_of(&scan("0109312345678907"), 0), "identifier_unknown");
        // A plain code nobody holds — still a thing somebody printed.
        assert_eq!(outcome_of(&scan("NOT-A-SKU"), 0), "identifier_unknown");
        // And nothing at all.
        assert_eq!(outcome_of(&scan(""), 0), "identifier_unrecognised");
        assert_eq!(outcome_of(&scan("\u{1D}"), 0), "identifier_unrecognised");
    }

    #[test]
    fn the_low_bar_admits_this_warehouses_own_codes() {
        // Real codes from the import. A tighter rule would report half the
        // catalogue as unrecognisable.
        for code in ["2019 Calendar", "$15.00 Freight", "2025-CAT-CONTRIBUTION"] {
            assert_eq!(outcome_of(&scan(code), 0), "identifier_unknown", "{code}");
        }
        assert!(!looks_like_a_code("   "));
        assert!(!looks_like_a_code("---"));
    }
}
