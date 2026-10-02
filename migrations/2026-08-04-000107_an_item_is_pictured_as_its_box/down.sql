-- Migration 107 down: items are pictured by their fronts again.
--
-- The drawings go; their files become unreferenced for the reaper. The cuts
-- they were drawn from are untouched.

DROP TABLE box_picture;
