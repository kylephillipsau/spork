-- Reverse of 2026-08-04-000029_policy_epoch.

DROP FUNCTION IF EXISTS policy_next_boundary(uuid, timestamptz);
DROP FUNCTION IF EXISTS policy_epoch(uuid);

REVOKE SELECT, UPDATE ON projection_step FROM nylonite_projection_owner;

-- Migration 24's orchestrator, without the stamp.
CREATE OR REPLACE FUNCTION public.projection_run_all(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    r record;
    n bigint;
    total bigint := 0;
BEGIN
    FOR r IN SELECT function_name FROM projection_step ORDER BY ordinal
    LOOP
        EXECUTE format('SELECT %I($1)', r.function_name) USING p_tenant INTO n;
        total := total + coalesce(n, 0);
    END LOOP;
    RETURN total;
END
$function$;

ALTER FUNCTION projection_run_all(uuid) OWNER TO nylonite_projection_owner;

ALTER TABLE projection_step DROP COLUMN IF EXISTS last_changed_at;
