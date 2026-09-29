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
//!
//! **A later export says more, in a person's shorthand.** The 313-row list of
//! 2026-09-30 adds inners to the count: `SKU-0644 (CTN x10 box, 1000pcs)`,
//! `SKU-0510 (CTN, 4x 10pack)`, `SKU-0822 (CTN 20 x box 12)`, and bare
//! `SKU-9600 (CTN)`. [`contents`] reads the shapes that say one thing and
//! nothing else, and leaves the rest unread rather than guessing: `(1 ROLL)`
//! does not say whether a roll is the unit, so that carton's contents stay
//! unrecorded and the dry run says so.
//!
//! **A name that says what is inside it is a product's carton, never a box.**
//! Seventeen of the new rows put a bracket after a code, and read whole, as the
//! first version of this did, every one of them looked like nothing in
//! particular and so like a container. The bracket is split off before the
//! catalogue is asked, and a name whose bracket states contents or says `CTN`
//! is at worst unresolved. A box the business packs into does not state how
//! many of something are in it. `Extra Small Box (1/2)` still reads as a box,
//! because `1/2` states nothing.

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
    /// A code counts towards every style above it, not only the nearest, so a
    /// row naming `SKU-3010` finds `SKU-3010B-06` through `SKU-3010B`. See
    /// [`styles_of`].
    ///
    /// **One colour is still a colour of a family.** A hyphenated split needs two
    /// members, because `SKU-7813-CTN` alone does not show that `-CTN` is a
    /// variant rather than part of the code. A colour letter needs one: the
    /// catalogue uses the same letters in 80 families, so a list naming
    /// `SKU-7002` when only `SKU-7002B` is on file names a family whose other
    /// colours are not stocked. Thirty-five rows of the 2026-09-30 list are
    /// that, and the person reading its dry run said to record them so.
    pub fn new(item_codes: impl IntoIterator<Item = String>) -> Self {
        let mut codes = HashMap::new();
        let mut styles: HashMap<String, usize> = HashMap::new();
        let mut colours: HashMap<String, ()> = HashMap::new();
        for code in item_codes {
            let mut at = code.clone();
            while let Some((up, by)) = split(&at) {
                let up = up.to_string();
                *styles.entry(up.clone()).or_insert(0) += 1;
                if by == Split::Colour {
                    colours.insert(up.clone(), ());
                }
                at = up;
            }
            codes.insert(code, ());
        }
        // A "style" with one member is just an item with a hyphen in it.
        styles.retain(|s, n| *n >= 2 || colours.contains_key(s));
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

    /// The codes that begin with a name, for a person working out why it did
    /// not resolve: `SKU-7813` against a catalogue holding only `SKU-7813-CTN`
    /// is a near miss, which is a different gap from nothing at all.
    pub fn starting_with(&self, name: &str) -> Vec<&str> {
        let mut out: Vec<&str> = self
            .codes
            .keys()
            .filter(|c| c.len() > name.len() && c.starts_with(name))
            .map(String::as_str)
            .collect();
        out.sort_unstable();
        out
    }
}

/// Which convention a split followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Split {
    Hyphen,
    Colour,
}

/// The nearest style a code could be a variant of, by either convention.
///
/// **A hyphenated suffix.** A variant suffix is the last hyphenated segment
/// when what precedes it is itself a plausible code. `STY-7720-08` splits to
/// `STY-7720` + `08`; `SKU-0180` does not split, because `DGN` alone is not a
/// code.
///
/// **A colour letter.** The 2026-09-30 catalogue marks colour with one capital
/// straight after the number, no hyphen: `SKU-9600B`, `-9600G`, `-9600R` for
/// blue, green and red, in 80 families of two or more. So a code ending in a
/// digit and one letter splits there, when what precedes the letter is shaped
/// like a code.
///
/// When both apply, the longer head is nearer. `U.AB-204V` is the `V` of
/// `U.AB-204`, not a size `204V` of `U.AB`.
fn split(code: &str) -> Option<(&str, Split)> {
    let hyphen = code.rsplit_once('-').and_then(|(head, tail)| {
        // A variant suffix is short and alphanumeric: a size, a colour, a length.
        if tail.is_empty() || tail.len() > 5 || !tail.chars().all(|c| c.is_alphanumeric() || c == '.') {
            return None;
        }
        // And what it hangs off has to look like a code in its own right, which is
        // what stops `SKU-0180` becoming style `DGN`.
        (head.contains(['-', '.']) || head.len() >= 4).then_some(head)
    });
    let colour = {
        let mut rev = code.chars().rev();
        match (rev.next(), rev.next()) {
            (Some(letter), Some(digit)) if letter.is_ascii_uppercase() && digit.is_ascii_digit() => {
                let head = &code[..code.len() - 1];
                looks_like_code(head).then_some(head)
            }
            _ => None,
        }
    };
    match (hyphen, colour) {
        (Some(h), Some(c)) if c.len() > h.len() => Some((c, Split::Colour)),
        (Some(h), _) => Some((h, Split::Hyphen)),
        (None, c) => c.map(|c| (c, Split::Colour)),
    }
}

/// `STY-7720-08` -> `STY-7720`, `SKU-9600B` -> `SKU-9600`. `SKU-0180` -> none.
pub fn style_of(code: &str) -> Option<String> {
    split(code).map(|(head, _)| head.to_string())
}

/// Every style a code could be a variant of, nearest first: `SKU-3010B-06` is
/// a size of `SKU-3010B`, which is a colour of `SKU-3010`.
pub fn styles_of(code: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut at = code.to_string();
    while let Some(up) = style_of(&at) {
        out.push(up.clone());
        at = up;
    }
    out
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

/// What a name says is in the carton it describes.
///
/// `item_packing_config`'s own two columns, so what is read here is what is
/// written, with no arithmetic in between. A bare count is units straight into
/// the carton, stored as that many inners of one, which is the shape the first
/// loader wrote for `(x12)`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pack {
    pub inners_per_carton: Option<u32>,
    pub units_per_inner: Option<u32>,
}

impl std::fmt::Display for Pack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.inners_per_carton, self.units_per_inner) {
            (Some(n), Some(1)) => write!(f, "{n} per carton"),
            (Some(n), Some(u)) => write!(f, "{n} inners of {u}, {} per carton", n * u),
            (Some(n), None) => write!(f, "{n} inners, their count unstated"),
            (None, _) => write!(f, "a carton, its count unstated"),
        }
    }
}

/// A name split at its trailing bracket: `SKU-0644 (CTN x10 box)` becomes
/// `SKU-0644` and `CTN x10 box`. `None` when the name does not end in one.
pub fn bracket(name: &str) -> Option<(&str, &str)> {
    let inner = name.trim().strip_suffix(')')?;
    let open = inner.rfind('(')?;
    let base = inner[..open].trim();
    if base.is_empty() {
        return None;
    }
    Some((base, inner[open + 1..].trim()))
}

/// Words, numbers and anything else, split where one becomes another, so
/// `x10`, `20x` and `10pack` read the same as `x 10`, `20 x` and `10 pack`.
fn words(said: &str) -> Vec<String> {
    #[derive(PartialEq)]
    enum Class {
        Digit,
        Letter,
        Other,
    }
    let class = |c: char| {
        if c.is_ascii_digit() {
            Class::Digit
        } else if c.is_alphabetic() {
            Class::Letter
        } else {
            Class::Other
        }
    };
    let mut out: Vec<String> = vec![];
    let mut last: Option<Class> = None;
    for c in said.to_lowercase().chars() {
        if c.is_whitespace() || c == ',' {
            last = None;
            continue;
        }
        let k = class(c);
        match out.last_mut() {
            Some(w) if last.as_ref() == Some(&k) => w.push(c),
            _ => out.push(c.to_string()),
        }
        last = Some(k);
    }
    out
}

fn says_carton(said: &str) -> bool {
    words(said)
        .iter()
        .any(|w| matches!(w.as_str(), "ctn" | "ctns" | "carton" | "cartons"))
}

/// What a bracket says about the carton, or `None` when it says something this
/// does not read. Case and wording vary because a person typed each one.
///
/// The shapes read, each of which says one thing:
///
/// - `6 UNITS`, `x12`, `20x`: units in the carton.
/// - `x6 box`, `4 x box`: inners in the carton, and nothing about what is in
///   them.
/// - `x8, box 40`, `20 x box 12`, `4x 10pack`: inners, and units in each.
/// - `x10 box, 1000pcs`: inners, and units in the whole carton. Read only when
///   the total divides evenly by the inners, because otherwise one of the two
///   numbers is wrong and there is no telling which.
/// - `CTN` alone: a carton, with nothing said about what is in it.
///
/// Anything else is not read: a fraction, a second product, a word this does
/// not know. `1 ROLL` is one of those, because a roll might be the unit or
/// might hold many of them.
pub fn contents(said: &str) -> Option<Pack> {
    #[derive(Clone, Copy)]
    enum T {
        N(u32),
        Inner,
        Unit,
    }
    let mut carton = false;
    let mut ts = vec![];
    for w in words(said) {
        match w.as_str() {
            "ctn" | "ctns" | "carton" | "cartons" => carton = true,
            "x" => {}
            "box" | "boxes" | "inner" | "inners" | "pack" | "packs" => ts.push(T::Inner),
            "unit" | "units" | "pcs" | "pc" | "each" | "ea" => ts.push(T::Unit),
            w => match w.parse::<u32>() {
                Ok(n) if n > 0 => ts.push(T::N(n)),
                _ => return None,
            },
        }
    }
    let pack = |inners: Option<u32>, units: Option<u32>| {
        Some(Pack {
            inners_per_carton: inners,
            units_per_inner: units,
        })
    };
    use T::*;
    match ts.as_slice() {
        [] if carton => pack(None, None),
        [N(n)] | [N(n), Unit] => pack(Some(*n), Some(1)),
        [N(n), Inner] => pack(Some(*n), None),
        [N(n), Inner, N(u)] | [N(n), N(u), Inner] => pack(Some(*n), Some(*u)),
        [N(n), Inner, N(total), Unit] if total % n == 0 => pack(Some(*n), Some(total / n)),
        _ => None,
    }
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
    /// What is in the carton, when the name said and this could read it.
    pub pack: Option<Pack>,
    /// The name's bracket, when it had one this could not read.
    pub unread: Option<String>,
    /// Why this row was read the way it was.
    pub because: String,
}

pub fn decide(name: &str, catalogue: &Catalogue) -> Decision {
    let name = name.trim();
    let whole = || {
        let (subject, because) = resolve(name, catalogue);
        Decision {
            subject,
            pack: None,
            unread: None,
            because,
        }
    };

    // A code with a bracket in it is still that code.
    if catalogue.lookup(name) != Known::Nothing {
        return whole();
    }
    let Some((base, said)) = bracket(name) else {
        return whole();
    };

    let pack = contents(said);
    let unread = pack.is_none().then(|| said.to_string());
    let reading = match pack {
        Some(p) => format!("the name states {p}"),
        None => format!(
            "the name's ({said}) is not a count this reads, so what is in the carton is left \
             unrecorded"
        ),
    };
    match resolve(base, catalogue) {
        // `Extra Small Box (1/2)`: nothing in the bracket says product.
        (Subject::Container { .. }, _) if pack.is_none() && !says_carton(said) => whole(),
        (Subject::Container { .. }, _) => Decision {
            subject: Subject::Unresolved {
                name: base.to_string(),
            },
            pack,
            unread,
            because: format!(
                "the name says what is in it, so it is a product's carton and not a box, and \
                 the catalogue has never heard of {base}; {reading}"
            ),
        },
        (subject, because) => Decision {
            subject,
            pack,
            unread,
            because: format!("{because}; {reading}"),
        },
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

    fn pack(inners: u32, units: Option<u32>) -> Option<Pack> {
        Some(Pack {
            inners_per_carton: Some(inners),
            units_per_inner: units,
        })
    }

    #[test]
    fn the_case_pack_is_read_out_of_the_name() {
        let read = |name: &str| bracket(name).and_then(|(_, said)| contents(said));
        assert_eq!(bracket("SKU-6013 (6 UNITS)"), Some(("SKU-6013", "6 UNITS")));
        assert_eq!(read("SKU-6013 (6 UNITS)"), pack(6, Some(1)));
        assert_eq!(read("SKU-0700 (X6)"), pack(6, Some(1)));
        assert_eq!(read("WAA-001 (x12)"), pack(12, Some(1)));
        assert_eq!(read("SKU-2105 (12 units)"), pack(12, Some(1)));
        assert_eq!(read("Trail Boot (x5)"), pack(5, Some(1)));
    }

    /// The shorthand of the 313-row export: a carton marker, inners, and units
    /// in each inner or in the whole carton.
    #[test]
    fn inners_are_read_out_of_the_name() {
        assert_eq!(contents("CTN, x25"), pack(25, Some(1)));
        assert_eq!(contents("CTN, 20x"), pack(20, Some(1)));
        assert_eq!(contents("CTN x6"), pack(6, Some(1)));
        assert_eq!(contents("CTN, x6 box"), pack(6, None));
        assert_eq!(contents("CTN, 4 x box"), pack(4, None));
        assert_eq!(contents("CTN x8, box 40"), pack(8, Some(40)));
        assert_eq!(contents("CTN 20 x box 12"), pack(20, Some(12)));
        assert_eq!(contents("CTN, 4x 10pack"), pack(4, Some(10)));
        assert_eq!(contents("CTN x10 box, 1000pcs"), pack(10, Some(100)));
        assert_eq!(
            contents("CTN"),
            Some(Pack::default()),
            "a carton, and nothing said about what is in it"
        );
    }

    /// A bracket that is not a count must not be read as one.
    #[test]
    fn a_description_in_brackets_is_not_a_case_pack() {
        assert_eq!(contents("1/2"), None, "1/2 is not a count");
        assert_eq!(contents("blue"), None);
        assert_eq!(contents("1 ROLL"), None, "a roll may be the unit or hold many");
        assert_eq!(contents("CTN, 10x each + a free gift"), None, "a second thing in the box");
        assert_eq!(
            contents("CTN x7 box, 1000pcs"),
            None,
            "1000 does not divide into 7 boxes, so one of the numbers is wrong"
        );
        assert_eq!(contents(""), None);
        assert_eq!(bracket("Plain name"), None);
        assert_eq!(bracket("Trailing ("), None);
        assert_eq!(bracket("(x6)"), None, "a bracket with nothing before it names nothing");
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
        assert_eq!(d.pack, pack(6, Some(1)));
        assert!(d.because.contains("6 per carton"));
    }

    /// **The one that would have invented seventeen box types.** Read whole,
    /// `SKU-0644 (CTN x10 box, 1000pcs)` is not shaped like a code, and a name
    /// that is neither a code nor in the catalogue is a container.
    #[test]
    fn a_product_carton_is_never_a_box() {
        let c = Catalogue::new(["SKU-0644", "SKU-9600"].into_iter().map(str::to_string));
        let d = decide("SKU-0644 (CTN x10 box, 1000pcs)", &c);
        assert_eq!(d.subject, Subject::Item { code: "SKU-0644".into() });
        assert_eq!(d.pack, pack(10, Some(100)));
        assert!(d.because.contains("10 inners of 100, 1000 per carton"), "{}", d.because);

        let d = decide("SKU-9600 (CTN)", &c);
        assert_eq!(d.subject, Subject::Item { code: "SKU-9600".into() });
        assert_eq!(d.pack, Some(Pack::default()));

        // Unknown to the catalogue, and still not a box.
        assert_eq!(
            decide("SKU-1015 (1 ROLL)", &c).subject,
            Subject::Unresolved { name: "SKU-1015".into() }
        );
        for name in ["Trail Boot (x5)", "Trail Boot (CTN, 10x each + a free gift)"] {
            let d = decide(name, &c);
            assert_eq!(
                d.subject,
                Subject::Unresolved { name: "Trail Boot".into() },
                "{name} states what is in it"
            );
        }
    }

    /// What was read and what was not, for the dry run to show.
    #[test]
    fn an_unread_bracket_is_kept_for_a_person() {
        let c = Catalogue::new(["SKU-1015"].into_iter().map(str::to_string));
        let d = decide("SKU-1015 (1 ROLL)", &c);
        assert_eq!(d.subject, Subject::Item { code: "SKU-1015".into() });
        assert_eq!(d.pack, None);
        assert_eq!(d.unread.as_deref(), Some("1 ROLL"));
        assert!(d.because.contains("left unrecorded"), "{}", d.because);
    }

    #[test]
    fn a_box_with_a_note_is_still_a_box() {
        let c = catalogue();
        assert_eq!(
            decide("Extra Small Box (1/2)", &c).subject,
            Subject::Container { name: "Extra Small Box (1/2)".into() },
            "1/2 says nothing about contents, so the name is read whole"
        );
        // A code with a bracket in it is still that code.
        let c = Catalogue::new(["KIT-0100 (A)"].into_iter().map(str::to_string));
        assert_eq!(
            decide("KIT-0100 (A)", &c).subject,
            Subject::Item { code: "KIT-0100 (A)".into() }
        );
    }

    /// The colour letter, and a family two levels deep.
    #[test]
    fn a_colour_letter_is_a_variant() {
        assert_eq!(style_of("SKU-9600B").as_deref(), Some("SKU-9600"));
        assert_eq!(
            style_of("U.AB-204V").as_deref(),
            Some("U.AB-204"),
            "the longer head is nearer"
        );
        assert_eq!(style_of("U.XQZ41B").as_deref(), Some("U.XQZ41"));
        assert_eq!(style_of("SKU-9350BOX"), None, "a word is not a colour");
        assert_eq!(style_of("SKU10207B"), None, "no separator, so nothing shaped like a code");
        assert_eq!(styles_of("SKU-3010B-06"), vec!["SKU-3010B", "SKU-3010"]);

        let c = Catalogue::new(
            [
                "SKU-9600B", "SKU-9600G", "SKU-9600R",
                "SKU-3010B-06", "SKU-3010B-07", "SKU-3010W-06",
                "SKU-7002B",
            ]
            .into_iter()
            .map(str::to_string),
        );
        assert_eq!(c.lookup("SKU-9600"), Known::Style(3));
        assert_eq!(c.lookup("SKU-3010"), Known::Style(3), "every size of every colour");
        assert_eq!(c.lookup("SKU-3010B"), Known::Style(2));
        assert_eq!(c.lookup("SKU-7002"), Known::Style(1), "one colour is still a colour of a family");
        assert_eq!(c.starting_with("SKU-3010B"), vec!["SKU-3010B-06", "SKU-3010B-07"]);
        assert_eq!(
            decide("SKU-9600 (CTN)", &c).subject,
            Subject::Style { code: "SKU-9600".into(), variants: 3 }
        );
    }
}
