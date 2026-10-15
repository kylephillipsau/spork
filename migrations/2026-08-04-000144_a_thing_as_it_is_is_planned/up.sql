-- Migration 144: a thing that ships as it is can be planned onto a pallet.
-- docs/packing-plan.md, left from M3.
--
-- A planned parcel was always a box of a box type. Something that ships as it
-- is (D196, migration 113) came onto the board only once made, so it could go
-- on a pallet only after it was shipped. Now a planned parcel is a box type,
-- or one of an item at a level as it is, as `package` says one (migrations 98
-- and 113), and is made from the plan as Ship would make it.

ALTER TABLE pack_plan_parcel
    ALTER COLUMN package_type_id DROP NOT NULL,
    ADD COLUMN own_item_id uuid,
    ADD COLUMN own_level packaging_level,
    ADD COLUMN item_packing_config_id uuid,
    ADD CONSTRAINT pack_plan_parcel_own_item_fk
        FOREIGN KEY (own_item_id, tenant_id) REFERENCES item(id, tenant_id),
    ADD CONSTRAINT pack_plan_parcel_config_fk
        FOREIGN KEY (item_packing_config_id, tenant_id)
        REFERENCES item_packing_config(id, tenant_id),
    ADD CONSTRAINT pack_plan_parcel_box_or_as_is_ck
        CHECK (num_nonnulls(package_type_id, own_item_id) = 1),
    ADD CONSTRAINT pack_plan_parcel_own_level_ck
        CHECK ((own_item_id IS NULL) = (own_level IS NULL)),
    ADD CONSTRAINT pack_plan_parcel_own_case_ck
        CHECK (item_packing_config_id IS NULL
               OR (own_item_id IS NOT NULL AND own_level IN ('carton', 'inner')));

COMMENT ON COLUMN pack_plan_parcel.own_item_id IS
    'Migration 144. One of this item at own_level, as it is (D196), planned: '
    'made as Ship makes one. Else the parcel is a box of package_type_id.';
