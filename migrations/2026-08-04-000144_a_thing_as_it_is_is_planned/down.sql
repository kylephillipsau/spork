-- Migration 144 down: a planned parcel is a box again. Things planned as they
-- are go from the plan, which is a draft; their lines go with them.

DELETE FROM pack_plan_parcel WHERE own_item_id IS NOT NULL;
ALTER TABLE pack_plan_parcel
    DROP COLUMN item_packing_config_id,
    DROP COLUMN own_level,
    DROP COLUMN own_item_id,
    ALTER COLUMN package_type_id SET NOT NULL;
