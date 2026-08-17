//! What a thing looks like, for somebody who has to find it.
//!
//! D141. `crate::capture` states flatly that **photographs do not inherit**:
//! showing a style's picture against a variant would claim a photograph of
//! *this* carton that is a photograph of a different subject's carton. That
//! stays true, and it is true because of the question the capture worklist is
//! asking — *has anyone looked at this subject* — which inheritance would
//! answer falsely.
//!
//! A picker is asking a different question. *What am I looking for on this
//! shelf.* A photograph of the size-8 carton is a perfectly good answer for
//! size 10, and refusing it leaves somebody hunting a bay with nothing to go
//! on. So the same bytes resolve differently for the two reads, and the rule
//! that keeps that honest is D108's: **the answer says whose picture it is.**
//! A screen that cannot tell an inherited picture from an own one shows
//! something nobody took against this code as though somebody had — the same
//! sentence the figures earned.
//!
//! # Front, and nothing else
//!
//! Seven faces are recorded and one is offered here. A `label` is a barcode
//! close-up and a `bottom` is a box lid: shown under the heading *what to look
//! for*, either misleads worse than showing nothing, because the picker will
//! believe it. If the front has never been photographed the answer is nothing,
//! and the screen says so.
//!
//! # Newest wins
//!
//! D132's rule for which picture of a face is current — a fold on
//! `package_event`'s own winning-row rule — applied here. A retake is a new row
//! and the new one is what the shelf looks like now.

use serde::Serialize;

/// A picture offered for recognition, and whose it is.
#[derive(Serialize, Debug, Clone)]
pub struct Picture {
    /// The content address. `GET /images/{digest}` serves the bytes, inside the
    /// tenant scope, so knowing one is not enough to read it.
    pub digest: String,
    /// `own` or `style`. D108's vocabulary, doing a second job.
    pub source: String,
}

/// Resolve the front-face picture per item, own before inherited.
///
/// A `WITH` block naming one CTE, `picture`, keyed on `item_id` — written as a
/// fragment rather than a function because its callers join it into a larger
/// read, and a per-row lookup against seven thousand items is the shape that
/// cost the capture worklist 1.4 seconds. Every join here is a plain equality
/// so the planner can hash.
pub const PICTURE_CTE: &str = "
picture AS (
    SELECT DISTINCT ON (c.item_id)
           c.item_id,
           oi.digest,
           CASE WHEN c.rank = 0 THEN 'own' ELSE 'style' END AS source
      FROM (
            -- The item's own looks, at any packaging level: a picture of the
            -- carton and a picture of the each are both pictures of the thing.
            SELECT i.id AS item_id, o.id AS observable_id, 0 AS rank
              FROM item i JOIN observable o ON o.item_id = i.id
            UNION ALL
            -- Its style's, which is the inheritance this module exists to
            -- allow and to label. D141.
            SELECT i.id, o.id, 1
              FROM item i
              JOIN observable o ON o.item_style_id = i.style_id
             WHERE i.style_id IS NOT NULL
           ) c
      JOIN observation_event e ON e.observable_id = c.observable_id
      JOIN observation_image oi ON oi.observation_event_id = e.id
     WHERE oi.face = 'front'
     ORDER BY c.item_id, c.rank, oi.captured_at DESC, oi.id DESC
)";

/// Build a [`Picture`] from a row's digest and source columns.
///
/// Both are null together — an item nobody has photographed has neither — so
/// this is one option rather than two fields a caller has to keep in step.
pub fn from_row(digest: Option<String>, source: Option<String>) -> Option<Picture> {
    match (digest, source) {
        (Some(digest), Some(source)) => Some(Picture { digest, source }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_needs_both_halves_or_neither() {
        // The pair is what makes it honest: a digest with no source is a
        // picture a screen cannot label, which is the one thing D141 forbids.
        assert!(from_row(Some("abc".into()), Some("own".into())).is_some());
        assert!(from_row(Some("abc".into()), None).is_none());
        assert!(from_row(None, Some("own".into())).is_none());
        assert!(from_row(None, None).is_none());
    }
}
