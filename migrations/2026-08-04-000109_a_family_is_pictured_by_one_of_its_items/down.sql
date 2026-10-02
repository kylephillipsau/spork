-- Migration 109 down: no item pictures its family.

ALTER TABLE item_style
    DROP CONSTRAINT IF EXISTS item_style_picture_item_fk,
    DROP COLUMN IF EXISTS picture_item_id;
