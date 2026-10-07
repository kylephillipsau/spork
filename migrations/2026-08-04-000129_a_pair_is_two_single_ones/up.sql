-- Migration 129: a pair is two single ones (D233).
--
-- NetSuite sells boots, gloves and insoles by the Pair, and D218 read its
-- "Pair" as one thing, the each. So a carton of 70 pairs of gloves, said as
-- what it holds, 140 gloves, was 140 of what NetSuite counts, and the pack
-- bench packed twice what was ordered. A pair is two of the single thing, and
-- the single glove is what the item's three physical levels are counted in:
-- a bag holds so many gloves, a carton so many bags.
--
-- # One in NetSuite is so many of a level
--
-- The unit said so far was a level (D218): one of what NetSuite counts is the
-- each, the pack or the carton. Now it is so many of a level, `quantity`: two
-- of the each for a pair, one of anything else. Unsaid, NetSuite's Pack Unit
-- decides by one rule beside `unit_level_of`, `unit_quantity_of`. A pair that
-- is one thing, safety glasses or earmuffs, is said as a single item, one of
-- the each, over it.
--
-- # The pair, when it is packed as one
--
-- Boots come a pair to a box, and gloves come a pair at a time, clipped or
-- banded together: then the pair is the item's pack (units_per_inner = 2),
-- something to weigh, measure and photograph, and what NetSuite counts one
-- of, as a box of 100 earplugs is. `item_unit_level` says so: a unit of two
-- of the each, where the pack in force holds two or nothing says what it
-- holds, is the pack. Where the pack is something else, ten bags of twelve
-- pairs, the pair is no package of its own, and the unit stays two of the
-- each.

ALTER TABLE item_unit
    ADD COLUMN quantity integer NOT NULL DEFAULT 1,
    ADD CONSTRAINT item_unit_quantity_ck CHECK (quantity BETWEEN 1 AND 1000);

COMMENT ON COLUMN item_unit.quantity IS
    'How many of the level one of what NetSuite counts is: two of the each for a pair. D233.';

-- **The one rule** from NetSuite's Pack Unit to how many of its level: a pair
-- by any of its names is two, anything else one.
CREATE FUNCTION unit_quantity_of(p_unit text) RETURNS integer
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE upper(btrim(coalesce(p_unit, '')))
               WHEN 'PAIR' THEN 2
               WHEN 'PAIRS' THEN 2
               WHEN 'PR' THEN 2
               WHEN 'PRS' THEN 2
               ELSE 1
           END
$$;

-- The case pack in force is looked up by item, newest first, by every reader.
CREATE INDEX item_packing_config_item_idx ON item_packing_config (item_id, effective_from DESC);

-- `singles` is how many of the each one in NetSuite is, where the unit is
-- counted in them: two for a pair, whether or not it is packed as one; one
-- for the each; null where the unit is a pack or carton of its own.
CREATE OR REPLACE VIEW item_unit_level WITH (security_invoker = true) AS
WITH unit AS (
    SELECT coalesce(said.item_id, ns.item_id) AS item_id,
           coalesce(said.level, unit_level_of(ns.selling_unit)) AS level,
           CASE WHEN said.item_id IS NOT NULL THEN said.quantity
                ELSE unit_quantity_of(ns.selling_unit) END AS quantity,
           said.level IS NOT NULL AS said,
           ns.selling_unit AS netsuite_unit
      FROM (SELECT DISTINCT ON (item_id) item_id, level, quantity FROM item_unit
             ORDER BY item_id, recorded_at DESC, id DESC) said
      FULL JOIN (SELECT DISTINCT ON (item_id) item_id, selling_unit FROM reported_item
                  ORDER BY item_id, as_at DESC) ns
        ON ns.item_id = said.item_id
),
packed AS (
    -- Whether the pack in force is the unit: it holds as many of the each,
    -- or nobody has said what it holds. Only asked of a unit of several eaches.
    SELECT u.*,
           u.level = 'each' AND u.quantity > 1
           AND coalesce((SELECT c.units_per_inner = u.quantity OR c.units_per_inner IS NULL
                           FROM item_packing_config c
                          WHERE u.level = 'each' AND u.quantity > 1
                            AND c.item_id = u.item_id AND c.effective_from <= current_date
                          ORDER BY c.effective_from DESC, c.id DESC
                          LIMIT 1), true) AS as_pack
      FROM unit u
)
SELECT item_id,
       CASE WHEN as_pack THEN 'inner'::packaging_level ELSE level END AS level,
       said,
       netsuite_unit,
       CASE WHEN level = 'each' THEN quantity END AS singles
  FROM packed;

COMMENT ON VIEW item_unit_level IS
    'Which level of an item is one in NetSuite, whether Spork said so, and how many single ones it is where '
    'it is counted in them (two for a pair, D233); a pair packed as one is the pack. An item not here is '
    'sold by the each. D218, migration 124; D233, migration 129.';
