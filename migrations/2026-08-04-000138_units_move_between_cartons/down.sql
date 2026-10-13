-- Migration 138 down: what a package holds is again what went into it.
--
-- Migration 94's fold, verbatim, then the view goes.
CREATE OR REPLACE FUNCTION public.projection_fulfilment_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    -- last changed: migration 94 (D172)
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

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
    -- D172: each external line's newest report, summed per line. A level, so
    -- the newest wins rather than adding up.
    reported AS (
        SELECT fulfilment_line_id, sum(quantity)::bigint AS external_picked
          FROM (SELECT DISTINCT ON (fulfilment_line_id, external_line)
                       fulfilment_line_id, quantity
                  FROM external_pick
                 WHERE tenant_id = p_tenant
                 ORDER BY fulfilment_line_id, external_line,
                          observed_at DESC, recorded_at DESC, id DESC) newest
         GROUP BY fulfilment_line_id
    ),
    updated AS (
        UPDATE fulfilment_line fl
           SET covered_quantity         = coalesce(i.covered, 0),
               picked_quantity          = coalesce(g.picked, 0),
               packed_quantity          = coalesce(g.packed, 0),
               despatched_quantity      = coalesce(g.despatched, 0),
               external_picked_quantity = coalesce(r.external_picked, 0)
          FROM fulfilment_line l
          LEFT JOIN intention i ON i.fulfilment_line_id = l.id
          LEFT JOIN ledger    g ON g.fulfilment_line_id = l.id
          LEFT JOIN reported  r ON r.fulfilment_line_id = l.id
         WHERE fl.id = l.id AND fl.tenant_id = p_tenant
           AND (fl.covered_quantity, fl.picked_quantity,
                fl.packed_quantity, fl.despatched_quantity,
                fl.external_picked_quantity)
               IS DISTINCT FROM
               (coalesce(i.covered, 0), coalesce(g.picked, 0),
                coalesce(g.packed, 0), coalesce(g.despatched, 0),
                coalesce(r.external_picked, 0))
        RETURNING fl.id)
    SELECT count(*) INTO touched FROM updated;

    RETURN touched;
END
$function$;

DROP FUNCTION package_status_now(uuid);
DROP VIEW package_line_content;
