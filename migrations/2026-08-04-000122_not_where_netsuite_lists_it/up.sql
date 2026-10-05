-- Migration 122: an item not where NetSuite lists it, said on the floor (D215).
--
-- D212 made NetSuite the keeper of the shelves: how much of an item is in
-- each bin is NetSuite's to say, loaded as `reported_stock` every five
-- minutes, and a difference found on the floor is a finding, fixed in
-- NetSuite. Two differences are the ones a person walking a list meets:
--
--   `not_in_listed_bin`      NetSuite lists it in a bin, and it isn't there.
--   `found_in_unlisted_bin`  It is in a bin where NetSuite lists none.
--
-- **Not `count_variance` and not `unexpected_stock`.** Those are about
-- Spork's own ledger: a count asserts against a cell Spork holds, and a
-- variance is put right by an adjustment that moves Spork's stock
-- (`adjusting.rs`). These are about NetSuite's report, Spork holds no cell to
-- adjust, and the fix is made in NetSuite. They route differently, which is
-- what D33 asks of a kind.
--
-- `ALTER TYPE ... ADD VALUE` has no inverse; migrations 30, 40 and 52 say so
-- and survive it the same way.

ALTER TYPE discrepancy_kind ADD VALUE IF NOT EXISTS 'not_in_listed_bin';
ALTER TYPE discrepancy_kind ADD VALUE IF NOT EXISTS 'found_in_unlisted_bin';
