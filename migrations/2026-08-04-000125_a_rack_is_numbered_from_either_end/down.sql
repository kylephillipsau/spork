-- Migration 125 down: every rack is numbered from the left again. A bin in a
-- rack numbered from the right moves to the column its code names from the
-- left, so it keeps its name. Taken off and put back, because a swap made in
-- one statement collides with itself on location_slot_key.

CREATE TEMP TABLE renumbered AS
SELECT l.id, l.place_id, l.slot_side, p.bays + 1 - l.slot_bay AS slot_bay,
       l.slot_level, l.slot_row, l.slot_position
  FROM location l
  JOIN place p ON p.id = l.place_id
 WHERE p.from_right;

UPDATE location
   SET place_id = NULL, slot_side = NULL, slot_bay = NULL, slot_level = NULL,
       slot_row = NULL, slot_position = NULL
 WHERE id IN (SELECT id FROM renumbered);

UPDATE location l
   SET place_id = r.place_id, slot_side = r.slot_side, slot_bay = r.slot_bay,
       slot_level = r.slot_level, slot_row = r.slot_row, slot_position = r.slot_position
  FROM renumbered r
 WHERE l.id = r.id;

DROP TABLE renumbered;

ALTER TABLE place DROP COLUMN IF EXISTS from_right;
