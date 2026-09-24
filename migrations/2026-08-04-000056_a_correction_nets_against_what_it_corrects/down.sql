-- Reverse of 2026-08-04-000056_a_correction_nets_against_what_it_corrects.
--
-- Both folds go back to counting an arrival and a pick at their full quantity
-- however much of each was later corrected, and the outbound one goes back to
-- dropping a partly-corrected movement entirely rather than netting it. A receipt
-- where 96 arrived reads 100 again, and a pick of twenty with five reversed reads
-- zero. Question 168 reopens and D100's correction is undone with it.

-- Migration 54's body, restored.
CREATE OR REPLACE FUNCTION projection_fulfilment_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    WITH ledger AS (
        SELECT m.fulfilment_line_id,
               -- D99's shape rules. Neither reads `reason`, which is text with no
               -- CHECK: re-handling after a pick is holder-to-holder, so a
               -- consolidation from tote to carton names the same line and is
               -- correctly not a second pick.
               sum(m.quantity) FILTER (
                   WHERE m.from_location_id IS NOT NULL)                 AS picked,
               sum(m.quantity) FILTER (
                   WHERE p.status IN ('sealed', 'despatched'))           AS packed,
               sum(m.quantity) FILTER (
                   WHERE m.to_location_id IS NULL
                     AND m.to_package_id IS NULL)                        AS despatched
          FROM stock_movement m
          LEFT JOIN package p ON p.id = m.to_package_id
         WHERE m.tenant_id = p_tenant
           AND m.fulfilment_line_id IS NOT NULL
           -- Section 2: the pair leaves together.
           AND m.reverses_movement_id IS NULL
           AND NOT EXISTS (SELECT 1 FROM stock_movement r
                            WHERE r.reverses_movement_id = m.id)
         GROUP BY m.fulfilment_line_id
    ),
    intention AS (
        -- Unchanged, and deliberately so. `fulfilled` is included here and excluded
        -- from stock.allocated_quantity, which is the whole reason those two columns
        -- stopped sharing a name.
        SELECT fulfilment_line_id, sum(quantity)::bigint AS covered
          FROM stock_allocation
         WHERE tenant_id = p_tenant
           AND state IN ('allocated','picking','picked','packed','fulfilled')
           AND fulfilment_line_id IS NOT NULL
         GROUP BY fulfilment_line_id
    ),
    updated AS (
        -- LEFT JOIN from the line rather than FROM either fold, for the reason D53
        -- gave and which now applies twice: a line whose last allocation was
        -- released, or whose only movement was reversed, has no group on that side
        -- and must go to zero. Joining the other way would leave the previous
        -- numbers standing, which is the failure mode a rebuild exists to be
        -- immune to.
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

ALTER FUNCTION projection_fulfilment_rebuild(uuid) OWNER TO spork_projection_owner;

-- Migration 27's body, restored -- which is the latest, not migration 21's. D65's
-- received_in_full close and D68's idempotency guard both live here, and rebuilding
-- this down from the wrong migration would revert two decisions silently.
CREATE OR REPLACE FUNCTION public.projection_expected_supply_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

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

    UPDATE expected_supply e
       SET quantity_received = coalesce(r.q, 0)
      FROM expected_supply c
      LEFT JOIN (SELECT grl.expected_supply_id, sum(m.quantity)::bigint AS q
                   FROM stock_movement m
                   JOIN goods_receipt_line grl ON grl.id = m.goods_receipt_line_id
                  WHERE grl.expected_supply_id IS NOT NULL
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
$function$;

ALTER FUNCTION projection_expected_supply_rebuild(uuid) OWNER TO spork_projection_owner;

COMMENT ON COLUMN expected_supply.quantity_received IS
    '@projection of stock_movement grouped by the receipt line''s supply row, via projection_expected_supply_rebuild (D45, D61, J26).';

COMMENT ON COLUMN fulfilment_line.picked_quantity IS
    '@projection of stock_movement via projection_fulfilment_rebuild (D99, D100, J68). '
    'Movements naming this line that left a storage location.';
COMMENT ON COLUMN fulfilment_line.packed_quantity IS
    '@projection of stock_movement and package.status via projection_fulfilment_rebuild '
    '(D99, D100, J68). Movements naming this line into a carton whose winning status '
    'is sealed or despatched. Not package.sealed_at, which the application may UPDATE.';
COMMENT ON COLUMN fulfilment_line.despatched_quantity IS
    '@projection of stock_movement via projection_fulfilment_rebuild (D99, D100, J68). '
    'Movements naming this line with no to side at all, which is D45''s arrival test '
    'mirrored.';
