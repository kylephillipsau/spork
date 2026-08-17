-- Reverse of 2026-08-04-000027_projection_churn.
--
-- Restores the unguarded bodies, which rewrite every row every run. The
-- property test that asserts otherwise fails after this, which is correct: the
-- claim and the code go back together.

CREATE OR REPLACE FUNCTION public.projection_stock_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH ledger AS (
        SELECT to_location_id AS holder_location_id, to_package_id AS holder_package_id,
               item_id, to_lot_id AS lot_id, to_status_id AS status_id,
               to_owner_id AS owner_id,
               quantity AS qty, catch_weight_g AS wt
          FROM stock_movement
         WHERE tenant_id = p_tenant
           AND num_nonnulls(to_location_id, to_package_id) = 1
        UNION ALL
        SELECT from_location_id, from_package_id,
               item_id, from_lot_id, from_status_id, from_owner_id,
               -quantity, -catch_weight_g
          FROM stock_movement
         WHERE tenant_id = p_tenant
           AND num_nonnulls(from_location_id, from_package_id) = 1
    ),
    folded AS (
        SELECT holder_location_id, holder_package_id, item_id, lot_id,
               status_id, owner_id,
               sum(qty) AS quantity,
               sum(wt)  AS weight_g
          FROM ledger
         GROUP BY 1, 2, 3, 4, 5, 6
    ),
    upserted AS (
        INSERT INTO stock AS s (
            tenant_id, item_id, holder_location_id, holder_package_id,
            lot_id, status_id, owner_id, quantity, weight_g,
            resolved_location_id, site_id)
        SELECT p_tenant, f.item_id, f.holder_location_id, f.holder_package_id,
               f.lot_id, f.status_id, f.owner_id, f.quantity, f.weight_g,
               f.holder_location_id,
               l.site_id
          FROM folded f
          LEFT JOIN location l ON l.id = f.holder_location_id
        ON CONFLICT (tenant_id, item_id, holder_location_id, holder_package_id,
                     lot_id, status_id, owner_id)
        DO UPDATE SET quantity = EXCLUDED.quantity,
                      weight_g = EXCLUDED.weight_g,
                      resolved_location_id = EXCLUDED.resolved_location_id,
                      site_id = EXCLUDED.site_id
        RETURNING s.id)
    SELECT count(*) INTO touched FROM upserted;

    UPDATE stock s
       SET quantity = 0, weight_g = NULL
     WHERE s.tenant_id = p_tenant
       AND s.quantity <> 0
       AND NOT EXISTS (
           SELECT 1 FROM stock_movement m
            WHERE m.tenant_id = p_tenant AND m.item_id = s.item_id);

    -- D12 as narrowed by D24, and J3's exact predicate. Cell-bound claims only,
    -- and never a reference test: a terminal allocation still holds a stock_id
    -- and contributes nothing. `fulfilled` is excluded here and included on the
    -- commitment side, which is the whole reason those two columns stopped
    -- sharing a name.
    UPDATE stock s
       SET allocated_quantity = coalesce(a.q, 0)
      FROM stock c
      LEFT JOIN (SELECT stock_id, sum(quantity)::bigint AS q
                   FROM stock_allocation
                  WHERE state IN ('allocated','picking','picked','packed')
                    AND stock_id IS NOT NULL
                  GROUP BY stock_id) a ON a.stock_id = c.id
     WHERE s.id = c.id AND s.tenant_id = p_tenant
       AND s.allocated_quantity IS DISTINCT FROM coalesce(a.q, 0);

    RETURN touched;
END
$function$;

ALTER FUNCTION projection_stock_rebuild(uuid) OWNER TO nylonite_projection_owner;

CREATE OR REPLACE FUNCTION public.projection_expected_supply_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
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
$function$;

ALTER FUNCTION projection_expected_supply_rebuild(uuid) OWNER TO nylonite_projection_owner;

CREATE OR REPLACE FUNCTION public.projection_order_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
    touched_lines bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    -- Last writer per covered column, in register order. Not one winning row:
    -- an amendment that changed only the promised window must not clear a state
    -- set by an earlier one, which is why the fold is per column rather than per
    -- row. Same shape as J6 and the same ordering.
    WITH folded AS (
        SELECT order_id,
               (array_remove(array_agg(new_promised_from ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS promised_from,
               (array_remove(array_agg(new_promised_to ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS promised_to,
               (array_remove(array_agg(new_required_by ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS required_by,
               (array_remove(array_agg(new_state ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS state
          FROM intention_amendment
         WHERE tenant_id = p_tenant
           AND order_line_id IS NULL
         GROUP BY order_id
    ),
    updated AS (
        UPDATE "order" o
           SET promised_from = COALESCE(f.promised_from, o.promised_from),
               promised_to   = COALESCE(f.promised_to,   o.promised_to),
               required_by   = COALESCE(f.required_by,   o.required_by),
               state         = COALESCE(f.state,         o.state)
          FROM folded f
         WHERE o.id = f.order_id AND o.tenant_id = p_tenant
        RETURNING o.id)
    SELECT count(*) INTO touched FROM updated;

    -- D51. The same fold under the same ordering, grouped by line.
    --
    -- The price pair travels together because `intention_amendment_price_pair_ck`
    -- guarantees no row ever set one half alone, so folding the halves separately
    -- can never produce a combination no amendment wrote.
    WITH folded_line AS (
        SELECT order_line_id,
               (array_remove(array_agg(new_quantity_ordered ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS quantity_ordered,
               (array_remove(array_agg(new_unit_price_minor ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS unit_price_minor,
               (array_remove(array_agg(new_price_basis_quantity ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS price_basis_quantity,
               (array_remove(array_agg(new_line_state ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS line_state
          FROM intention_amendment
         WHERE tenant_id = p_tenant
           AND order_line_id IS NOT NULL
         GROUP BY order_line_id
    ),
    updated_line AS (
        UPDATE order_line l
           SET quantity_ordered     = COALESCE(f.quantity_ordered,     l.quantity_ordered),
               unit_price_minor     = COALESCE(f.unit_price_minor,     l.unit_price_minor),
               price_basis_quantity = COALESCE(f.price_basis_quantity, l.price_basis_quantity),
               line_state           = COALESCE(f.line_state,           l.line_state)
          FROM folded_line f
         WHERE l.id = f.order_line_id AND l.tenant_id = p_tenant
        RETURNING l.id)
    SELECT count(*) INTO touched_lines FROM updated_line;

    RETURN touched + touched_lines;
END
$function$;

ALTER FUNCTION projection_order_rebuild(uuid) OWNER TO nylonite_projection_owner;

CREATE OR REPLACE FUNCTION public.projection_package_stamp(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH winner AS (
        SELECT DISTINCT ON (package_id) package_id, id, occurred_at
          FROM package_event
         WHERE tenant_id = p_tenant AND asserts_placement
         ORDER BY package_id, occurred_at DESC, recorded_at DESC, id DESC
    )
    UPDATE package p
       SET placement_event_id = w.id, placement_occurred_at = w.occurred_at
      FROM winner w
     WHERE p.id = w.package_id AND p.tenant_id = p_tenant;

    GET DIAGNOSTICS touched = ROW_COUNT;
    RETURN touched;
END
$function$;

ALTER FUNCTION projection_package_stamp(uuid) OWNER TO nylonite_projection_owner;

CREATE OR REPLACE FUNCTION public.projection_stock_resolve_locations(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    UPDATE stock s
       SET resolved_location_id = COALESCE(s.holder_location_id, pkg.resolved_location_id)
      FROM package pkg
     WHERE s.tenant_id = p_tenant
       AND s.holder_package_id = pkg.id;

    GET DIAGNOSTICS touched = ROW_COUNT;

    UPDATE stock s
       SET site_id = l.site_id
      FROM location l
     WHERE s.tenant_id = p_tenant AND s.resolved_location_id = l.id;

    RETURN touched;
END
$function$;

ALTER FUNCTION projection_stock_resolve_locations(uuid) OWNER TO nylonite_projection_owner;

CREATE OR REPLACE FUNCTION public.projection_item_class_closure_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH RECURSIVE tree AS (
        -- Every node is its own ancestor at depth zero. That row is what makes
        -- "ancestor-or-self" a single lookup rather than a lookup and a special
        -- case, which is the whole reason D22 calls the matching language
        -- cardinality one.
        SELECT c.tenant_id, c.id AS ancestor_id, c.id AS descendant_id, 0 AS depth
          FROM item_class c
         WHERE c.tenant_id = p_tenant
        UNION ALL
        SELECT t.tenant_id, t.ancestor_id, c.id, t.depth + 1
          FROM tree t
          JOIN item_class c
            ON c.parent_id = t.descendant_id AND c.tenant_id = t.tenant_id
    ) CYCLE descendant_id SET is_cycle USING cycle_path,
    computed AS (
        SELECT tenant_id, ancestor_id, descendant_id, min(depth) AS depth
          FROM tree
         WHERE NOT is_cycle
         GROUP BY tenant_id, ancestor_id, descendant_id
    ),
    removed AS (
        DELETE FROM item_class_closure x
         WHERE x.tenant_id = p_tenant
           AND NOT EXISTS (SELECT 1 FROM computed c
                            WHERE c.ancestor_id = x.ancestor_id
                              AND c.descendant_id = x.descendant_id)
        RETURNING 1
    ),
    written AS (
        INSERT INTO item_class_closure AS x (tenant_id, ancestor_id, descendant_id, depth)
        SELECT tenant_id, ancestor_id, descendant_id, depth FROM computed
        ON CONFLICT (ancestor_id, descendant_id)
        DO UPDATE SET depth = EXCLUDED.depth, tenant_id = EXCLUDED.tenant_id
        RETURNING 1)
    SELECT (SELECT count(*) FROM written) + (SELECT count(*) FROM removed) INTO touched;

    RETURN touched;
END
$function$;

ALTER FUNCTION projection_item_class_closure_rebuild(uuid) OWNER TO nylonite_projection_owner;

CREATE OR REPLACE FUNCTION public.projection_party_class_closure_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH RECURSIVE tree AS (
        SELECT c.tenant_id, c.id AS ancestor_id, c.id AS descendant_id, 0 AS depth
          FROM party_class c
         WHERE c.tenant_id = p_tenant
        UNION ALL
        SELECT t.tenant_id, t.ancestor_id, c.id, t.depth + 1
          FROM tree t
          JOIN party_class c
            ON c.parent_id = t.descendant_id AND c.tenant_id = t.tenant_id
    ) CYCLE descendant_id SET is_cycle USING cycle_path,
    computed AS (
        SELECT tenant_id, ancestor_id, descendant_id, min(depth) AS depth
          FROM tree
         WHERE NOT is_cycle
         GROUP BY tenant_id, ancestor_id, descendant_id
    ),
    removed AS (
        DELETE FROM party_class_closure x
         WHERE x.tenant_id = p_tenant
           AND NOT EXISTS (SELECT 1 FROM computed c
                            WHERE c.ancestor_id = x.ancestor_id
                              AND c.descendant_id = x.descendant_id)
        RETURNING 1
    ),
    written AS (
        INSERT INTO party_class_closure AS x (tenant_id, ancestor_id, descendant_id, depth)
        SELECT tenant_id, ancestor_id, descendant_id, depth FROM computed
        ON CONFLICT (ancestor_id, descendant_id)
        DO UPDATE SET depth = EXCLUDED.depth, tenant_id = EXCLUDED.tenant_id
        RETURNING 1)
    SELECT (SELECT count(*) FROM written) + (SELECT count(*) FROM removed) INTO touched;

    RETURN touched;
END
$function$;

ALTER FUNCTION projection_party_class_closure_rebuild(uuid) OWNER TO nylonite_projection_owner;

CREATE OR REPLACE FUNCTION public.projection_package_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    -- FORCE RLS applies to the definer, so the rebuild scopes itself to its
    -- argument or reads nothing. Same reason as projection_stock_rebuild.
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH winning_placement AS (
        -- The last placement assertion in register order wins. DISTINCT ON with
        -- a matching ORDER BY is the compare-and-set, evaluated over the whole
        -- log rather than incrementally, which is what makes a rebuild from
        -- scratch agree with an incremental apply.
        SELECT DISTINCT ON (package_id)
               package_id, parent_package_id, location_id
          FROM package_event
         WHERE tenant_id = p_tenant AND asserts_placement
         ORDER BY package_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    winning_identity AS (
        SELECT DISTINCT ON (package_id)
               package_id, sscc, barcode,
               CASE WHEN sscc IS NOT NULL THEN 'sscc'
                    WHEN barcode IS NOT NULL THEN 'barcode' END AS identifier_kind
          FROM package_event
         WHERE tenant_id = p_tenant
           AND kind IN ('identified', 'relabelled', 'created')
           AND (sscc IS NOT NULL OR barcode IS NOT NULL)
         ORDER BY package_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    winning_status AS (
        SELECT DISTINCT ON (package_id) package_id, kind AS status
          FROM package_event
         WHERE tenant_id = p_tenant
           AND kind IN ('sealed', 'opened', 'despatched', 'voided')
         ORDER BY package_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE package p
           SET parent_package_id = wp.parent_package_id,
               location_id       = wp.location_id,
               status            = ws.status,
               sscc              = COALESCE(wi.sscc, p.sscc),
               barcode           = COALESCE(wi.barcode, p.barcode),
               identifier_kind   = wi.identifier_kind
          FROM winning_placement wp
          LEFT JOIN winning_identity wi ON wi.package_id = wp.package_id
          LEFT JOIN winning_status   ws ON ws.package_id = wp.package_id
         WHERE p.id = wp.package_id AND p.tenant_id = p_tenant
        RETURNING p.id)
    SELECT count(*) INTO touched FROM updated;

    -- resolved_location_id and depth walk the holder chain. Recursive because
    -- the chain is up to three levels and the walk is a cold path: the hot
    -- receiving screen reads the projection, never this.
    WITH RECURSIVE chain AS (
        SELECT id, location_id, location_id AS resolved, 0 AS depth
          FROM package
         WHERE tenant_id = p_tenant AND location_id IS NOT NULL
        UNION ALL
        SELECT c.id, c.location_id, parent.resolved, parent.depth + 1
          FROM package c
          JOIN chain parent ON parent.id = c.parent_package_id
         WHERE c.tenant_id = p_tenant
    )
    UPDATE package p
       SET resolved_location_id = ch.resolved, depth = ch.depth
      FROM chain ch
     WHERE p.id = ch.id AND p.tenant_id = p_tenant;

    RETURN touched;
END
$function$;

ALTER FUNCTION projection_package_rebuild(uuid) OWNER TO nylonite_projection_owner;

CREATE OR REPLACE FUNCTION public.projection_package_containment_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    built bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    -- This one is rebuilt rather than upserted, and that is a real difference
    -- from stock. Nothing holds a durable foreign key to a containment interval,
    -- so identity across rebuilds buys nothing, and the intervals change shape
    -- rather than value when a late event lands: one row becomes two. J33's
    -- argument does not reach here.
    DELETE FROM package_containment WHERE tenant_id = p_tenant;

    WITH ordered AS (
        SELECT id, package_id, parent_package_id, location_id, occurred_at,
               lead(occurred_at) OVER (
                   PARTITION BY package_id
                   ORDER BY occurred_at, recorded_at, id) AS next_at
          FROM package_event
         WHERE tenant_id = p_tenant AND asserts_placement
    )
    INSERT INTO package_containment
        (tenant_id, package_id, parent_package_id, location_id, valid, source_event_id)
    SELECT p_tenant, package_id, parent_package_id, location_id,
           -- Half-open, so consecutive intervals abut without overlapping and
           -- the exclusion constraint is satisfiable rather than merely lucky.
           tstzrange(occurred_at, next_at, '[)'),
           id
      FROM ordered
     -- A zero-width interval is an event superseded at the same instant by a
     -- later-recorded one. It held the placement for no time, so it gets no row,
     -- and the register still keeps the event itself.
     WHERE next_at IS NULL OR next_at > occurred_at;

    GET DIAGNOSTICS built = ROW_COUNT;
    RETURN built;
END
$function$;

ALTER FUNCTION projection_package_containment_rebuild(uuid) OWNER TO nylonite_projection_owner;
