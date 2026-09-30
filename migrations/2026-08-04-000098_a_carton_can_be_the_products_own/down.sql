-- Migration 98 down: packages are box types or nothing again.

ALTER TABLE package DROP CONSTRAINT IF EXISTS package_box_or_own_carton_ck;
ALTER TABLE package DROP CONSTRAINT IF EXISTS package_item_packing_config_fk;
ALTER TABLE package DROP COLUMN IF EXISTS item_packing_config_id;
