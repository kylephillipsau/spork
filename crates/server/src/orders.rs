//! Reading a sales order export: what a line commits, and what is left of it.
//!
//! Pure, and tested without a database, for the reason [`crate::prepack`] gives:
//! the judgement is the difficulty and the I/O is not.
//!
//! # The three decisions in here
//!
//! **A date is day-first.** `15/7/2026` is the fifteenth of July. Every row in
//! the export is Australian-formatted and the ambiguous ones — `7/8/2026` —
//! cannot be told apart by looking, so this is a decision rather than a
//! detection, made once and written down.
//!
//! **A customer needs a code and the export has none.** `party.code` is NOT NULL
//! and unique per tenant, and the file carries only a display name. So the code
//! is derived from the name, deterministically, and a collision between two
//! different names is broken by a hash of the full name rather than by a
//! counter — a counter would depend on the order rows happen to arrive in, and
//! the loader would stop being idempotent.
//!
//! **What is left is not what was ordered.** 362 of 430 lines on `Pending
//! Fulfillment` orders have already been partly shipped. Loading the ordered
//! figure as work to do would put units on the bench that left the building
//! last week.

use chrono::NaiveDate;

/// `15/7/2026` -> 2026-07-15.
///
/// Day-first, always. NetSuite renders per the account's locale and this one is
/// Australian; a row like `7/8/2026` is genuinely ambiguous in the file, so the
/// convention is asserted here rather than guessed per row.
pub fn parse_date(raw: &str) -> Option<NaiveDate> {
    let mut parts = raw.trim().split(['/', '-']);
    let d: u32 = parts.next()?.trim().parse().ok()?;
    let m: u32 = parts.next()?.trim().parse().ok()?;
    let y: i32 = parts.next()?.trim().parse().ok()?;
    let y = if y < 100 { 2000 + y } else { y };
    NaiveDate::from_ymd_opt(y, m, d)
}

/// `PMFresh Pty Ltd : PMFresh Pty Ltd Colmslie` -> the whole thing.
///
/// **The parent is kept.** A sub-customer's own name is often a site or a
/// department — "Colmslie", "Head Office" — which is meaningless alone and
/// duplicated across parents. The display name is what a person reading a pack
/// docket needs, so it is what is stored.
pub fn customer_name(raw: &str) -> String {
    raw.split(" : ")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" : ")
}

/// `DUP : SKU-9454-07` -> `SKU-9454-07`. `SKU-2008` -> itself.
///
/// **The opposite of [`customer_name`], and for the opposite reason.** A matrix
/// item renders as parent then child, and the child is the sellable code — the
/// thing on the label, the thing the catalogue is keyed by. The parent is a
/// style and joins to nothing.
///
/// Missing this cost 84 of 430 lines on the first dry run, all of them reported
/// as items the catalogue had never heard of.
pub fn item_code(raw: &str) -> String {
    raw.rsplit(" : ").next().unwrap_or(raw).trim().to_string()
}

/// A stable code for a customer that has none.
///
/// Uppercase alphanumerics of the name, truncated. Two different customers can
/// slug to the same thing — `PMFresh Colmslie` and `PMFresh Coolum` both start
/// the same way — so the caller passes the codes already taken and a short hash
/// of the *full* name disambiguates. Deterministic: the same file always
/// produces the same codes, whatever order the rows are in.
pub fn customer_code(name: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let base: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(24)
        .collect::<String>()
        .to_ascii_uppercase();
    let base = if base.is_empty() { "PARTY".to_string() } else { base };
    if !taken(&base) {
        return base;
    }
    // FNV-1a over the full name: short, stable, and not the row's position.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    let suffix = format!("{:04X}", (h & 0xFFFF) as u16);
    let head: String = base.chars().take(19).collect();
    format!("{head}-{suffix}")
}

/// What is still to be picked: ordered, less what has already gone.
///
/// Never negative. An over-shipped line — fulfilled beyond ordered — is a real
/// thing and it means nothing is outstanding, not that the bench owes units
/// back.
pub fn outstanding(ordered: i64, fulfilled: i64) -> i64 {
    (ordered - fulfilled).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_is_day_first() {
        assert_eq!(parse_date("15/7/2026"), NaiveDate::from_ymd_opt(2026, 7, 15));
        // The one that cannot be detected, only decided.
        assert_eq!(parse_date("7/8/2026"), NaiveDate::from_ymd_opt(2026, 8, 7));
        assert_eq!(parse_date(" 1/1/2026 "), NaiveDate::from_ymd_opt(2026, 1, 1));
        assert_eq!(parse_date("31/2/2026"), None, "a date that does not exist is none");
        assert_eq!(parse_date(""), None);
        assert_eq!(parse_date("not a date"), None);
    }

    #[test]
    fn a_sub_customer_keeps_its_parent() {
        assert_eq!(
            customer_name("PMFresh Pty Ltd : PMFresh Pty Ltd Colmslie"),
            "PMFresh Pty Ltd : PMFresh Pty Ltd Colmslie"
        );
        assert_eq!(customer_name("  Acme  "), "Acme");
    }

    /// The property that keeps a re-run a no-op: same input, same codes,
    /// whatever order the rows arrive in.
    #[test]
    fn a_code_is_derived_and_stable() {
        let none = |_: &str| false;
        assert_eq!(customer_code("Acme Pty Ltd", &none), "ACMEPTYLTD");
        assert_eq!(
            customer_code("Acme Pty Ltd", &none),
            customer_code("Acme Pty Ltd", &none)
        );
    }

    #[test]
    fn two_customers_that_slug_alike_get_different_codes() {
        let a = customer_code("PMFresh Pty Ltd : Colmslie", &|_| false);
        // Now that one is taken, the next must not collide with it.
        let b = customer_code("PMFresh Pty Ltd : Coolum", &|c| c == a);
        assert_ne!(a, b);
        // And it is a function of the name, not of when it was seen.
        let b_again = customer_code("PMFresh Pty Ltd : Coolum", &|c| c == a);
        assert_eq!(b, b_again);
    }

    #[test]
    fn a_matrix_item_gives_up_its_child() {
        assert_eq!(item_code("DUP : SKU-9454-07"), "SKU-9454-07");
        assert_eq!(item_code("HVE- : HVE-1127P"), "HVE-1127P");
        assert_eq!(item_code("SKU-2008"), "SKU-2008");
        assert_eq!(item_code("  spaced  "), "spaced");
        // A customer keeps its parent; an item does not. Same shape, opposite
        // rule, which is why they are separate functions.
        assert_ne!(
            item_code("PMFresh Pty Ltd : Colmslie"),
            customer_name("PMFresh Pty Ltd : Colmslie")
        );
    }

    #[test]
    fn what_is_left_is_never_negative() {
        assert_eq!(outstanding(12, 0), 12);
        assert_eq!(outstanding(12, 5), 7);
        assert_eq!(outstanding(12, 12), 0);
        // Over-shipped: nothing outstanding, and the bench is not owed units.
        assert_eq!(outstanding(12, 15), 0);
    }
}
