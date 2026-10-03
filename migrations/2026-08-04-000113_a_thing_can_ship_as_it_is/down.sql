-- Migration 113 down: nothing ships as it is but a product's own carton, and
-- every box may be suggested.

ALTER TABLE package DROP CONSTRAINT IF EXISTS package_box_or_own_ck;
ALTER TABLE package DROP CONSTRAINT IF EXISTS package_own_case_ck;
ALTER TABLE package DROP CONSTRAINT IF EXISTS package_own_level_ck;
ALTER TABLE package DROP CONSTRAINT IF EXISTS package_own_item_fk;
-- A package that was one of an each or an inner has no case-pack form to go
-- back to: it becomes a package of nothing, which migration 98 allowed.
ALTER TABLE package DROP COLUMN IF EXISTS own_level;
ALTER TABLE package DROP COLUMN IF EXISTS own_item_id;
ALTER TABLE package ADD CONSTRAINT package_box_or_own_carton_ck
    CHECK (num_nonnulls(package_type_id, item_packing_config_id) <= 1);

DROP FUNCTION ships_as_is(uuid, uuid, uuid, uuid, packaging_level);
DROP TABLE subject_shipping;

ALTER TABLE package_type DROP COLUMN suggested;
