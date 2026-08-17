-- Migration 51: an event that asserts a placement says where.
--
-- D97, found by the outbound walk on its first run.
--
-- **Two constraints disagreed, and the maintainer is where they met.**
--
--   package_event      num_nonnulls(parent_package_id, location_id) <= 1
--   package_containment num_nonnulls(parent_package_id, location_id)  = 1
--
-- `asserts_placement` is generated as `kind IN ('created','placed','contained')`,
-- and of those three only `placed` and `contained` have a CHECK requiring a
-- holder. **`created` requires neither**, so a shipping carton brought into
-- existence at a packing bench -- the ordinary first act of packing an order --
-- is a legal event that the containment fold cannot represent.
--
-- The consequence is worse than a bad row. `projection_package_containment_rebuild`
-- **raises**, so `projection_run_all` aborts at ordinal 60 and rolls back, and
-- every projection after it never runs: `stock.resolved_location_id`, the order
-- fold, the fulfilment fold, the inbound shipment fold, expected supply. One
-- carton created without a holder stops every projection for that tenant until
-- somebody deletes the row.
--
-- That is the exact inverse of D8, which says a disagreement becomes a finding and
-- never stops the floor. Here a disagreement stops the floor and produces no
-- finding at all.

-- ---------------------------------------------------------------------------
-- 1. Say it once, where it can be derived rather than remembered
-- ---------------------------------------------------------------------------
--
-- Not a third kind-specific CHECK. The property is not about `created`: it is that
-- **anything asserting a placement has to say what the thing is placed in**, and
-- `asserts_placement` is already the column that decides which kinds do. Deriving
-- the CHECK from it means the next placement-asserting kind is covered on the day
-- it is added, by whoever adds it, without noticing.
--
-- `package_event_placed_ck` and `package_event_contained_ck` stay: they pin *which*
-- holder each kind names, which is a different sentence from *whether* it names
-- one.

DO $$
DECLARE
    offenders bigint;
BEGIN
    SELECT count(*) INTO offenders FROM package_event
     WHERE asserts_placement
       AND num_nonnulls(parent_package_id, location_id) <> 1;
    IF offenders > 0 THEN
        RAISE NOTICE 'D97: % placement event(s) name no holder. The containment fold '
                     'has been unable to run for their tenants; each needs a location '
                     'or a parent before this migration can add its constraint.',
                     offenders;
    END IF;
END $$;

ALTER TABLE package_event
    ADD CONSTRAINT package_event_placement_needs_holder_ck
        CHECK (NOT asserts_placement
               OR num_nonnulls(parent_package_id, location_id) = 1);

COMMENT ON CONSTRAINT package_event_placement_needs_holder_ck ON package_event IS
    'Every event that asserts a placement names exactly one holder, which is what '
    'package_containment requires of the interval it folds into. Derived from '
    'asserts_placement rather than listed per kind, so a kind added later is '
    'covered by the column that already decides it asserts placement. D97.';
