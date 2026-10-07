-- Reverse of 2026-08-04-000131_what_netsuite_says_an_item_looks_like.
--
-- NetSuite's pictures and figures go, and what Spork said of the pictures.
-- The next load of the item details is its unit and supplier's part again.

DROP VIEW IF EXISTS item_netsuite_differs;
DROP TABLE IF EXISTS item_picture_said;
DROP TABLE IF EXISTS reported_item_picture;
ALTER TABLE reported_item
    DROP COLUMN IF EXISTS picture_file,
    DROP COLUMN IF EXISTS weight_g,
    DROP COLUMN IF EXISTS length_mm,
    DROP COLUMN IF EXISTS width_mm,
    DROP COLUMN IF EXISTS height_mm,
    DROP COLUMN IF EXISTS upc;
