-- Migration 55: a cell held in a carton settles.
--
-- D101. Found by the fixture the moment it grew its first package-held stock cell,
-- which D100's outbound section added on the previous commit.
--
-- **`stock` never converged for a package-held cell, and had not since migration 3.**
-- Two maintainers own one column between them and neither knew it:
--
--   30  projection_stock_rebuild            writes resolved_location_id = holder_location_id
--   70  projection_stock_resolve_locations  writes resolved_location_id = the package's
--
-- For a cell held in a location the two agree. For a cell held in a *package*
-- `holder_location_id` is NULL by the key CHECK, so 30 writes NULL over what 70
-- resolved, and 70 resolves it again on the next run. The same happens to `site_id`,
-- which 30 reads off a location it does not have.
--
-- So every run wrote that row three times -- 30 nulls two columns, 70 restores each
-- -- and the fold oscillated forever rather than settling. D68 states the property
-- this breaks: *"a rebuild that changes nothing must still write nothing"*, because
-- under MVCC each rewrite is a dead tuple and a projection rebuilt hourly churns its
-- whole table hourly.
--
-- **Nothing was wrong with the values.** Every run ended with the right answer, which
-- is why no fold check ever complained: J1 and J7 compare values, and the values were
-- correct at the moment they were read. What was wrong was that arriving at them cost
-- three writes per cell per run, forever, and the only check that could see it is the
-- one that counts writes rather than reads.
--
-- It went unseen for fifty-two migrations because **the fixture had no package-held
-- stock cell**. Not an untested branch -- an untested *shape*, reachable only by
-- putting stock into a carton and leaving it there, which nothing did until an order
-- was picked into one.

-- ---------------------------------------------------------------------------
-- 1. Step 30 stops writing what step 70 owns
-- ---------------------------------------------------------------------------
--
-- 30 cannot resolve a container's location and should not pretend to: a package's
-- placement is folded at 40, nine steps later, which is the whole reason 70 exists.
-- The ordinal is the dependency order, so the fix is not to reorder anything but to
-- stop the earlier step asserting a value it has no source for.
--
-- The INSERT arm still writes NULL for both, and that is correct: a cell in a carton
-- is created with its container arm unresolved and 70 fills it in the same run. One
-- write on creation, and none afterwards.

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
$$;

ALTER FUNCTION projection_stock_rebuild(uuid) OWNER TO nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- 2. Step 70 stops writing when it has nothing to say
-- ---------------------------------------------------------------------------
--
-- The second half, and it is a plain D68 omission rather than a shared-ownership
-- problem: the container-arm UPDATE carried no distinctness guard at all, so it
-- rewrote every package-held cell on every run even when the package had not moved.
-- Its sibling below it, the site_id update, has had one since it was written.
--
-- `touched` now counts cells whose container arm actually moved, which is what the
-- orchestrator reports and what D95's freshness stamp is measured against. A step
-- that reports work it did not do is a step nobody can use to find a stuck fold.

CREATE OR REPLACE FUNCTION projection_stock_resolve_locations(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

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
$$;

ALTER FUNCTION projection_stock_resolve_locations(uuid) OWNER TO nylonite_projection_owner;
