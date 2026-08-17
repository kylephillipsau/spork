//! What a prepack row is about, decided before anything is written.
//!
//! A file of this kind is 186 rows describing three different kinds of thing in one
//! shape, because NetSuite had one shape to offer. A row is a box the business
//! packs into, a stock code with its carton measured, or a *style* whose carton
//! stands for every size in it — and the only way to tell is to ask the item
//! master.
//!
//! # Why this is a module and not a loop in the importer
//!
//! Deciding what a row means is the whole difficulty, and it is a pure function
//! of the row and the catalogue. The importer's job is I/O. This is the same
//! split [`crate::receiving`] and [`crate::observing`] already use: the judgement
//! is testable without a database, and the thing that touches the database has no
//! judgement in it.
//!
//! # The codes below are a mix, and it is worth knowing which
//!
//! `SKU-0180` and `STY-7720` are invented stand-ins: the real ones are a
//! customer's catalogue and were swept out on 2026-08-31. The counts are the
//! export's and are the load-bearing part — 81 rows, 13 rows, 186 rows.
//! **The rest of the codes quoted here are still the customer's** and are on
//! the same list; they are left for now because they are cited as evidence of
//! what the export actually contains, and inventing them would quietly turn a
//! measurement into an illustration. See the handover's debts.
//!
//! # The rules, and the evidence for each
//!
//! **A name that resolves to the item master is an item.** Exactly, or as a
//! style prefix shared by two or more codes. `SKU-0180` is not an item; the
//! catalogue sells `SKU-0180-S`, `-M`, `-L` and `-XL`, and one carton covers all
//! four. D108.
//!
//! **A name that looks like nothing in particular is a container.** `Medium
//! Box`, `Pallet`, `Satchel`. The first version of this used "has no weight" as
//! the test, which is wrong on its own: `Small Box` carries 3 kg. Resolving
//! against the catalogue is the principled rule and the weight is corroboration.
//!
//! **A name that looks like a stock code and is not in the catalogue is
//! neither.** 81 rows of the real file are exactly that — `SKU-7002`,
//! `SKU-1230`, `SKU-0240` — absent from a 7,240-row item export in any form.
//! Treating them as containers would invent 81 box types out of an incomplete
//! export. They are reported and not written, because the honest reading is that
//! the export is missing them rather than that the warehouse packs into a box
//! called `SKU-1230`.
//!
//! **The case pack is in the name**, because there was nowhere else to put it:
//! `SKU-6013 (6 UNITS)`, `WAA-001 (x12)`, `Bekina StepliteX (x5)`. Thirteen rows
//! carry it and none of the thirteen has a row of its own, so for those items the
//! carton is all there is.

use std::collections::HashMap;

/// What the catalogue says about a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Known {
    /// One item has exactly this code.
    Item,
    /// No item has this code, but `n` items are variants of it.
    Style(usize),
    /// The catalogue has never heard of it.
    Nothing,
}

/// The catalogue, indexed the two ways a prepack name can hit it.
pub struct Catalogue {
    codes: HashMap<String, ()>,
    styles: HashMap<String, usize>,
}

impl Catalogue {
    /// **The style split is a convention, applied here and nowhere deeper.**
    /// `SKU-0180-L` looking like a variant of `SKU-0180` is a naming habit this
    /// database has never been told about. Migration 73 deliberately infers
    /// nothing; the inference lives here, where a person reads its output before
    /// anything is written.
    ///
    /// A variant suffix is the last hyphenated segment when what precedes it is
    /// itself a plausible code. `STY-7720-08` splits to `STY-7720` + `08`;
    /// `SKU-0180` does not split, because `DGN` alone is not a code.
    pub fn new(item_codes: impl IntoIterator<Item = String>) -> Self {
        let mut codes = HashMap::new();
        let mut styles: HashMap<String, usize> = HashMap::new();
        for code in item_codes {
            if let Some(style) = style_of(&code) {
                *styles.entry(style).or_insert(0) += 1;
            }
            codes.insert(code, ());
        }
        // A "style" with one member is just an item with a hyphen in it.
        styles.retain(|_, n| *n >= 2);
        Self { codes, styles }
    }

    pub fn lookup(&self, name: &str) -> Known {
        if self.codes.contains_key(name) {
            Known::Item
        } else if let Some(n) = self.styles.get(name) {
            Known::Style(*n)
        } else {
            Known::Nothing
        }
    }
}

/// `STY-7720-08` -> `STY-7720`. `SKU-0180` -> none.
pub fn style_of(code: &str) -> Option<String> {
    let (head, tail) = code.rsplit_once('-')?;
    // A variant suffix is short and alphanumeric: a size, a colour, a length.
    if tail.is_empty() || tail.len() > 5 || !tail.chars().all(|c| c.is_alphanumeric() || c == '.') {
        return None;
    }
    // And what it hangs off has to look like a code in its own right, which is
    // what stops `SKU-0180` becoming style `DGN`.
    if head.contains(['-', '.']) || head.len() >= 4 {
        Some(head.to_string())
    } else {
        None
    }
}

/// Two to four letters, a separator, then digits: `SKU-0180`, `U.73.132`.
///
/// Deliberately shape-only. Whether such a thing exists is the catalogue's
/// question; this answers whether somebody meant it to be a product.
pub fn looks_like_code(name: &str) -> bool {
    let Some((head, tail)) = name.split_once(['-', '.']) else {
        return false;
    };
    !head.is_empty()
        && head.len() <= 4
        && head.chars().all(|c| c.is_ascii_uppercase())
        && tail.len() >= 3
        && tail.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        && tail.chars().any(|c| c.is_ascii_digit())
}

/// `SKU-6013 (6 UNITS)` -> (`SKU-6013`, 6). Case and wording vary because a
/// person typed each one.
pub fn multipack(name: &str) -> Option<(String, u32)> {
    let open = name.rfind('(')?;
    if !name.trim_end().ends_with(')') {
        return None;
    }
    let inner = name[open + 1..name.trim_end().len() - 1].trim();
    let digits: String = inner.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    // Everything that is not the number must be `x`, `units`, `unit` or space —
    // otherwise the parenthetical is a description, not a count.
    let rest: String = inner
        .chars()
        .filter(|c| !c.is_ascii_digit())
        .collect::<String>()
        .to_ascii_lowercase()
        .replace(['x', ' '], "");
    if !matches!(rest.as_str(), "" | "units" | "unit") {
        return None;
    }
    let n: u32 = digits.parse().ok()?;
    if n == 0 {
        return None;
    }
    Some((name[..open].trim().to_string(), n))
}

/// What a row is about, and what the importer should therefore write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// One stock code, measured.
    Item { code: String },
    /// A style, measured once for every size in it.
    Style { code: String, variants: usize },
    /// A box the business packs into. `package_type`, not an observation.
    Container { name: String },
    /// Shaped like a stock code and unknown to the catalogue. Written nowhere:
    /// the export is incomplete, and inventing a subject for it would bury that.
    Unresolved { name: String },
}

/// The decision for one row, with the reasoning kept so a person can read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub subject: Subject,
    /// Units per carton, when the name said so.
    pub per_carton: Option<u32>,
    /// Why this row was read the way it was.
    pub because: String,
}

pub fn decide(name: &str, catalogue: &Catalogue) -> Decision {
    let name = name.trim();

    if let Some((base, n)) = multipack(name) {
        let (subject, because) = resolve(&base, catalogue);
        return Decision {
            subject,
            per_carton: Some(n),
            because: format!("{because}; the name states {n} per carton"),
        };
    }

    let (subject, because) = resolve(name, catalogue);
    Decision {
        subject,
        per_carton: None,
        because,
    }
}

fn resolve(name: &str, catalogue: &Catalogue) -> (Subject, String) {
    match catalogue.lookup(name) {
        Known::Item => (
            Subject::Item {
                code: name.to_string(),
            },
            "the catalogue has this exact code".to_string(),
        ),
        Known::Style(n) => (
            Subject::Style {
                code: name.to_string(),
                variants: n,
            },
            format!("no such code; {n} items are variants of it"),
        ),
        Known::Nothing if looks_like_code(name) => (
            Subject::Unresolved {
                name: name.to_string(),
            },
            "shaped like a stock code, and the catalogue has never heard of it".to_string(),
        ),
        Known::Nothing => (
            Subject::Container {
                name: name.to_string(),
            },
            "not a code, and the catalogue has never heard of it".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> Catalogue {
        Catalogue::new(
            [
                // A style in four sizes.
                "SKU-0180-S", "SKU-0180-M", "SKU-0180-L", "SKU-0180-XL",
                // A boot in two, with a fractional size.
                "STY-7720-08", "STY-7720-06.5",
                // A code that is nobody's variant.
                "SKU-2008",
                // A hyphenated code whose tail is too long to be a size.
                "SKU-7813-CTN",
            ]
            .into_iter()
            .map(str::to_string),
        )
    }

    #[test]
    fn a_style_is_a_prefix_two_or_more_codes_share() {
        let c = catalogue();
        assert_eq!(c.lookup("SKU-0180"), Known::Style(4));
        assert_eq!(c.lookup("STY-7720"), Known::Style(2));
        assert_eq!(c.lookup("SKU-0180-L"), Known::Item);
        assert_eq!(c.lookup("Medium Box"), Known::Nothing);
    }

    /// The thing that would quietly ruin the import: every `AAA-####` code
    /// collapsing to a style named `AAA`, so three unrelated products share a
    /// carton.
    #[test]
    fn a_code_is_not_a_variant_of_its_own_prefix() {
        assert_eq!(style_of("SKU-0180"), None, "DGN is not a style");
        assert_eq!(style_of("SKU-2008"), None);
        assert_eq!(style_of("STY-7720-08").as_deref(), Some("STY-7720"));
        assert_eq!(style_of("STY-7720-06.5").as_deref(), Some("STY-7720"));
        // A suffix that is a word rather than a size.
        assert_eq!(style_of("SKU-7813-CTN").as_deref(), Some("SKU-7813"));
        let c = catalogue();
        assert_eq!(
            c.lookup("SKU-7813"),
            Known::Nothing,
            "one member is not a style, so this stays a container until somebody says otherwise"
        );
    }

    #[test]
    fn the_case_pack_is_read_out_of_the_name() {
        assert_eq!(multipack("SKU-6013 (6 UNITS)"), Some(("SKU-6013".into(), 6)));
        assert_eq!(multipack("SKU-0700 (X6)"), Some(("SKU-0700".into(), 6)));
        assert_eq!(multipack("WAA-001 (x12)"), Some(("WAA-001".into(), 12)));
        assert_eq!(multipack("SKU-2105 (12 units)"), Some(("SKU-2105".into(), 12)));
        assert_eq!(
            multipack("Bekina StepliteX (x5)"),
            Some(("Bekina StepliteX".into(), 5))
        );
    }

    /// A parenthetical that is not a count must not be read as one.
    #[test]
    fn a_description_in_brackets_is_not_a_case_pack() {
        assert_eq!(multipack("Extra Small Box (1/2)"), None, "1/2 is not a count");
        assert_eq!(multipack("Something (blue)"), None);
        assert_eq!(multipack("Plain name"), None);
        assert_eq!(multipack("Trailing ("), None);
    }

    #[test]
    fn a_row_is_read_against_the_catalogue_not_its_weight() {
        let c = catalogue();
        assert_eq!(
            decide("SKU-0180", &c).subject,
            Subject::Style { code: "SKU-0180".into(), variants: 4 }
        );
        assert_eq!(
            decide("SKU-2008", &c).subject,
            Subject::Item { code: "SKU-2008".into() }
        );
        // `Small Box` carries a weight and is still a box, which is why "no
        // weight" was the wrong test.
        assert_eq!(
            decide("Small Box", &c).subject,
            Subject::Container { name: "Small Box".into() }
        );
    }

    /// **The one that would have invented 81 box types.** The item export does
    /// not cover the prepack list, and a code the catalogue has never heard of
    /// is a gap in the export, not a carton the warehouse packs into.
    #[test]
    fn a_code_the_catalogue_lacks_is_not_a_box() {
        let c = catalogue();
        assert_eq!(
            decide("SKU-1230", &c).subject,
            Subject::Unresolved { name: "SKU-1230".into() }
        );
        assert_eq!(
            decide("SKU-0240", &c).subject,
            Subject::Unresolved { name: "SKU-0240".into() }
        );
        assert_eq!(
            decide("U.73.132", &c).subject,
            Subject::Unresolved { name: "U.73.132".into() },
            "supplier-derived codes are still codes"
        );
        // And the things that really are boxes stay boxes.
        for box_name in ["Medium Box", "Pallet", "Satchel", "BLUE SKID", "XS - G"] {
            assert!(
                matches!(decide(box_name, &c).subject, Subject::Container { .. }),
                "{box_name} should read as a container"
            );
        }
    }

    #[test]
    fn a_multipack_resolves_its_base_and_keeps_the_count() {
        let c = Catalogue::new(
            ["SKU-6013-S", "SKU-6013-M"].into_iter().map(str::to_string),
        );
        let d = decide("SKU-6013 (6 UNITS)", &c);
        assert_eq!(d.subject, Subject::Style { code: "SKU-6013".into(), variants: 2 });
        assert_eq!(d.per_carton, Some(6));
        assert!(d.because.contains("6 per carton"));
    }
}
