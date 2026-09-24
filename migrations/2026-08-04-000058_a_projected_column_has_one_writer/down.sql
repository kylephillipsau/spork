-- Migration 58 down: strip the last-changed markers. Fold bodies otherwise unchanged.
-- S55 and S56 go with the suite; this only undoes the body comments.

-- projection_expected_supply_rebuild
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

-- projection_fulfilment_rebuild
CREATE OR REPLACE FUNCTION public.projection_fulfilment_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

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

-- projection_inbound_shipment_rebuild
CREATE OR REPLACE FUNCTION public.projection_inbound_shipment_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    n bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    WITH latest_stance AS (
        SELECT DISTINCT ON (s.assertion_id)
               s.assertion_id, s.stance
          FROM assertion_stance s
         WHERE s.tenant_id = p_tenant
         ORDER BY s.assertion_id, s.occurred_at DESC, s.recorded_at DESC, s.id DESC
    ),
    claims AS (
        SELECT d.inbound_shipment_id AS shipment_id, a.id AS assertion_id,
               a.asserted_at, ls.stance
          FROM despatch_advice d
          JOIN assertion a ON a.id = d.assertion_id
          LEFT JOIN latest_stance ls ON ls.assertion_id = a.id
         WHERE a.tenant_id = p_tenant AND d.inbound_shipment_id IS NOT NULL
    ),
    in_force AS (
        SELECT DISTINCT ON (shipment_id) shipment_id, assertion_id
          FROM claims WHERE stance = 'in_force'
         ORDER BY shipment_id, asserted_at DESC NULLS LAST, assertion_id DESC
    ),
    rolled AS (
        SELECT c.shipment_id,
               min(c.asserted_at) AS first_asserted_at,
               count(*) FILTER (WHERE c.stance = 'superseded')::integer AS superseded_count
          FROM claims c GROUP BY c.shipment_id
    ),
    measured AS (
        SELECT f.shipment_id, f.assertion_id,
               (SELECT count(*)::integer FROM asserted_unit u
                 WHERE u.assertion_id = f.assertion_id) AS unit_count,
               (SELECT sum(cc.quantity) FROM asserted_unit_content cc
                  JOIN asserted_unit u ON u.id = cc.asserted_unit_id
                 WHERE u.assertion_id = f.assertion_id) AS base_quantity
          FROM in_force f
    ),
    updated AS (
        UPDATE inbound_shipment s
           SET in_force_assertion_id  = m.assertion_id,
               asserted_unit_count    = m.unit_count,
               asserted_base_quantity = m.base_quantity,
               first_asserted_at      = r.first_asserted_at,
               superseded_count       = coalesce(r.superseded_count, 0)
          FROM rolled r
          LEFT JOIN measured m ON m.shipment_id = r.shipment_id
         WHERE s.id = r.shipment_id AND s.tenant_id = p_tenant
           AND (s.in_force_assertion_id  IS DISTINCT FROM m.assertion_id
             OR s.asserted_unit_count    IS DISTINCT FROM m.unit_count
             OR s.asserted_base_quantity IS DISTINCT FROM m.base_quantity
             OR s.first_asserted_at      IS DISTINCT FROM r.first_asserted_at
             OR s.superseded_count       IS DISTINCT FROM coalesce(r.superseded_count, 0))
        RETURNING s.id)
    SELECT count(*) INTO n FROM updated;

    RETURN n;
END
$function$;

-- projection_item_class_closure_rebuild
CREATE OR REPLACE FUNCTION public.projection_item_class_closure_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

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
        WHERE (x.depth, x.tenant_id) IS DISTINCT FROM (EXCLUDED.depth, EXCLUDED.tenant_id)
        RETURNING 1)
    SELECT (SELECT count(*) FROM written) + (SELECT count(*) FROM removed) INTO touched;

    RETURN touched;
END
$function$;

-- projection_observation_current_rebuild
CREATE OR REPLACE FUNCTION public.projection_observation_current_rebuild(p_tenant uuid, p_decisions observation_precedence_decision[] DEFAULT NULL::observation_precedence_decision[])
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    n bigint;
    m bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    -- D78's rule as a row, so "nobody resolved anything" and "the resolver said
    -- exactly this" travel the same path and there is one code path rather than
    -- two. `projection_run_all` still calls with no decisions at all.
    CREATE TEMP TABLE decision ON COMMIT DROP AS
    SELECT * FROM unnest(coalesce(p_decisions,
        ARRAY[(NULL, true, true, NULL)::observation_precedence_decision]));

    CREATE TEMP TABLE winner ON COMMIT DROP AS
    WITH applicable AS (
        -- The metric-scoped decision where there is one, the fallback otherwise.
        SELECT o.id AS observation_id, d.prefer_own, d.accept_counterparty,
               d.observation_precedence_policy_id
          FROM observation o
          JOIN LATERAL (
              SELECT * FROM decision dd
               WHERE dd.metric_id = o.metric_id OR dd.metric_id IS NULL
               ORDER BY (dd.metric_id IS NULL)
               LIMIT 1) d ON true
         WHERE o.tenant_id = p_tenant
    ),
    eligible AS (
        SELECT o.id, o.tenant_id, o.observable_id, o.metric_id, o.result_kind,
               o.value_numeric, o.value_instant, o.value_code_id, o.value_boolean,
               o.value_text, o.observed_at, e.recorded_at,
               e.method, o.confidence, o.uncertainty_numeric,
               e.asserted_by_party_id,
               a.prefer_own, a.observation_precedence_policy_id
          FROM observation o
          JOIN observation_event e ON e.id = o.observation_event_id
          JOIN applicable a ON a.observation_id = o.id
         WHERE o.tenant_id = p_tenant
           AND o.retracts_observation_id IS NULL
           AND NOT EXISTS (SELECT 1 FROM observation r
                            WHERE r.retracts_observation_id = o.id)
           AND NOT EXISTS (SELECT 1 FROM observation c
                            WHERE c.corrects_observation_id = o.id)
           AND (e.asserted_by_party_id IS NULL
                OR (a.accept_counterparty
                    AND EXISTS (SELECT 1 FROM observation_acceptance oa
                                 WHERE oa.observation_id = o.id)))
    )
    SELECT DISTINCT ON (observable_id, metric_id) *
      FROM eligible
     ORDER BY observable_id, metric_id,
              (prefer_own AND asserted_by_party_id IS NOT NULL),
              observed_at DESC, recorded_at DESC, id DESC;

    WITH upserted AS (
        INSERT INTO observation_current AS oc (
            tenant_id, observable_id, metric_id, observation_id, result_kind,
            value_numeric, value_instant, value_code_id, value_boolean, value_text,
            observed_at, recorded_at, method, confidence, uncertainty_numeric,
            asserted_by_party_id, observation_precedence_policy_id)
        SELECT tenant_id, observable_id, metric_id, id, result_kind,
               value_numeric, value_instant, value_code_id, value_boolean, value_text,
               observed_at, recorded_at, method, confidence, uncertainty_numeric,
               asserted_by_party_id, observation_precedence_policy_id
          FROM winner
        ON CONFLICT (observable_id, metric_id) DO UPDATE
           SET observation_id = excluded.observation_id,
               result_kind    = excluded.result_kind,
               value_numeric  = excluded.value_numeric,
               value_instant  = excluded.value_instant,
               value_code_id  = excluded.value_code_id,
               value_boolean  = excluded.value_boolean,
               value_text     = excluded.value_text,
               observed_at    = excluded.observed_at,
               recorded_at    = excluded.recorded_at,
               method         = excluded.method,
               confidence     = excluded.confidence,
               uncertainty_numeric = excluded.uncertainty_numeric,
               asserted_by_party_id = excluded.asserted_by_party_id,
               observation_precedence_policy_id =
                   excluded.observation_precedence_policy_id
         WHERE oc.observation_id IS DISTINCT FROM excluded.observation_id
            OR oc.value_numeric  IS DISTINCT FROM excluded.value_numeric
            OR oc.value_instant  IS DISTINCT FROM excluded.value_instant
            OR oc.value_code_id  IS DISTINCT FROM excluded.value_code_id
            OR oc.value_boolean  IS DISTINCT FROM excluded.value_boolean
            OR oc.value_text     IS DISTINCT FROM excluded.value_text
            OR oc.observed_at    IS DISTINCT FROM excluded.observed_at
            OR oc.observation_precedence_policy_id
                 IS DISTINCT FROM excluded.observation_precedence_policy_id
        RETURNING 1)
    SELECT count(*) INTO n FROM upserted;

    WITH reaped AS (
        DELETE FROM observation_current oc
         WHERE oc.tenant_id = p_tenant
           AND NOT EXISTS (SELECT 1 FROM winner w
                            WHERE w.observable_id = oc.observable_id
                              AND w.metric_id = oc.metric_id)
        RETURNING 1)
    SELECT count(*) INTO m FROM reaped;

    DROP TABLE winner;
    DROP TABLE decision;
    RETURN n + m;
END
$function$;

-- projection_order_rebuild
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
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

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
           AND (o.promised_from, o.promised_to, o.required_by, o.state)
               IS DISTINCT FROM
               (COALESCE(f.promised_from, o.promised_from),
                COALESCE(f.promised_to,   o.promised_to),
                COALESCE(f.required_by,   o.required_by),
                COALESCE(f.state,         o.state))
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
           AND (l.quantity_ordered, l.unit_price_minor, l.price_basis_quantity, l.line_state)
               IS DISTINCT FROM
               (COALESCE(f.quantity_ordered,     l.quantity_ordered),
                COALESCE(f.unit_price_minor,     l.unit_price_minor),
                COALESCE(f.price_basis_quantity, l.price_basis_quantity),
                COALESCE(f.line_state,           l.line_state))
        RETURNING l.id)
    SELECT count(*) INTO touched_lines FROM updated_line;

    RETURN touched + touched_lines;
END
$function$;

-- projection_package_containment_rebuild
CREATE OR REPLACE FUNCTION public.projection_package_containment_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    stored_hash text;
    computed_hash text;
    built bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    -- This one is rebuilt rather than upserted, and that is a real difference
    -- from stock. Nothing holds a durable foreign key to a containment interval,
    -- so identity across rebuilds buys nothing, and the intervals change shape
    -- rather than value when a late event lands: one row becomes two. J33's
    -- argument does not reach here.
    -- D68. Recomputed rather than upserted, for the reason above -- but a
    -- recompute that replaces an identical set is pure churn: N dead tuples and
    -- N inserts every run, on a table that only changes when a placement event
    -- lands. So the set is compared before it is replaced.
    --
    -- A hash rather than a row-by-row diff, because the alternative to replacing
    -- the set is skipping it entirely, and that is a single yes-or-no question.
    SELECT md5(coalesce(string_agg(
               package_id::text || coalesce(parent_package_id::text,'-')
                 || coalesce(location_id::text,'-') || valid::text
                 || source_event_id::text, '|' ORDER BY source_event_id), ''))
      INTO stored_hash
      FROM package_containment WHERE tenant_id = p_tenant;


    WITH ordered AS (
        SELECT id, package_id, parent_package_id, location_id, occurred_at,
               lead(occurred_at) OVER (
                   PARTITION BY package_id
                   ORDER BY occurred_at, recorded_at, id) AS next_at
          FROM package_event
         WHERE tenant_id = p_tenant AND asserts_placement
    )
    SELECT md5(coalesce(string_agg(
               package_id::text || coalesce(parent_package_id::text,'-')
                 || coalesce(location_id::text,'-')
                 || tstzrange(occurred_at, next_at, '[)')::text
                 || id::text, '|' ORDER BY id), ''))
      INTO computed_hash
      FROM ordered
     WHERE next_at IS NULL OR next_at > occurred_at;

    IF computed_hash IS NOT DISTINCT FROM stored_hash THEN
        RETURN 0;
    END IF;

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

-- projection_package_rebuild
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
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

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
           -- D68. Mirrors the SET clause exactly, aliases included: status
           -- comes from the status winner and the identifiers from the identity
           -- winner, and a guard that compared the wrong ones would either never
           -- fire or fire always.
           AND (p.parent_package_id, p.location_id, p.status, p.sscc, p.barcode,
                p.identifier_kind)
               IS DISTINCT FROM
               (wp.parent_package_id, wp.location_id, ws.status,
                COALESCE(wi.sscc, p.sscc), COALESCE(wi.barcode, p.barcode),
                wi.identifier_kind)
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
     WHERE p.id = ch.id AND p.tenant_id = p_tenant
       AND (p.resolved_location_id, p.depth) IS DISTINCT FROM (ch.resolved, ch.depth);

    RETURN touched;
END
$function$;

-- projection_package_stamp
CREATE OR REPLACE FUNCTION public.projection_package_stamp(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    WITH winner AS (
        SELECT DISTINCT ON (package_id) package_id, id, occurred_at
          FROM package_event
         WHERE tenant_id = p_tenant AND asserts_placement
         ORDER BY package_id, occurred_at DESC, recorded_at DESC, id DESC
    )
    UPDATE package p
       SET placement_event_id = w.id, placement_occurred_at = w.occurred_at
      FROM winner w
     WHERE p.id = w.package_id AND p.tenant_id = p_tenant
       AND (p.placement_event_id, p.placement_occurred_at)
           IS DISTINCT FROM (w.id, w.occurred_at);

    GET DIAGNOSTICS touched = ROW_COUNT;
    RETURN touched;
END
$function$;

-- projection_party_class_closure_rebuild
CREATE OR REPLACE FUNCTION public.projection_party_class_closure_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

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
        WHERE (x.depth, x.tenant_id) IS DISTINCT FROM (EXCLUDED.depth, EXCLUDED.tenant_id)
        RETURNING 1)
    SELECT (SELECT count(*) FROM written) + (SELECT count(*) FROM removed) INTO touched;

    RETURN touched;
END
$function$;

-- projection_run_all
CREATE OR REPLACE FUNCTION public.projection_run_all(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    r        record;
    n        bigint;
    total    bigint := 0;
    started  timestamptz;
    err      text;
    err_code text;
BEGIN
    FOR r IN SELECT function_name FROM projection_step ORDER BY ordinal
    LOOP
        started := clock_timestamp();
        BEGIN
            EXECUTE format('SELECT %I($1)', r.function_name) USING p_tenant INTO n;
        EXCEPTION WHEN OTHERS THEN
            GET STACKED DIAGNOSTICS err = MESSAGE_TEXT, err_code = RETURNED_SQLSTATE;

            INSERT INTO projection_freshness
                (tenant_id, function_name, last_run_at, last_run_ms, rows_touched,
                 last_error, last_error_at)
            VALUES (p_tenant, r.function_name, NULL, 0, 0,
                    err_code || ': ' || err, now())
            ON CONFLICT (tenant_id, function_name) DO UPDATE
               SET last_error    = excluded.last_error,
                   last_error_at = excluded.last_error_at;

            INSERT INTO discrepancy (tenant_id, kind, detail, automation_key)
            VALUES (p_tenant, 'projection_failed',
                    r.function_name || ' raised ' || err_code || ': ' || err
                        || '. Every step after it was skipped, because the ordinal '
                        || 'is a dependency order and they would fold over inputs '
                        || 'that were never written.',
                    'projection-run');

            RETURN total;
        END;

        n := coalesce(n, 0);
        total := total + n;

        -- D70. Only on work, so D68's property survives.
        IF n > 0 THEN
            UPDATE projection_step
               SET last_changed_at = now()
             WHERE function_name = r.function_name;
        END IF;

        -- D95, and D98 clears the error a previous run recorded.
        INSERT INTO projection_freshness
            (tenant_id, function_name, last_run_at, last_run_ms, rows_touched)
        VALUES (p_tenant, r.function_name, now(),
                (extract(epoch FROM clock_timestamp() - started) * 1000)::integer, n)
        ON CONFLICT (tenant_id, function_name) DO UPDATE
           SET last_run_at  = excluded.last_run_at,
               last_run_ms  = excluded.last_run_ms,
               rows_touched = excluded.rows_touched,
               last_error    = NULL,
               last_error_at = NULL;
    END LOOP;
    RETURN total;
END
$function$;

-- projection_stock_rebuild
CREATE OR REPLACE FUNCTION public.projection_stock_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

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
        -- D101. The container arm belongs to step 70 and this step must not
        -- overwrite it. `holder_location_id` is NULL for a package-held cell, so
        -- the unguarded form wrote NULL over a resolved value every run and 70 put
        -- it back, which is a fold that never settles rather than a wrong number.
        DO UPDATE SET quantity = EXCLUDED.quantity,
                      weight_g = EXCLUDED.weight_g,
                      resolved_location_id = CASE
                          WHEN s.holder_package_id IS NOT NULL
                          THEN s.resolved_location_id
                          ELSE EXCLUDED.resolved_location_id END,
                      site_id = CASE
                          WHEN s.holder_package_id IS NOT NULL
                          THEN s.site_id
                          ELSE EXCLUDED.site_id END
        -- D68. Without this the upsert rewrites every cell every run, whether or
        -- not the fold moved it. Idempotent in value is not the same as writing
        -- nothing, and under MVCC the difference is a dead tuple per row per run.
        -- The guard mirrors the SET clause exactly, which is what D68 asks of every
        -- one of these and what makes the two impossible to drift apart.
        WHERE (s.quantity, s.weight_g)
              IS DISTINCT FROM (EXCLUDED.quantity, EXCLUDED.weight_g)
           OR (s.holder_package_id IS NULL
               AND (s.resolved_location_id, s.site_id)
                   IS DISTINCT FROM (EXCLUDED.resolved_location_id, EXCLUDED.site_id))
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

-- projection_stock_resolve_locations
CREATE OR REPLACE FUNCTION public.projection_stock_resolve_locations(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    UPDATE stock s
       SET resolved_location_id = COALESCE(s.holder_location_id, pkg.resolved_location_id)
      FROM package pkg
     WHERE s.tenant_id = p_tenant
       AND s.holder_package_id = pkg.id
       AND s.resolved_location_id
           IS DISTINCT FROM COALESCE(s.holder_location_id, pkg.resolved_location_id);

    GET DIAGNOSTICS touched = ROW_COUNT;

    UPDATE stock s
       SET site_id = l.site_id
      FROM location l
     WHERE s.tenant_id = p_tenant AND s.resolved_location_id = l.id
       AND s.site_id IS DISTINCT FROM l.site_id;

    RETURN touched;
END
$function$;

-- projection_taxonomy_rebuild
CREATE OR REPLACE FUNCTION public.projection_taxonomy_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
    n bigint;
    total bigint := 0;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    WITH folded AS (
        SELECT DISTINCT ON (item_class_id)
               item_class_id, new_item_parent_id AS new_parent_id
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('created', 'reparented')
           AND item_class_id IS NOT NULL
         ORDER BY item_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE item_class c
           SET parent_id = f.new_parent_id
          FROM folded f
         WHERE c.id = f.item_class_id AND c.tenant_id = p_tenant
           AND c.parent_id IS DISTINCT FROM f.new_parent_id
        RETURNING c.id)
    SELECT count(*) INTO touched FROM updated;
    total := total + touched;

    WITH folded AS (
        SELECT DISTINCT ON (party_class_id)
               party_class_id, new_party_parent_id AS new_parent_id
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('created', 'reparented')
           AND party_class_id IS NOT NULL
         ORDER BY party_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE party_class c
           SET parent_id = f.new_parent_id
          FROM folded f
         WHERE c.id = f.party_class_id AND c.tenant_id = p_tenant
           AND c.parent_id IS DISTINCT FROM f.new_parent_id
        RETURNING c.id)
    SELECT count(*) INTO n FROM updated;
    total := total + n;

    WITH folded AS (
        SELECT DISTINCT ON (item_class_id)
               item_class_id,
               CASE WHEN change_kind = 'retired' THEN occurred_at END AS retired_at
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('retired', 'reinstated')
           AND item_class_id IS NOT NULL
         ORDER BY item_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE item_class c
           SET retired_at = f.retired_at
          FROM folded f
         WHERE c.id = f.item_class_id AND c.tenant_id = p_tenant
           AND c.retired_at IS DISTINCT FROM f.retired_at
        RETURNING c.id)
    SELECT count(*) INTO n FROM updated;
    total := total + n;

    WITH folded AS (
        SELECT DISTINCT ON (party_class_id)
               party_class_id,
               CASE WHEN change_kind = 'retired' THEN occurred_at END AS retired_at
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('retired', 'reinstated')
           AND party_class_id IS NOT NULL
         ORDER BY party_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE party_class c
           SET retired_at = f.retired_at
          FROM folded f
         WHERE c.id = f.party_class_id AND c.tenant_id = p_tenant
           AND c.retired_at IS DISTINCT FROM f.retired_at
        RETURNING c.id)
    SELECT count(*) INTO n FROM updated;
    total := total + n;

    RETURN total;
END
$function$;

