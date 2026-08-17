-- Reverse of 2026-08-04-000052_one_failure_costs_one_projection.
--
-- One maintainer that raises goes back to costing every projection for the tenant,
-- and leaving no record that it happened.
--
-- `ALTER TYPE ... ADD VALUE` has no inverse in Postgres, so `discrepancy_kind`
-- keeps `projection_failed` after this runs -- the residue migrations 30 and 40
-- documented, survivable for the same reason: the up migration adds it with
-- IF NOT EXISTS.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM projection_freshness WHERE last_error IS NOT NULL;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D98 destroys the recorded failure of % maintainer(s): '
                     'why a projection stopped folding becomes unanswerable again', n;
    END IF;
END $$;

DELETE FROM projection_freshness WHERE last_run_at IS NULL;
ALTER TABLE projection_freshness ALTER COLUMN last_run_at SET NOT NULL;

CREATE OR REPLACE FUNCTION projection_run_all(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    r       record;
    n       bigint;
    total   bigint := 0;
    started timestamptz;
BEGIN
    FOR r IN SELECT function_name FROM projection_step ORDER BY ordinal
    LOOP
        started := clock_timestamp();
        EXECUTE format('SELECT %I($1)', r.function_name) USING p_tenant INTO n;
        n := coalesce(n, 0);
        total := total + n;

        IF n > 0 THEN
            UPDATE projection_step
               SET last_changed_at = now()
             WHERE function_name = r.function_name;
        END IF;

        INSERT INTO projection_freshness
            (tenant_id, function_name, last_run_at, last_run_ms, rows_touched)
        VALUES (p_tenant, r.function_name, now(),
                (extract(epoch FROM clock_timestamp() - started) * 1000)::integer, n)
        ON CONFLICT (tenant_id, function_name) DO UPDATE
           SET last_run_at  = excluded.last_run_at,
               last_run_ms  = excluded.last_run_ms,
               rows_touched = excluded.rows_touched;
    END LOOP;
    RETURN total;
END
$$;

COMMENT ON FUNCTION projection_run_all(uuid) IS NULL;

ALTER TABLE projection_freshness
    DROP CONSTRAINT IF EXISTS projection_freshness_error_pair_ck,
    DROP COLUMN IF EXISTS last_error_at,
    DROP COLUMN IF EXISTS last_error;
