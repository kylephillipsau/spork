-- Migration 109: a family is pictured by one of its items. D188.
--
-- A family of thirteen glove sizes looks the same in every size, and nobody
-- photographs thirteen boxes of gloves. One size, photographed and drawn as
-- its box (D186), can stand for the family: every variant with no picture of
-- its own shows that one's.

ALTER TABLE item_style
    ADD COLUMN picture_item_id uuid,
    ADD CONSTRAINT item_style_picture_item_fk
        FOREIGN KEY (picture_item_id, tenant_id) REFERENCES item(id, tenant_id);

COMMENT ON COLUMN item_style.picture_item_id IS
    'The variant whose picture stands for the family, for variants with none '
    'of their own. Migration 109, D188.';
