-- Migration 106: an item says which of its variants is its carton. D184.
--
-- An item whose cartons come printed two ways (D182) has each printing as a
-- variant, photographed and measured on its own, and its own carton card
-- still asking for photographs of a carton that is one of those two. So the
-- item names the variant that stands for its carton: the carton card shows
-- that variant's figures and photographs, until the carton is measured or
-- photographed as itself, which wins.

ALTER TABLE item
    ADD COLUMN default_lot_id uuid,
    ADD CONSTRAINT item_default_lot_fk
        FOREIGN KEY (default_lot_id, tenant_id) REFERENCES lot(id, tenant_id);

COMMENT ON COLUMN item.default_lot_id IS
    'The variant (a lot of this item) that stands for its carton until the '
    'carton is recorded as itself. Migration 106, D184.';

GRANT UPDATE (default_lot_id) ON item TO spork_app;
