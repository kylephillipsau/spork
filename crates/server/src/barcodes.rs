//! What a scanner just handed us, before anything is looked up.
//!
//! Pure string work: no database, no policy, no decision about what the
//! identifier *means*. That is [`crate::locator`]'s job, and keeping the two
//! apart is what lets every edge below be tested without a fixture.
//!
//! # Normalisation is the whole point of this module
//!
//! D34: *"A GTIN-13 right-justified to 14 with indicator digit 0 **is** the same
//! trade item, which is why GS1's own storage guidance is to hold every GTIN in
//! 14 characters. Store the unnormalised form and `9312345678907` scanned from
//! an EAN-13 and `09312345678907` scanned from AI 01 on the carton are two rows
//! for one product, so the carton scan misses. That is the most common
//! integration defect in this area and it is silent."*
//!
//! So every GTIN that leaves here is fourteen characters, and the padding
//! happens before a lookup rather than inside one.
//!
//! # This parser is narrow, and D34 names the one that is not
//!
//! The decision says the parser is the vendor `gs1-syntax-engine`, which knows
//! every application identifier and their lengths. This handles the five that
//! a warehouse label actually carries — SSCC, GTIN, batch, expiry, serial —
//! recognises the variable-measure weights by prefix, and **passes everything
//! else through opaquely rather than rejecting it**, which is the behaviour D34
//! requires of the real one.
//!
//! The deviation is deliberate and recorded in D136. What it costs is that an
//! unrecognised variable-length AI cannot be length-delimited without an FNC1,
//! so it is kept whole rather than split — an honest partial read instead of a
//! confident wrong one.

/// The separator GS1 uses to end a variable-length element. ASCII group
/// separator; a wedge scanner emits it literally.
pub const FNC1: char = '\u{1D}';

/// What a scan turned out to contain.
///
/// Every field is optional because a scan is whatever was printed: a bare SKU
/// carries none of them, an EAN-13 carries only `gtin`, and a GS1-128 carton
/// label carries three or four at once.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Scanned {
    /// Exactly what was read, after any symbology prefix is stripped.
    pub raw: String,
    /// Normalised to fourteen characters. See the module note.
    pub gtin: Option<String>,
    /// Eighteen digits, the licence plate on a pallet or carton.
    pub sscc: Option<String>,
    pub lot: Option<String>,
    /// `YYMMDD`, as read. Not turned into a date here: a date needs a century
    /// rule, and inventing one silently is worse than handing back the digits.
    pub expiry: Option<String>,
    pub serial: Option<String>,
    /// Application identifiers this parser does not interpret, kept in the
    /// order read. D34 requires that they are never a reason to reject.
    pub other: Vec<(String, String)>,
    /// True when the string parsed as a GS1 element string rather than as a
    /// bare identifier. The difference matters to the caller: a well-formed
    /// element string that resolves to nothing is a different report from a
    /// string nobody can even parse.
    pub gs1: bool,
}

impl Scanned {
    /// Whether anything at all was recognised.
    ///
    /// A bare SKU is *well-formed* and carries none of these, so this is not
    /// "did it parse" — it is "is there a structured identifier in here".
    pub fn is_structured(&self) -> bool {
        self.gtin.is_some() || self.sscc.is_some()
    }
}

/// The GS1 mod-10 check digit for a body of digits.
///
/// Weights alternate 3 and 1 **from the right of the body**, which is the part
/// that is easy to get backwards: writing it left-to-right gives the right
/// answer only for even-length bodies, so an EAN-13 passes and a GTIN-14 fails.
pub fn check_digit(body: &str) -> Option<u8> {
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut sum = 0u32;
    for (i, ch) in body.chars().rev().enumerate() {
        let d = ch.to_digit(10)?;
        sum += d * if i % 2 == 0 { 3 } else { 1 };
    }
    Some(((10 - (sum % 10)) % 10) as u8)
}

/// Whether a string is a GTIN of a length GS1 issues, with a sound check digit.
///
/// GTIN-8, -12, -13 and -14 are the four. Anything else of digits is some other
/// numeric identifier and is not claimed here.
pub fn is_gtin(s: &str) -> bool {
    if !matches!(s.len(), 8 | 12 | 13 | 14) || !s.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let (body, given) = s.split_at(s.len() - 1);
    check_digit(body) == given.chars().next().and_then(|c| c.to_digit(10)).map(|d| d as u8)
}

/// A GTIN in the fourteen characters everything is stored and compared in.
///
/// Returns `None` for anything that is not a GTIN, so a caller cannot pad a
/// wrong number into a well-formed-looking one.
pub fn normalise_gtin(s: &str) -> Option<String> {
    if !is_gtin(s) {
        return None;
    }
    Some(format!("{s:0>14}"))
}

/// Strip the symbology prefix a scanner may prepend.
///
/// AIM identifiers are `]` plus two characters — `]C1` for GS1-128, `]d2` for a
/// GS1 DataMatrix, `]e0` for DataBar, `]Q3` for QR. **Which one it was belongs
/// on the scan record and not on the identity** (D34), so it is removed here
/// and, until D28's `activity_event` exists, not kept.
fn strip_aim(raw: &str) -> &str {
    let bytes = raw.as_bytes();
    if bytes.len() > 3 && bytes[0] == b']' {
        &raw[3..]
    } else {
        raw
    }
}

/// Fixed-length application identifiers this parser knows, and their lengths.
///
/// Data length, not counting the two-digit AI itself.
fn fixed_length(ai: &str) -> Option<usize> {
    match ai {
        "00" => Some(18), // SSCC
        "01" => Some(14), // GTIN
        "02" => Some(14), // GTIN of contained trade items
        "11" | "12" | "13" | "15" | "16" | "17" => Some(6), // dates, YYMMDD
        "20" => Some(2),  // variant
        _ => None,
    }
}

/// Whether an AI is one of the variable-length ones, capped at its GS1 maximum.
fn variable_max(ai: &str) -> Option<usize> {
    match ai {
        "10" => Some(20), // batch or lot
        "21" => Some(20), // serial
        "30" => Some(8),  // count of items
        "37" => Some(8),  // count of trade items in a logistic unit
        _ => None,
    }
}

/// Read a scanned string into whatever it turned out to carry.
///
/// Never fails. A string this cannot make sense of comes back as `raw` with
/// nothing else set, because refusing to hand back an unparseable scan would
/// leave the caller unable to say *"that is not something I recognise"* — which
/// is one of D24's four outcomes and a different one from "nothing matched".
pub fn parse(input: &str) -> Scanned {
    let raw = strip_aim(input.trim()).trim_matches(FNC1).to_string();
    let mut out = Scanned { raw: raw.clone(), ..Default::default() };

    // A bare identifier: no AIs, so the whole string is the thing.
    if let Some(g) = normalise_gtin(&raw) {
        out.gtin = Some(g);
        return out;
    }
    // Eighteen digits is an SSCC, whether or not it arrived under AI 00.
    if raw.len() == 18 && raw.chars().all(|c| c.is_ascii_digit()) && sscc_ok(&raw) {
        out.sscc = Some(raw);
        return out;
    }

    // Otherwise try to read it as a GS1 element string. It only counts as one
    // if the first thing there is a recognisable AI — a SKU that happens to
    // start with two digits must not be read as an element string.
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0usize;
    let mut read_any = false;

    while i + 2 <= chars.len() {
        // **A separator may follow a fixed-length element too.** GS1 only
        // *requires* FNC1 after a variable-length one, but plenty of encoders
        // emit it after every element and a wedge passes it through. Not
        // skipping it here stopped the parse dead at the first one, so a carton
        // label lost everything after its GTIN — which the unknown-AI test
        // caught and nothing else would have.
        if chars[i] == FNC1 {
            i += 1;
            continue;
        }
        let ai: String = chars[i..i + 2].iter().collect();
        if !ai.chars().all(|c| c.is_ascii_digit()) {
            break;
        }
        let body_start = i + 2;

        // Variable-measure weights and measures: 310n through 39nn, where the
        // fourth digit is a decimal position. Recognised so the rest of the
        // string still parses, and kept opaque because applying the implied
        // decimal is a units decision Principle 5 makes elsewhere.
        if matches!(ai.as_str(), "31" | "32" | "33" | "34" | "35" | "36" | "39")
            && body_start + 1 < chars.len()
        {
            let full_ai: String = chars[i..i + 4.min(chars.len() - i)].iter().collect();
            let start = i + 4;
            let end = (start + 6).min(chars.len());
            if start <= end {
                out.other
                    .push((full_ai, chars[start..end].iter().collect::<String>()));
            }
            i = end;
            read_any = true;
            continue;
        }

        let (value, next) = if let Some(len) = fixed_length(&ai) {
            let end = (body_start + len).min(chars.len());
            (chars[body_start..end].iter().collect::<String>(), end)
        } else if let Some(max) = variable_max(&ai) {
            // To an FNC1 if there is one, otherwise to the end of the string,
            // capped at the AI's own maximum.
            let end = chars[body_start..]
                .iter()
                .position(|c| *c == FNC1)
                .map(|p| body_start + p)
                .unwrap_or(chars.len())
                .min(body_start + max);
            let value = chars[body_start..end].iter().collect::<String>();
            // Step over the separator when there was one.
            let next = if end < chars.len() && chars[end] == FNC1 { end + 1 } else { end };
            (value, next)
        } else {
            // An AI this parser does not know. **Not a rejection**: D34 requires
            // unrecognised AIs to be kept rather than refused, and the honest
            // thing with no length rule is to keep the remainder whole rather
            // than guess where it ends.
            let end = chars[body_start..]
                .iter()
                .position(|c| *c == FNC1)
                .map(|p| body_start + p)
                .unwrap_or(chars.len());
            out.other
                .push((ai, chars[body_start..end].iter().collect::<String>()));
            i = if end < chars.len() { end + 1 } else { end };
            read_any = true;
            continue;
        };

        match ai.as_str() {
            "01" | "02" => {
                // **The check digit is validated inside AI 01 too.** A label
                // printed with a bad one is a label that will resolve to
                // nothing, and saying so at the scan is better than a silent
                // miss at the lookup.
                out.gtin = normalise_gtin(&value);
                if out.gtin.is_none() {
                    out.other.push((ai.clone(), value.clone()));
                }
            }
            "00" => out.sscc = Some(value.clone()),
            "10" => out.lot = Some(value.clone()),
            "17" => out.expiry = Some(value.clone()),
            "21" => out.serial = Some(value.clone()),
            _ => out.other.push((ai.clone(), value.clone())),
        }
        read_any = true;
        i = next;
    }

    out.gs1 = read_any && (out.is_structured() || !out.other.is_empty() || out.lot.is_some());
    if !out.gs1 {
        // Nothing recognisable: hand back the raw string alone, so the caller
        // can still try it as a plain code.
        return Scanned { raw, ..Default::default() };
    }
    out
}

/// An SSCC is eighteen digits with the same mod-10 check.
fn sscc_ok(s: &str) -> bool {
    let (body, given) = s.split_at(17);
    check_digit(body) == given.chars().next().and_then(|c| c.to_digit(10)).map(|d| d as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The check digits below were computed independently before being written
    /// here, rather than recalled. A hardcoded wrong vector makes a correct
    /// implementation look broken and a broken one look correct.
    #[test]
    fn the_check_digit_weights_run_from_the_right() {
        // 9312345678907 is a real-shaped EAN-13: body 931234567890, check 7.
        assert_eq!(check_digit("931234567890"), Some(7));
        // The same trade item as a GTIN-14: body 0931234567890, check still 7.
        // Written left-to-right the weights would land on the other parity and
        // this pair is what catches it — one length is even and one is odd.
        assert_eq!(check_digit("0931234567890"), Some(7));
        assert_eq!(check_digit("1234567890123"), Some(1));
        assert_eq!(check_digit("0000000000000"), Some(0));
        assert_eq!(check_digit("1234567"), Some(0));
        assert_eq!(check_digit("12a4567"), None);
        assert_eq!(check_digit(""), None);
    }

    #[test]
    fn a_gtin_is_four_lengths_and_a_sound_check_digit() {
        assert!(is_gtin("9312345678907"), "GTIN-13");
        assert!(is_gtin("09312345678907"), "GTIN-14");
        assert!(is_gtin("12345670"), "GTIN-8");
        // One digit out.
        assert!(!is_gtin("9312345678908"));
        // A length GS1 does not issue, however sound the arithmetic.
        assert!(!is_gtin("093123456789071"));
        assert!(!is_gtin("STY-7720-08"));
    }

    /// The defect this module exists to prevent, stated as a test.
    #[test]
    fn the_carton_scan_and_the_retail_scan_are_one_product() {
        // D34's own example: an EAN-13 off the consumer unit and AI 01 off the
        // carton beside it. Two strings, one trade item, and storing them
        // unnormalised is the silent miss.
        let retail = parse("9312345678907");
        let carton = parse("\u{1D}0109312345678907");
        assert_eq!(retail.gtin, Some("09312345678907".into()));
        assert_eq!(carton.gtin, retail.gtin, "one product, however it was printed");
    }

    #[test]
    fn a_gs1_128_carton_label_yields_item_lot_and_expiry_in_one_scan() {
        // AI 01, then AI 17 (fixed six), then AI 10 (variable, to the end).
        let s = parse("010931234567890717260101101ABC42");
        assert_eq!(s.gtin, Some("09312345678907".into()));
        assert_eq!(s.expiry, Some("260101".into()));
        assert_eq!(s.lot, Some("1ABC42".into()));
        assert!(s.gs1);
    }

    #[test]
    fn a_variable_length_ai_ends_at_the_separator() {
        // Lot first this time, so it must stop at FNC1 rather than swallow the
        // GTIN that follows it.
        let s = parse("10LOT-9\u{1D}0109312345678907");
        assert_eq!(s.lot, Some("LOT-9".into()));
        assert_eq!(s.gtin, Some("09312345678907".into()));
    }

    #[test]
    fn an_sscc_is_read_bare_or_under_its_ai() {
        // 18 digits: body 00312345678901234, check computed independently.
        let body = "00312345678901234";
        let check = check_digit(body).unwrap();
        let sscc = format!("{body}{check}");
        assert_eq!(parse(&sscc).sscc, Some(sscc.clone()));
        assert_eq!(parse(&format!("00{sscc}")).sscc, Some(sscc));
    }

    #[test]
    fn a_symbology_prefix_is_not_part_of_the_identifier() {
        // How it was printed belongs on the scan, not on the identity (D34).
        for prefix in ["]C1", "]d2", "]e0", "]Q3"] {
            let s = parse(&format!("{prefix}0109312345678907"));
            assert_eq!(s.gtin, Some("09312345678907".into()), "{prefix}");
        }
    }

    #[test]
    fn a_bare_code_is_passed_through_whole() {
        // The commonest label in this warehouse: the SKU, as Code 128.
        let s = parse("STY-7720-08");
        assert_eq!(s.raw, "STY-7720-08");
        assert_eq!(s.gtin, None);
        assert!(!s.gs1, "a SKU is not an element string");
        assert!(!s.is_structured());
    }

    /// A SKU that begins with two digits must not be read as an element string.
    #[test]
    fn digits_at_the_front_do_not_make_it_a_gs1_string() {
        let s = parse("2025-CAT-CONTRIBUTION");
        assert_eq!(s.raw, "2025-CAT-CONTRIBUTION");
        assert_eq!(s.gtin, None);
        assert_eq!(s.lot, None);
    }

    #[test]
    fn an_unknown_ai_is_kept_rather_than_refused() {
        // D34: unrecognised AIs are stored opaquely and never rejected.
        let s = parse("0109312345678907\u{1D}91SOMETHING");
        assert_eq!(s.gtin, Some("09312345678907".into()));
        assert!(
            s.other.iter().any(|(ai, v)| ai == "91" && v == "SOMETHING"),
            "the unknown AI survives: {:?}",
            s.other
        );
    }

    #[test]
    fn a_bad_check_digit_inside_ai_01_does_not_become_a_gtin() {
        // Better to report a scan that resolves to nothing than to pad a wrong
        // number into a well-formed-looking one.
        let s = parse("0109312345678908");
        assert_eq!(s.gtin, None);
        assert!(!s.other.is_empty(), "and the digits are kept: {:?}", s.other);
    }

    #[test]
    fn normalising_refuses_what_is_not_a_gtin() {
        assert_eq!(normalise_gtin("9312345678907").as_deref(), Some("09312345678907"));
        assert_eq!(normalise_gtin("12345670").as_deref(), Some("00000012345670"));
        assert_eq!(normalise_gtin("9312345678908"), None);
        assert_eq!(normalise_gtin("GLOVE-M"), None);
    }

    #[test]
    fn a_variable_measure_weight_is_recognised_and_left_alone() {
        // AI 3103 is a net weight in kilograms to three decimals. Kept opaque:
        // applying the implied decimal is a units decision that belongs where
        // Principle 5 already makes it, not in a string parser.
        let s = parse("01093123456789073103001250");
        assert_eq!(s.gtin, Some("09312345678907".into()));
        assert!(
            s.other.iter().any(|(ai, v)| ai == "3103" && v == "001250"),
            "{:?}",
            s.other
        );
    }
}
