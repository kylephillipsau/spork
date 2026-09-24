-- Migration 62: the drain sets the tenant, so RLS does not hide dirty rows.
--
-- D107's projection_run_dirty selected from projection_dirty without a tenant
-- setting. FORCE RLS applies to spork_projection_owner as well, so the
-- single tenant-scoped policy (tenant_id = current_tenant()) returned zero rows,
-- the loop never ran, and nothing was cleared. Mark-dirty still succeeded.
--
-- Fix the function, not the policy: for each tenant, SET LOCAL the tenant id,
-- then see and clear that tenant's dirty row under the same shape S9 already
-- permits. No USING (true) policy — that is outside the three RLS shapes.
--
-- The owner needs SELECT on tenant to walk the set; it already receives tenant
-- ids as parameters on every other maintainer and never listed them itself.

GRANT SELECT ON tenant TO spork_projection_owner;

CREATE OR REPLACE FUNCTION projection_run_dirty()
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    r     record;
    n     bigint;
    total bigint := 0;
BEGIN
    -- last changed: migration 62 (D107)
    FOR r IN
        SELECT id AS tenant_id FROM tenant ORDER BY id
    LOOP
        -- RLS on projection_dirty is tenant_id = current_tenant(); without this
        -- the definer sees nothing even as table owner under FORCE.
        PERFORM set_config('spork.tenant_id', r.tenant_id::text, true);
        IF EXISTS (
            SELECT 1 FROM projection_dirty WHERE tenant_id = r.tenant_id
        ) THEN
            n := projection_run_all(r.tenant_id);
            total := total + coalesce(n, 0);
            DELETE FROM projection_dirty WHERE tenant_id = r.tenant_id;
        END IF;
    END LOOP;
    RETURN total;
END
$$;

ALTER FUNCTION projection_run_dirty() OWNER TO spork_projection_owner;

COMMENT ON FUNCTION projection_run_dirty() IS
    'Scheduler entry: for each tenant, set the tenant context, and if that tenant '
    'is dirty run projection_run_all and clear the dirty row. Owned by '
    'spork_projection_owner. D107; loop shape fixed in migration 62 so FORCE '
    'RLS does not hide the dirty set.';

REVOKE ALL ON FUNCTION projection_run_dirty() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_run_dirty() TO spork_scheduler;
GRANT EXECUTE ON FUNCTION projection_run_dirty() TO spork_platform;
GRANT EXECUTE ON FUNCTION projection_run_dirty() TO spork_projection_owner;
