-- Migration 15: progress is three fractions, and one column cannot hold them.
--
-- Question 134, raised by D49 when migration 12 found `projection_fulfilment_rebuild`
-- registered by migration 9 and never written. The question was what it computes.
-- Answering it turned out to require deciding what `progress` means, which D25
-- had sketched and never settled, and then finding that the source it named
-- cannot be reached.
--
-- D25's sketch:
--
--     fulfilment  state     -- DECLARED: planned | released | cancelled
--                 progress  -- @projection: allocations, movements,
--                           --   packages, consignment
--
-- `stock_movement` carries no reference to a fulfilment or an order line. There
-- is no path from a movement to the commitment it served, so "folded from the
-- movements against this fulfilment's lines" — migration 9's own column comment —
-- describes a fold that cannot be computed. That is why nobody wrote it.

-- ---------------------------------------------------------------------------
-- 1. What a commitment's progress actually is
-- ---------------------------------------------------------------------------
--
-- Three quantities, not one label. A fulfilment can be fully picked, partly
-- packed and not despatched at the same instant, and every one of those is
-- something somebody asks about. A single value has to pick one and discard the
-- other two, which is what made this a D15 question rather than a fold.
--
-- The reachable source is `stock_allocation`, through the `fulfilment_line_id`
-- migration 9 added. Its state machine already enumerates the stages, and
-- mechanism-design states the lifecycle in terms of them: an allocation holds
-- its quantity through `allocated | picking | picked | packed`, releasing only
-- at despatch or on explicit release. The states are monotone, so each quantity
-- is a sum over a nested set:
--
--     covered     allocated, picking, picked, packed, fulfilled
--     picked                            picked, packed, fulfilled
--     packed                                    packed, fulfilled
--     despatched                                        fulfilled
--
-- `short` and `released` appear in none of them. Both are terminal ways for an
-- allocation to stop covering anything, and counting either would report a
-- commitment as met by supply that went elsewhere.

-- ---------------------------------------------------------------------------
-- 2. The rename, which is the reason the first attempt was wrong
-- ---------------------------------------------------------------------------
--
-- `stock.allocated_quantity` and `fulfilment_line.allocated_quantity` are the
-- same name for two different questions, and they need different state sets.
--
--   On the cell, it asks how much of this stock is claimed. A despatched
--   allocation must NOT count: the stock has left, and D24 narrowed the column
--   to cell-bound claims precisely so availability is one indexed read.
--
--   On the commitment, it asks how much of what we promised is covered. A
--   despatched allocation MUST count. It is the most covered a line can be.
--
-- Under the shared name the second borrowed the first's answer. `uncovered_quantity`
-- is `quantity - allocated_quantity`, and `fulfilment_line_uncovered_idx` indexes
-- `WHERE uncovered_quantity > 0` to find lines still needing supply. With
-- `fulfilled` excluded, a fully despatched line drops back to zero covered, its
-- uncovered quantity returns to the full amount, and it reappears in the index of
-- outstanding work forever.
--
-- D48 renamed `adjustment_class` to `revision_class` because two names for one
-- question is a near-duplicate. This is the inverse and the more dangerous shape:
-- one name for two questions, which does not look like a duplicate at all. It
-- already cost something — J31 was implemented from J3's state set one commit ago,
-- because the column name said they were the same thing.

ALTER TABLE fulfilment DROP COLUMN progress;

DROP INDEX fulfilment_line_uncovered_idx;
ALTER TABLE fulfilment_line DROP COLUMN uncovered_quantity;

ALTER TABLE fulfilment_line RENAME COLUMN allocated_quantity TO covered_quantity;

ALTER TABLE fulfilment_line
    ADD COLUMN picked_quantity     bigint NOT NULL DEFAULT 0,
    ADD COLUMN packed_quantity     bigint NOT NULL DEFAULT 0,
    ADD COLUMN despatched_quantity bigint NOT NULL DEFAULT 0;

ALTER TABLE fulfilment_line
    ADD COLUMN uncovered_quantity bigint
        GENERATED ALWAYS AS (quantity - covered_quantity) STORED;

CREATE INDEX fulfilment_line_uncovered_idx
    ON fulfilment_line (fulfilment_id) WHERE uncovered_quantity > 0;

-- The defaults stay for the same reason `order_line.line_state`'s does: a line
-- is created covering nothing, zero is the only legal value at insert, and the
-- fold replaces rather than accumulates. None of the four is in the application's
-- INSERT grant, so nothing can create a line already claiming to be despatched.

COMMENT ON COLUMN fulfilment_line.covered_quantity IS
    '@projection of stock_allocation via projection_fulfilment_rebuild (D53, J31). '
    'How much of this commitment is covered by supply that still stands, including '
    'what has despatched. Not stock.allocated_quantity, which excludes despatch '
    'because the stock has left.';
COMMENT ON COLUMN fulfilment_line.picked_quantity IS
    '@projection of stock_allocation via projection_fulfilment_rebuild (D53, J31).';
COMMENT ON COLUMN fulfilment_line.packed_quantity IS
    '@projection of stock_allocation via projection_fulfilment_rebuild (D53, J31).';
COMMENT ON COLUMN fulfilment_line.despatched_quantity IS
    '@projection of stock_allocation via projection_fulfilment_rebuild (D53, J31).';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('fulfilment_line', 'covered_quantity',    'projection_fulfilment_rebuild'),
    ('fulfilment_line', 'picked_quantity',     'projection_fulfilment_rebuild'),
    ('fulfilment_line', 'packed_quantity',     'projection_fulfilment_rebuild'),
    ('fulfilment_line', 'despatched_quantity', 'projection_fulfilment_rebuild');

-- ---------------------------------------------------------------------------
-- 3. Why `fulfilment.progress` is dropped rather than defined
-- ---------------------------------------------------------------------------
--
-- D25 dropped `order.fulfilment_status` because it would be a third hop —
-- `stock_movement → fulfilment.progress → order.fulfilment_status` — and said
-- an order has few fulfilments, so compute it on read.
--
-- The same argument applies one level down and D25 did not apply it. A fulfilment
-- has few lines. Any label `progress` could hold is a function of the four line
-- quantities, so storing it creates a value that can disagree with its own
-- inputs, which is what S40 refuses for a line total and S36 for a received
-- quantity. It would also have to choose which of the three fractions it means.
--
-- So the header keeps `state`, which is DECLARED and genuinely its own, and how
-- far along it is falls out of its lines. S43 asserts the absence, because a
-- column dropped for a reason comes back unless something says why it should not.

-- ---------------------------------------------------------------------------
-- 4. The maintainer, finally written
-- ---------------------------------------------------------------------------

CREATE FUNCTION projection_fulfilment_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH folded AS (
        SELECT fulfilment_line_id,
               sum(quantity) FILTER (WHERE state IN
                   ('allocated','picking','picked','packed','fulfilled'))  AS covered,
               sum(quantity) FILTER (WHERE state IN
                   ('picked','packed','fulfilled'))                        AS picked,
               sum(quantity) FILTER (WHERE state IN
                   ('packed','fulfilled'))                                 AS packed,
               sum(quantity) FILTER (WHERE state = 'fulfilled')            AS despatched
          FROM stock_allocation
         WHERE tenant_id = p_tenant
           AND fulfilment_line_id IS NOT NULL
         GROUP BY fulfilment_line_id
    ),
    updated AS (
        -- LEFT JOIN from the line rather than FROM the fold, because a line whose
        -- last allocation was released has no group and must go to zero. Joining
        -- the other way would leave the previous numbers standing, which is the
        -- failure mode a rebuild exists to be immune to.
        UPDATE fulfilment_line fl
           SET covered_quantity    = coalesce(f.covered, 0),
               picked_quantity     = coalesce(f.picked, 0),
               packed_quantity     = coalesce(f.packed, 0),
               despatched_quantity = coalesce(f.despatched, 0)
          FROM fulfilment_line l
          LEFT JOIN folded f ON f.fulfilment_line_id = l.id
         WHERE fl.id = l.id AND fl.tenant_id = p_tenant
           AND (fl.covered_quantity, fl.picked_quantity,
                fl.packed_quantity, fl.despatched_quantity)
               IS DISTINCT FROM
               (coalesce(f.covered, 0), coalesce(f.picked, 0),
                coalesce(f.packed, 0), coalesce(f.despatched, 0))
        RETURNING fl.id)
    SELECT count(*) INTO touched FROM updated;

    RETURN touched;
END
$$;

ALTER FUNCTION projection_fulfilment_rebuild(uuid) OWNER TO nylonite_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_fulfilment_rebuild(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_fulfilment_rebuild(uuid)
    TO nylonite_scheduler, nylonite_platform;

COMMENT ON FUNCTION projection_fulfilment_rebuild(uuid) IS
    'Maintainer for fulfilment_line''s four coverage quantities. D53. There is no '
    'fulfilment arm: progress was dropped rather than defined, because a fulfilment '
    'has few lines and a stored label can disagree with them.';

GRANT SELECT, UPDATE ON fulfilment_line TO nylonite_projection_owner;
GRANT SELECT ON stock_allocation TO nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- 5. A registered maintainer that maintained nothing
-- ---------------------------------------------------------------------------
--
-- Found on the way here, and it is the same defect one level deeper than the one
-- that raised 134. Migration 3 registered five columns to `projection_stock_rebuild`:
--
--     stock.quantity, weight_g, allocated_quantity, resolved_location_id, site_id
--
-- The function body never mentions `allocated_quantity`. It has been registered
-- to a maintainer that does not maintain it since migration 3.
--
-- S5's three-way diff cannot see this. The table exists, the column exists, the
-- function exists, and the column is commented and registered, so all three legs
-- agree. What none of them checks is whether the function named actually writes
-- the column claimed. That is S43's second half.
--
-- It has been harmless only because no allocation has ever existed. The moment
-- one does, J3 finds drift on every run and nothing anywhere fixes it.

CREATE OR REPLACE FUNCTION projection_stock_rebuild(p_tenant uuid)
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
$$;

ALTER FUNCTION projection_stock_rebuild(uuid) OWNER TO nylonite_projection_owner;
