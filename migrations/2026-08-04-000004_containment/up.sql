-- Migration 4: containment.
--
-- D24 made a package's placement a fact rather than a column. Before it,
-- package.parent_package_id and package.location_id were columns that
-- overwrote: split, merge, re-palletise and relabel left no evidence, and it was
-- the only physical relationship in the model whose history was destroyed on
-- update. It is also the one an inbound investigation has to walk.
--
-- Placement is a register, not a counter, and that is a different CRDT class
-- from stock. D5 ruled out last-writer-wins because it silently discards a pick,
-- which is right and is about quantities. Two concurrent picks are both true.
-- Two concurrent claims that a carton is on P1 and on P2 cannot both be true,
-- and choosing a winner there is not data loss. Quantities are counters;
-- relationships are registers.

-- ---------------------------------------------------------------------------
-- The log (D24, D29)
-- ---------------------------------------------------------------------------

CREATE TABLE package_event (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         uuid NOT NULL REFERENCES tenant(id),
    site_id           uuid,
    client_event_id   uuid NOT NULL,

    occurred_at       timestamptz NOT NULL,   -- device clock; orders the register
    recorded_at       timestamptz NOT NULL DEFAULT now(),   -- server; first tiebreak

    recorded_by_id    uuid REFERENCES person(id),
    automation_key    text,
    authorised_by_id  uuid REFERENCES person(id),
    device_id         uuid,

    package_id        uuid NOT NULL REFERENCES package(id),   -- the subject, always one

    kind              text NOT NULL,
    parent_package_id uuid REFERENCES package(id),
    location_id       uuid REFERENCES location(id),
    sscc              char(18),
    barcode           text,

    -- D24 gained 'keyed' because a hand-keyed SSCC at the dock recorded as
    -- operator_scan is a false fact under D24's own "the fact recorded is the
    -- fact observed". GS1 mandates human-readable interpretation on logistic
    -- labels precisely so that workaround exists.
    source            text NOT NULL,

    asserts_placement boolean GENERATED ALWAYS AS
        (kind IN ('created', 'placed', 'contained')) STORED,

    CONSTRAINT package_event_kind_ck CHECK (kind IN
        ('created','placed','contained','observed','identified',
         'sealed','opened','relabelled','despatched','voided')),
    CONSTRAINT package_event_source_ck CHECK (source IN
        ('operator_scan','label','asn','derived','correction','keyed')),
    CONSTRAINT package_event_actor_ck
        CHECK (num_nonnulls(recorded_by_id, automation_key) = 1),

    -- A holder is a parent package or a location, never both.
    CONSTRAINT package_event_holder_ck
        CHECK (parent_package_id IS NULL OR location_id IS NULL),
    CONSTRAINT package_event_contained_ck
        CHECK (kind <> 'contained' OR parent_package_id IS NOT NULL),
    CONSTRAINT package_event_placed_ck
        CHECK (kind <> 'placed' OR location_id IS NOT NULL),
    CONSTRAINT package_event_not_own_parent_ck
        CHECK (parent_package_id IS DISTINCT FROM package_id),

    CONSTRAINT package_event_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id)
);

COMMENT ON TABLE package_event IS
    'FACT. Append-only. The register that package placement folds from, ordered '
    'by (occurred_at, recorded_at, id). D24.';

-- The fold reads in register order, per package.
CREATE INDEX package_event_fold_idx
    ON package_event (tenant_id, package_id, occurred_at, recorded_at, id);

ALTER TABLE package_event ENABLE ROW LEVEL SECURITY;
ALTER TABLE package_event FORCE ROW LEVEL SECURITY;
CREATE POLICY package_event_tenant_scoped ON package_event
    USING (tenant_id = current_tenant());

GRANT SELECT, INSERT ON package_event TO nylonite_app;
GRANT SELECT ON package_event TO nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- The projections (D24, J6)
-- ---------------------------------------------------------------------------

ALTER TABLE package
    ADD COLUMN parent_package_id    uuid REFERENCES package(id),
    ADD COLUMN location_id          uuid REFERENCES location(id),
    ADD COLUMN resolved_location_id uuid REFERENCES location(id),
    ADD COLUMN status               text,
    ADD COLUMN depth                integer,
    ADD COLUMN identifier_kind      text;

COMMENT ON COLUMN package.parent_package_id IS '@projection of package_event. J6.';
COMMENT ON COLUMN package.location_id IS '@projection of package_event. J6.';
COMMENT ON COLUMN package.resolved_location_id IS
    '@projection: own location, or the location the holder chain resolves to. J6.';
COMMENT ON COLUMN package.status IS '@projection of package_event. J6.';
COMMENT ON COLUMN package.depth IS
    '@projection: 0 at a location, +1 per containment. D24 raised D6''s cap from '
    'two levels to three because overwrap to pallet to carton is physically '
    'real. It is NOT a CHECK: depth is a projection column, so a CHECK on it '
    'would make a four-level contained event a valid fact the projection cannot '
    'represent, and since the projection must rebuild from the log in any '
    'arrival order, the rebuild wedges too. The cap is a finding.';
COMMENT ON COLUMN package.identifier_kind IS '@projection of package_event. J6.';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('package', 'parent_package_id',    'projection_package_rebuild'),
    ('package', 'location_id',          'projection_package_rebuild'),
    ('package', 'resolved_location_id', 'projection_package_rebuild'),
    ('package', 'status',               'projection_package_rebuild'),
    ('package', 'depth',                'projection_package_rebuild'),
    ('package', 'sscc',                 'projection_package_rebuild'),
    ('package', 'barcode',              'projection_package_rebuild'),
    ('package', 'identifier_kind',      'projection_package_rebuild');

-- ---------------------------------------------------------------------------
-- The maintainer (D24, D35, J6)
-- ---------------------------------------------------------------------------

-- Compare-and-set ordered by (occurred_at, recorded_at, id), so a late event
-- carrying an earlier occurred_at loses without touching the current value.
-- That is what makes the projection commutative and idempotent, and it is why
-- shuffling arrival order is a property test rather than a hope.
--
-- The loser is retained in the log and raised as a finding, which is the reason
-- Yjs stays ruled out: Y.Map keeps no loser, imposes no device-clock order, and
-- raises nothing.
CREATE FUNCTION projection_package_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    -- FORCE RLS applies to the definer, so the rebuild scopes itself to its
    -- argument or reads nothing. Same reason as projection_stock_rebuild.
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH winning_placement AS (
        -- The last placement assertion in register order wins. DISTINCT ON with
        -- a matching ORDER BY is the compare-and-set, evaluated over the whole
        -- log rather than incrementally, which is what makes a rebuild from
        -- scratch agree with an incremental apply.
        SELECT DISTINCT ON (package_id)
               package_id, parent_package_id, location_id
          FROM package_event
         WHERE tenant_id = p_tenant AND asserts_placement
         ORDER BY package_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    winning_identity AS (
        SELECT DISTINCT ON (package_id)
               package_id, sscc, barcode,
               CASE WHEN sscc IS NOT NULL THEN 'sscc'
                    WHEN barcode IS NOT NULL THEN 'barcode' END AS identifier_kind
          FROM package_event
         WHERE tenant_id = p_tenant
           AND kind IN ('identified', 'relabelled', 'created')
           AND (sscc IS NOT NULL OR barcode IS NOT NULL)
         ORDER BY package_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    winning_status AS (
        SELECT DISTINCT ON (package_id) package_id, kind AS status
          FROM package_event
         WHERE tenant_id = p_tenant
           AND kind IN ('sealed', 'opened', 'despatched', 'voided')
         ORDER BY package_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE package p
           SET parent_package_id = wp.parent_package_id,
               location_id       = wp.location_id,
               status            = ws.status,
               sscc              = COALESCE(wi.sscc, p.sscc),
               barcode           = COALESCE(wi.barcode, p.barcode),
               identifier_kind   = wi.identifier_kind
          FROM winning_placement wp
          LEFT JOIN winning_identity wi ON wi.package_id = wp.package_id
          LEFT JOIN winning_status   ws ON ws.package_id = wp.package_id
         WHERE p.id = wp.package_id AND p.tenant_id = p_tenant
        RETURNING p.id)
    SELECT count(*) INTO touched FROM updated;

    -- resolved_location_id and depth walk the holder chain. Recursive because
    -- the chain is up to three levels and the walk is a cold path: the hot
    -- receiving screen reads the projection, never this.
    WITH RECURSIVE chain AS (
        SELECT id, location_id, location_id AS resolved, 0 AS depth
          FROM package
         WHERE tenant_id = p_tenant AND location_id IS NOT NULL
        UNION ALL
        SELECT c.id, c.location_id, parent.resolved, parent.depth + 1
          FROM package c
          JOIN chain parent ON parent.id = c.parent_package_id
         WHERE c.tenant_id = p_tenant
    )
    UPDATE package p
       SET resolved_location_id = ch.resolved, depth = ch.depth
      FROM chain ch
     WHERE p.id = ch.id AND p.tenant_id = p_tenant;

    RETURN touched;
END
$$;

ALTER FUNCTION projection_package_rebuild(uuid) OWNER TO nylonite_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_package_rebuild(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_package_rebuild(uuid)
    TO nylonite_scheduler, nylonite_platform;

GRANT SELECT, UPDATE ON package TO nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- stock resolves through the package arm now (J5)
-- ---------------------------------------------------------------------------

-- Migration 3 left resolved_location_id null for anything held in a package,
-- because there was no containment to resolve through. There is now.
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
     WHERE s.tenant_id = p_tenant AND s.resolved_location_id = l.id;

    RETURN touched;
END
$$;

ALTER FUNCTION projection_stock_resolve_locations(uuid) OWNER TO nylonite_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_stock_resolve_locations(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_stock_resolve_locations(uuid)
    TO nylonite_scheduler, nylonite_platform;

GRANT SELECT ON package TO nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- The grant narrows when the projections arrive (D25, J36)
-- ---------------------------------------------------------------------------

-- Migration 2 granted table-wide UPDATE on package, when package held nothing
-- but identity. This migration just gave it eight projection columns, and a
-- table-wide grant now silently disarms every one of them. D25 names this
-- exactly: "a table-wide GRANT UPDATE would silently disarm every projection
-- guard in the schema."
--
-- So the grant becomes column-level. The application owns what a package is;
-- the maintainer owns where it is and what it is called, because those are
-- folds of package_event.
-- Revoke only. At this point every column on package except its identity is a
-- fold of package_event, so there is nothing here the application should write.
-- Migration 9 grants back the despatch columns it adds, which are the
-- application's.
REVOKE INSERT, UPDATE, DELETE ON package FROM nylonite_app;
