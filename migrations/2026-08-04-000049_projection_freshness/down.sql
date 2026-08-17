-- Reverse of 2026-08-04-000049_projection_freshness.
--
-- Projections go back to being of unknown age, and the cadence goes back to being
-- nobody's decision.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM projection_freshness;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D95 destroys the run record of % projection(s): how old a '
                     'projected number is stops being answerable', n;
    END IF;
END $$;

DROP FUNCTION IF EXISTS projection_age(uuid, text);

CREATE OR REPLACE FUNCTION projection_run_all(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    r record;
    n bigint;
    total bigint := 0;
BEGIN
    FOR r IN SELECT function_name FROM projection_step ORDER BY ordinal
    LOOP
        EXECUTE format('SELECT %I($1)', r.function_name) USING p_tenant INTO n;
        n := coalesce(n, 0);
        total := total + n;

        -- D70. Only on work, so D68's property survives: a rebuild that changes
        -- nothing writes nothing, and that now includes this row.
        IF n > 0 THEN
            UPDATE projection_step
               SET last_changed_at = now()
             WHERE function_name = r.function_name;
        END IF;
    END LOOP;
    RETURN total;
END
$$;

DROP TABLE IF EXISTS projection_freshness;

ALTER TABLE projection_step
    DROP CONSTRAINT IF EXISTS projection_step_freshness_positive_ck;
ALTER TABLE projection_step DROP COLUMN IF EXISTS freshness_bound;
