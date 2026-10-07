-- Reverse of 2026-08-04-000129_a_pair_is_two_single_ones.
--
-- An item's unit is a level again, and a pair is the each. What was said of
-- how many of a level a unit is goes with its column.

DROP VIEW IF EXISTS item_unit_level;

CREATE VIEW item_unit_level WITH (security_invoker = true) AS
SELECT coalesce(said.item_id, ns.item_id) AS item_id,
       coalesce(said.level, unit_level_of(ns.selling_unit)) AS level,
       said.level IS NOT NULL AS said,
       ns.selling_unit AS netsuite_unit
  FROM (SELECT DISTINCT ON (item_id) item_id, level FROM item_unit
         ORDER BY item_id, recorded_at DESC, id DESC) said
  FULL JOIN (SELECT DISTINCT ON (item_id) item_id, selling_unit FROM reported_item
              ORDER BY item_id, as_at DESC) ns
    ON ns.item_id = said.item_id;

COMMENT ON VIEW item_unit_level IS
    'Which level of an item is one in NetSuite, and whether Spork said so; an item not here is sold by '
    'the each. D218, migration 124.';

GRANT SELECT ON item_unit_level TO spork_app;

DROP INDEX IF EXISTS item_packing_config_item_idx;
DROP FUNCTION IF EXISTS unit_quantity_of(text);
ALTER TABLE item_unit DROP COLUMN IF EXISTS quantity;
