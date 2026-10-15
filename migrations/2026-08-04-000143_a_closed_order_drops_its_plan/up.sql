-- Migration 143: an order's plan is dropped when its fulfilment closes.
-- docs/packing-plan.md (settled 4), left from M3.
--
-- The board's plan (migration 142) is a draft per fulfilment, used up as its
-- parcels are made. A fulfilment that closed with parcels still planned kept
-- them, unseen, and one opened again (D187) would have found them there.
--
-- Now the scheduler drops them after each fold. A fulfilment is closed when it
-- is cancelled, closed in the other system, or has nothing left to despatch,
-- as the fold counts it; that is the fold's to know, so the drop follows it
-- rather than each act that might close one. Not a trigger (S7).

GRANT SELECT, DELETE ON pack_plan_parcel, pack_plan_line TO spork_projection_owner;
-- Whether a fulfilment is closed, and no more of it.
GRANT SELECT (id, state, closed_elsewhere) ON fulfilment TO spork_projection_owner;

CREATE FUNCTION pack_plan_drop_closed()
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
    FOR r IN
        SELECT id AS tenant_id FROM tenant ORDER BY id
    LOOP
        -- FORCE row-level security holds the definer too (S53).
        PERFORM set_config('spork.tenant_id', r.tenant_id::text, true);
        -- A pallet and the cartons planned onto it go together, and their
        -- lines with them (ON DELETE CASCADE, migration 142).
        DELETE FROM pack_plan_parcel p
         USING fulfilment f
         WHERE f.id = p.fulfilment_id
           AND p.tenant_id = r.tenant_id
           AND (f.state = 'cancelled'
                OR f.closed_elsewhere IS NOT NULL
                OR NOT EXISTS (SELECT 1 FROM fulfilment_line l
                                WHERE l.fulfilment_id = f.id
                                  AND l.despatched_quantity < l.quantity));
        GET DIAGNOSTICS n = ROW_COUNT;
        total := total + n;
    END LOOP;
    RETURN total;
END
$$;
ALTER FUNCTION pack_plan_drop_closed() OWNER TO spork_projection_owner;

COMMENT ON FUNCTION pack_plan_drop_closed() IS
    'Settled 4 of docs/packing-plan.md. For each tenant, drop the board''s plan '
    'of every fulfilment that is cancelled, closed in the other system, or '
    'despatched in full. The scheduler runs it after each fold. Migration 143.';

REVOKE ALL ON FUNCTION pack_plan_drop_closed() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION pack_plan_drop_closed() TO spork_scheduler;
