//! What a scanned identifier is about to be made to mean.
//!
//! The other half of [`crate::locator`]. That module answers *what does this
//! string resolve to*; this one answers *may this string be made to resolve to
//! that*, which is the question nothing could ask before D164 because there was
//! no write path to `item_barcode` at all.
//!
//! # Why this is a module and not a block in the handler
//!
//! The same split `receiving`, `observing` and `prepack` already use: deciding
//! what a proposal amounts to is a pure function of the proposal, and the thing
//! that touches the database has no judgement in it. Every rule below is
//! testable without a database, and every one of them was a rule the table's
//! CHECK constraints already held — stated here so a refusal arrives as a
//! sentence rather than as a constraint violation with a constraint name in it.
//!
//! # The rules, and where each comes from
//!
//! **A GTIN is normalised to fourteen and its check digit is arithmetic.** A
//! GTIN-13 padded with indicator 0 is the same trade item, and two rows for it
//! make the carton scan miss — migration 79's own words. `barcodes` already
//! does both and this calls it rather than repeating it.
//!
//! **Anything else is `internal`.** The third scheme, `supplier_reference`,
//! means *this party's code and nothing outside that* and therefore needs an
//! issuer party. A person at a shelf has no way to say whose code they are
//! holding, so the floor cannot mint one — a binding that claimed to be a
//! supplier's without naming the supplier is exactly the row migration 79 says
//! resolves confidently and wrongly.
//!
//! **A non-GTIN carries a count or it is refused**, because `item_barcode`'s
//! `quantity_ck` says so: only a GTIN has anywhere to put a variable measure
//! (AI 310n rides in the barcode itself). `each` supplies its own count, since
//! one scan of a single thing is one of it — that is what the level means
//! rather than a default being applied.

/// What somebody at a shelf is proposing.
#[derive(Debug, Clone)]
pub struct Proposed {
    /// Exactly what the scanner or the keyboard produced.
    pub raw: String,
    /// One of the five `packaging_level` values.
    pub packaging_level: String,
    /// Base units per scan, when the operator knows it.
    pub quantity: Option<i64>,
}

/// The five, and the vocabulary is the database's rather than this file's.
pub const LEVELS: [&str; 5] = ["each", "inner", "carton", "layer", "pallet"];

#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    Empty,
    /// Fourteen digits that do not check. Not "not a GTIN" — a mistyped or
    /// misread GTIN is the case worth naming, because the alternative reading
    /// is that it is an internal code, and binding it as one would make a
    /// typo permanent.
    CheckDigit,
    UnknownLevel(String),
    /// A count is required and there is nowhere for one to come from.
    CountMissing,
    CountNotPositive,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::Empty => write!(f, "nothing was scanned"),
            Problem::CheckDigit => write!(
                f,
                "that reads as a GTIN and its check digit does not agree: it was \
                 misread or mistyped"
            ),
            Problem::UnknownLevel(l) => write!(
                f,
                "{l} is not a packaging level; it is one of {}",
                LEVELS.join(", ")
            ),
            Problem::CountMissing => write!(
                f,
                "a code that is not a GTIN has to say how many base units one \
                 scan of it means"
            ),
            Problem::CountNotPositive => write!(f, "one scan means at least one of something"),
        }
    }
}

/// A binding, as the row would be written.
#[derive(Debug, PartialEq, Eq)]
pub struct Binding {
    pub barcode: String,
    pub scheme: &'static str,
    pub packaging_level: String,
    pub quantity: Option<i64>,
    /// Said rather than refused. A binding can be complete and still be worth
    /// a sentence.
    pub warnings: Vec<String>,
}

/// What this proposal amounts to, or why it is not a binding.
pub fn decide(p: &Proposed) -> Result<Binding, Problem> {
    let raw = p.raw.trim();
    if raw.is_empty() {
        return Err(Problem::Empty);
    }
    if !LEVELS.contains(&p.packaging_level.as_str()) {
        return Err(Problem::UnknownLevel(p.packaging_level.clone()));
    }
    if let Some(q) = p.quantity {
        if q <= 0 {
            return Err(Problem::CountNotPositive);
        }
    }

    // **Digits of a GTIN length are a GTIN or they are a mistake.** Reading a
    // failed check digit as "then it must be an internal code" is how a misread
    // scan becomes a permanent binding, which is the failure this table's whole
    // design is against.
    let looks_numeric = raw.chars().all(|c| c.is_ascii_digit());
    let gtin_length = matches!(raw.len(), 8 | 12 | 13 | 14);
    let (barcode, scheme) = if looks_numeric && gtin_length {
        match crate::barcodes::normalise_gtin(raw) {
            Some(g) => (g, "gtin"),
            None => return Err(Problem::CheckDigit),
        }
    } else {
        (raw.to_string(), "internal")
    };

    // One scan of a single thing is one of it. Not a default: that is what the
    // level means, and the count would be 1 however it was arrived at.
    let quantity = match (p.quantity, p.packaging_level.as_str()) {
        (Some(q), _) => Some(q),
        (None, "each") => Some(1),
        (None, _) if scheme == "gtin" => None,
        (None, _) => return Err(Problem::CountMissing),
    };

    let mut warnings = vec![];
    if quantity.is_none() {
        // Legal, and rarely what somebody at a shelf means. `item_barcode`
        // confines a null count to GTINs because that is where a variable
        // measure can ride in the barcode itself; a carton of gloves is not
        // one, and a scan of it will not say how many gloves.
        warnings.push(
            "this binding says nothing about how many: a scan of it will resolve the item \
             and not a quantity"
                .into(),
        );
    }

    Ok(Binding {
        barcode,
        scheme,
        packaging_level: p.packaging_level.clone(),
        quantity,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposed(raw: &str, level: &str, quantity: Option<i64>) -> Proposed {
        Proposed {
            raw: raw.into(),
            packaging_level: level.into(),
            quantity,
        }
    }

    #[test]
    fn a_gtin_is_normalised_to_fourteen() {
        // The case migration 79 names: a GTIN-13 padded with indicator 0 is the
        // same trade item, and two rows for it make the carton scan miss.
        let b = decide(&proposed("9312345678907", "each", None)).expect("a GTIN");
        assert_eq!(b.barcode.len(), 14, "{b:?}");
        assert_eq!(b.barcode, "09312345678907");
        assert_eq!(b.scheme, "gtin");
    }

    #[test]
    fn a_misread_gtin_is_a_refusal_and_not_an_internal_code() {
        // **The rule this file exists for.** Falling back to `internal` when
        // the check digit fails makes a misread scan a permanent binding, and
        // the row would look exactly like a deliberate one afterwards.
        assert_eq!(
            decide(&proposed("9312345678901", "each", None)).unwrap_err(),
            Problem::CheckDigit
        );
    }

    #[test]
    fn a_code_that_is_not_digits_is_an_internal_one() {
        let b = decide(&proposed("SPP-0020R", "each", None)).expect("an internal code");
        assert_eq!(b.scheme, "internal");
        assert_eq!(b.barcode, "SPP-0020R");
        // `each` supplies its own count, so the table's quantity CHECK is met
        // without anybody being asked a question with one answer.
        assert_eq!(b.quantity, Some(1));
    }

    #[test]
    fn an_internal_code_above_each_has_to_say_how_many() {
        // `item_barcode_quantity_ck`: only a GTIN may carry a null count.
        assert_eq!(
            decide(&proposed("CTN-0041", "carton", None)).unwrap_err(),
            Problem::CountMissing
        );
        let b = decide(&proposed("CTN-0041", "carton", Some(12))).expect("with a count");
        assert_eq!(b.quantity, Some(12));
        assert!(b.warnings.is_empty(), "{b:?}");
    }

    #[test]
    fn a_carton_gtin_with_no_count_is_allowed_and_said_out_loud() {
        let b = decide(&proposed("09312345678907", "carton", None)).expect("a carton GTIN");
        assert_eq!(b.quantity, None);
        assert_eq!(b.warnings.len(), 1, "{b:?}");
    }

    #[test]
    fn the_level_is_the_databases_vocabulary_and_not_this_files() {
        // A level the enum has never heard of is a rejection, which is the
        // check working rather than a bug to route around.
        assert_eq!(
            decide(&proposed("09312345678907", "box", None)).unwrap_err(),
            Problem::UnknownLevel("box".into())
        );
        for level in LEVELS {
            assert!(
                decide(&proposed("09312345678907", level, Some(6))).is_ok(),
                "{level} is in the enum and was refused"
            );
        }
    }

    #[test]
    fn nothing_and_nonsense_are_refused_before_anything_else() {
        assert_eq!(decide(&proposed("   ", "each", None)).unwrap_err(), Problem::Empty);
        assert_eq!(
            decide(&proposed("09312345678907", "carton", Some(0))).unwrap_err(),
            Problem::CountNotPositive
        );
    }
}
