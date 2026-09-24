-- Migration 29: what makes a cached resolution stale, and the half nobody writes.
--
-- D70, settling question 80. The question's full text lives in
-- `mechanism-design.md` rather than in the register, which is worth noticing on
-- the way past — the register carries the one-line title and the reasoning stayed
-- behind, which is the failure the register exists to stop. It reads:
--
--   "Resolver cache invalidation. A missed invalidation means the floor runs on
--    stale weights and nothing detects it. `policy_change.id` as a monotonic
--    epoch per (tenant, kind), checked on every resolution rather than a TTL --
--    but it needs designing, not assuming."
--
-- The instinct is right and the mechanism is short by three things.

-- ---------------------------------------------------------------------------
-- What actually changes a resolution
-- ---------------------------------------------------------------------------
--
--   1. A value version arrives or retires        -> policy_change
--   2. A binding is added or superseded          -> policy_binding
--   3. The taxonomy is re-parented               -> the closures
--   4. **An effective range opens or closes**    -> nothing at all
--
-- `policy_change.id` sees the first. It does not see the second: D22 makes a
-- binding's scope immutable and a change a *new* binding superseding the old, and
-- a binding is created without a `policy_change` — measured, one of three in the
-- fixture has no change record and is correct not to.
--
-- It does not see the third, which is question 78's territory.
--
-- **And nothing sees the fourth.** `%_policy.effective` is a `tstzrange`. A
-- resolution taken at 09:00 can be wrong at 09:01 because a version's range
-- ended, and no row was written when it did. An epoch — any epoch — is
-- insensitive to it by construction, because an epoch is a fact about writes.
-- A cache keyed on an epoch alone serves that stale answer forever, which is
-- exactly the failure the question names: *the floor runs on stale weights and
-- nothing detects it*.
--
-- So validity is two conditions and both are exact:
--
--   the epoch is unchanged        covers everything that is written
--   now < expires_at              covers the one thing that is not
--
-- Neither is a TTL. A TTL is a guess about how wrong you are willing to be, and
-- the thing being guessed about is which stock ships.

-- ---------------------------------------------------------------------------
-- 1. The taxonomy arm, from the maintainer's own report
-- ---------------------------------------------------------------------------
--
-- The closures have no timestamp and S7 forbids the trigger that would give them
-- one. But D64 already has every maintainer reporting how many rows it touched,
-- and D68 made that number zero when nothing changed. So the stamp is free: the
-- orchestrator records when a step last did something.
--
-- Stamped only when rows were touched, which is what keeps D68's property true —
-- a run that changes nothing still writes nothing.

ALTER TABLE projection_step ADD COLUMN last_changed_at timestamptz;

COMMENT ON COLUMN projection_step.last_changed_at IS
    'When this step last touched a row. Written by projection_run_all, and only '
    'when the step reported work, so a no-op run stays a no-op. It is what lets '
    'the policy epoch see a taxonomy change that no policy table records. D70.';

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

ALTER FUNCTION projection_run_all(uuid) OWNER TO spork_projection_owner;
GRANT SELECT, UPDATE ON projection_step TO spork_projection_owner;

-- ---------------------------------------------------------------------------
-- 2. The epoch, derived rather than stored
-- ---------------------------------------------------------------------------
--
-- Every other counter in this repository that was stored went wrong — the
-- invariant count, the question total, the `@projection` markers. This one is a
-- `greatest()` over three indexed reads and is correct by not existing.
--
-- **The taxonomy arm is deliberately coarse.** `projection_step` is global, so a
-- closure rebuild for one tenant bumps every tenant's epoch. That over-invalidates
-- and is safe; the opposite mistake serves a wrong answer, and D22's whole
-- position is that a resolution nobody can explain is worse than a slow one.
--
-- STABLE rather than IMMUTABLE: it reads tables. Not SECURITY DEFINER, because it
-- should see exactly what its caller sees.

CREATE FUNCTION policy_epoch(p_tenant uuid) RETURNS timestamptz
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    SELECT greatest(
        -- 1. Value versions. Every one has a policy_change, which J13 asserts.
        (SELECT max(recorded_at) FROM policy_change
          WHERE tenant_id = p_tenant OR tenant_id IS NULL),
        -- 2. Scope. A superseded binding is a new row, and it carries no
        --    policy_change of its own, which is why the epoch cannot read only
        --    the change log.
        (SELECT max(created_at) FROM policy_binding
          WHERE tenant_id = p_tenant OR tenant_id IS NULL),
        -- 3. The taxonomy, through the maintainer that rebuilds its closure.
        (SELECT max(last_changed_at) FROM projection_step
          WHERE function_name LIKE 'projection\_%\_class\_closure\_rebuild')
    )
$$;

COMMENT ON FUNCTION policy_epoch(uuid) IS
    'The instant after which a cached resolution may be wrong because something '
    'was written. Read once per unit of work, never per resolution -- the same '
    'discipline S23 imposes on the resolver itself. D22, D70.';

GRANT EXECUTE ON FUNCTION policy_epoch(uuid) TO spork_app, spork_platform,
    spork_scheduler, spork_projection_owner;

-- ---------------------------------------------------------------------------
-- 3. The half no write announces
-- ---------------------------------------------------------------------------
--
-- The next instant at which a resolution could change because a range opened or
-- closed. A cache entry computed now is valid until this, exactly — not for some
-- number of seconds somebody chose.
--
-- NULL means no boundary is coming, which is the ordinary case: a policy set with
-- open-ended ranges and no future version has nothing scheduled to change.

CREATE FUNCTION policy_next_boundary(p_tenant uuid, p_at timestamptz DEFAULT now())
    RETURNS timestamptz
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    SELECT min(b) FROM (
        SELECT lower(effective) AS b FROM allocation_policy v
          JOIN policy_binding pb ON pb.id = v.policy_binding_id
         WHERE (pb.tenant_id = p_tenant OR pb.tenant_id IS NULL) AND lower(effective) > p_at
        UNION ALL
        SELECT upper(effective) FROM allocation_policy v
          JOIN policy_binding pb ON pb.id = v.policy_binding_id
         WHERE (pb.tenant_id = p_tenant OR pb.tenant_id IS NULL) AND upper(effective) > p_at
        UNION ALL
        SELECT lower(effective) FROM receiving_policy v
          JOIN policy_binding pb ON pb.id = v.policy_binding_id
         WHERE (pb.tenant_id = p_tenant OR pb.tenant_id IS NULL) AND lower(effective) > p_at
        UNION ALL
        SELECT upper(effective) FROM receiving_policy v
          JOIN policy_binding pb ON pb.id = v.policy_binding_id
         WHERE (pb.tenant_id = p_tenant OR pb.tenant_id IS NULL) AND upper(effective) > p_at
        UNION ALL
        SELECT lower(effective) FROM shelf_life_policy v
          JOIN policy_binding pb ON pb.id = v.policy_binding_id
         WHERE (pb.tenant_id = p_tenant OR pb.tenant_id IS NULL) AND lower(effective) > p_at
        UNION ALL
        SELECT upper(effective) FROM shelf_life_policy v
          JOIN policy_binding pb ON pb.id = v.policy_binding_id
         WHERE (pb.tenant_id = p_tenant OR pb.tenant_id IS NULL) AND upper(effective) > p_at
    ) boundaries
$$;

COMMENT ON FUNCTION policy_next_boundary(uuid, timestamptz) IS
    'The next instant a resolution could change with nothing written, because an '
    'effective range opens or closes. A cache entry is valid until exactly this. '
    'NULL means nothing is scheduled. D22, D70.';

GRANT EXECUTE ON FUNCTION policy_next_boundary(uuid, timestamptz) TO spork_app,
    spork_platform, spork_scheduler, spork_projection_owner;

-- ---------------------------------------------------------------------------
-- 4. What is not built here
-- ---------------------------------------------------------------------------
--
-- The cache. There is no resolver yet, so there is nothing to cache and the
-- storage is the resolver's own decision. What is built is the two questions a
-- cache has to be able to ask, and S50 is what keeps them able to answer.
--
-- Re-parenting still does not announce itself as an act — the epoch sees it only
-- because a maintainer noticed the closure moved, which is a consequence rather
-- than a record. That is question 78 and it stays open.
