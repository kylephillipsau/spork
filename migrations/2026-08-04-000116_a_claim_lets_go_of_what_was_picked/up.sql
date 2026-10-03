-- Migration 116: a claim lets go of what was picked. D203, amending J3.
--
-- A claim on a stock cell said "this much of what is here is spoken for". The
-- states it was meant to move through (allocated, picking, picked, packed) were
-- never written: nothing advances an allocation, and the progress a person reads
-- is folded from the movements instead (D99, D100). So after a pick the claim
-- went on holding its full quantity against the cell the goods had left. In the
-- test database a bin held 37 and showed 50 claimed: 13 fewer than nothing
-- available, found while planning picking in Spork (docs/picking-plan.md).
--
-- **A claim's hold on a cell is reduced by what has been picked from it.** For
-- each line with active claims on a cell, the hold is what it claimed less what
-- picks for that line have taken out of that cell, net of corrections, and never
-- below nothing. A pick with no claim behind it lets go of nothing. The goods
-- picked go on being the line's in the carton or at the station, where nothing
-- reads availability: the bench offers cells held at locations, and the walk
-- offers storage.
--
-- One view states it; the stock fold writes it and J3 checks it against the
-- same view, so the two cannot disagree.

CREATE VIEW stock_claim_hold WITH (security_invoker = true) AS
WITH claims AS (
    SELECT stock_id, fulfilment_line_id, sum(quantity) AS claimed
      FROM stock_allocation
     WHERE state IN ('allocated', 'picking', 'picked', 'packed')
       AND stock_id IS NOT NULL
     GROUP BY stock_id, fulfilment_line_id
),
picked AS (
    SELECT m.tenant_id, m.fulfilment_line_id, m.item_id,
           m.from_location_id, m.from_package_id, m.from_lot_id,
           m.from_status_id, m.from_owner_id,
           sum(v.effective_quantity) AS taken
      FROM stock_movement m
      JOIN stock_movement_effective v ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
     WHERE m.reason = 'pick' AND m.fulfilment_line_id IS NOT NULL
     GROUP BY 1, 2, 3, 4, 5, 6, 7, 8
)
SELECT c.stock_id, k.tenant_id,
       sum(greatest(c.claimed - coalesce(p.taken, 0), 0))::bigint AS held
  FROM claims c
  JOIN stock k ON k.id = c.stock_id
  LEFT JOIN picked p
    ON p.tenant_id = k.tenant_id
   AND p.fulfilment_line_id = c.fulfilment_line_id
   AND p.item_id = k.item_id
   AND p.from_location_id IS NOT DISTINCT FROM k.holder_location_id
   AND p.from_package_id IS NOT DISTINCT FROM k.holder_package_id
   AND p.from_lot_id IS NOT DISTINCT FROM k.lot_id
   AND p.from_status_id = k.status_id
   AND p.from_owner_id = k.owner_id
 GROUP BY c.stock_id, k.tenant_id;

COMMENT ON VIEW stock_claim_hold IS
    'What a stock cell''s active claims still hold of it: each line''s claims less '
    'what its picks have taken out of the cell, net of corrections, never below '
    'nothing. J3''s fold, as amended by D203, migration 116.';

-- The stock fold runs as its owner and reads the view as that role.
GRANT SELECT ON stock_claim_hold TO spork_projection_owner;
GRANT SELECT ON stock_claim_hold TO spork_app;

CREATE OR REPLACE FUNCTION public.projection_stock_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    -- last changed: migration 116 (D203)
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    WITH ledger AS (
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
               f.holder_location_id,
               l.site_id
          FROM folded f
          LEFT JOIN location l ON l.id = f.holder_location_id
        ON CONFLICT (tenant_id, item_id, holder_location_id, holder_package_id,
                     lot_id, status_id, owner_id)
        -- D101. The container arm belongs to step 70 and this step must not
        -- overwrite it. `holder_location_id` is NULL for a package-held cell, so
        -- the unguarded form wrote NULL over a resolved value every run and 70 put
        -- it back, which is a fold that never settles rather than a wrong number.
        DO UPDATE SET quantity = EXCLUDED.quantity,
                      weight_g = EXCLUDED.weight_g,
                      resolved_location_id = CASE
                          WHEN s.holder_package_id IS NOT NULL
                          THEN s.resolved_location_id
                          ELSE EXCLUDED.resolved_location_id END,
                      site_id = CASE
                          WHEN s.holder_package_id IS NOT NULL
                          THEN s.site_id
                          ELSE EXCLUDED.site_id END
        -- D68. Without this the upsert rewrites every cell every run, whether or
        -- not the fold moved it. Idempotent in value is not the same as writing
        -- nothing, and under MVCC the difference is a dead tuple per row per run.
        -- The guard mirrors the SET clause exactly, which is what D68 asks of every
        -- one of these and what makes the two impossible to drift apart.
        WHERE (s.quantity, s.weight_g)
              IS DISTINCT FROM (EXCLUDED.quantity, EXCLUDED.weight_g)
           OR (s.holder_package_id IS NULL
               AND (s.resolved_location_id, s.site_id)
                   IS DISTINCT FROM (EXCLUDED.resolved_location_id, EXCLUDED.site_id))
        RETURNING s.id)
    SELECT count(*) INTO touched FROM upserted;

    UPDATE stock s
       SET quantity = 0, weight_g = NULL
     WHERE s.tenant_id = p_tenant
       AND s.quantity <> 0
       AND NOT EXISTS (
           SELECT 1 FROM stock_movement m
            WHERE m.tenant_id = p_tenant AND m.item_id = s.item_id);

    -- J3 as amended by D203: what the cell's active claims still hold of it,
    -- read from `stock_claim_hold` so the fold and its check are one definition.
    UPDATE stock s
       SET allocated_quantity = coalesce(h.held, 0)
      FROM stock c
      LEFT JOIN stock_claim_hold h ON h.stock_id = c.id
     WHERE s.id = c.id AND s.tenant_id = p_tenant
       AND s.allocated_quantity IS DISTINCT FROM coalesce(h.held, 0);

    RETURN touched;
END
$function$;
