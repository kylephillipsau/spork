-- Migration 25: a promise that has been delivered is still a promise.
--
-- D65, found by question 122's year of seeded history on its first load.
--
-- D24 specifies the receiving index set and says the partial predicate is the
-- whole point:
--
--   expected_supply (tenant_id, inbound_shipment_id) WHERE closed_at IS NULL
--
--   "Inbound at this scale is roughly thirty deliveries a day at thirty lines, so
--    about 230,000 expected_supply rows a year a site, of which a few thousand
--    are open at any moment. A year in, the live set is about one percent of the
--    table, and an unpartial index makes the planner do the wrong thing."
--
-- **The premise is false, and only a year of data says so.** Measured against the
-- generated history: 35,040 promises, of which **32,963 are fully received and
-- still open**. The live set is not one percent. It is a hundred percent, because
-- `projection_expected_supply_rebuild` closes a row when its purchase order is
-- cancelled and on no other occasion.
--
-- So the partial index D24 designed would have indexed the entire table, and the
-- reasoning that justified it would have read as sound the whole time. This is
-- exactly the class of thing question 122 was raised to catch: *"the receiving
-- queries are written and reasoned about, not measured."*

-- ---------------------------------------------------------------------------
-- What closes, and what does not
-- ---------------------------------------------------------------------------
--
-- `received_in_full` was already in D24's `closed_reason` set, waiting for
-- something to use it.
--
-- **Only a fully delivered promise closes itself.** A short receipt leaves
-- `quantity_outstanding` above zero and stays open, because closing it is
-- `short_closed` — the supplier agreeing to release the remainder — and that is a
-- conversation with a counterparty rather than arithmetic. D24 lists the two
-- reasons separately for this reason, and a rebuild that collapsed them would be
-- deciding a commercial question by rounding.
--
-- The close is idempotent and one-way here: a row already closed for another
-- reason keeps the reason it has, because `cancelled` and `received_in_full` are
-- different facts about how a promise ended and the later run should not rewrite
-- the earlier answer.

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
$$;

ALTER FUNCTION projection_expected_supply_rebuild(uuid) OWNER TO nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- The index D24 designed, now that its predicate means something
-- ---------------------------------------------------------------------------
--
-- D24's version keys on `inbound_shipment_id`, which arrives with the assertion
-- arm. This is the same index over the arm that exists, and it is partial for
-- D24's stated reason — which is a reason rather than a guess now that the live
-- set is measurable.

CREATE INDEX expected_supply_open_idx
    ON expected_supply (tenant_id, site_id, expected_to)
    WHERE closed_at IS NULL;
