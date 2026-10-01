-- Migration 99 down: every rack has one face again. Bins on a back come off
-- the layout, since their cells would collide with the front's.

DO $$
DECLARE
    n bigint;
BEGIN
    SELECT count(*) INTO n FROM location WHERE slot_side = 2;
    IF n > 0 THEN
        UPDATE location
           SET place_id = NULL, slot_side = NULL, slot_bay = NULL, slot_level = NULL,
               slot_row = NULL, slot_position = NULL
         WHERE slot_side = 2;
        RAISE NOTICE 'reversing migration 99 takes % bin(s) on the back of a rack off the '
                     'layout: they wait in the tray to be placed again', n;
    END IF;
END $$;

DROP INDEX IF EXISTS location_slot_key;
CREATE UNIQUE INDEX location_slot_key
    ON location (place_id, slot_bay, slot_level, slot_row, slot_position)
    WHERE place_id IS NOT NULL;

ALTER TABLE location DROP CONSTRAINT IF EXISTS location_slot_ck;
ALTER TABLE location ADD CONSTRAINT location_slot_ck CHECK (
    slot_bay >= 1 AND slot_level >= 1 AND slot_row >= 1 AND slot_position >= 1);
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_slot_whole_ck;
ALTER TABLE location DROP COLUMN IF EXISTS slot_side;
ALTER TABLE location ADD CONSTRAINT location_slot_whole_ck CHECK (
    num_nonnulls(place_id, slot_bay, slot_level, slot_row, slot_position) IN (0, 5));

ALTER TABLE place DROP CONSTRAINT IF EXISTS place_numbering_ck;
ALTER TABLE place DROP CONSTRAINT IF EXISTS place_sides_rectangular_ck;
ALTER TABLE place DROP CONSTRAINT IF EXISTS place_sides_solid_ck;
ALTER TABLE place DROP CONSTRAINT IF EXISTS place_sides_ck;
ALTER TABLE place DROP COLUMN IF EXISTS sides;
ALTER TABLE place ADD CONSTRAINT place_numbering_ck CHECK (
    bay_step <> 0 AND first_bay >= 0 AND first_level >= 0
    AND first_bay + (bays - 1) * bay_step >= 0);
