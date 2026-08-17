-- Reverse of 2026-08-04-000018_history_and_identifiers.
--
-- The columns drop, so the history they carried goes with them. Unlike migration
-- 14's down this destroys no rows: every column here is nullable and nothing
-- depends on one being set, so reversing loses information without making any
-- surviving row unrepresentable.

DROP INDEX IF EXISTS stock_movement_packing_config_idx;

ALTER TABLE stock_movement
    DROP CONSTRAINT IF EXISTS stock_movement_config_needs_level_ck,
    DROP CONSTRAINT IF EXISTS stock_movement_entered_needs_config_ck,
    DROP CONSTRAINT IF EXISTS stock_movement_entered_positive_ck,
    DROP CONSTRAINT IF EXISTS stock_movement_entered_pair_ck,
    DROP CONSTRAINT IF EXISTS stock_movement_packing_config_fk,
    DROP CONSTRAINT IF EXISTS stock_movement_cost_currency_ck,
    DROP CONSTRAINT IF EXISTS stock_movement_cost_sign_ck,
    DROP CONSTRAINT IF EXISTS stock_movement_cost_basis_ck,
    DROP CONSTRAINT IF EXISTS stock_movement_cost_triple_ck;

ALTER TABLE stock_movement
    DROP COLUMN IF EXISTS item_packing_config_id,
    DROP COLUMN IF EXISTS entered_packaging_level,
    DROP COLUMN IF EXISTS entered_quantity,
    DROP COLUMN IF EXISTS cost_currency,
    DROP COLUMN IF EXISTS cost_basis_quantity,
    DROP COLUMN IF EXISTS unit_cost_minor;

ALTER TABLE lot
    DROP CONSTRAINT IF EXISTS lot_production_before_expiry_ck,
    DROP CONSTRAINT IF EXISTS lot_country_of_origin_ck;

ALTER TABLE lot
    DROP COLUMN IF EXISTS production_date,
    DROP COLUMN IF EXISTS country_of_origin;

-- Identifiers back to random. Derived the same way the forward direction is, so
-- the pair stays symmetric as tables are added.
DO $$
DECLARE
    r record;
BEGIN
    FOR r IN
        SELECT c.relname AS tbl, a.attname AS col
          FROM pg_class c
          JOIN pg_namespace ns ON ns.oid = c.relnamespace
          JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
          JOIN pg_attrdef d ON d.adrelid = c.oid AND d.adnum = a.attnum
         WHERE ns.nspname = 'public'
           AND c.relkind = 'r'
           AND pg_get_expr(d.adbin, d.adrelid) = 'uuidv7()'
    LOOP
        EXECUTE format('ALTER TABLE %I ALTER COLUMN %I SET DEFAULT gen_random_uuid()', r.tbl, r.col);
    END LOOP;
END
$$;
