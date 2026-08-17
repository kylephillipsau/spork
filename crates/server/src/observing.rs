//! Measuring something, and converting what was entered into what is stored.
//!
//! This is the second spine table's write path, and it arrived long after the
//! first. Architecture: *"Two tables carry everything, and both are only ever
//! added to"* — `stock_movement` and `observation`. Twenty-five endpoints wrote
//! the first and none wrote the second, which is question 173's shape one level
//! up: the schema was complete and nothing could reach it.
//!
//! # Why conversion lives here rather than in SQL
//!
//! Principle 5, stated by migration 46 while deliberately *not* building this:
//!
//! > This is not the write path. Principle 5 has the writer convert and store the
//! > canonical value, and there is no SQL conversion function for units either —
//! > `unit.factor_num/factor_den` is data the writer reads.
//!
//! So the factor is read from `unit` and applied here. `observation` has no unit
//! column beside `value_numeric`, which makes non-canonical storage
//! unrepresentable rather than merely discouraged, and S22 keeps every canonical
//! unit at factor 1/1 with no offset so "canonical" is a property of the data.
//!
//! # Why the entered value is a string
//!
//! `entered_value` is `numeric` and is the only record of what the operator
//! actually typed. A JSON number arrives as an IEEE double, and 12.1 is not a
//! double — so accepting one would round the evidence before storing it and then
//! store the rounded thing as the original. The same argument D50 makes for
//! prices, which are minor units and a basis quantity rather than a float,
//! because *"3.45 per 100 is exact as `(345, 100)` and 0.0345 each is not
//! expressible in cents at all."*
//!
//! Conversion is therefore exact integer arithmetic over the digits as written,
//! and rounds only at the last step, where the canonical column is a `bigint` and
//! something has to give: 1.5 inches is 38.1 millimetres and the column holds
//! whole ones. The entered pair survives beside it, so the rounding is
//! recoverable rather than the only record.

use uuid::Uuid;

/// A decimal as the operator wrote it: digits and where the point sits.
///
/// `12.5` is `(125, 1)`. Kept as an integer mantissa rather than a float so that
/// every conversion below is exact until the deliberate rounding at the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entered {
    pub mantissa: i128,
    pub scale: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ValueProblem {
    NotANumber { text: String },
    TooManyDigits { text: String },
    NotPositive,
    /// The unit named is not one of the metric's dimension.
    WrongDimension { unit: String, metric: String },
    /// The metric does not apply to this kind of subject (`metric.applies_to`).
    WrongSubject { metric: String, kind: String },
    /// A factor of zero would make every value zero rather than fail.
    DegenerateFactor { unit: String },
    Overflow,
    /// An absence word the column holds and a capture may not write. D138.
    UnrecordableAbsence { reason: String, because: &'static str },
    /// A single loose thing's size, with no record of how it was arranged. D138.
    ArrangementUnstated { metric: String },
}

impl std::fmt::Display for ValueProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueProblem::NotANumber { text } => {
                write!(f, "{text:?} is not a decimal number")
            }
            ValueProblem::TooManyDigits { text } => write!(
                f,
                "{text:?} has more digits than a measurement needs; this is a \
                 mistyped value rather than a precise one"
            ),
            ValueProblem::NotPositive => {
                write!(f, "a measurement must be greater than zero")
            }
            ValueProblem::WrongDimension { unit, metric } => write!(
                f,
                "unit {unit} does not measure what {metric} measures; recording a \
                 length in grams is a mistake the dimensions exist to catch"
            ),
            ValueProblem::WrongSubject { metric, kind } => {
                write!(f, "metric {metric} does not apply to a {kind}")
            }
            ValueProblem::DegenerateFactor { unit } => {
                write!(f, "unit {unit} has a zero factor and cannot convert anything")
            }
            ValueProblem::Overflow => {
                write!(f, "that value is too large to store in base units")
            }
            ValueProblem::UnrecordableAbsence { reason, because } => {
                write!(f, "{reason} is not an absence a capture records: {because}")
            }
            ValueProblem::ArrangementUnstated { metric } => write!(
                f,
                "a {metric} of a single thing needs the presentation it was measured \
                 in; an apron folded and an apron in a heap are not the same \
                 measurement, and without the word neither one can be reproduced"
            ),
        }
    }
}

/// Parse the digits the operator typed, without going through a float.
///
/// Bounded at eighteen significant digits, which is far past any real
/// measurement and short of overflowing the arithmetic below. A longer string is
/// rejected as mistyped rather than truncated, because silently keeping the
/// first eighteen digits of a wrong number is the failure this whole module is
/// arranged against.
pub fn parse_entered(text: &str) -> Result<Entered, ValueProblem> {
    let t = text.trim();
    if t.is_empty() {
        return Err(ValueProblem::NotANumber { text: text.into() });
    }
    let (negative, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };

    let mut digits = String::new();
    let mut scale = 0u32;
    let mut seen_point = false;
    for ch in rest.chars() {
        match ch {
            '0'..='9' => {
                digits.push(ch);
                if seen_point {
                    scale += 1;
                }
            }
            '.' if !seen_point => seen_point = true,
            _ => return Err(ValueProblem::NotANumber { text: text.into() }),
        }
    }
    if digits.is_empty() || digits.len() > 18 {
        return Err(if digits.is_empty() {
            ValueProblem::NotANumber { text: text.into() }
        } else {
            ValueProblem::TooManyDigits { text: text.into() }
        });
    }

    let mantissa: i128 = digits
        .parse()
        .map_err(|_| ValueProblem::TooManyDigits { text: text.into() })?;
    if mantissa == 0 {
        return Err(ValueProblem::NotPositive);
    }
    if negative {
        return Err(ValueProblem::NotPositive);
    }
    Ok(Entered { mantissa, scale })
}

/// The unit's conversion to the dimension's canonical unit, as `unit` holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Factor {
    pub num: i64,
    pub den: i64,
}

/// Convert an entered decimal to canonical base units.
///
/// `entered × num / den`, rounded half away from zero exactly once, at the end.
/// Every intermediate step is integer: a kilogram is `1000/1` so 12.5 kg is
/// 12500 g with nothing to round, and an inch is `254/10` so 1.5 in is 381/10
/// millimetres, which rounds to 38 and keeps `1.5 in` beside it.
pub fn to_canonical(entered: Entered, factor: Factor) -> Result<i64, ValueProblem> {
    if factor.num <= 0 || factor.den <= 0 {
        return Err(ValueProblem::DegenerateFactor {
            unit: format!("{}/{}", factor.num, factor.den),
        });
    }
    let scale_div = 10i128
        .checked_pow(entered.scale)
        .ok_or(ValueProblem::Overflow)?;
    let numerator = entered
        .mantissa
        .checked_mul(factor.num as i128)
        .ok_or(ValueProblem::Overflow)?;
    let denominator = (factor.den as i128)
        .checked_mul(scale_div)
        .ok_or(ValueProblem::Overflow)?;

    // Half away from zero. Everything here is positive — `parse_entered` refuses
    // a negative and a zero — so this is half up, written so it stays correct if
    // that ever changes.
    let doubled = numerator.checked_mul(2).ok_or(ValueProblem::Overflow)?;
    let rounded = (doubled + denominator) / (denominator * 2);
    if rounded <= 0 {
        return Err(ValueProblem::NotPositive);
    }
    i64::try_from(rounded).map_err(|_| ValueProblem::Overflow)
}

/// A metric as the vocabulary defines it, and the subject it is being applied to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetricSpec {
    pub id: Uuid,
    pub code: String,
    pub dimension_id: Option<Uuid>,
    /// `metric.applies_to`, the subject kinds this may be recorded against.
    pub applies_to: Vec<String>,
}

/// Refuse a measurement that does not belong to its subject or its dimension.
///
/// Both are foreign keys in the schema — `metric_id_dimension_id_fkey` catches
/// the second at write time — and neither is checked before the row is built, so
/// the caller gets a constraint name instead of a sentence. `applies_to` has no
/// constraint behind it at all: nothing stops a temperature being recorded
/// against an `asserted_unit_content`, which is a list the vocabulary keeps and
/// nothing reads.
pub fn check_applicable(
    metric: &MetricSpec,
    subject_kind: &str,
    unit_dimension_id: Uuid,
    unit_code: &str,
) -> Result<(), ValueProblem> {
    check_subject(metric, subject_kind)?;
    if metric.dimension_id != Some(unit_dimension_id) {
        return Err(ValueProblem::WrongDimension {
            unit: unit_code.to_string(),
            metric: metric.code.clone(),
        });
    }
    Ok(())
}

/// The subject half of [`check_applicable`], on its own.
///
/// **An absence has no unit**, so the dimension check has nothing to check —
/// but *this metric does not apply to this kind of thing* is exactly as true of
/// a declared absence as of a number, and declaring that a location has no
/// gross weight is no more meaningful than measuring one.
pub fn check_subject(metric: &MetricSpec, subject_kind: &str) -> Result<(), ValueProblem> {
    if !metric.applies_to.iter().any(|k| k == subject_kind) {
        return Err(ValueProblem::WrongSubject {
            metric: metric.code.clone(),
            kind: subject_kind.to_string(),
        });
    }
    Ok(())
}

/// Which of `absent_reason`'s four words a capture session may write. D138.
///
/// The column has held all four since migration 7 and two of them belong to
/// other mechanisms entirely. `not_measured` is what the absence of a row
/// already says, and writing it turns "nobody has looked" into a fact somebody
/// recorded — the exact confusion this decision exists to end. `retracted` is
/// `retracts_observation_id`'s job, and a second way to spell a retraction is a
/// second answer to *which rows count*.
///
/// So two: the thing has no such measurement, or the person could not read it.
pub fn check_absent_reason(reason: &str) -> Result<(), ValueProblem> {
    match reason {
        "not_applicable" | "unreadable" => Ok(()),
        "not_measured" => Err(ValueProblem::UnrecordableAbsence {
            reason: reason.to_string(),
            because: "a subject with no row already says nobody has measured it, and \
                      recording it as a fact makes silence indistinguishable from an answer",
        }),
        "retracted" => Err(ValueProblem::UnrecordableAbsence {
            reason: reason.to_string(),
            because: "a retraction is an observation naming the one it retracts, not a \
                      word on a fresh row",
        }),
        _ => Err(ValueProblem::UnrecordableAbsence {
            reason: reason.to_string(),
            because: "the absences are not_applicable and unreadable",
        }),
    }
}

/// Whether a figure needs the arrangement it was taken in recorded with it. D138.
///
/// **Only lengths, and only at `each`.** A carton is rigid: there is one way it
/// sits on the bench and recording that is ceremony. A single loose thing has as
/// many sizes as ways of folding it, so a length with no presentation beside it
/// is a number the next operator cannot reproduce and cannot argue with.
///
/// Weight is not on the list because arranging a thing does not change what it
/// weighs.
pub fn presentation_required(metric_code: &str, packaging_level: Option<&str>) -> bool {
    matches!(metric_code, "length" | "width" | "height") && packaging_level == Some("each")
}

/// The four metrics `package` caches, and the column each one lands in.
///
/// **J12 is why this list exists in the writer.** It asserts that an unsealed
/// package's dimension columns equal `observation_current`, compared with
/// `IS DISTINCT FROM` — so a package that carries NULL against a recorded
/// observation is a finding exactly as loudly as one carrying the wrong number.
/// Writing the observation without the cache would raise `identity_mismatch` on
/// the next rebuild, which makes the cache part of the act rather than a
/// convenience beside it.
///
/// Sealed packages are excluded, which is J12's own scope: a sealed carton's
/// dimensions are what it was despatched as, and a later measurement is evidence
/// about it rather than a correction of it.
pub fn package_dimension_column(metric_code: &str) -> Option<PackageColumn> {
    match metric_code {
        // The three lengths are `integer` on `package` and `bigint` on
        // `observation`, so the cache is narrower than the fact it caches. The
        // width is recorded here rather than discovered as a Postgres range
        // error, which reads as a driver problem rather than as a measurement
        // nobody can store.
        "length" => Some(PackageColumn { name: "length_mm", narrow: true }),
        "width" => Some(PackageColumn { name: "width_mm", narrow: true }),
        "height" => Some(PackageColumn { name: "height_mm", narrow: true }),
        "gross_weight" => Some(PackageColumn { name: "gross_weight_g", narrow: false }),
        _ => None,
    }
}

/// A cached dimension column, and whether it is the narrow `integer` kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackageColumn {
    pub name: &'static str,
    pub narrow: bool,
}

impl PackageColumn {
    /// Refuse a value the column cannot hold, before Postgres does.
    ///
    /// Two point one billion millimetres is two thousand kilometres, so this
    /// never fires on a pallet — which is the argument for checking rather than
    /// for widening the column. What it stops is a mis-entered unit turning into
    /// a range error from the driver.
    pub fn fits(&self, canonical: i64) -> bool {
        !self.narrow || i32::try_from(canonical).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn digits_are_kept_as_written() {
        assert_eq!(parse_entered("12.5").unwrap(), Entered { mantissa: 125, scale: 1 });
        assert_eq!(parse_entered("1200").unwrap(), Entered { mantissa: 1200, scale: 0 });
        assert_eq!(parse_entered(" 0.750 ").unwrap(), Entered { mantissa: 750, scale: 3 });
    }

    #[test]
    fn a_kilogram_is_exactly_a_thousand_grams() {
        // The case the whole string-not-float argument is about: 12.1 is not a
        // double, and this has to be 12100 rather than 12099.999...
        let e = parse_entered("12.1").unwrap();
        assert_eq!(to_canonical(e, Factor { num: 1000, den: 1 }).unwrap(), 12100);
    }

    #[test]
    fn a_canonical_unit_changes_nothing() {
        // S22: every canonical unit is factor 1/1 with no offset, so this is the
        // identity and not an approximation of one.
        let e = parse_entered("845").unwrap();
        assert_eq!(to_canonical(e, Factor { num: 1, den: 1 }).unwrap(), 845);
    }

    #[test]
    fn an_inch_rounds_once_and_keeps_what_was_typed() {
        // 1.5 in = 38.1 mm. The column is whole millimetres, so this rounds --
        // and `entered_value` still says 1.5 with `in` beside it, which is the
        // half Principle 5 exists to preserve.
        let e = parse_entered("1.5").unwrap();
        assert_eq!(to_canonical(e, Factor { num: 254, den: 10 }).unwrap(), 38);
        // 2.5 in = 63.5 mm, the exact half, away from zero.
        let e = parse_entered("2.5").unwrap();
        assert_eq!(to_canonical(e, Factor { num: 254, den: 10 }).unwrap(), 64);
    }

    #[test]
    fn a_float_would_have_lost_this() {
        // 0.1 + 0.2 in doubles is 0.30000000000000004. Entered as digits it is
        // three tenths of a metre and exactly 300 millimetres.
        let e = parse_entered("0.3").unwrap();
        assert_eq!(to_canonical(e, Factor { num: 1000, den: 1 }).unwrap(), 300);
    }

    #[test]
    fn nonsense_is_refused_rather_than_coerced() {
        assert!(matches!(parse_entered("").unwrap_err(), ValueProblem::NotANumber { .. }));
        assert!(matches!(parse_entered("12kg").unwrap_err(), ValueProblem::NotANumber { .. }));
        assert!(matches!(parse_entered("1.2.3").unwrap_err(), ValueProblem::NotANumber { .. }));
        assert!(matches!(parse_entered("-5").unwrap_err(), ValueProblem::NotPositive));
        assert!(matches!(parse_entered("0.00").unwrap_err(), ValueProblem::NotPositive));
        assert!(matches!(
            parse_entered("1234567890123456789").unwrap_err(),
            ValueProblem::TooManyDigits { .. }
        ));
    }

    #[test]
    fn a_length_may_not_be_recorded_in_grams() {
        let metric = MetricSpec {
            id: u(1),
            code: "height".into(),
            dimension_id: Some(u(10)),
            applies_to: vec!["item".into(), "package".into()],
        };
        // Right dimension, right subject.
        assert!(check_applicable(&metric, "package", u(10), "mm").is_ok());
        // Mass against a length metric.
        assert!(matches!(
            check_applicable(&metric, "package", u(20), "g").unwrap_err(),
            ValueProblem::WrongDimension { .. }
        ));
        // A subject the vocabulary does not list.
        assert!(matches!(
            check_applicable(&metric, "location", u(10), "mm").unwrap_err(),
            ValueProblem::WrongSubject { .. }
        ));
    }

    #[test]
    fn the_narrow_columns_are_range_checked() {
        let h = package_dimension_column("height").unwrap();
        assert!(h.narrow, "height_mm is integer, not bigint");
        assert!(h.fits(1_450), "an ordinary pallet height fits");
        assert!(!h.fits(3_000_000_000), "and a mis-entered one does not");

        let w = package_dimension_column("gross_weight").unwrap();
        assert!(!w.narrow, "gross_weight_g is bigint");
        assert!(w.fits(3_000_000_000), "so it takes what the observation holds");
    }

    #[test]
    fn the_package_cache_covers_exactly_what_j12_watches() {
        for code in ["length", "width", "height", "gross_weight"] {
            assert!(package_dimension_column(code).is_some(), "{code} is in J12's set");
        }
        // Recorded against a package and deliberately not cached: J12 names four.
        assert_eq!(package_dimension_column("tare_weight"), None);
        assert_eq!(package_dimension_column("net_weight"), None);
        assert_eq!(package_dimension_column("temperature"), None);
    }
}
