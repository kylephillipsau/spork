-- Migration 97 down: sites stop saying where they pack, and the bench goes back
-- to having nothing to read.

ALTER TABLE site DROP CONSTRAINT IF EXISTS site_pack_location_fk;
ALTER TABLE site DROP CONSTRAINT IF EXISTS site_pack_location_tenant_fk;
ALTER TABLE site DROP COLUMN IF EXISTS pack_location_id;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_site_key;
