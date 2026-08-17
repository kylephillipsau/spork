-- Reverse of 2026-08-04-000020_expected_supply.
--
-- ON DELETE RESTRICT on stock_allocation.expected_supply_id means dropping the
-- column has to come before dropping the table it points at, which is J30's
-- argument running backwards: the reference is what makes the identity matter.

DELETE FROM projection_rebuild WHERE function_name = 'projection_expected_supply_rebuild';

DROP FUNCTION IF EXISTS projection_expected_supply_rebuild(uuid);

REVOKE SELECT ON purchase_order, purchase_order_line FROM nylonite_projection_owner;

DROP INDEX IF EXISTS stock_allocation_expected_supply_idx;
ALTER TABLE stock_allocation DROP CONSTRAINT IF EXISTS stock_allocation_supply_ck;
ALTER TABLE stock_allocation DROP COLUMN IF EXISTS expected_supply_id;

DROP TABLE IF EXISTS expected_supply;

DROP TYPE IF EXISTS supply_closed_reason;
DROP TYPE IF EXISTS date_confidence;
