-- Migration 138: units move between cartons. D246.
--
-- A repack is one movement from one package to another, naming its line: the
-- shape D99 already counts as not picked again. What it changes is what a
-- package holds. Every reader summed the movements *into* a package, so units
-- moved on to a second carton read as in both, and packed counted them twice
-- once the first was sealed again.
--
-- So what a package holds of a line is said once, here: what went into it,
-- less what has since gone from it into another holder. Units that left the
-- building from it (no `to` side) are still what it carried, as despatched
-- cartons are listed by what they held. Never below nought: a pick from a
-- pallet in storage takes from a package that never held that line, and must
-- not make it hold less than nothing.
--
-- Packed is then what a line has in sealed or despatched packages (D100,
-- amended by D246).

CREATE VIEW package_line_content
    WITH (security_invoker = true) AS
SELECT x.tenant_id, x.package_id, x.fulfilment_line_id, x.item_id, x.lot_id,
       greatest(sum(x.quantity), 0)::bigint AS quantity
  FROM (
        SELECT m.tenant_id, m.to_package_id AS package_id, m.fulfilment_line_id,
               m.item_id, m.to_lot_id AS lot_id, v.effective_quantity AS quantity
          FROM stock_movement m
          JOIN stock_movement_effective v
            ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
         WHERE m.to_package_id IS NOT NULL
           AND m.fulfilment_line_id IS NOT NULL
        UNION ALL
        SELECT m.tenant_id, m.from_package_id, m.fulfilment_line_id,
               m.item_id, m.from_lot_id, -v.effective_quantity
          FROM stock_movement m
          JOIN stock_movement_effective v
            ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
         WHERE m.from_package_id IS NOT NULL
           AND m.fulfilment_line_id IS NOT NULL
           AND (m.to_package_id IS NOT NULL OR m.to_location_id IS NOT NULL)
       ) x
 GROUP BY x.tenant_id, x.package_id, x.fulfilment_line_id, x.item_id, x.lot_id;

COMMENT ON VIEW package_line_content IS
    'D246. What a package holds of each line, from the ledger: what went into it, '
    'less what has since gone from it into another holder, never below nought. '
    'Units despatched from it are still what it carried. The one definition behind '
    'packed (D100), the bench, the packing queue, the packing list, the contents '
    'read, despatch and the check before a seal.';

GRANT SELECT ON package_line_content TO spork_projection_owner;
GRANT SELECT ON package_line_content TO spork_app;

-- A package's status now, from its events rather than the lagging fold: the
-- newest of sealed, opened, despatched and voided, the fold's own rule. The
-- live reads took the newest event of any kind, placement included, so a
-- sealed carton put on a pallet read as not sealed (D246).
CREATE FUNCTION package_status_now(p_package uuid)
RETURNS text
LANGUAGE sql STABLE AS $$
    SELECT kind FROM package_event
     WHERE package_id = p_package
       AND kind IN ('sealed', 'opened', 'despatched', 'voided')
     ORDER BY occurred_at DESC, recorded_at DESC, id DESC
     LIMIT 1
$$;

COMMENT ON FUNCTION package_status_now(uuid) IS
    'A package''s status from its events, as the fold will have it: the newest of '
    'sealed, opened, despatched and voided; NULL for one never sealed. D246.';

CREATE OR REPLACE FUNCTION public.projection_fulfilment_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    -- last changed: migration 138 (D246)
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
                   WHERE m.to_location_id IS NULL
                     AND m.to_package_id IS NULL)                        AS despatched
          FROM stock_movement m
          JOIN stock_movement_effective v
            ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
          LEFT JOIN location fl ON fl.id = m.from_location_id
         WHERE m.tenant_id = p_tenant
           AND m.fulfilment_line_id IS NOT NULL
         GROUP BY m.fulfilment_line_id
    ),
    -- D100, amended by D246: what a line has in packages whose winning status
    -- is sealed or despatched, as `package_line_content` holds it.
    packing AS (
        SELECT c.fulfilment_line_id, sum(c.quantity)::bigint AS packed
          FROM package_line_content c
          JOIN package p ON p.id = c.package_id
         WHERE c.tenant_id = p_tenant
           AND p.status IN ('sealed', 'despatched')
         GROUP BY c.fulfilment_line_id
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
               packed_quantity          = coalesce(k.packed, 0),
               despatched_quantity      = coalesce(g.despatched, 0),
               external_picked_quantity = coalesce(r.external_picked, 0)
          FROM fulfilment_line l
          LEFT JOIN intention i ON i.fulfilment_line_id = l.id
          LEFT JOIN ledger    g ON g.fulfilment_line_id = l.id
          LEFT JOIN packing   k ON k.fulfilment_line_id = l.id
          LEFT JOIN reported  r ON r.fulfilment_line_id = l.id
         WHERE fl.id = l.id AND fl.tenant_id = p_tenant
           AND (fl.covered_quantity, fl.picked_quantity,
                fl.packed_quantity, fl.despatched_quantity,
                fl.external_picked_quantity)
               IS DISTINCT FROM
               (coalesce(i.covered, 0), coalesce(g.picked, 0),
                coalesce(k.packed, 0), coalesce(g.despatched, 0),
                coalesce(r.external_picked, 0))
        RETURNING fl.id)
    SELECT count(*) INTO touched FROM updated;

    RETURN touched;
END
$function$;
