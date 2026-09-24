-- Reverse of 2026-08-04-000015_fulfilment_progress.

-- Migration 3's body, restored. Same cost as migration 14's down: CREATE OR
-- REPLACE means the reverse has to carry the text it is reverting to.
CREATE OR REPLACE FUNCTION projection_stock_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
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
        DO UPDATE SET quantity = EXCLUDED.quantity,
                      weight_g = EXCLUDED.weight_g,
                      resolved_location_id = EXCLUDED.resolved_location_id,
                      site_id = EXCLUDED.site_id
        RETURNING s.id)
    SELECT count(*) INTO touched FROM upserted;

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

ALTER FUNCTION projection_stock_rebuild(uuid) OWNER TO spork_projection_owner;

REVOKE SELECT ON stock_allocation FROM spork_projection_owner;
REVOKE SELECT, UPDATE ON fulfilment_line FROM spork_projection_owner;

DROP FUNCTION IF EXISTS projection_fulfilment_rebuild(uuid);

DELETE FROM projection_rebuild
 WHERE table_name = 'fulfilment_line' AND function_name = 'projection_fulfilment_rebuild';

DROP INDEX IF EXISTS fulfilment_line_uncovered_idx;
ALTER TABLE fulfilment_line DROP COLUMN IF EXISTS uncovered_quantity;

ALTER TABLE fulfilment_line
    DROP COLUMN IF EXISTS despatched_quantity,
    DROP COLUMN IF EXISTS packed_quantity,
    DROP COLUMN IF EXISTS picked_quantity;

ALTER TABLE fulfilment_line RENAME COLUMN covered_quantity TO allocated_quantity;

ALTER TABLE fulfilment_line
    ADD COLUMN uncovered_quantity bigint
        GENERATED ALWAYS AS (quantity - allocated_quantity) STORED;

CREATE INDEX fulfilment_line_uncovered_idx
    ON fulfilment_line (fulfilment_id) WHERE uncovered_quantity > 0;

-- Migration 12's text, which is what this migration replaced.
COMMENT ON COLUMN fulfilment_line.allocated_quantity IS
    '@projection(pending) of stock_allocation; same missing maintainer. Question 134.';

ALTER TABLE fulfilment ADD COLUMN progress text;
COMMENT ON COLUMN fulfilment.progress IS
    '@projection(pending) of fulfilment_line; projection_fulfilment_rebuild was registered by migration 9 and never written. Question 134.';
