-- Reverse of 2026-08-04-000055_a_cell_in_a_carton_settles.
--
-- `stock` goes back to never settling for a package-held cell: step 30 writes NULL
-- over the container arm step 70 resolved, and over the site_id derived from it, and
-- 70 restores both on the next run. Three writes per cell per run, forever, with the
-- right value at the end of each one.
--
-- Nothing is destroyed and no value changes. What returns is the churn, which is why
-- this reverses cleanly and why it went unnoticed for fifty-two migrations.

-- Migration 15's body as it stood before D101 -- the D68 guard, without the
-- shared-ownership CASE.
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
        -- D68. Without this the upsert rewrites every cell every run, whether or
        -- not the fold moved it. Idempotent in value is not the same as writing
        -- nothing, and under MVCC the difference is a dead tuple per row per run.
        WHERE (s.quantity, s.weight_g, s.resolved_location_id, s.site_id)
              IS DISTINCT FROM
              (EXCLUDED.quantity, EXCLUDED.weight_g, EXCLUDED.resolved_location_id,
               EXCLUDED.site_id)
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

-- The unguarded container-arm update, restored.
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
       AND s.holder_package_id = pkg.id;

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
