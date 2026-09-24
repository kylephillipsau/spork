-- Migration 61: a tenant may ask for a rebuild, and dirty tenants run first.
--
-- D107, settling the "mediated refresh" product gap after outbound write paths.
--
-- The ledger is true on INSERT. Projected columns lag until maintainers run
-- (D25, D95). Three mechanisms, in the order the product needs them:
--
--   A. Live ledger views in the HTTP layer (no schema) — O(line), never write.
--   B. projection_dirty — write paths mark the tenant; the scheduler drains it.
--   C. projection_refresh_tenant — app may request a full-tenant rebuild through
--      the projection owner, rate-limited, bound to current_tenant().
--
-- D95 rejected granting the app EXECUTE on projection_run_all. This migration
-- does not reverse that. It grants EXECUTE on a *wrapper* that cannot rebuild
-- another tenant and cannot be spammed.

-- ---------------------------------------------------------------------------
-- B. Dirty set
-- ---------------------------------------------------------------------------

CREATE TABLE projection_dirty (
    tenant_id uuid PRIMARY KEY REFERENCES tenant(id),
    dirty_at  timestamptz NOT NULL,
    -- Optional free-text for operators: 'pick', 'despatch', …
    reason    text
);

COMMENT ON TABLE projection_dirty IS
    'REFERENCE, tenant-scoped. Tenants whose projections lag a write that just '
    'happened. The scheduler drains this set by calling projection_run_dirty. '
    'Not a log: one row per tenant, upserted. D107.';

ALTER TABLE projection_dirty ENABLE ROW LEVEL SECURITY;
ALTER TABLE projection_dirty FORCE ROW LEVEL SECURITY;
CREATE POLICY projection_dirty_tenant_scoped ON projection_dirty
    USING (tenant_id = current_tenant());

-- App marks itself dirty after a floor write. Scheduler/platform may scan all.
GRANT SELECT, INSERT, UPDATE, DELETE ON projection_dirty TO spork_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON projection_dirty TO spork_scheduler;
GRANT SELECT, INSERT, UPDATE, DELETE ON projection_dirty TO spork_platform;
GRANT SELECT, INSERT, UPDATE, DELETE ON projection_dirty TO spork_projection_owner;

CREATE OR REPLACE FUNCTION projection_mark_dirty(p_tenant uuid, p_reason text DEFAULT NULL)
    RETURNS void
    LANGUAGE sql
    SECURITY INVOKER
    SET search_path = pg_catalog, public
    AS $$
    -- last changed: migration 61 (D107)
    INSERT INTO projection_dirty (tenant_id, dirty_at, reason)
    VALUES (p_tenant, now(), p_reason)
    ON CONFLICT (tenant_id) DO UPDATE
       SET dirty_at = excluded.dirty_at,
           reason   = coalesce(excluded.reason, projection_dirty.reason);
$$;

COMMENT ON FUNCTION projection_mark_dirty(uuid, text) IS
    'Upsert this tenant into the dirty set. Callable by the app after a write. D107.';

GRANT EXECUTE ON FUNCTION projection_mark_dirty(uuid, text) TO spork_app;
GRANT EXECUTE ON FUNCTION projection_mark_dirty(uuid, text) TO spork_scheduler;
GRANT EXECUTE ON FUNCTION projection_mark_dirty(uuid, text) TO spork_platform;

-- Scheduler drain: rebuild every dirty tenant, clear on success.
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

-- ---------------------------------------------------------------------------
-- C. Rate-limited on-demand refresh for the current tenant
-- ---------------------------------------------------------------------------

-- Track last on-demand refresh separately from dirty/scheduler stamps so a
-- scheduler run does not open a free refresh window and vice versa.
ALTER TABLE projection_freshness
    ADD COLUMN IF NOT EXISTS last_on_demand_at timestamptz;

COMMENT ON COLUMN projection_freshness.last_on_demand_at IS
    'When projection_refresh_tenant last accepted a rebuild for this step. '
    'Rate limit is the max of these across steps for the tenant. D107.';

CREATE OR REPLACE FUNCTION projection_refresh_tenant(p_tenant uuid DEFAULT NULL)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    t           uuid;
    current_t   uuid;
    last_demand timestamptz;
    n           bigint;
    min_gap     interval := interval '5 seconds';
BEGIN
    -- last changed: migration 61 (D107)
    current_t := nullif(current_setting('spork.tenant_id', true), '')::uuid;
    IF current_t IS NULL THEN
        RAISE EXCEPTION 'projection_refresh_tenant requires spork.tenant_id'
            USING ERRCODE = '42501';
    END IF;

    t := coalesce(p_tenant, current_t);
    IF t IS DISTINCT FROM current_t THEN
        RAISE EXCEPTION 'projection_refresh_tenant cannot rebuild another tenant'
            USING ERRCODE = '42501';
    END IF;

    SELECT max(last_on_demand_at) INTO last_demand
      FROM projection_freshness
     WHERE tenant_id = t;

    IF last_demand IS NOT NULL AND now() - last_demand < min_gap THEN
        -- Still fresh enough from an on-demand call. NULL is not 0: a warm
        -- rebuild under D68 legitimately touches zero rows, and the HTTP
        -- surface must not confuse "rate limited" with "accepted, idle".
        RETURN NULL;
    END IF;

    n := projection_run_all(t);

    -- run_all stamps freshness rows for every step; this column is only for
    -- the on-demand rate limit and is independent of last_run_at (scheduler).
    UPDATE projection_freshness
       SET last_on_demand_at = now()
     WHERE tenant_id = t;

    DELETE FROM projection_dirty WHERE tenant_id = t;

    RETURN n;
END
$$;

ALTER FUNCTION projection_refresh_tenant(uuid) OWNER TO spork_projection_owner;

COMMENT ON FUNCTION projection_refresh_tenant(uuid) IS
    'App-callable full-tenant rebuild for current_tenant() only, rate-limited to '
    'one accepted call per 5 seconds. Owned by spork_projection_owner — the '
    'app does not become the maintainer; it may request a rebuild. D95 forbade '
    'EXECUTE on projection_run_all; this wrapper is the allowed path. D107.';

REVOKE ALL ON FUNCTION projection_refresh_tenant(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_refresh_tenant(uuid) TO spork_app;
GRANT EXECUTE ON FUNCTION projection_refresh_tenant(uuid) TO spork_scheduler;
GRANT EXECUTE ON FUNCTION projection_refresh_tenant(uuid) TO spork_platform;
GRANT EXECUTE ON FUNCTION projection_refresh_tenant(uuid) TO spork_projection_owner;
