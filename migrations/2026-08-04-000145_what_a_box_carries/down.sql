-- Migration 145 down: nothing says what it takes on top or carries, and the
-- workspace says nothing of itself. The four inheriting functions write out
-- whose saying it is again, each as its own migration had it.

DO $$
DECLARE said bigint; boxes bigint; seals bigint;
BEGIN
    SELECT count(*) INTO said FROM subject_load;
    SELECT count(*) INTO boxes FROM package_type
     WHERE num_nonnulls(board_walls, wall_load_g, lid_load_g) > 0;
    SELECT count(*) INTO seals FROM package_event WHERE hollow IS NOT NULL;
    IF said > 0 OR boxes > 0 OR seals > 0 THEN
        RAISE NOTICE 'reversing D249 forgets % saying(s) of what a thing takes on top, the limits of % box(es) and whether % seal(s) were hollow', said, boxes, seals;
    END IF;
END
$$;

ALTER TABLE package_event DROP CONSTRAINT package_event_hollow_ck;
ALTER TABLE package_event DROP COLUMN hollow;

DROP TABLE workspace_setting;

DROP VIEW package_type_effective;
CREATE VIEW package_type_effective
    WITH (security_invoker = true) AS
SELECT t.id, t.tenant_id, t.name, t.carrier_package_code, t.dimensions_fixed,
       t.length_mm, t.width_mm, t.height_mm,
       coalesce(t.tare_weight_g, f.tare_weight_g) AS tare_weight_g,
       coalesce(t.max_payload_g, f.max_payload_g) AS max_payload_g,
       t.suggested, t.reusable, t.effective_from,
       t.cut_from_id,
       f.name AS cut_from_name,
       f.height_mm AS full_height_mm,
       coalesce(t.carrier_package_code, '') IN ('PAL', 'SKI', 'SKD') AS carries,
       (t.cut_from_id IS NOT NULL
        OR EXISTS (SELECT 1 FROM package_type c WHERE c.cut_from_id = t.id)) AS height_is_set,
       coalesce(t.cut_from_id, t.id) AS pictured_as
  FROM package_type t
  LEFT JOIN package_type f ON f.id = t.cut_from_id;

COMMENT ON VIEW package_type_effective IS
    'D252. A box type with a cut''s empty weight and limit taken from its full box '
    'unless it says its own, whether it carries cartons, whether its height is '
    'set, and whose photographs it wears.';

GRANT SELECT ON package_type_effective TO spork_app;

ALTER TABLE package_type DROP CONSTRAINT package_type_load_ck;
ALTER TABLE package_type DROP CONSTRAINT package_type_board_ck;
ALTER TABLE package_type
    DROP COLUMN lid_load_g,
    DROP COLUMN wall_load_g,
    DROP COLUMN board_ect,
    DROP COLUMN board_walls;

DROP FUNCTION takes_on_top(uuid, uuid, uuid, uuid, packaging_level);
DROP TABLE subject_load;

CREATE OR REPLACE FUNCTION packed_in(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (packaging_type text, source text)
LANGUAGE sql STABLE AS $$
    WITH asked (rank, source, item_id, item_style_id, lot_id, item_part_id, level) AS (
        SELECT 0, 'own', p_item, p_style, p_lot, p_part, p_level
        UNION ALL
        SELECT 1, 'item', l.item_id, NULL::uuid, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM lot l
         WHERE l.id = p_lot
        UNION ALL
        SELECT 2, 'style', NULL::uuid, i.style_id, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM item i
         WHERE i.style_id IS NOT NULL
           AND ((i.id = p_item AND p_level = 'carton')
                OR i.id = (SELECT l.item_id FROM lot l WHERE l.id = p_lot))
    )
    SELECT sp.packaging_type, a.source
      FROM asked a
      JOIN subject_packaging sp
        ON sp.item_id IS NOT DISTINCT FROM a.item_id
       AND sp.item_style_id IS NOT DISTINCT FROM a.item_style_id
       AND sp.lot_id IS NOT DISTINCT FROM a.lot_id
       AND sp.item_part_id IS NOT DISTINCT FROM a.item_part_id
       AND sp.packaging_level IS NOT DISTINCT FROM a.level
     ORDER BY a.rank, sp.recorded_at DESC, sp.id DESC
     LIMIT 1
$$;

CREATE OR REPLACE FUNCTION ships_as_is(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (as_it_is boolean, source text)
LANGUAGE sql STABLE AS $$
    WITH asked (rank, source, item_id, item_style_id, lot_id, item_part_id, level) AS (
        SELECT 0, 'own', p_item, p_style, p_lot, p_part, p_level
        UNION ALL
        SELECT 1, 'item', l.item_id, NULL::uuid, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM lot l
         WHERE l.id = p_lot
        UNION ALL
        SELECT 2, 'style', NULL::uuid, i.style_id, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM item i
         WHERE i.style_id IS NOT NULL
           AND ((i.id = p_item AND p_level = 'carton')
                OR i.id = (SELECT l.item_id FROM lot l WHERE l.id = p_lot))
    ),
    said AS (
        SELECT ss.as_it_is, a.source
          FROM asked a
          JOIN subject_shipping ss
            ON ss.item_id IS NOT DISTINCT FROM a.item_id
           AND ss.item_style_id IS NOT DISTINCT FROM a.item_style_id
           AND ss.lot_id IS NOT DISTINCT FROM a.lot_id
           AND ss.item_part_id IS NOT DISTINCT FROM a.item_part_id
           AND ss.packaging_level IS NOT DISTINCT FROM a.level
         ORDER BY a.rank, ss.recorded_at DESC, ss.id DESC
         LIMIT 1
    )
    SELECT as_it_is, source FROM said
    UNION ALL
    -- A part has no level, and a comparison with nothing is nothing, not false.
    SELECT p_lot IS NOT NULL OR coalesce(p_level = 'carton', false), 'default'
     WHERE NOT EXISTS (SELECT 1 FROM said)
$$;

CREATE OR REPLACE FUNCTION keeps_upright(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (upright boolean, source text)
LANGUAGE sql STABLE AS $$
    WITH asked (rank, source, item_id, item_style_id, lot_id, item_part_id, level) AS (
        SELECT 0, 'own', p_item, p_style, p_lot, p_part, p_level
        UNION ALL
        SELECT 1, 'item', l.item_id, NULL::uuid, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM lot l
         WHERE l.id = p_lot
        UNION ALL
        SELECT 2, 'style', NULL::uuid, i.style_id, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM item i
         WHERE i.style_id IS NOT NULL
           AND ((i.id = p_item AND p_level = 'carton')
                OR i.id = (SELECT l.item_id FROM lot l WHERE l.id = p_lot))
    ),
    said AS (
        SELECT su.upright, a.source
          FROM asked a
          JOIN subject_upright su
            ON su.item_id IS NOT DISTINCT FROM a.item_id
           AND su.item_style_id IS NOT DISTINCT FROM a.item_style_id
           AND su.lot_id IS NOT DISTINCT FROM a.lot_id
           AND su.item_part_id IS NOT DISTINCT FROM a.item_part_id
           AND su.packaging_level IS NOT DISTINCT FROM a.level
         ORDER BY a.rank, su.recorded_at DESC, su.id DESC
         LIMIT 1
    )
    SELECT upright, source FROM said
    UNION ALL
    SELECT false, 'default' WHERE NOT EXISTS (SELECT 1 FROM said)
$$;

CREATE OR REPLACE FUNCTION is_box(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS boolean
LANGUAGE sql STABLE AS $$
    WITH asked (rank, item_id, item_style_id, lot_id, item_part_id, level) AS (
        SELECT 0, p_item, p_style, p_lot, p_part, p_level
        UNION ALL
        SELECT 1, l.item_id, NULL::uuid, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM lot l
         WHERE l.id = p_lot
        UNION ALL
        SELECT 2, NULL::uuid, i.style_id, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM item i
         WHERE i.style_id IS NOT NULL
           AND ((i.id = p_item AND p_level = 'carton')
                OR i.id = (SELECT l.item_id FROM lot l WHERE l.id = p_lot))
    )
    SELECT CASE
               WHEN t.code IS NULL OR t.six_sided THEN true
               WHEN t.round THEN false
               ELSE coalesce((
                   SELECT ss.box_shaped
                     FROM asked a
                     JOIN subject_shape ss
                       ON ss.item_id IS NOT DISTINCT FROM a.item_id
                      AND ss.item_style_id IS NOT DISTINCT FROM a.item_style_id
                      AND ss.lot_id IS NOT DISTINCT FROM a.lot_id
                      AND ss.item_part_id IS NOT DISTINCT FROM a.item_part_id
                      AND ss.packaging_level IS NOT DISTINCT FROM a.level
                    ORDER BY a.rank, ss.recorded_at DESC, ss.id DESC
                    LIMIT 1), false)
           END
      FROM (SELECT 1) one
      LEFT JOIN LATERAL packed_in(p_item, p_style, p_lot, p_part, p_level) pk ON true
      LEFT JOIN packaging_type t ON t.code = pk.packaging_type
$$;

DROP FUNCTION subject_asked(uuid, uuid, uuid, uuid, packaging_level);
