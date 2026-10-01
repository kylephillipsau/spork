-- Migration 99: a rack with a face on each side. D173.
--
-- Racking often carries bins on both of its faces, numbered round the rack:
-- E-01 to E-18 along the front, then E-19 to E-36 back along the other side, so
-- E-36 is behind E-01. Walk round to the back and it still reads left to right,
-- 19 to 36.
--
-- # One rack, not two places
--
-- The rack is one thing: people call it Rack E, it stands in one spot, and
-- what is behind E-01 is a fact about it. Drawn as two places back to back,
-- the back would carry copies of facts about the front (where its numbers
-- start, where it stands), and nothing would keep the copies true when the
-- front is moved or renumbered. So a place says how many sides its grid has,
-- and a cell says which side it is on.
--
-- # The cell
--
-- A cell's bay is its column along the rack, counted from the front's left as
-- the front is faced, on either side. So the bin behind another is the same
-- column on the other side, exactly, without arithmetic on labels. What the
-- column is called on the back follows from the numbering: round, the back's
-- labels run on from the front's last, the other way along the rack.
--
-- Rows are still depth from the face a bin is picked from, so a rack two deep
-- from each side is two rows and two sides.

ALTER TABLE place ADD COLUMN sides smallint NOT NULL DEFAULT 1;

ALTER TABLE place
    ADD CONSTRAINT place_sides_ck CHECK (sides IN (1, 2)),
    -- A floor has nothing to put a back face on.
    ADD CONSTRAINT place_sides_solid_ck CHECK (sides = 1 OR solid),
    -- A grid is rows and columns, on one face or two.
    ADD CONSTRAINT place_sides_rectangular_ck CHECK (sides = 1 OR outline IS NULL);

-- The last label is on the back when there are two sides.
ALTER TABLE place DROP CONSTRAINT place_numbering_ck;
ALTER TABLE place ADD CONSTRAINT place_numbering_ck CHECK (
    bay_step <> 0 AND first_bay >= 0 AND first_level >= 0
    AND first_bay + (bays * sides - 1) * bay_step >= 0);

COMMENT ON COLUMN place.sides IS
    'How many faces its grid has: 1, or 2 for racking with bins on both sides, '
    'numbered round it (the back''s labels run on from the front''s last, the '
    'other way along, so the last is behind the first). D173.';

ALTER TABLE location ADD COLUMN slot_side smallint;
UPDATE location SET slot_side = 1 WHERE place_id IS NOT NULL;

ALTER TABLE location DROP CONSTRAINT location_slot_whole_ck;
ALTER TABLE location ADD CONSTRAINT location_slot_whole_ck CHECK (
    num_nonnulls(place_id, slot_side, slot_bay, slot_level, slot_row, slot_position) IN (0, 6));
ALTER TABLE location DROP CONSTRAINT location_slot_ck;
ALTER TABLE location ADD CONSTRAINT location_slot_ck CHECK (
    slot_side IN (1, 2) AND slot_bay >= 1 AND slot_level >= 1 AND slot_row >= 1
    AND slot_position >= 1);

-- One bin to a cell, and the back of a column is another cell.
DROP INDEX location_slot_key;
CREATE UNIQUE INDEX location_slot_key
    ON location (place_id, slot_side, slot_bay, slot_level, slot_row, slot_position)
    WHERE place_id IS NOT NULL;

COMMENT ON COLUMN location.slot_side IS
    'Which face of its place the bin is on: 1 the front, 2 the back. Its bay is '
    'the column along the place from the front''s left, the same on both sides, '
    'so the bin behind another is the same column on the other side. D173.';
