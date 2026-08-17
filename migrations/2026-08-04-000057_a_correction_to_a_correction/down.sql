-- Reverse of 2026-08-04-000057_a_correction_to_a_correction.
--
-- The folds go back to netting one level deep, so a correction to a correction stops
-- returning the original to its full quantity: the second correction is subtracted
-- from the first, which is not in the fold, and contributes nothing anywhere.
-- Question 170 reopens and J71 returns to reporting the shape.
--
-- The view goes last, because both functions read it.

-- Migration 56's bodies, restored.
CREATE OR REPLACE FUNCTION projection_fulfilment_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH corrected AS (
        -- What has been taken back off each movement. Grouped rather than joined
        -- one-to-one because a movement may be corrected more than once, in parts.
        SELECT reverses_movement_id AS movement_id, sum(quantity)::bigint AS qty
          FROM stock_movement
         WHERE tenant_id = p_tenant
           AND reverses_movement_id IS NOT NULL
         GROUP BY reverses_movement_id
    ),
    ledger AS (
        SELECT m.fulfilment_line_id,
               -- D99's shape rules over D102's effective quantity. Neither reads
               -- `reason`, which is text with no CHECK.
               sum(m.quantity - coalesce(c.qty, 0)) FILTER (
                   WHERE m.from_location_id IS NOT NULL)                 AS picked,
               sum(m.quantity - coalesce(c.qty, 0)) FILTER (
                   WHERE p.status IN ('sealed', 'despatched'))           AS packed,
               sum(m.quantity - coalesce(c.qty, 0)) FILTER (
                   WHERE m.to_location_id IS NULL
                     AND m.to_package_id IS NULL)                        AS despatched
          FROM stock_movement m
          LEFT JOIN package   p ON p.id = m.to_package_id
          LEFT JOIN corrected c ON c.movement_id = m.id
         WHERE m.tenant_id = p_tenant
           AND m.fulfilment_line_id IS NOT NULL
           -- A correction contributes only as the subtraction above. Its own shape
           -- would otherwise read as work: reversing a despatch is a movement into
           -- a sealed carton.
           AND m.reverses_movement_id IS NULL
         GROUP BY m.fulfilment_line_id
    ),
    intention AS (
        SELECT fulfilment_line_id, sum(quantity)::bigint AS covered
          FROM stock_allocation
         WHERE tenant_id = p_tenant
           AND state IN ('allocated','picking','picked','packed','fulfilled')
           AND fulfilment_line_id IS NOT NULL
         GROUP BY fulfilment_line_id
    ),
    updated AS (
        UPDATE fulfilment_line fl
           SET covered_quantity    = coalesce(i.covered, 0),
               picked_quantity     = coalesce(g.picked, 0),
               packed_quantity     = coalesce(g.packed, 0),
               despatched_quantity = coalesce(g.despatched, 0)
          FROM fulfilment_line l
          LEFT JOIN intention i ON i.fulfilment_line_id = l.id
          LEFT JOIN ledger    g ON g.fulfilment_line_id = l.id
         WHERE fl.id = l.id AND fl.tenant_id = p_tenant
           AND (fl.covered_quantity, fl.picked_quantity,
                fl.packed_quantity, fl.despatched_quantity)
               IS DISTINCT FROM
               (coalesce(i.covered, 0), coalesce(g.picked, 0),
                coalesce(g.packed, 0), coalesce(g.despatched, 0))
        RETURNING fl.id)
    SELECT count(*) INTO touched FROM updated;

    RETURN touched;
END
$$;

ALTER FUNCTION projection_fulfilment_rebuild(uuid) OWNER TO nylonite_projection_owner;

CREATE OR REPLACE FUNCTION projection_expected_supply_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH ordered AS (
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
        -- D68. 35,040 of the 39,565 rows a no-op run rewrote were this one.
        WHERE (e.site_id, e.item_id, e.owner_id, e.status_id, e.expected_from,
               e.expected_to, e.date_confidence, e.quantity_expected)
              IS DISTINCT FROM
              (EXCLUDED.site_id, EXCLUDED.item_id, EXCLUDED.owner_id,
               EXCLUDED.status_id, EXCLUDED.expected_from, EXCLUDED.expected_to,
               EXCLUDED.date_confidence, EXCLUDED.quantity_expected)
        RETURNING e.id)
    SELECT count(*) INTO touched FROM upserted;

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

    -- D61, J26, corrected by D102. Only movements that put goods somewhere count:
    -- a putaway afterwards names the same receipt line and must not be counted as a
    -- second arrival, which is the double-count a stored accumulator would have made
    -- permanent. And an arrival contributes what is left of it after any correction,
    -- which is what makes 96 arriving read as 96.
    UPDATE expected_supply e
       SET quantity_received = coalesce(r.q, 0)
      FROM expected_supply c
      LEFT JOIN (SELECT grl.expected_supply_id,
                        sum(m.quantity - coalesce(x.qty, 0))::bigint AS q
                   FROM stock_movement m
                   JOIN goods_receipt_line grl ON grl.id = m.goods_receipt_line_id
                   LEFT JOIN (SELECT reverses_movement_id AS movement_id,
                                     sum(quantity)::bigint AS qty
                                FROM stock_movement
                               WHERE reverses_movement_id IS NOT NULL
                               GROUP BY reverses_movement_id) x
                          ON x.movement_id = m.id
                  WHERE grl.expected_supply_id IS NOT NULL
                    AND m.reverses_movement_id IS NULL
                    AND m.from_location_id IS NULL AND m.from_package_id IS NULL
                  GROUP BY grl.expected_supply_id) r ON r.expected_supply_id = c.id
     WHERE e.id = c.id AND e.tenant_id = p_tenant
       AND e.quantity_received IS DISTINCT FROM coalesce(r.q, 0);

    UPDATE expected_supply e
       SET closed_at = now(), closed_reason = 'cancelled'
      FROM purchase_order_line l
      JOIN purchase_order po ON po.id = l.purchase_order_id
     WHERE e.purchase_order_line_id = l.id
       AND e.tenant_id = p_tenant
       AND po.state = 'cancelled'
       AND e.closed_at IS NULL;

    -- D65. Delivered in full, so it promises nothing further. A short receipt is
    -- deliberately not here: it stays open until somebody agrees to release the
    -- remainder, which is `short_closed` and is a conversation rather than
    -- arithmetic.
    UPDATE expected_supply e
       SET closed_at = now(), closed_reason = 'received_in_full'
     WHERE e.tenant_id = p_tenant
       AND e.closed_at IS NULL
       AND e.quantity_outstanding <= 0;

    RETURN touched;
END
$$;

ALTER FUNCTION projection_expected_supply_rebuild(uuid) OWNER TO nylonite_projection_owner;

DROP VIEW stock_movement_effective;

COMMENT ON COLUMN expected_supply.quantity_received IS
    '@projection of stock_movement grouped by the receipt line''s supply row, net of '
    'corrections against each arrival, via projection_expected_supply_rebuild '
    '(D45, D61, D102, J26).';

COMMENT ON COLUMN fulfilment_line.picked_quantity IS
    '@projection of stock_movement via projection_fulfilment_rebuild (D99, D100, D102, '
    'J68). Movements naming this line that left a storage location, net of corrections.';
COMMENT ON COLUMN fulfilment_line.packed_quantity IS
    '@projection of stock_movement and package.status via projection_fulfilment_rebuild '
    '(D99, D100, D102, J68). Movements naming this line into a carton whose winning '
    'status is sealed or despatched, net of corrections. Not package.sealed_at, which '
    'the application may UPDATE.';
COMMENT ON COLUMN fulfilment_line.despatched_quantity IS
    '@projection of stock_movement via projection_fulfilment_rebuild (D99, D100, D102, '
    'J68). Movements naming this line with no to side at all, which is D45''s arrival '
    'test mirrored, net of corrections.';
