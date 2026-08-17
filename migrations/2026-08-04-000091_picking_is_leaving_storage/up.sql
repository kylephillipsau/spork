-- Migration 91: picking is leaving storage, not leaving a location.
--
-- D166. `picked` has folded as `from_location_id IS NOT NULL` since migration 54
-- — *anything that left a location* — and that was exact while a pick was one
-- movement: out of a bin, into the carton the goods ship in. The pack bench
-- still does exactly that and is untouched by this.
--
-- **It stops being exact the moment a pick has two legs**, which is what the
-- floor turns out to do. A picker with a trolley takes goods off the shelf and
-- puts them down at the packing station; the packer then puts them in a box.
-- Both movements leave a location, so both counted, and one order's units were
-- picked twice — which J56 raises as a finding (`picked > covered`) about a
-- warehouse that did nothing wrong.
--
-- **Picking is taking goods out of storage.** `pick_face`, `bulk` and
-- `overflow` are storage; `staging` and `dock` are where things are put down on
-- the way somewhere. So the second leg moves goods that were already picked and
-- does not pick them again, and the first leg counts exactly once whether the
-- goods land in a carton, on a pallet or on the floor by the bench.
--
-- `location.kind` has carried these five values since migration 1 and no fold
-- has ever read it. This is the reading it was for.
--
-- **`packed` and `despatched` are untouched.** Packed is still *the destination
-- package is sealed*, which a staging leg cannot satisfy and a bench leg can;
-- despatched is still *went nowhere*, which is what leaving the building looks
-- like in a ledger of holders.
CREATE OR REPLACE FUNCTION public.projection_fulfilment_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    -- last changed: migration 91 (D166)
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH ledger AS (
        SELECT m.fulfilment_line_id,
               -- D99's shape rules over D103's effective quantity. None reads
               -- `reason`, which is text with no CHECK.
               --
               -- D166: out of *storage*, not merely out of a location. A leg
               -- from `staging` to a carton moves goods that were picked
               -- already.
               sum(v.effective_quantity) FILTER (
                   WHERE fl.kind IN ('pick_face', 'bulk', 'overflow'))    AS picked,
               sum(v.effective_quantity) FILTER (
                   WHERE p.status IN ('sealed', 'despatched'))           AS packed,
               sum(v.effective_quantity) FILTER (
                   WHERE m.to_location_id IS NULL
                     AND m.to_package_id IS NULL)                        AS despatched
          FROM stock_movement m
          JOIN stock_movement_effective v
            ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
          LEFT JOIN package p ON p.id = m.to_package_id
          LEFT JOIN location fl ON fl.id = m.from_location_id
         WHERE m.tenant_id = p_tenant
           AND m.fulfilment_line_id IS NOT NULL
         GROUP BY m.fulfilment_line_id
    ),
    intention AS (
        SELECT fulfilment_line_id, sum(quantity)::bigint AS covered
          FROM stock_allocation
         WHERE tenant_id = p_tenant
           AND state IN ('allocated','picking','picked','packed','fulfilled')
           AND fulfilment_line_id IS NOT NULL
         GROUP BY fulfilment_line_id
    ),
    updated AS (
        UPDATE fulfilment_line fl
           SET covered_quantity    = coalesce(i.covered, 0),
               picked_quantity     = coalesce(g.picked, 0),
               packed_quantity     = coalesce(g.packed, 0),
               despatched_quantity = coalesce(g.despatched, 0)
          FROM fulfilment_line l
          LEFT JOIN intention i ON i.fulfilment_line_id = l.id
          LEFT JOIN ledger    g ON g.fulfilment_line_id = l.id
         WHERE fl.id = l.id AND fl.tenant_id = p_tenant
           AND (fl.covered_quantity, fl.picked_quantity,
                fl.packed_quantity, fl.despatched_quantity)
               IS DISTINCT FROM
               (coalesce(i.covered, 0), coalesce(g.picked, 0),
                coalesce(g.packed, 0), coalesce(g.despatched, 0))
        RETURNING fl.id)
    SELECT count(*) INTO touched FROM updated;

    RETURN touched;
END
$function$;
