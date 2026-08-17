-- Migration 24: the order the maintainers run in, written down.
--
-- D64, settling question 139. A rebuild family is several functions.
-- `projection_package_rebuild` folds the log and `projection_package_stamp`
-- writes `placement_event_id` afterwards; `projection_stock_rebuild` folds the
-- ledger and `projection_stock_resolve_locations` resolves the container arm once
-- packages have been placed.
--
-- **Nothing said so.** `projection_rebuild` records which rebuild owns a column.
-- S5's third leg requires every `projection_%_rebuild` function to have a
-- registry row — and the two step functions do not match that pattern, so
-- nothing required them to be registered, and nothing anywhere said they had to
-- run at all, let alone in what order.
--
-- The order existed in exactly one place: fifteen hand-written calls scattered
-- through `fixtures/seed.sql`. **A scheduler written from the decision record
-- rather than from the fixture would have called eight functions and skipped
-- two**, leaving `package.placement_event_id` and `stock.resolved_location_id`
-- stale forever, and S43 would have passed throughout because the family does
-- write those columns.

-- ---------------------------------------------------------------------------
-- 1. The order becomes a row
-- ---------------------------------------------------------------------------

CREATE TABLE projection_step (
    function_name text PRIMARY KEY,
    -- Sparse on purpose. Inserting a step between two others should not mean
    -- renumbering the ones after it, because renumbering is how an ordering
    -- silently changes while looking like it was tidied.
    ordinal       integer NOT NULL,
    note          text NOT NULL,
    CONSTRAINT projection_step_ordinal_key UNIQUE (ordinal),
    CONSTRAINT projection_step_ordinal_ck CHECK (ordinal > 0)
);

COMMENT ON TABLE projection_step IS
    'REFERENCE, platform-owned. Every projection maintainer and the order it runs '
    'in. The order was previously implicit in the fixture, which is not a place a '
    'scheduler reads. D35, D64.';
COMMENT ON COLUMN projection_step.note IS
    'What this step needs to have happened first. A dependency stated in prose '
    'because the alternative is a graph, and ten functions in one order do not '
    'need one.';

GRANT SELECT ON projection_step TO nylonite_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON projection_step TO nylonite_platform;
GRANT SELECT ON projection_step TO nylonite_projection_owner;

INSERT INTO projection_step (function_name, ordinal, note) VALUES
    ('projection_item_class_closure_rebuild', 10,
     'Independent. The taxonomy closure reads only item_class.'),
    ('projection_party_class_closure_rebuild', 20,
     'Independent. The taxonomy closure reads only party_class.'),
    ('projection_stock_rebuild', 30,
     'Folds stock_movement into cells, and folds allocations onto them. Creates the rows everything else in the stock family updates.'),
    ('projection_package_rebuild', 40,
     'Folds package_event into placement, identity and depth.'),
    ('projection_package_stamp', 50,
     'Writes placement_event_id and placement_occurred_at. After the fold, because it stamps which event won it.'),
    ('projection_package_containment_rebuild', 60,
     'Folds the same log into intervals. Independent of the stamp, ordered after it so the package family runs contiguously.'),
    ('projection_stock_resolve_locations', 70,
     'Resolves the container arm of stock.resolved_location_id. Needs cells from 30 and placements from 40.'),
    ('projection_order_rebuild', 80,
     'Folds intention_amendment onto order and order_line. Independent of the stock family.'),
    ('projection_fulfilment_rebuild', 90,
     'Folds stock_allocation onto the commitment. Reads allocations rather than cells, so it does not depend on 30.'),
    ('projection_expected_supply_rebuild', 100,
     'Folds purchase orders, allocations and receipts into promises. Reads the ledger directly rather than through stock.');

-- ---------------------------------------------------------------------------
-- 2. One entry point, driven by the table
-- ---------------------------------------------------------------------------
--
-- The order is read rather than written into the function, for the reason every
-- other correction in this record reaches: a list in code and a list in a table
-- agree until they stop, and the one nobody re-reads is the one that rots.
--
-- Dynamic SQL over a name from a table that only the platform may write, with the
-- identifier quoted. The search_path is pinned for the same reason every other
-- SECURITY DEFINER maintainer pins it: a mutable one here is remote code
-- execution as the owner.

CREATE FUNCTION projection_run_all(p_tenant uuid)
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
        total := total + coalesce(n, 0);
    END LOOP;
    RETURN total;
END
$$;

COMMENT ON FUNCTION projection_run_all(uuid) IS
    'Runs every projection maintainer for one tenant, in the declared order. The '
    'scheduler calls this and nothing else, so skipping a step stops being '
    'something a caller can do by omission. D64.';

ALTER FUNCTION projection_run_all(uuid) OWNER TO nylonite_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_run_all(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_run_all(uuid) TO nylonite_scheduler, nylonite_platform;

-- ---------------------------------------------------------------------------
-- 3. What S49 adds that S5 could not
-- ---------------------------------------------------------------------------
--
-- S5's third leg reads `projection_%_rebuild` and is right to: a *rebuild* with
-- no registry row is a projection nobody declared. It cannot see a *step*,
-- because a step is not a rebuild and does not own a column — its whole job is to
-- finish one another function started.
--
-- S49 is the same bidirectional diff over the wider set. Every function named
-- `projection_%` is a step exactly once, and every step names a live function.
-- `projection_run_all` is excluded by name and it is the only exclusion: an
-- orchestrator that ran itself would not terminate.
