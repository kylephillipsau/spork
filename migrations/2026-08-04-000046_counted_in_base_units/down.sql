-- Reverse of 2026-08-04-000046_counted_in_base_units.
--
-- The receipt line goes back to storing a vocabulary and not a meaning, and the
-- variance goes back to reading the ledger for a number the line should hold.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM goods_receipt_line
     WHERE quantity IS NOT NULL
       AND NOT EXISTS (SELECT 1 FROM stock_movement m
                        WHERE m.goods_receipt_line_id = goods_receipt_line.id
                          AND m.from_location_id IS NULL
                          AND m.from_package_id IS NULL);
    IF n > 0 THEN
        RAISE NOTICE 'reversing D92 destroys the counted quantity of % line(s) that landed '
                     'nothing in the ledger: what a rejected line counted becomes unrecoverable', n;
    END IF;
END $$;

DROP FUNCTION goods_receipt_variance(uuid);

CREATE FUNCTION goods_receipt_variance(p_receipt uuid)
    RETURNS TABLE (
        goods_receipt_line_id     uuid,
        declared_base_quantity    numeric,
        counted_base_quantity     numeric,
        base_quantity_variance    numeric,
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
           arrived.base,
           arrived.base - cc.quantity,
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
             AND m.from_package_id IS NULL) arrived ON true
     WHERE l.goods_receipt_id = p_receipt
$$;

GRANT EXECUTE ON FUNCTION goods_receipt_variance(uuid)
    TO nylonite_app, nylonite_platform, nylonite_scheduler;

DROP FUNCTION IF EXISTS packing_factor(uuid, packaging_level);

ALTER TABLE goods_receipt_line
    DROP CONSTRAINT IF EXISTS goods_receipt_line_quantity_positive_ck,
    DROP CONSTRAINT IF EXISTS goods_receipt_line_quantity_pair_ck;
ALTER TABLE goods_receipt_line DROP COLUMN IF EXISTS quantity;
