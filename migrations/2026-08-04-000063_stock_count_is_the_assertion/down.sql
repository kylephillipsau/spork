-- Migration 63 down: stock_count and its discrepancy arm go.

ALTER TABLE discrepancy DROP CONSTRAINT IF EXISTS discrepancy_source_ck;
ALTER TABLE discrepancy DROP CONSTRAINT IF EXISTS discrepancy_stock_count_fk;
ALTER TABLE discrepancy DROP COLUMN IF EXISTS stock_count_id;

ALTER TABLE discrepancy
    ADD CONSTRAINT discrepancy_source_ck
        CHECK (num_nonnulls(stock_movement_id, package_event_id) <= 1);

DROP TABLE IF EXISTS stock_count;
