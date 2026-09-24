-- Reverse of 2026-08-04-000022_receipt_handover.

REVOKE INSERT (origin_expected_supply_id), UPDATE (origin_expected_supply_id)
    ON stock_allocation FROM spork_app;

DROP INDEX IF EXISTS stock_allocation_origin_idx;

ALTER TABLE stock_allocation
    DROP CONSTRAINT IF EXISTS stock_allocation_origin_implies_stock_ck,
    DROP CONSTRAINT IF EXISTS stock_allocation_origin_ck;

ALTER TABLE stock_allocation DROP COLUMN IF EXISTS origin_expected_supply_id;
