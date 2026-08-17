-- Migration 45: the two halves of a receipt, joined.
--
-- D91, settling question 157, which D90 raised by failing to enforce half of a
-- rule: D21 freezes a resolution once "an `assertion_check` **or a
-- `goods_receipt_line`** references it", and only the first was checkable.
--
-- The reason is chronological rather than considered. D62 built the receiving
-- path before the assertion tables existed, and D77 built the assertion tables
-- without going back, so **what a supplier said would arrive and what we counted
-- have never been joined by a foreign key.** Two tables about the same pallet,
-- with no relationship between them.
--
-- The cost is larger than D90's freeze. D21 describes the receipt comparing
-- declared values *"line by line"* -- lot, expiry, quantity -- and **no query
-- could perform that comparison**, which for a food-safety distributor is the
-- one thing the dock most needs to catch.

-- ---------------------------------------------------------------------------
-- 1. Nullable, because blind receipt is a schema property
-- ---------------------------------------------------------------------------
--
-- D21: *"nothing on it is NOT NULL that requires an assertion, so blind receipt
-- -- rung zero of the degradation ladder -- is a schema property, not a workflow
-- branch."* A truck with no paperwork still gets received, and the column being
-- optional is what says so. S17 asserts it across the whole assertion set.

ALTER TABLE goods_receipt_line
    ADD COLUMN asserted_unit_content_id uuid,
    ADD CONSTRAINT goods_receipt_line_asserted_content_fk
        FOREIGN KEY (asserted_unit_content_id, tenant_id)
        REFERENCES asserted_unit_content(id, tenant_id);

COMMENT ON COLUMN goods_receipt_line.asserted_unit_content_id IS
    'The declared line this count was made against, when there was one. Nullable '
    'because blind receipt is rung zero of the degradation ladder and must stay a '
    'schema property. D21, D91.';

CREATE INDEX goods_receipt_line_asserted_content_idx
    ON goods_receipt_line (asserted_unit_content_id)
    WHERE asserted_unit_content_id IS NOT NULL;

GRANT INSERT (asserted_unit_content_id), UPDATE (asserted_unit_content_id)
    ON goods_receipt_line TO nylonite_app;

-- ---------------------------------------------------------------------------
-- 2. D21's freeze, both halves
-- ---------------------------------------------------------------------------
--
-- D90 could only count checks. Now it can count receipts, which is the sentence
-- D21 actually wrote.

CREATE OR REPLACE FUNCTION asserted_unit_content_resolve(
        p_content uuid, p_item uuid, p_po_line uuid, p_actor uuid, p_method text)
    RETURNS void
    LANGUAGE plpgsql
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    checks   bigint;
    receipts bigint;
BEGIN
    PERFORM 1 FROM asserted_unit_content WHERE id = p_content FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'no asserted unit content %', p_content;
    END IF;

    SELECT count(*) INTO checks FROM assertion_check
     WHERE asserted_unit_content_id = p_content;
    SELECT count(*) INTO receipts FROM goods_receipt_line
     WHERE asserted_unit_content_id = p_content;

    IF checks + receipts > 0 THEN
        RAISE EXCEPTION
            'asserted unit content % is frozen: % check(s) and % receipt line(s) have used it',
            p_content, checks, receipts
            USING HINT = 'a correction writes a new assertion, which is D21''s rule and D51''s shape';
    END IF;

    UPDATE asserted_unit_content
       SET resolved_item_id = coalesce(p_item, resolved_item_id),
           resolved_purchase_order_line_id =
               coalesce(p_po_line, resolved_purchase_order_line_id),
           resolved_at = now(), resolved_by_id = p_actor, resolution_method = p_method
     WHERE id = p_content;
END
$$;

-- ---------------------------------------------------------------------------
-- 3. The comparison D21 describes, as a query somebody can run
-- ---------------------------------------------------------------------------
--
-- Declared against counted, for one receipt. Not a stored variance: the numbers
-- either side are facts and their difference is arithmetic, which is D23's rule
-- about derived values and D73's about counts that can be re-derived.
--
-- `expiry` is the one that matters most here and the one nothing else would catch:
-- a supplier declaring a date the goods do not carry is not a quantity problem and
-- would pass every count check in the system. Lot and expiry need no conversion,
-- which is why they are the two this function compares outright.
--
-- **Quantity does need one, and the two sides do not speak the same language.**
-- `asserted_unit_content.quantity` is base units. `goods_receipt_line.entered_quantity`
-- is a count of whatever `entered_packaging_level` says -- ten cartons, in the
-- fixture -- and D58 is explicit that a packaging level is not a unit, because its
-- factor varies by item and by date. Subtracting one from the other produces a
-- number that looks authoritative and is wrong by the case pack.
--
-- So the base comparison reads the ledger rather than converting anything. D45
-- already defines what arrived as `SUM(stock_movement.quantity)` grouped by
-- `goods_receipt_line_id`, in base units by D10, and the two projections that fold
-- it exclude internal moves the same way -- a putaway names the same receipt line
-- and is not a second arrival. This is that predicate's third use rather than a
-- fourth definition of the same sentence.
--
-- The entered pair is reported beside it in its own vocabulary and never
-- subtracted. Where the conversion between them should live is question 158.

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

COMMENT ON FUNCTION goods_receipt_variance(uuid) IS
    'What was declared against what was counted, line by line -- quantity, lot and '
    'expiry. D21 describes this comparison and until D91 no query could perform '
    'it, because the two halves of a receipt were not joined. The base quantities '
    'are commensurable and subtracted; the entered pair is reported in its own '
    'vocabulary and is not, because a packaging level is not a unit. D21, D91.';

GRANT EXECUTE ON FUNCTION goods_receipt_variance(uuid)
    TO nylonite_app, nylonite_platform, nylonite_scheduler;
