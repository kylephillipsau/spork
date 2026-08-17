-- Migration 5: containment history.
--
-- Migration 4 folded package_event into where a package is *now*. This folds the
-- same log into where it was *then*.
--
-- The inbound analysis named what is missing without it: "where was pallet P on
-- 15 June is unanswerable, per-pallet storage billing has no interval to query,
-- an orphan SSCC scanned at the dock cannot be re-parented when the ASN arrives,
-- and split/merge is a mutable column plus a delete."
--
-- D40 then leaned on it: pallet-days for third-party storage billing fall out of
-- these intervals, and D40 named the derivation as a consumer so that a later
-- change to this table knows it has one.

-- Required for an exclusion constraint mixing uuid equality with range overlap.
-- The uuid = operator has no gist opclass in core.
CREATE EXTENSION IF NOT EXISTS btree_gist;

-- ---------------------------------------------------------------------------
-- The register's version stamp (D24)
-- ---------------------------------------------------------------------------

-- Which event produced the placement currently projected. Without it, a
-- compare-and-set that loses is indistinguishable from one that never happened,
-- and the losing event's discrepancy has nothing to point at.
ALTER TABLE package
    ADD COLUMN placement_event_id    uuid REFERENCES package_event(id),
    ADD COLUMN placement_occurred_at timestamptz;

COMMENT ON COLUMN package.placement_event_id IS
    '@projection: the package_event that won the register. D24.';
COMMENT ON COLUMN package.placement_occurred_at IS
    '@projection: that event''s device clock, which is what a later event has to '
    'beat to take the placement. D24.';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('package', 'placement_event_id',    'projection_package_rebuild'),
    ('package', 'placement_occurred_at', 'projection_package_rebuild');

-- ---------------------------------------------------------------------------
-- The interval form (D24, D40)
-- ---------------------------------------------------------------------------

CREATE TABLE package_containment (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         uuid NOT NULL REFERENCES tenant(id),
    package_id        uuid NOT NULL REFERENCES package(id),
    parent_package_id uuid REFERENCES package(id),
    location_id       uuid REFERENCES location(id),
    valid             tstzrange NOT NULL,
    source_event_id   uuid NOT NULL REFERENCES package_event(id),

    CONSTRAINT package_containment_holder_ck
        CHECK (num_nonnulls(parent_package_id, location_id) = 1),

    -- One package is in one place at a time. An exclusion constraint rather
    -- than PG18's WITHOUT OVERLAPS: the same guarantee, enforced by DDL rather
    -- than by a job, available since 9.x, and it makes the interval idiom
    -- identical to the one %_policy.effective and order_tolerance_band already
    -- use. One CI assertion template covers all three.
    --
    -- The deployment floor stays PostgreSQL 15, which UNIQUE NULLS NOT DISTINCT
    -- on the stock key genuinely requires. Depending on a PG18 feature here
    -- would raise that floor for no additional guarantee, and D18 keeps
    -- self-hosted deployment on the table.
    CONSTRAINT package_containment_no_overlap
        EXCLUDE USING gist (package_id WITH =, valid WITH &&)
);

COMMENT ON TABLE package_containment IS
    'PROJECTION of package_event, interval form. Answers where a package was at '
    'a time, which the current-state columns cannot. D24, and D40 derives '
    'pallet-days from it.';

CREATE INDEX package_containment_at_time_idx
    ON package_containment USING gist (package_id, valid);
CREATE INDEX package_containment_location_idx
    ON package_containment (location_id) WHERE location_id IS NOT NULL;

ALTER TABLE package_containment ENABLE ROW LEVEL SECURITY;
ALTER TABLE package_containment FORCE ROW LEVEL SECURITY;
CREATE POLICY package_containment_tenant_scoped ON package_containment
    USING (tenant_id = current_tenant());

GRANT SELECT ON package_containment TO nylonite_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON package_containment TO nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- The maintainer, extended (D35)
-- ---------------------------------------------------------------------------

CREATE FUNCTION projection_package_containment_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    built bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    -- This one is rebuilt rather than upserted, and that is a real difference
    -- from stock. Nothing holds a durable foreign key to a containment interval,
    -- so identity across rebuilds buys nothing, and the intervals change shape
    -- rather than value when a late event lands: one row becomes two. J33's
    -- argument does not reach here.
    DELETE FROM package_containment WHERE tenant_id = p_tenant;

    WITH ordered AS (
        SELECT id, package_id, parent_package_id, location_id, occurred_at,
               lead(occurred_at) OVER (
                   PARTITION BY package_id
                   ORDER BY occurred_at, recorded_at, id) AS next_at
          FROM package_event
         WHERE tenant_id = p_tenant AND asserts_placement
    )
    INSERT INTO package_containment
        (tenant_id, package_id, parent_package_id, location_id, valid, source_event_id)
    SELECT p_tenant, package_id, parent_package_id, location_id,
           -- Half-open, so consecutive intervals abut without overlapping and
           -- the exclusion constraint is satisfiable rather than merely lucky.
           tstzrange(occurred_at, next_at, '[)'),
           id
      FROM ordered
     -- A zero-width interval is an event superseded at the same instant by a
     -- later-recorded one. It held the placement for no time, so it gets no row,
     -- and the register still keeps the event itself.
     WHERE next_at IS NULL OR next_at > occurred_at;

    GET DIAGNOSTICS built = ROW_COUNT;
    RETURN built;
END
$$;

ALTER FUNCTION projection_package_containment_rebuild(uuid)
    OWNER TO nylonite_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_package_containment_rebuild(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_package_containment_rebuild(uuid)
    TO nylonite_scheduler, nylonite_platform;

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('package_containment', 'valid', 'projection_package_containment_rebuild');

-- The current-state maintainer gains the version stamp, so the two projections
-- agree about which event won.
CREATE OR REPLACE FUNCTION projection_package_stamp(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH winner AS (
        SELECT DISTINCT ON (package_id) package_id, id, occurred_at
          FROM package_event
         WHERE tenant_id = p_tenant AND asserts_placement
         ORDER BY package_id, occurred_at DESC, recorded_at DESC, id DESC
    )
    UPDATE package p
       SET placement_event_id = w.id, placement_occurred_at = w.occurred_at
      FROM winner w
     WHERE p.id = w.package_id AND p.tenant_id = p_tenant;

    GET DIAGNOSTICS touched = ROW_COUNT;
    RETURN touched;
END
$$;

ALTER FUNCTION projection_package_stamp(uuid) OWNER TO nylonite_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_package_stamp(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_package_stamp(uuid)
    TO nylonite_scheduler, nylonite_platform;
