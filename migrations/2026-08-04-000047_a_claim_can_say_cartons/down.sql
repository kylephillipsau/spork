-- Reverse of 2026-08-04-000047_a_claim_can_say_cartons.
--
-- The claim goes back to having one vocabulary and no record of the author's.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM asserted_unit_content WHERE raw_unit_code IS NOT NULL;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D93 destroys the author''s own unit code on % claim line(s): '
                     'what they said becomes unrecoverable and only our reading of it survives', n;
    END IF;

    SELECT count(*) INTO n FROM asserted_unit_content
     WHERE resolved_packaging_level IS NOT NULL;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D93 destroys the declared packaging level of % claim line(s): '
                     'a claim stating cartons stops being representable', n;
    END IF;
END $$;

DROP INDEX IF EXISTS asserted_unit_content_config_idx;

ALTER TABLE asserted_unit_content
    DROP CONSTRAINT IF EXISTS asserted_unit_content_config_fk,
    DROP CONSTRAINT IF EXISTS asserted_unit_content_config_needs_level_ck,
    DROP CONSTRAINT IF EXISTS asserted_unit_content_level_needs_config_ck,
    DROP CONSTRAINT IF EXISTS asserted_unit_content_one_vocabulary_ck;

ALTER TABLE asserted_unit_content
    DROP COLUMN IF EXISTS item_packing_config_id,
    DROP COLUMN IF EXISTS resolved_packaging_level,
    DROP COLUMN IF EXISTS raw_unit_code;

-- Before the function, which reads the column under its old name.
ALTER TABLE asserted_unit_content
    RENAME COLUMN resolved_unit_id TO entered_unit_id;

CREATE OR REPLACE FUNCTION goods_receipt_variance(p_receipt uuid)
    RETURNS TABLE (
        goods_receipt_line_id     uuid,
        declared_base_quantity    numeric,
        counted_base_quantity     bigint,
        base_quantity_variance    numeric,
        landed_base_quantity      numeric,
        declared_entered_quantity numeric,
        declared_entered_unit     text,
        counted_entered_quantity  bigint,
        counted_packaging_level   packaging_level,
        declared_lot              text,
        counted_lot               text,
        declared_expiry           date,
        counted_expiry            date)
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    SELECT l.id,
           cc.quantity,
           l.quantity,
           l.quantity - cc.quantity,
           landed.base,
           cc.entered_quantity,
           u.code,
           l.entered_quantity,
           l.entered_packaging_level,
           cc.lot_code,
           lo.code,
           cc.expiry_date,
           lo.expiry_date
      FROM goods_receipt_line l
      JOIN asserted_unit_content cc ON cc.id = l.asserted_unit_content_id
      LEFT JOIN unit u ON u.id = cc.entered_unit_id
      LEFT JOIN lot lo ON lo.id = l.lot_id
      LEFT JOIN LATERAL (
          SELECT sum(m.quantity) AS base
            FROM stock_movement m
           WHERE m.goods_receipt_line_id = l.id
             AND m.from_location_id IS NULL
             AND m.from_package_id IS NULL) landed ON true
     WHERE l.goods_receipt_id = p_receipt
$$;

COMMENT ON FUNCTION asserted_unit_content_resolve(uuid, uuid, uuid, uuid, text) IS NULL;
