-- Migration 96 down: every place goes, and bins go back to having no cell. The
-- six millimetre columns come back empty, as migration 1 left them.

ALTER TABLE location
    ADD COLUMN x_mm integer,
    ADD COLUMN y_mm integer,
    ADD COLUMN z_mm integer,
    ADD COLUMN length_mm integer,
    ADD COLUMN width_mm integer,
    ADD COLUMN height_mm integer;

DROP INDEX IF EXISTS location_slot_key;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_slot_ck;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_slot_whole_ck;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_place_site_fk;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_place_tenant_fk;
ALTER TABLE location DROP COLUMN IF EXISTS slot_position;
ALTER TABLE location DROP COLUMN IF EXISTS slot_row;
ALTER TABLE location DROP COLUMN IF EXISTS slot_level;
ALTER TABLE location DROP COLUMN IF EXISTS slot_bay;
ALTER TABLE location DROP COLUMN IF EXISTS place_id;

DROP TABLE IF EXISTS place;
