-- Migration 106 down: no variant stands for an item's carton.

ALTER TABLE item
    DROP CONSTRAINT IF EXISTS item_default_lot_fk,
    DROP COLUMN IF EXISTS default_lot_id;
