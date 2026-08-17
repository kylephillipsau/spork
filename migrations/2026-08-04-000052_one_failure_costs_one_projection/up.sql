-- Migration 52: a maintainer that raises costs one projection, not all of them.
--
-- D98, settling question 166, which D97 raised by fixing the first way in and
-- noticing the blast radius belonged somewhere else.
--
-- `projection_run_all` walks `projection_step` in one transaction. A maintainer
-- that raises aborts the run **and rolls back the steps that had already
-- succeeded**, so the tenant is left with no fold at all rather than a partial
-- one, and nothing records that it happened. D97 closed the one reachable case;
-- the next disagreement between a source table and a projection's own constraints
-- does the same thing.
--
-- That is D8 inverted, and D8 is the rule this system is built on: a disagreement
-- becomes a finding and never stops the floor.

-- ---------------------------------------------------------------------------
-- 1. A fold that could not run is a finding, and the queue already exists
-- ---------------------------------------------------------------------------
--
-- Not `projection_drift`, which is a projection holding a **wrong** value. This is
-- a projection holding **no** value, because the fold never completed, and the two
-- want different responses: drift is fixed by rebuilding, and this is fixed by
-- repairing whatever the maintainer choked on.
--
-- `ALTER TYPE ... ADD VALUE` has no inverse, which migrations 30 and 40 already
-- documented and survived the same way.

ALTER TYPE discrepancy_kind ADD VALUE IF NOT EXISTS 'projection_failed';

-- ---------------------------------------------------------------------------
-- 2. Why it failed, beside when it last worked
-- ---------------------------------------------------------------------------
--
-- **`last_run_at` deliberately does not move on a failure.** D95 made it the
-- answer to "how old is this number", and a fold that raised produced no number --
-- so the honest record is that the projection is ageing, which is what J66 already
-- watches for. `last_error` says why it stopped ageing well.

ALTER TABLE projection_freshness
    ADD COLUMN last_error    text,
    ADD COLUMN last_error_at timestamptz,
    ADD CONSTRAINT projection_freshness_error_pair_ck
        CHECK ((last_error IS NULL) = (last_error_at IS NULL));

COMMENT ON COLUMN projection_freshness.last_error IS
    'Why this maintainer last refused to complete, or NULL if its last attempt '
    'succeeded. last_run_at is not advanced by a failure, so a projection that '
    'keeps raising goes stale and J66 reports it. D98.';

-- ---------------------------------------------------------------------------
-- 3. The orchestrator survives its steps
-- ---------------------------------------------------------------------------
--
-- A PL/pgSQL block with an EXCEPTION clause is an implicit savepoint, so a step
-- that raises undoes its own work and nothing else. What it does **not** do is
-- carry on down the list.
--
-- **The ordinal is a dependency order, not a preference.** `projection_step` says
-- so in its own notes -- *"Needs cells from 30 and placements from 40"* -- so
-- everything after a failed step may be about to fold over inputs that were never
-- written. Running them would turn one broken projection into several wrong ones,
-- which is worse than the thing this migration is fixing.
--
-- So the sequence stops, and the difference from today is the part that matters:
-- **what already succeeded is kept**, the failure is recorded where a human will
-- see it, and the steps that never ran go stale rather than silently holding
-- yesterday's values with no explanation.

CREATE OR REPLACE FUNCTION projection_run_all(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    r        record;
    n        bigint;
    total    bigint := 0;
    started  timestamptz;
    err      text;
    err_code text;
BEGIN
    FOR r IN SELECT function_name FROM projection_step ORDER BY ordinal
    LOOP
        started := clock_timestamp();
        BEGIN
            EXECUTE format('SELECT %I($1)', r.function_name) USING p_tenant INTO n;
        EXCEPTION WHEN OTHERS THEN
            GET STACKED DIAGNOSTICS err = MESSAGE_TEXT, err_code = RETURNED_SQLSTATE;

            INSERT INTO projection_freshness
                (tenant_id, function_name, last_run_at, last_run_ms, rows_touched,
                 last_error, last_error_at)
            VALUES (p_tenant, r.function_name, NULL, 0, 0,
                    err_code || ': ' || err, now())
            ON CONFLICT (tenant_id, function_name) DO UPDATE
               SET last_error    = excluded.last_error,
                   last_error_at = excluded.last_error_at;

            INSERT INTO discrepancy (tenant_id, kind, detail, automation_key)
            VALUES (p_tenant, 'projection_failed',
                    r.function_name || ' raised ' || err_code || ': ' || err
                        || '. Every step after it was skipped, because the ordinal '
                        || 'is a dependency order and they would fold over inputs '
                        || 'that were never written.',
                    'projection-run');

            RETURN total;
        END;

        n := coalesce(n, 0);
        total := total + n;

        -- D70. Only on work, so D68's property survives.
        IF n > 0 THEN
            UPDATE projection_step
               SET last_changed_at = now()
             WHERE function_name = r.function_name;
        END IF;

        -- D95, and D98 clears the error a previous run recorded.
        INSERT INTO projection_freshness
            (tenant_id, function_name, last_run_at, last_run_ms, rows_touched)
        VALUES (p_tenant, r.function_name, now(),
                (extract(epoch FROM clock_timestamp() - started) * 1000)::integer, n)
        ON CONFLICT (tenant_id, function_name) DO UPDATE
           SET last_run_at  = excluded.last_run_at,
               last_run_ms  = excluded.last_run_ms,
               rows_touched = excluded.rows_touched,
               last_error    = NULL,
               last_error_at = NULL;
    END LOOP;
    RETURN total;
END
$$;

COMMENT ON FUNCTION projection_run_all(uuid) IS
    'Runs every declared maintainer in ordinal order. A step that raises is '
    'recorded against its freshness row and as a projection_failed discrepancy, '
    'the steps before it are kept, and the sequence stops -- the ordinal is a '
    'dependency order, so continuing would fold over inputs nobody wrote. D64, '
    'D95, D98.';

-- `last_run_at` becomes nullable, because a projection whose first attempt failed
-- has never run and must not claim a time.
ALTER TABLE projection_freshness ALTER COLUMN last_run_at DROP NOT NULL;
