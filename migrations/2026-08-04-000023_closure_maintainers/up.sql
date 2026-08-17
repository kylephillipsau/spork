-- Migration 23: the closure tables get the maintainer their comment promised.
--
-- D63, settling question 140. `item_class_closure` has carried this table comment
-- since migration 1:
--
--   "PROJECTION of item_class. Maintained by a named function under D35, never
--    written by the application role."
--
-- The second half is enforced: the application holds SELECT and nothing else. The
-- first half names a function that does not exist. In a deployment these tables
-- can only be written by a superuser, and the fixture populates them because it
-- runs as one.
--
-- **D22's entire matching language is "is this node an ancestor-or-self of that
-- node" over these two tables.** A resolver in production would read an empty
-- closure and every scoped policy would quietly fail to match — not error, not
-- warn, just never apply. Found by a draft of S45 in D55 and marked
-- `@projection(pending)` rather than given an invented maintainer.

-- ---------------------------------------------------------------------------
-- 1. Total rebuild, and why that is safe here and forbidden next door
-- ---------------------------------------------------------------------------
--
-- J30 forbids truncate-and-regenerate on `expected_supply` because
-- `stock_allocation.expected_supply_id` is `ON DELETE RESTRICT`: live claims hold
-- those ids, so regenerating would fail outright or orphan a commitment.
--
-- **The closure is the opposite case and the difference is identity.** It has no
-- surrogate key — its primary key is `(ancestor_id, descendant_id)`, which is the
-- fact itself — and nothing in the schema references a closure row. A rebuilt row
-- is not a new row that replaced the old one; it is the same fact recomputed. So
-- a total rebuild per tenant is correct, and it is chosen over an incremental one
-- for the reason question 122 exists: an incremental rebuild optimises a cost
-- nobody has measured, and re-parenting is question 78's problem before it is a
-- performance one.
--
-- **Cycle safety is not optional.** `item_class_not_own_parent_ck` forbids a node
-- being its own parent and nothing forbids A → B → A, which is insertable today
-- and was verified so before this was written. A recursive walk over that does not
-- terminate. The `CYCLE` clause stops the walk and drops the cyclic branch, so a
-- corrupted taxonomy degrades to a partial closure rather than hanging the
-- maintainer; J59 is what says the corruption is there.

CREATE FUNCTION projection_item_class_closure_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH RECURSIVE tree AS (
        -- Every node is its own ancestor at depth zero. That row is what makes
        -- "ancestor-or-self" a single lookup rather than a lookup and a special
        -- case, which is the whole reason D22 calls the matching language
        -- cardinality one.
        SELECT c.tenant_id, c.id AS ancestor_id, c.id AS descendant_id, 0 AS depth
          FROM item_class c
         WHERE c.tenant_id = p_tenant
        UNION ALL
        SELECT t.tenant_id, t.ancestor_id, c.id, t.depth + 1
          FROM tree t
          JOIN item_class c
            ON c.parent_id = t.descendant_id AND c.tenant_id = t.tenant_id
    ) CYCLE descendant_id SET is_cycle USING cycle_path,
    computed AS (
        SELECT tenant_id, ancestor_id, descendant_id, min(depth) AS depth
          FROM tree
         WHERE NOT is_cycle
         GROUP BY tenant_id, ancestor_id, descendant_id
    ),
    removed AS (
        DELETE FROM item_class_closure x
         WHERE x.tenant_id = p_tenant
           AND NOT EXISTS (SELECT 1 FROM computed c
                            WHERE c.ancestor_id = x.ancestor_id
                              AND c.descendant_id = x.descendant_id)
        RETURNING 1
    ),
    written AS (
        INSERT INTO item_class_closure AS x (tenant_id, ancestor_id, descendant_id, depth)
        SELECT tenant_id, ancestor_id, descendant_id, depth FROM computed
        ON CONFLICT (ancestor_id, descendant_id)
        DO UPDATE SET depth = EXCLUDED.depth, tenant_id = EXCLUDED.tenant_id
        RETURNING 1)
    SELECT (SELECT count(*) FROM written) + (SELECT count(*) FROM removed) INTO touched;

    RETURN touched;
END
$$;

CREATE FUNCTION projection_party_class_closure_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH RECURSIVE tree AS (
        SELECT c.tenant_id, c.id AS ancestor_id, c.id AS descendant_id, 0 AS depth
          FROM party_class c
         WHERE c.tenant_id = p_tenant
        UNION ALL
        SELECT t.tenant_id, t.ancestor_id, c.id, t.depth + 1
          FROM tree t
          JOIN party_class c
            ON c.parent_id = t.descendant_id AND c.tenant_id = t.tenant_id
    ) CYCLE descendant_id SET is_cycle USING cycle_path,
    computed AS (
        SELECT tenant_id, ancestor_id, descendant_id, min(depth) AS depth
          FROM tree
         WHERE NOT is_cycle
         GROUP BY tenant_id, ancestor_id, descendant_id
    ),
    removed AS (
        DELETE FROM party_class_closure x
         WHERE x.tenant_id = p_tenant
           AND NOT EXISTS (SELECT 1 FROM computed c
                            WHERE c.ancestor_id = x.ancestor_id
                              AND c.descendant_id = x.descendant_id)
        RETURNING 1
    ),
    written AS (
        INSERT INTO party_class_closure AS x (tenant_id, ancestor_id, descendant_id, depth)
        SELECT tenant_id, ancestor_id, descendant_id, depth FROM computed
        ON CONFLICT (ancestor_id, descendant_id)
        DO UPDATE SET depth = EXCLUDED.depth, tenant_id = EXCLUDED.tenant_id
        RETURNING 1)
    SELECT (SELECT count(*) FROM written) + (SELECT count(*) FROM removed) INTO touched;

    RETURN touched;
END
$$;

ALTER FUNCTION projection_item_class_closure_rebuild(uuid)
    OWNER TO nylonite_projection_owner;
ALTER FUNCTION projection_party_class_closure_rebuild(uuid)
    OWNER TO nylonite_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_item_class_closure_rebuild(uuid) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION projection_party_class_closure_rebuild(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_item_class_closure_rebuild(uuid)
    TO nylonite_scheduler, nylonite_platform;
GRANT EXECUTE ON FUNCTION projection_party_class_closure_rebuild(uuid)
    TO nylonite_scheduler, nylonite_platform;

-- DELETE is granted here and nowhere near the application. S30 asserts the app
-- holds no DELETE on a projection; a total rebuild needs one, and the maintainer
-- role is the only thing that has it.
GRANT SELECT, INSERT, UPDATE, DELETE ON item_class_closure, party_class_closure
    TO nylonite_projection_owner;
GRANT SELECT ON item_class, party_class TO nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- 2. Mark and register, which is the half D49 exists to enforce
-- ---------------------------------------------------------------------------

COMMENT ON COLUMN item_class_closure.tenant_id IS
    '@projection of item_class via projection_item_class_closure_rebuild (D22, D63).';
COMMENT ON COLUMN item_class_closure.ancestor_id IS
    '@projection of item_class via projection_item_class_closure_rebuild (D22, D63).';
COMMENT ON COLUMN item_class_closure.descendant_id IS
    '@projection of item_class via projection_item_class_closure_rebuild (D22, D63).';
COMMENT ON COLUMN item_class_closure.depth IS
    '@projection of item_class via projection_item_class_closure_rebuild (D22, D63).';

COMMENT ON COLUMN party_class_closure.tenant_id IS
    '@projection of party_class via projection_party_class_closure_rebuild (D22, D63).';
COMMENT ON COLUMN party_class_closure.ancestor_id IS
    '@projection of party_class via projection_party_class_closure_rebuild (D22, D63).';
COMMENT ON COLUMN party_class_closure.descendant_id IS
    '@projection of party_class via projection_party_class_closure_rebuild (D22, D63).';
COMMENT ON COLUMN party_class_closure.depth IS
    '@projection of party_class via projection_party_class_closure_rebuild (D22, D63).';

COMMENT ON TABLE item_class_closure IS
    'PROJECTION of item_class. Maintained by projection_item_class_closure_rebuild '
    'under D35, never written by the application role. Both halves are now true. D63.';
COMMENT ON TABLE party_class_closure IS
    'PROJECTION of party_class. The closure is what makes the matching language '
    'cardinality one: ancestor-or-self is a lookup, not a recursion. D22, D63.';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('item_class_closure',  'tenant_id',     'projection_item_class_closure_rebuild'),
    ('item_class_closure',  'ancestor_id',   'projection_item_class_closure_rebuild'),
    ('item_class_closure',  'descendant_id', 'projection_item_class_closure_rebuild'),
    ('item_class_closure',  'depth',         'projection_item_class_closure_rebuild'),
    ('party_class_closure', 'tenant_id',     'projection_party_class_closure_rebuild'),
    ('party_class_closure', 'ancestor_id',   'projection_party_class_closure_rebuild'),
    ('party_class_closure', 'descendant_id', 'projection_party_class_closure_rebuild'),
    ('party_class_closure', 'depth',         'projection_party_class_closure_rebuild');
