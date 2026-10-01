-- Migration 103: a rack says which of its levels are within reach. D180.
--
-- The bin holding the most of an item is often at the top of the racking,
-- where nobody gets to it without a forklift. Somebody sent to weigh one, or
-- to pick one, wants the bin they can reach from the floor. Which levels that
-- is depends on the rack: one with tall levels has only its bottom one in
-- reach, and a rack of lower levels may have three.
--
-- # Counted, not measured
--
-- A rack says how many of its levels, counting up from its floor, a person
-- standing on the floor can reach. The layout is drawn relative and never
-- measured (D173), and reach is the question being asked, so it is the
-- answer kept: a height in millimetres would be a measurement nobody takes,
-- turned into reach by a rule nobody agreed.
--
-- # The default is the bottom level
--
-- That is the common rack, and the one the other system's bin types already
-- describe: its "Pick" bins are the bottom level. A floor place has one level,
-- so all of it is in reach. Zero is a rack entirely above reach.

ALTER TABLE place
    ADD COLUMN reach_levels smallint NOT NULL DEFAULT 1,
    ADD CONSTRAINT place_reach_levels_ck CHECK (reach_levels >= 0);

COMMENT ON COLUMN place.reach_levels IS
    'How many of its levels, counting up from its floor, can be reached by '
    'somebody standing on the floor, without a forklift. A count, not a '
    'height (D173). Migration 103, D180.';
