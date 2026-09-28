-- Migration 96 down: every rack, floor area and bin box is lost, and bins go
-- back to having no place.

DROP TABLE IF EXISTS floor_area;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_site_key;

DROP INDEX IF EXISTS location_geometry_idx;
DROP INDEX IF EXISTS location_rack_idx;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_geometry_sourced_ck;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_geometry_source_ck;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_geometry_extent_ck;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_geometry_whole_ck;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_rack_site_fk;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_rack_tenant_fk;
ALTER TABLE location DROP COLUMN IF EXISTS geometry_source;
ALTER TABLE location DROP COLUMN IF EXISTS rack_id;
-- Migration 1 left these without comments, and a down leaves what it found.
COMMENT ON COLUMN location.x_mm IS NULL;
COMMENT ON COLUMN location.length_mm IS NULL;
COMMENT ON COLUMN location.width_mm IS NULL;
COMMENT ON COLUMN location.height_mm IS NULL;

DROP TABLE IF EXISTS rack;
