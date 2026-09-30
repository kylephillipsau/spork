-- Migration 98: a carton can be the product's own.
--
-- Most of what leaves a warehouse like this one does not go into a box the
-- warehouse chose. A carton of six brooms leaves in the carton the brooms came
-- in, and the other system's packing presets are mostly named by item code for
-- exactly that reason: the preset is "one carton of this item", with the
-- carton's size and weight. Spork's packages knew only box types (package_type),
-- so a product's own carton could be recorded only as a box it was not.
--
-- # What the row says
--
-- `package.item_packing_config_id`: this package is one carton of the case
-- pack named, which says the item and how many are in a carton. Its size and
-- weight are **not copied onto the package.** They are observations of the
-- item's carton (or its style's, D108), and the bench reads them from there
-- when it reads the package, as it reads a box type's stated size. A copied
-- figure would be indistinguishable from one taken of this carton, and
-- `package.length_mm` and its neighbours are for what this carton measured.
--
-- A package is a box type or a product's own carton, not both: a box type
-- says what the warehouse packed into, and a product's carton is not that.

ALTER TABLE package
    ADD COLUMN item_packing_config_id uuid,
    ADD CONSTRAINT package_item_packing_config_fk
        FOREIGN KEY (item_packing_config_id, tenant_id)
        REFERENCES item_packing_config(id, tenant_id),
    ADD CONSTRAINT package_box_or_own_carton_ck
        CHECK (num_nonnulls(package_type_id, item_packing_config_id) <= 1);

COMMENT ON COLUMN package.item_packing_config_id IS
    'This package is one carton of this case pack: the product''s own carton, '
    'rather than a box type. Its size and weight are read from the item''s carton '
    'observations, never copied here. Migration 98.';

GRANT INSERT (item_packing_config_id) ON package TO spork_app;
