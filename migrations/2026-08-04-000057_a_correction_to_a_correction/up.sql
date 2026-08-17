-- Migration 57: a correction to a correction.
--
-- D103, settling question 170, which D102 raised by declining to build it.
--
-- D102 made a movement contribute what is left of it after the corrections against
-- it. That is right one level deep and wrong beyond. A correction *to* a correction
-- means the units were right after all, so the original should return to its full
-- quantity -- and instead the second correction was subtracted from the first, which
-- is not in the fold and so contributed nothing anywhere.
--
--     effective(m)  =  m.quantity  -  SUM over its direct corrections r of effective(r)
--
-- That is the definition, and it is recursive because J52 permits a chain of any
-- depth: it asserts only that `reverses_movement_id` is acyclic.

-- ---------------------------------------------------------------------------
-- 1. The recursion has a closed form, and it is one signed sum
-- ---------------------------------------------------------------------------
--
-- Expanding the definition collapses it:
--
--     effective(P) = P - effective(R1)
--                  = P - (R1 - effective(R2))
--                  = P - R1 + R2 - effective(R3)
--                  = ...
--
-- **Every level alternates sign.** So the effective quantity of a movement is a
-- single signed sum over its whole correction subtree, positive at even depth and
-- negative at odd, and it holds when the tree branches -- a movement corrected twice
-- in parts has two children at depth one and both are subtracted.
--
-- That matters beyond elegance. A per-level walk with an aggregate at each step is
-- not expressible as one `WITH RECURSIVE`, because SQL's recursion is top-down and
-- this definition is bottom-up. The closed form turns a bottom-up fold into a
-- top-down walk plus a `GROUP BY`, which is one query and one pass.
--
-- It is also exactly how `stock.quantity` has always worked, which is why that fold
-- never had this bug: it is a signed sum over every movement. Question 170 said the
-- clue was there.

-- ---------------------------------------------------------------------------
-- 2. One definition, five readers
-- ---------------------------------------------------------------------------
--
-- `quantity_received`, the three progress quantities, J26, J68 and J51 all need this
-- number. D102 duplicated a simpler version of it into four places and nearly paid
-- for it: rebuilding one copy from the wrong migration silently reverted two
-- decisions, and only D68's write-counting test noticed.
--
-- So it is a view. One definition, and a reader that drifts from it has to do so
-- visibly by not using it.
--
-- `security_invoker` because RLS should apply as the role asking rather than as the
-- view's owner. The two views this schema already has predate that option being
-- available and read as their owner, which for a superuser-owned view means no
-- tenant filtering at all -- they are saved by every caller filtering explicitly.
-- This one does not rely on that.

CREATE VIEW stock_movement_effective
    WITH (security_invoker = true) AS
WITH RECURSIVE chain AS (
    -- Roots only. A correction is never a root, and never carries an effective
    -- quantity of its own: it exists in this view as a term in its target's sum.
    SELECT m.id AS root_id, m.tenant_id AS root_tenant_id,
           m.id AS movement_id, m.quantity, 0 AS depth
      FROM stock_movement m
     WHERE m.reverses_movement_id IS NULL
    UNION ALL
    SELECT c.root_id, c.root_tenant_id, r.id, r.quantity, c.depth + 1
      FROM chain c
      JOIN stock_movement r ON r.reverses_movement_id = c.movement_id
)
-- J52 asserts acyclicity and reports a loop as a finding; it does not prevent one.
-- A maintainer that hangs is worse than a maintainer that raises, which D98 already
-- reasoned about one class up, so this stops rather than trusting the check.
CYCLE movement_id SET is_cycle USING correction_path
SELECT root_id AS movement_id,
       root_tenant_id AS tenant_id,
       sum(CASE WHEN depth % 2 = 0 THEN quantity ELSE -quantity END)::bigint
           AS effective_quantity
  FROM chain
 WHERE NOT is_cycle
 GROUP BY root_id, root_tenant_id;

COMMENT ON VIEW stock_movement_effective IS
    'D103. What is left of each uncorrected movement after its whole correction '
    'subtree: a signed sum, positive at even depth and negative at odd, which is the '
    'closed form of effective(m) = m.quantity - sum of effective(corrections to m). '
    'Rows exist for roots only. The single definition behind quantity_received, the '
    'three fulfilment progress quantities, J26, J51 and J68.';

GRANT SELECT ON stock_movement_effective TO nylonite_projection_owner;
GRANT SELECT ON stock_movement_effective TO nylonite_app;

-- ---------------------------------------------------------------------------
-- 3. Outbound
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
$$;

ALTER FUNCTION projection_fulfilment_rebuild(uuid) OWNER TO nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- 4. Inbound
-- ---------------------------------------------------------------------------
--
-- Rebuilt from migration 56's body, which was rebuilt from 27's. D102 records why
-- that sentence has to be written down: a CREATE OR REPLACE carries no record of
-- what it replaced, and the decision record names the migration that introduced a
-- function rather than the one that last changed it.

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
$$;

ALTER FUNCTION projection_expected_supply_rebuild(uuid) OWNER TO nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- 5. The column comments
-- ---------------------------------------------------------------------------

COMMENT ON COLUMN expected_supply.quantity_received IS
    '@projection of stock_movement grouped by the receipt line''s supply row, through '
    'stock_movement_effective, via projection_expected_supply_rebuild '
    '(D45, D61, D102, D103, J26).';
COMMENT ON COLUMN fulfilment_line.picked_quantity IS
    '@projection of stock_movement via projection_fulfilment_rebuild (D99, D100, D103, '
    'J68). Movements naming this line that left a storage location, through '
    'stock_movement_effective.';
COMMENT ON COLUMN fulfilment_line.packed_quantity IS
    '@projection of stock_movement and package.status via projection_fulfilment_rebuild '
    '(D99, D100, D103, J68). Movements naming this line into a carton whose winning '
    'status is sealed or despatched, through stock_movement_effective. Not '
    'package.sealed_at, which the application may UPDATE.';
COMMENT ON COLUMN fulfilment_line.despatched_quantity IS
    '@projection of stock_movement via projection_fulfilment_rebuild (D99, D100, D103, '
    'J68). Movements naming this line with no to side at all, which is D45''s arrival '
    'test mirrored, through stock_movement_effective.';
