-- Migration 3: the fold.
--
-- Migration 2 gave stock a shape and nothing to fill it. This adds the
-- maintainer, so stock_movement actually projects into stock, and adds
-- stock_allocation, which is the one table permitted a durable reference to a
-- stock cell.
--
-- D35 decided the shape: the projection maintainer is a set of named functions,
-- not a role. Writes to stock happen inside SECURITY DEFINER functions owned by
-- a role nobody can become, and by nothing else.

-- ---------------------------------------------------------------------------
-- The owner nobody can become (D35)
-- ---------------------------------------------------------------------------

DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'nylonite_projection_owner') THEN
        -- NOLOGIN and no members, ever. J-class asserts zero rows in
        -- pg_auth_members for it: the point of a definer role is that its
        -- privileges are reachable only through the functions it owns.
        CREATE ROLE nylonite_projection_owner NOLOGIN;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'nylonite_scheduler') THEN
        CREATE ROLE nylonite_scheduler NOLOGIN;
    END IF;
END
$$;

-- ---------------------------------------------------------------------------
-- The registry S5 diffs against (D25)
-- ---------------------------------------------------------------------------

-- S5 asserts every @projection column has a registered rebuild function. Until
-- now there was nothing to register against, so the check was pending on this
-- table rather than on any schema object.
CREATE TABLE projection_rebuild (
    table_name    text NOT NULL,
    column_name   text NOT NULL,
    function_name text NOT NULL,
    PRIMARY KEY (table_name, column_name)
);

COMMENT ON TABLE projection_rebuild IS
    'REFERENCE, platform-owned. The declared side of S5''s bidirectional diff: '
    'every column commented @projection appears here, and every function named '
    'here exists. D25.';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('stock', 'quantity',             'projection_stock_rebuild'),
    ('stock', 'weight_g',             'projection_stock_rebuild'),
    ('stock', 'allocated_quantity',   'projection_stock_rebuild'),
    ('stock', 'resolved_location_id', 'projection_stock_rebuild'),
    ('stock', 'site_id',              'projection_stock_rebuild');

-- ---------------------------------------------------------------------------
-- Allocation: the one durable reference to a cell (D12, D24, S28)
-- ---------------------------------------------------------------------------

CREATE TABLE stock_allocation (
    id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id          uuid NOT NULL REFERENCES tenant(id),

    -- D24: exactly one FK in the schema targets stock(id) and this is it,
    -- declared ON DELETE RESTRICT because stock is reapable and an allocation
    -- holding a cell is what makes that cell un-reapable.
    stock_id           uuid REFERENCES stock(id) ON DELETE RESTRICT,

    -- The second supply arm is expected_supply_id, which arrives with the
    -- supply migration. Left out rather than stubbed: a nullable FK to a table
    -- that does not exist is not a placeholder, it is a lie about the shape.

    quantity           bigint NOT NULL,
    state              text NOT NULL DEFAULT 'allocated',
    firm               boolean NOT NULL DEFAULT false,
    bound_at           timestamptz NOT NULL DEFAULT now(),
    expires_at         timestamptz,

    CONSTRAINT stock_allocation_quantity_ck CHECK (quantity > 0),
    CONSTRAINT stock_allocation_state_ck CHECK (state IN
        ('allocated','picking','picked','packed','fulfilled','short','released'))
);

-- S28 requires a plain btree here rather than leaving the FK unindexed: the
-- reaper's existence predicate reads it per cell.
CREATE INDEX stock_allocation_stock_idx ON stock_allocation (stock_id);

COMMENT ON TABLE stock_allocation IS
    'INTENTION. Advisory, per D12: an allocation is a plan and the ledger does '
    'not wait on it.';

ALTER TABLE stock_allocation ENABLE ROW LEVEL SECURITY;
ALTER TABLE stock_allocation FORCE ROW LEVEL SECURITY;
CREATE POLICY stock_allocation_tenant_scoped ON stock_allocation
    USING (tenant_id = current_tenant());

GRANT SELECT, INSERT, UPDATE, DELETE ON stock_allocation TO nylonite_app;

-- ---------------------------------------------------------------------------
-- The maintainer (D25, D35)
-- ---------------------------------------------------------------------------

-- Per tenant, never global. D25 forces RLS on projection tables, which makes the
-- per-tenant form mandatory rather than merely tidy: a global rebuild would have
-- to bypass the policy it exists to respect.
--
-- Upsert rather than truncate-and-regenerate. J33: rebuilding stock preserves
-- row identity, and truncating is forbidden while any allocation holds a
-- stock_id. A cell whose movements now cancel out is set to zero rather than
-- deleted, because deleting it would break the allocation pointing at it.
CREATE FUNCTION projection_stock_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    -- FORCE ROW LEVEL SECURITY applies to the definer as well, which is the
    -- whole point of S8: the owner is not exempt from its own policy. So a
    -- SECURITY DEFINER maintainer with no tenant context reads zero rows and
    -- folds them into nothing, silently and successfully.
    --
    -- Setting the context here is what makes D25's "a rebuild is scoped by
    -- tenant" structural rather than a convention someone remembers. The scope
    -- is the argument, and a rebuild cannot reach past it even by accident.
    -- Local to the transaction, so it does not leak to the caller's session.
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH ledger AS (
        -- The signed two-sided fold. Every movement subtracts from one cell and
        -- adds to another, so one row contributes to two cells with opposite
        -- signs. J1 is the assertion that this equals stock.quantity.
        SELECT to_location_id AS holder_location_id, to_package_id AS holder_package_id,
               item_id, to_lot_id AS lot_id, to_status_id AS status_id,
               to_owner_id AS owner_id,
               quantity AS qty, catch_weight_g AS wt
          FROM stock_movement
         WHERE tenant_id = p_tenant
           AND num_nonnulls(to_location_id, to_package_id) = 1
        UNION ALL
        SELECT from_location_id, from_package_id,
               item_id, from_lot_id, from_status_id, from_owner_id,
               -quantity, -catch_weight_g
          FROM stock_movement
         WHERE tenant_id = p_tenant
           AND num_nonnulls(from_location_id, from_package_id) = 1
    ),
    folded AS (
        SELECT holder_location_id, holder_package_id, item_id, lot_id,
               status_id, owner_id,
               sum(qty) AS quantity,
               sum(wt)  AS weight_g
          FROM ledger
         GROUP BY 1, 2, 3, 4, 5, 6
    ),
    upserted AS (
        INSERT INTO stock AS s (
            tenant_id, item_id, holder_location_id, holder_package_id,
            lot_id, status_id, owner_id, quantity, weight_g,
            resolved_location_id, site_id)
        SELECT p_tenant, f.item_id, f.holder_location_id, f.holder_package_id,
               f.lot_id, f.status_id, f.owner_id, f.quantity, f.weight_g,
               -- J5: the holder location, or the holder package's. The package
               -- arm resolves through package_containment, which arrives with
               -- the containment migration, so it is null until then rather
               -- than wrong.
               f.holder_location_id,
               l.site_id
          FROM folded f
          LEFT JOIN location l ON l.id = f.holder_location_id
        ON CONFLICT (tenant_id, item_id, holder_location_id, holder_package_id,
                     lot_id, status_id, owner_id)
        DO UPDATE SET quantity = EXCLUDED.quantity,
                      weight_g = EXCLUDED.weight_g,
                      resolved_location_id = EXCLUDED.resolved_location_id,
                      site_id = EXCLUDED.site_id
        RETURNING s.id)
    SELECT count(*) INTO touched FROM upserted;

    -- A cell the ledger no longer reaches is zeroed, not removed. Removing it
    -- would break any allocation holding it, and D24 made stock reapable
    -- precisely so that emptiness is a state rather than an absence.
    UPDATE stock s
       SET quantity = 0, weight_g = NULL
     WHERE s.tenant_id = p_tenant
       AND s.quantity <> 0
       AND NOT EXISTS (
           SELECT 1 FROM stock_movement m
            WHERE m.tenant_id = p_tenant AND m.item_id = s.item_id);

    RETURN touched;
END
$$;

ALTER FUNCTION projection_stock_rebuild(uuid) OWNER TO nylonite_projection_owner;

-- D35: EXECUTE is granted to PUBLIC on every new function, which on a SECURITY
-- DEFINER function is the whole privilege. The REVOKE goes in the same migration
-- that creates it, never a later one.
REVOKE EXECUTE ON FUNCTION projection_stock_rebuild(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_stock_rebuild(uuid)
    TO nylonite_scheduler, nylonite_platform;

COMMENT ON FUNCTION projection_stock_rebuild(uuid) IS
    'Maintainer for stock. SECURITY DEFINER, owned by a role with no members, '
    'search_path pinned. A mutable search_path here would be remote code '
    'execution as the owner. D35.';

-- stock is written by the maintainer and read by everyone else. The application
-- role already holds only SELECT from migration 2; this makes the intent
-- explicit for the projection owner.
GRANT SELECT, INSERT, UPDATE ON stock TO nylonite_projection_owner;
GRANT SELECT ON stock_movement, location TO nylonite_projection_owner;
GRANT SELECT ON projection_rebuild TO nylonite_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON projection_rebuild TO nylonite_platform;
