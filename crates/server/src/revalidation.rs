//! Which measurements have gone stale, and which of those are worth a trip to
//! the scale.
//!
//! # The problem this answers
//!
//! NetSuite holds one weight in one field, so a figure typed off a supplier's
//! sheet in 2019 and a figure read off a dock scale this morning are the same
//! kind of thing and neither can be told from the other. This model already
//! distinguishes them — `observation_event.method` is `transcribed` for one and
//! `instrument` for the other, and `observation_precedence_policy` already
//! decides which wins.
//!
//! What nothing says is **how long a value stays believable**. That is what this
//! computes.
//!
//! # Derived, never stored
//!
//! `observation_current` carries `observed_at` and `method`. Age and trust are
//! both functions of those two columns, so a `needs_confirming` flag would be a
//! third thing that can disagree with the two behind it — S44's shape exactly.
//! Nothing here is written down; it is worked out on read.
//!
//! # Two classes, not seven
//!
//! `observation_method` has seven members and they do not need seven intervals.
//! The question that matters is **did anybody put this on a scale**, which is
//! two answers. A per-method table would be an expression language in a policy,
//! which D22 refuses everywhere else.

use chrono::{DateTime, Duration, Utc};

/// How a value came to be, reduced to the distinction that governs ageing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// An instrument or a scanner produced it. Ages slowly: a thing's weight
    /// does not drift, so what expires is our confidence that the thing is
    /// still the same thing.
    Measured,
    /// A person or a document asserted it — keyed, transcribed, estimated,
    /// derived, or claimed by a counterparty. Ages fast, because the reason to
    /// doubt it was there on the day it was written.
    Stated,
}

/// The methods that mean an instrument produced the value.
///
/// **One definition, used twice.** The classifier below reads it, and so does
/// the count behind the nav badge — which runs in SQL and would otherwise hold a
/// second copy of the same list, free to drift from this one.
pub const MEASURED_METHODS: [&str; 2] = ["instrument", "scan"];

pub fn trust_of(method: &str) -> Trust {
    if MEASURED_METHODS.contains(&method.trim()) {
        Trust::Measured
    } else {
        Trust::Stated
    }
}

/// Until the policy carries them, these are the intervals.
///
/// A year for something weighed, a quarter for something asserted. Both are
/// placeholders with a defensible shape rather than researched numbers, and the
/// point of the read is to find out whether they are anywhere near right.
pub const MEASURED_DAYS: i64 = 365;
pub const STATED_DAYS: i64 = 90;

pub fn interval(trust: Trust) -> Duration {
    Duration::days(match trust {
        Trust::Measured => MEASURED_DAYS,
        Trust::Stated => STATED_DAYS,
    })
}

/// **Whether a value has ever been confirmed by an instrument.**
///
/// This turns out to matter more than age, and the read is what showed it: 115
/// of 116 weights on file are `transcribed`, and their `observed_at` is the
/// moment the importer ran, not the day anybody established the figure. The
/// spreadsheet does not record when its numbers were written and neither does
/// NetSuite, so the age of a stated value is not merely old — it is unknown.
///
/// So there are two lists, not one. A value nobody has measured needs a first
/// weighing whatever its apparent age; a value somebody did measure needs
/// re-weighing when it gets old. Running both through one interval would have
/// reported nothing due, which is what happened.
pub fn ever_measured(method: &str) -> bool {
    trust_of(method) == Trust::Measured
}

/// How far past due, as a multiple of the interval.
///
/// 1.0 is exactly due, 2.0 is twice as old as it should be, below 1.0 is fine.
/// A ratio rather than a count of days so that a measured value and a stated one
/// can be ranked against each other without the units fighting.
pub fn staleness(observed_at: DateTime<Utc>, method: &str, now: DateTime<Utc>) -> f64 {
    let age = now - observed_at;
    let due = interval(trust_of(method));
    if due.num_seconds() <= 0 {
        return 0.0;
    }
    age.num_seconds() as f64 / due.num_seconds() as f64
}

/// What to walk to first.
///
/// **Staleness alone would send somebody to weigh things nobody sells.** A value
/// that is wrong matters in proportion to how often it is used, so this is the
/// same argument ABC analysis makes about cycle counting, applied to weights
/// instead of quantities.
///
/// `demand` is how many order lines the item appears on. The `+ 1` keeps an item
/// nobody has ordered on the list rather than at zero for ever, because the
/// first order for a thing is exactly when a stale weight starts costing money.
pub fn priority(staleness: f64, demand: i64) -> f64 {
    if staleness < 1.0 {
        return 0.0;
    }
    staleness * ((demand.max(0) + 1) as f64).sqrt()
}

/// How far apart two weights have to be before a person should look.
///
/// **A fraction and a floor, because neither works alone.** Ten per cent of a
/// 200 g glove is 20 g, which a bench scale can argue about; ten per cent of a
/// 300 kg pallet is 30 kg, which nothing should. So the fraction catches the
/// large and the floor stops the small from crying wolf.
pub const MATERIAL_FRACTION: f64 = 0.10;
/// Five grams. Below this, two honest weighings of the same thing routinely
/// differ — packaging, tape, a damp box — and reporting it teaches people to
/// dismiss findings.
pub const MATERIAL_FLOOR_G: i64 = 5;

/// Whether a fresh reading disagrees with what was held.
///
/// Both in grams, which is canonical. Returns false when there was nothing held:
/// a first weighing cannot contradict anything, it can only establish it.
pub fn materially_differs(previous_g: i64, recorded_g: i64) -> bool {
    let gap = (previous_g - recorded_g).abs();
    if gap <= MATERIAL_FLOOR_G {
        return false;
    }
    let base = previous_g.max(1) as f64;
    gap as f64 / base > MATERIAL_FRACTION
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(days: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_770_000_000, 0).unwrap() - Duration::days(days)
    }
    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_770_000_000, 0).unwrap()
    }

    #[test]
    fn a_scale_is_trusted_and_a_spreadsheet_is_not() {
        assert_eq!(trust_of("instrument"), Trust::Measured);
        assert_eq!(trust_of("scan"), Trust::Measured);
        for stated in ["transcribed", "keyed", "estimated", "derived", "asserted"] {
            assert_eq!(trust_of(stated), Trust::Stated, "{stated}");
        }
        // An unknown method is not trusted. A vocabulary that grows must not
        // silently grant a year of confidence to something nobody classified.
        assert_eq!(trust_of("something-new"), Trust::Stated);
    }

    /// The list and the classifier cannot drift, because there is one list.
    #[test]
    fn the_measured_methods_are_the_ones_the_classifier_trusts() {
        for m in MEASURED_METHODS {
            assert_eq!(trust_of(m), Trust::Measured, "{m}");
        }
    }

    #[test]
    fn staleness_is_measured_against_its_own_interval() {
        // 100 days: fine for a weighing, well past due for an assertion.
        assert!(staleness(t(100), "instrument", now()) < 1.0);
        assert!(staleness(t(100), "transcribed", now()) > 1.0);
        // Exactly at the line.
        assert!((staleness(t(90), "transcribed", now()) - 1.0).abs() < 0.01);
        assert!((staleness(t(365), "instrument", now()) - 1.0).abs() < 0.01);
    }

    /// The distinction the first version missed, and the data caught.
    #[test]
    fn a_transcribed_value_has_never_been_measured_however_fresh_it_looks() {
        assert!(!ever_measured("transcribed"));
        assert!(ever_measured("instrument"));
        // Freshly imported and never confirmed are different things, and only
        // the second is a fact about the value rather than about the import.
        assert!(!ever_measured("keyed"));
    }

    #[test]
    fn nothing_under_due_gets_a_priority() {
        assert_eq!(priority(0.9, 1000), 0.0, "not due is not urgent, however popular");
        assert!(priority(1.1, 0) > 0.0);
    }

    #[test]
    fn a_disagreement_needs_to_be_both_large_and_relative() {
        // 200 g held, 260 g weighed: 30% out, and worth a look.
        assert!(materially_differs(200, 260));
        // 300 kg held, 303 kg weighed: 1% out, and not.
        assert!(!materially_differs(300_000, 303_000));
        // 20 g held, 24 g weighed: 20% out, but four grams. The floor stops it.
        assert!(!materially_differs(20, 24));
        // 20 g held, 40 g weighed: past both, so it fires.
        assert!(materially_differs(20, 40));
        // Symmetric: lighter than held is as interesting as heavier.
        assert!(materially_differs(260, 200));
        assert!(!materially_differs(100, 100));
    }

    /// The ordering the read exists to produce.
    #[test]
    fn what_sells_is_weighed_before_what_does_not() {
        let busy = priority(2.0, 100);
        let idle = priority(2.0, 0);
        assert!(busy > idle);
        // But age still counts: something ancient and unsold beats something
        // barely due and popular.
        assert!(priority(20.0, 0) > priority(1.01, 50));
    }
}
