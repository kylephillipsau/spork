-- Migration 62 down: restore the broken scan (no tenant set in the loop).

REVOKE SELECT ON tenant FROM spork_projection_owner;

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
    -- last changed: migration 61 (D107)
    FOR r IN
        SELECT tenant_id FROM projection_dirty ORDER BY dirty_at
    LOOP
        n := projection_run_all(r.tenant_id);
        total := total + coalesce(n, 0);
        DELETE FROM projection_dirty WHERE tenant_id = r.tenant_id;
    END LOOP;
    RETURN total;
END
$$;

ALTER FUNCTION projection_run_dirty() OWNER TO spork_projection_owner;

COMMENT ON FUNCTION projection_run_dirty() IS
    'Scheduler entry: run projection_run_all for every dirty tenant, oldest first, '
    'and clear each on success. Owned by spork_projection_owner. D107.';

REVOKE ALL ON FUNCTION projection_run_dirty() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_run_dirty() TO spork_scheduler;
GRANT EXECUTE ON FUNCTION projection_run_dirty() TO spork_platform;
GRANT EXECUTE ON FUNCTION projection_run_dirty() TO spork_projection_owner;
