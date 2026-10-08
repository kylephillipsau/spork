-- Reverse of 2026-08-04-000132_netsuite_said_as_it_said_it.
--
-- What NetSuite said field by field goes, with what each was read as, and
-- D237's own columns for its picture, weight, size and UPC come back empty
-- until the next load of the item details fills them.

DROP VIEW IF EXISTS item_netsuite_differs;
DROP VIEW IF EXISTS reported_item_said;
DROP FUNCTION IF EXISTS reported_field_meaning(text, text);
DROP FUNCTION IF EXISTS art_numbers_in(text);
DROP FUNCTION IF EXISTS millimetres_of(text, text);
DROP FUNCTION IF EXISTS grams_of(text, text);
DROP FUNCTION IF EXISTS number_in(text);
DROP TABLE IF EXISTS reported_field_said;
DROP TABLE IF EXISTS reported_field_default;
DROP FUNCTION IF EXISTS reported_field_role_ok(text);
DROP TABLE IF EXISTS reported_item_field;

ALTER TABLE reported_item
    ADD COLUMN picture_file text,
    ADD COLUMN weight_g integer,
    ADD COLUMN length_mm integer,
    ADD COLUMN width_mm integer,
    ADD COLUMN height_mm integer,
    ADD COLUMN upc text;

CREATE VIEW item_netsuite_differs WITH (security_invoker = true) AS
WITH ns AS (
    SELECT DISTINCT ON (r.item_id) r.item_id, r.weight_g, r.length_mm, r.width_mm, r.height_mm, r.upc
      FROM reported_item r
     ORDER BY r.item_id, r.as_at DESC
),
unit AS (
    SELECT ns.*, i.style_id, coalesce(u.level::text, 'each') AS level, coalesce(u.singles, 1) AS singles
      FROM ns
      JOIN item i ON i.id = ns.item_id
      LEFT JOIN item_unit_level u ON u.item_id = ns.item_id
),
measured AS (
    SELECT unit.*,
           (SELECT max(oc.value_numeric) FILTER (WHERE m.code = 'gross_weight')
              FROM observable o
              JOIN observation_current oc ON oc.observable_id = o.id
              JOIN metric m ON m.id = oc.metric_id
             WHERE (o.item_id = unit.item_id OR (unit.style_id IS NOT NULL AND o.item_style_id = unit.style_id))
               AND o.packaging_level::text = unit.level
               AND oc.method IN ('instrument', 'scan', 'keyed', 'derived', 'photographed')
               AND unit.singles = 1) AS spork_weight_g,
           (SELECT array_agg(oc.value_numeric ORDER BY oc.value_numeric DESC)
              FROM observable o
              JOIN observation_current oc ON oc.observable_id = o.id
              JOIN metric m ON m.id = oc.metric_id
             WHERE (o.item_id = unit.item_id OR (unit.style_id IS NOT NULL AND o.item_style_id = unit.style_id))
               AND o.packaging_level::text = unit.level
               AND m.code IN ('length', 'width', 'height')
               AND oc.value_numeric IS NOT NULL
               AND oc.method IN ('instrument', 'scan', 'keyed', 'derived', 'photographed')
               AND unit.singles = 1) AS spork_sides,
           (SELECT array_agg(s ORDER BY s DESC)
              FROM unnest(ARRAY[unit.length_mm, unit.width_mm, unit.height_mm]) s
             WHERE s IS NOT NULL) AS netsuite_sides,
           (SELECT array_agg(b.barcode)
              FROM item_barcode b
             WHERE b.item_id = unit.item_id AND b.scheme = 'gtin') AS spork_gtins
      FROM unit
)
SELECT item_id,
       weight_g AS netsuite_weight_g,
       spork_weight_g::bigint AS spork_weight_g,
       weight_g IS NOT NULL AND spork_weight_g IS NOT NULL
         AND abs(spork_weight_g - weight_g) > greatest(10, 0.05 * weight_g) AS weight_differs,
       cardinality(netsuite_sides) = 3 AND cardinality(spork_sides) = 3
         AND EXISTS (SELECT 1 FROM generate_series(1, 3) k
                      WHERE abs(spork_sides[k] - netsuite_sides[k]) > greatest(5, 0.05 * netsuite_sides[k]))
         AS size_differs,
       upc AS netsuite_upc,
       upc IS NOT NULL AND spork_gtins IS NOT NULL
         AND NOT (lpad(regexp_replace(upc, '[^0-9]', '', 'g'), 14, '0') = ANY (spork_gtins)) AS upc_differs
  FROM measured;

COMMENT ON VIEW item_netsuite_differs IS
    'Where NetSuite''s weight, size or UPC for an item disagree with what Spork measured and scanned: '
    'a reading of both words, never stored. D237, migration 131.';

GRANT SELECT ON item_netsuite_differs TO spork_app;
