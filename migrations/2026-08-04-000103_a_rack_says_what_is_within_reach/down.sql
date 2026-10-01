-- Migration 103 down: racks say nothing of reach.
--
-- What each rack said is lost; its bins and their cells are untouched.

ALTER TABLE place
    DROP CONSTRAINT IF EXISTS place_reach_levels_ck,
    DROP COLUMN IF EXISTS reach_levels;
