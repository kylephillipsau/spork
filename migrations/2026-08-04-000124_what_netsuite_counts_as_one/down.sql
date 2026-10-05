-- Reverse of 2026-08-04-000124_what_netsuite_counts_as_one.
--
-- What was said of an item's unit goes; NetSuite's Pack Unit is still on file
-- in reported_item.

DROP VIEW IF EXISTS item_unit_level;
DROP FUNCTION IF EXISTS unit_level_of(text);
DROP TABLE IF EXISTS item_unit;
