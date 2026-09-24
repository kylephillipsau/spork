-- Reverse of 2026-08-04-000021_goods_receipt.

DELETE FROM projection_rebuild
 WHERE table_name = 'expected_supply' AND column_name = 'quantity_received';

COMMENT ON COLUMN expected_supply.quantity_received IS
    '@projection(pending) of receipts; goods_receipt_line arrives next. J26.';

-- Migration 20's body, restored.
CREATE OR REPLACE FUNCTION projection_expected_supply_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    WITH ordered AS (
        -- Only an issued order promises anything. A draft is a document somebody
        -- is still writing and a cancelled one promises nothing, which is the
        -- DECLARED half of D25's split doing exactly the work it was split for.
        SELECT l.id AS line_id, l.tenant_id, po.site_id, l.item_id,
               l.owner_party_id, l.status_id,
               l.expected_from, l.expected_to, l.quantity_ordered
          FROM purchase_order_line l
          JOIN purchase_order po ON po.id = l.purchase_order_id
         WHERE l.tenant_id = p_tenant AND po.state = 'issued'
    ),
    upserted AS (
        INSERT INTO expected_supply AS e (
            tenant_id, site_id, item_id, owner_id, status_id,
            purchase_order_line_id, expected_from, expected_to, date_confidence,
            quantity_expected)
        SELECT o.tenant_id, o.site_id, o.item_id, o.owner_party_id, o.status_id,
               o.line_id, o.expected_from, o.expected_to,
               -- The dates came off the order rather than an advice, and saying
               -- so is what stops a promise being made against a guess.
               'ordered'::date_confidence,
               o.quantity_ordered
          FROM ordered o
        ON CONFLICT (tenant_id, purchase_order_line_id)
            WHERE purchase_order_line_id IS NOT NULL
        DO UPDATE SET site_id = EXCLUDED.site_id,
                      item_id = EXCLUDED.item_id,
                      owner_id = EXCLUDED.owner_id,
                      status_id = EXCLUDED.status_id,
                      expected_from = EXCLUDED.expected_from,
                      expected_to = EXCLUDED.expected_to,
                      date_confidence = EXCLUDED.date_confidence,
                      quantity_expected = EXCLUDED.quantity_expected
        RETURNING e.id)
    SELECT count(*) INTO touched FROM upserted;

    -- J4. Active allocations naming this row, on the same state set J3 uses for
    -- the cell: a despatched or released allocation has stopped claiming supply.
    UPDATE expected_supply e
       SET quantity_allocated = coalesce(a.q, 0)
      FROM expected_supply c
      LEFT JOIN (SELECT expected_supply_id, sum(quantity)::bigint AS q
                   FROM stock_allocation
                  WHERE state IN ('allocated','picking','picked','packed')
                    AND expected_supply_id IS NOT NULL
                  GROUP BY expected_supply_id) a ON a.expected_supply_id = c.id
     WHERE e.id = c.id AND e.tenant_id = p_tenant
       AND e.quantity_allocated IS DISTINCT FROM coalesce(a.q, 0);

    -- A promise whose order was cancelled leaves the promisable pool, with the
    -- reason recorded rather than the row deleted. D24's closed_reason set exists
    -- so that "why is this not promisable" always has an answer.
    UPDATE expected_supply e
       SET closed_at = now(), closed_reason = 'cancelled'
      FROM purchase_order_line l
      JOIN purchase_order po ON po.id = l.purchase_order_id
     WHERE e.purchase_order_line_id = l.id
       AND e.tenant_id = p_tenant
       AND po.state = 'cancelled'
       AND e.closed_at IS NULL;

    RETURN touched;
END
$$;

ALTER FUNCTION projection_expected_supply_rebuild(uuid) OWNER TO spork_projection_owner;

-- The received quantity had a source and now does not. Zeroing it is the honest
-- reversal: leaving the folded numbers behind would leave a projection standing
-- with nothing that could ever reproduce it.
UPDATE expected_supply SET quantity_received = 0 WHERE quantity_received <> 0;

REVOKE SELECT ON goods_receipt_line FROM spork_projection_owner;
REVOKE INSERT (goods_receipt_line_id) ON stock_movement FROM spork_app;

DROP INDEX IF EXISTS stock_movement_receipt_line_idx;
ALTER TABLE stock_movement
    DROP CONSTRAINT IF EXISTS stock_movement_cause_ck,
    DROP CONSTRAINT IF EXISTS stock_movement_receipt_line_fk;
ALTER TABLE stock_movement DROP COLUMN IF EXISTS goods_receipt_line_id;

DROP INDEX IF EXISTS goods_receipt_po_idx;
DROP INDEX IF EXISTS goods_receipt_line_supply_idx;
DROP INDEX IF EXISTS goods_receipt_line_receipt_idx;

DROP TABLE IF EXISTS goods_receipt_line;
DROP TABLE IF EXISTS goods_receipt;
