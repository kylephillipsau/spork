-- Migration 60: the fold that joined the whole ledger.
--
-- D106, settling question 164. Measured rather than redesigned.
--
-- D95 measured `projection_run_all` at about two seconds against a year of
-- history and raised 164: every maintainer is a full-tenant fold, so cost is
-- O(history) and the cadence has a ceiling that belongs to the fold.
--
-- Remeasured after D99–D105 against the same year (289,080 movements):
--
--   full projection_run_all:   ~12.8 s median
--   of which expected_supply:  ~12.2 s
--   stock_rebuild:                171 ms
--   fulfilment_rebuild:           269 ms
--
-- **Almost the whole regression is one query.** `quantity_received` joined
-- `stock_movement_effective` — a recursive view over every uncorrected movement
-- — into a nested loop over every receipt line. The plan materialised 289,080
-- effective rows and joined them to 35,040 arrivals by filter, removing about
-- ten billion intermediate rows. Execution: six minutes for that join alone
-- under EXPLAIN ANALYZE. The same arithmetic restricted to receipt roots finishes
-- in about 50 ms.
--
-- So 164's first answer is not watermarks. It is: **do not join the global
-- effective view into a fold of a subset of the ledger.** The view remains the
-- single definition for the jobs and for documentation; the maintainers restate
-- its closed form over the roots they actually need, which is the same signed sum
-- D103 named, scoped.
--
-- O(delta) — watermarks, dirty cells — stays deferred. After this fix the year
-- fold is back under the two-second region D95 measured, which is well under the
-- five-minute stock bound and leaves room for the sequential-tenant cadence
-- arithmetic 164 asked for. D68 still holds: a no-op rebuild writes nothing.

-- ---------------------------------------------------------------------------
-- 1. Expected supply: scope the effective sum to arrival roots
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION projection_expected_supply_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    -- last changed: migration 60 (D106)
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

    -- D61, J26, D102, D103, D106. Same closed form as stock_movement_effective
    -- (signed sum over the correction subtree), but rooted only at arrivals that
    -- name a supply row. Joining the global view here nested-looped every receipt
    -- against every movement in the tenant.
    WITH RECURSIVE roots AS (
        SELECT m.id, m.tenant_id, m.quantity, grl.expected_supply_id
          FROM stock_movement m
          JOIN goods_receipt_line grl ON grl.id = m.goods_receipt_line_id
         WHERE m.tenant_id = p_tenant
           AND m.reverses_movement_id IS NULL
           AND m.from_location_id IS NULL AND m.from_package_id IS NULL
           AND grl.expected_supply_id IS NOT NULL
    ),
    chain AS (
        SELECT id AS root_id, tenant_id, id AS movement_id, quantity,
               0 AS depth, expected_supply_id
          FROM roots
        UNION ALL
        SELECT c.root_id, c.tenant_id, r.id, r.quantity, c.depth + 1,
               c.expected_supply_id
          FROM chain c
          JOIN stock_movement r ON r.reverses_movement_id = c.movement_id
    ),
    received AS (
        SELECT expected_supply_id,
               sum(CASE WHEN depth % 2 = 0 THEN quantity ELSE -quantity END)::bigint
                   AS q
          FROM chain
         GROUP BY expected_supply_id
    )
    UPDATE expected_supply e
       SET quantity_received = coalesce(r.q, 0)
      FROM expected_supply c
      LEFT JOIN received r ON r.expected_supply_id = c.id
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

    UPDATE expected_supply e
       SET closed_at = now(), closed_reason = 'received_in_full'
     WHERE e.tenant_id = p_tenant
       AND e.closed_at IS NULL
       AND e.quantity_outstanding <= 0;

    RETURN touched;
END
$$;

ALTER FUNCTION projection_expected_supply_rebuild(uuid) OWNER TO nylonite_projection_owner;

COMMENT ON FUNCTION projection_expected_supply_rebuild(uuid) IS
    'Folds promises from purchase orders, allocations and arrivals. quantity_received '
    'uses D103''s signed correction sum over arrival roots only (D106), not a join to '
    'the global stock_movement_effective view. last changed: migration 60 (D106).';

-- ---------------------------------------------------------------------------
-- 2. Fulfilment progress: same scoped form over line-serving roots
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION projection_fulfilment_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    -- last changed: migration 60 (D106)
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    -- D99 shape rules over D103's effective quantity, rooted only at movements
    -- that name a fulfilment line. Corrections hang off those roots; the root's
    -- from/to shape is what the FILTER clauses read.
    WITH RECURSIVE roots AS (
        SELECT m.id, m.tenant_id, m.quantity, m.fulfilment_line_id,
               m.from_location_id, m.to_location_id, m.to_package_id
          FROM stock_movement m
         WHERE m.tenant_id = p_tenant
           AND m.fulfilment_line_id IS NOT NULL
           AND m.reverses_movement_id IS NULL
    ),
    chain AS (
        SELECT id AS root_id, tenant_id, id AS movement_id, quantity, 0 AS depth,
               fulfilment_line_id, from_location_id, to_location_id, to_package_id
          FROM roots
        UNION ALL
        SELECT c.root_id, c.tenant_id, r.id, r.quantity, c.depth + 1,
               c.fulfilment_line_id, c.from_location_id, c.to_location_id,
               c.to_package_id
          FROM chain c
          JOIN stock_movement r ON r.reverses_movement_id = c.movement_id
    ),
    effective AS (
        SELECT root_id AS movement_id, fulfilment_line_id,
               from_location_id, to_location_id, to_package_id,
               sum(CASE WHEN depth % 2 = 0 THEN quantity ELSE -quantity END)::bigint
                   AS effective_quantity
          FROM chain
         GROUP BY root_id, fulfilment_line_id, from_location_id, to_location_id,
                  to_package_id
    ),
    ledger AS (
        SELECT e.fulfilment_line_id,
               sum(e.effective_quantity) FILTER (
                   WHERE e.from_location_id IS NOT NULL)                 AS picked,
               sum(e.effective_quantity) FILTER (
                   WHERE p.status IN ('sealed', 'despatched'))           AS packed,
               sum(e.effective_quantity) FILTER (
                   WHERE e.to_location_id IS NULL
                     AND e.to_package_id IS NULL)                        AS despatched
          FROM effective e
          LEFT JOIN package p ON p.id = e.to_package_id
         GROUP BY e.fulfilment_line_id
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

COMMENT ON FUNCTION projection_fulfilment_rebuild(uuid) IS
    'Maintainer for fulfilment_line coverage and progress. Progress uses D103''s '
    'signed correction sum over movements that name the line (D106), not a join to '
    'the global stock_movement_effective view. last changed: migration 60 (D106).';

-- ---------------------------------------------------------------------------
-- 3. The view stays the definition; say what it costs to join whole
-- ---------------------------------------------------------------------------

COMMENT ON VIEW stock_movement_effective IS
    'D103. What is left of each uncorrected movement after its whole correction '
    'subtree: a signed sum, positive at even depth and negative at odd. Rows exist '
    'for roots only. The single *definition* behind quantity_received, the three '
    'fulfilment progress quantities, J26, J51 and J68. '
    'D106: maintainers must not join this view over the whole ledger against a '
    'filtered movement set — the planner nested-loops and the cost is O(roots × '
    'filters). Restate the closed form over the roots the fold needs. Jobs that '
    'examine small populations may still read the view directly.';

COMMENT ON COLUMN expected_supply.quantity_received IS
    '@projection of stock_movement grouped by the receipt line''s supply row, '
    'through D103''s signed correction sum over arrival roots, via '
    'projection_expected_supply_rebuild (D45, D61, D102, D103, D106, J26).';
COMMENT ON COLUMN fulfilment_line.picked_quantity IS
    '@projection of stock_movement via projection_fulfilment_rebuild (D99, D100, '
    'D103, D106, J68). Movements naming this line that left a storage location, '
    'through D103''s signed correction sum over those roots.';
COMMENT ON COLUMN fulfilment_line.packed_quantity IS
    '@projection of stock_movement and package.status via projection_fulfilment_rebuild '
    '(D99, D100, D103, D106, J68). Movements naming this line into a carton whose '
    'winning status is sealed or despatched. Not package.sealed_at.';
COMMENT ON COLUMN fulfilment_line.despatched_quantity IS
    '@projection of stock_movement via projection_fulfilment_rebuild (D99, D100, '
    'D103, D106, J68). Movements naming this line with no to side at all, through '
    'D103''s signed correction sum over those roots.';
