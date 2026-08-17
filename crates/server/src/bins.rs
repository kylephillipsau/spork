//! Reading a bin list: what a bin is called, where it is, and what kind it is.
//!
//! 6,310 bins across five warehouses arrive as `Bin Number, Location, WMS Bin
//! Type, WMS Picking Order, WMS Bin Sequence`. Three of those columns map onto
//! things this schema already has and one does not, and the interesting part is
//! which is which.
//!
//! Pure, and tested without a database, for [`crate::prepack`]'s reason: the
//! judgement is the difficulty and the I/O is not.
//!
//! # A bin code is already structured
//!
//! `location` has carried `aisle`, `bay`, `level` and `position` since migration
//! 1 and nothing has ever filled them. The codes decompose on sight —
//! `A-01-01`, `I.1.2`, `A01-01-2` — so the structure that was always implicit in
//! the string becomes four columns, and "everything in aisle G" stops being a
//! `LIKE 'G-%'`.
//!
//! # The type vocabulary is nearly the same vocabulary
//!
//! `location_kind_ck` allows pick_face, bulk, staging, dock and overflow. The
//! export says Pick, Bulk Storage, Staging and Receiving. Four of five line up;
//! `Receiving` becomes `dock`, which is what a receiving bin is here.
//!
//! **A site can arrive with no bin type stated on any of its shelves**, which is
//! the case this refuses on: a whole warehouse of them at once.
//! That is a fact about that warehouse rather than scattered gaps, and `kind` is
//! NOT NULL with no value meaning "unknown". So they are reported and skipped
//! unless a person says what to assume — the same refusal [`crate::prepack`]
//! makes about case packs.

/// How a bin code breaks up. Every part is optional because a code like `3PL`
/// has none of them.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Parts {
    pub aisle: Option<String>,
    pub bay: Option<String>,
    pub level: Option<String>,
    pub position: Option<String>,
}

/// `A-01-01` -> aisle A, bay 01, level 01. `I.1.2` and `A01-01-2` likewise.
///
/// Splits on either separator because the export uses both, and keeps the
/// original text rather than parsing to a number: `01` and `1` are different
/// labels on a rack even when they are the same integer.
pub fn decompose(code: &str) -> Parts {
    let parts: Vec<&str> = code
        .split(['-', '.'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut p = Parts::default();
    // A single token is a name, not a coordinate.
    if parts.len() < 2 {
        return p;
    }
    let mut it = parts.into_iter();
    p.aisle = it.next().map(str::to_string);
    p.bay = it.next().map(str::to_string);
    p.level = it.next().map(str::to_string);
    p.position = it.next().map(str::to_string);
    p
}

/// The export's word for a bin type, in this schema's vocabulary.
pub fn kind_of(wms_bin_type: &str) -> Option<&'static str> {
    match wms_bin_type.trim().to_ascii_lowercase().as_str() {
        "pick" => Some("pick_face"),
        "bulk storage" | "bulk" => Some("bulk"),
        "staging" => Some("staging"),
        // A receiving bin is where goods land off a truck, which is what `dock`
        // means here. Migration 1's vocabulary has no separate `receiving`.
        "receiving" => Some("dock"),
        "overflow" => Some("overflow"),
        _ => None,
    }
}

/// `Melbourne Warehouse` -> `Melbourne`.
pub fn site_name(location: &str) -> String {
    location
        .trim()
        .strip_suffix("Warehouse")
        .unwrap_or(location.trim())
        .trim()
        .to_string()
}

/// `Melbourne Warehouse` -> `MEL`.
///
/// First three letters, which is not a guess: the two sites already on file are
/// `MEL` and `SYD`, so this is the convention the business already uses.
pub fn site_code(location: &str) -> String {
    site_name(location)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(3)
        .collect::<String>()
        .to_ascii_uppercase()
}

/// Where the clock is, which matters because Brisbane does not observe daylight
/// saving and Melbourne and Sydney do.
///
/// `None` means nobody here knows, and the importer says so rather than
/// defaulting a site into the wrong hour twice a year.
pub fn timezone_for(location: &str) -> Option<&'static str> {
    match site_name(location).to_ascii_lowercase().as_str() {
        "melbourne" => Some("Australia/Melbourne"),
        "sydney" => Some("Australia/Sydney"),
        "brisbane" => Some("Australia/Brisbane"),
        // Adelaide runs half an hour behind the eastern states and DOES keep
        // daylight saving, so it is neither of the two cases above.
        "adelaide" => Some("Australia/Adelaide"),
        // Perth appears on order and fulfilment lines but was absent from the
        // bin export, so it has no shelves yet. Two hours behind the east, and
        // no daylight saving at all.
        "perth" => Some("Australia/Perth"),
        _ => None,
    }
}

/// A place in the export that is probably not a building of this business.
///
/// An export routinely carries a handful of bins under a name that is somebody
/// else's premises - a third party's site, a bonded store, a partner's floor -
/// and creating a `site` for one asserts that this business has a warehouse
/// there. The importer reports them and leaves them out until told otherwise.
///
/// Matched on the words that say whose building it is rather than on a list of
/// particular names, so an export from anywhere is read the same way.
pub fn looks_external(location: &str) -> bool {
    let n = site_name(location).to_ascii_lowercase();
    ["3pl", "partner", "bonded", "external", "third party"].iter().any(|w| n.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bin_code_becomes_four_columns() {
        assert_eq!(
            decompose("A-01-01"),
            Parts {
                aisle: Some("A".into()),
                bay: Some("01".into()),
                level: Some("01".into()),
                position: None,
            }
        );
        // The other two shapes in the same file.
        assert_eq!(decompose("I.1.2").aisle.as_deref(), Some("I"));
        assert_eq!(decompose("I.1.2").level.as_deref(), Some("2"));
        assert_eq!(decompose("A01-01-2").aisle.as_deref(), Some("A01"));
        assert_eq!(decompose("A01-01-2").level.as_deref(), Some("2"));
    }

    /// `01` and `1` are different labels on a rack even where they are the same
    /// number, so the text survives.
    #[test]
    fn the_label_is_kept_as_written() {
        assert_eq!(decompose("A-01-01").bay.as_deref(), Some("01"));
        assert_eq!(decompose("A-1-1").bay.as_deref(), Some("1"));
    }

    #[test]
    fn a_name_is_not_a_coordinate() {
        assert_eq!(decompose("3PL"), Parts::default());
        assert_eq!(decompose("DOCK"), Parts::default());
        assert_eq!(decompose(""), Parts::default());
    }

    #[test]
    fn the_type_vocabularies_line_up_except_for_receiving() {
        assert_eq!(kind_of("Pick"), Some("pick_face"));
        assert_eq!(kind_of("Bulk Storage"), Some("bulk"));
        assert_eq!(kind_of("Staging"), Some("staging"));
        assert_eq!(kind_of("Receiving"), Some("dock"));
        // The 421 that state nothing, which is not a kind and must not become one.
        assert_eq!(kind_of(""), None);
        assert_eq!(kind_of("Something Else"), None);
    }

    #[test]
    fn a_site_keeps_the_convention_already_on_file() {
        assert_eq!(site_code("Melbourne Warehouse"), "MEL");
        assert_eq!(site_code("Sydney Warehouse"), "SYD");
        assert_eq!(site_code("Brisbane Warehouse"), "BRI");
        assert_eq!(site_name("Perth Warehouse"), "Perth");
    }

    /// The one that would be wrong twice a year.
    #[test]
    fn brisbane_does_not_keep_melbourne_time() {
        assert_eq!(timezone_for("Brisbane Warehouse"), Some("Australia/Brisbane"));
        assert_eq!(timezone_for("Melbourne Warehouse"), Some("Australia/Melbourne"));
        assert_eq!(timezone_for("Adelaide Warehouse"), Some("Australia/Adelaide"));
        assert_eq!(timezone_for("Perth Warehouse"), Some("Australia/Perth"));
        assert_eq!(timezone_for("Somewhere New"), None, "unknown is not a default");
    }

    #[test]
    fn a_third_partys_warehouse_is_flagged_rather_than_created() {
        assert!(looks_external("Partner Warehouse"));
        assert!(looks_external("3PL Overflow"));
        assert!(looks_external("Bonded Store"));
        assert!(!looks_external("Brisbane Warehouse"));
    }
}
