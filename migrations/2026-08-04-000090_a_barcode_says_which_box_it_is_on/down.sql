-- Migration 90 down: a barcode stops saying which box it is on.
--
-- Both columns go. Nothing else references them, and the resolver's behaviour
-- for a row that does not name a level is the behaviour it had before this
-- migration — so reversing leaves a working system rather than a broken one.
ALTER TABLE item_barcode DROP COLUMN bound_by_person_id;
ALTER TABLE item_barcode DROP COLUMN packaging_level;
