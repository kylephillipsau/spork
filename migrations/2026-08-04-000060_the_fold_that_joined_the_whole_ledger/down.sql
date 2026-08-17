-- Migration 60 down: restore the global-view joins. Question 164 reopens on the
-- measurement that made expected_supply dominate the year fold.

CREATE OR REPLACE FUNCTION public.projection_fulfilment_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    -- last changed: migration 58 (D104)
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH ledger AS (
        SELECT m.fulfilment_line_id,
               -- D99's shape rules over D103's effective quantity. None reads
               -- `reason`, which is text with no CHECK.
               sum(v.effective_quantity) FILTER (
                   WHERE m.from_location_id IS NOT NULL)                 AS picked,
               sum(v.effective_quantity) FILTER (
                   WHERE p.status IN ('sealed', 'despatched'))           AS packed,
               sum(v.effective_quantity) FILTER (
                   WHERE m.to_location_id IS NULL
                     AND m.to_package_id IS NULL)                        AS despatched
          FROM stock_movement m
          JOIN stock_movement_effective v
            ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
          LEFT JOIN package p ON p.id = m.to_package_id
         WHERE m.tenant_id = p_tenant
           AND m.fulfilment_line_id IS NOT NULL
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
$function$;

CREATE OR REPLACE FUNCTION public.projection_expected_supply_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    -- last changed: migration 58 (D104)
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

    -- D61, J26, corrected by D102 and D103. Only movements that put goods somewhere
    -- count: a putaway afterwards names the same receipt line and must not be counted
    -- as a second arrival. And an arrival contributes what is left of it after its
    -- whole correction subtree, not merely after the first level of it.
    UPDATE expected_supply e
       SET quantity_received = coalesce(r.q, 0)
      FROM expected_supply c
      LEFT JOIN (SELECT grl.expected_supply_id,
                        sum(v.effective_quantity)::bigint AS q
                   FROM stock_movement m
                   JOIN stock_movement_effective v
                     ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
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
