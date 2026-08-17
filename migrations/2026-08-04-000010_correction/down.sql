-- Reverse of 2026-08-04-000010_correction.

DROP INDEX IF EXISTS stock_movement_reverses_idx;

ALTER TABLE stock_movement
    DROP CONSTRAINT IF EXISTS stock_movement_reversal_not_self_ck,
    DROP CONSTRAINT IF EXISTS stock_movement_reversal_has_reason_ck;

ALTER TABLE stock_movement
    DROP COLUMN IF EXISTS adjustment_reason_id,
    DROP COLUMN IF EXISTS reverses_movement_id;

DROP TABLE IF EXISTS adjustment_reason;

DROP TYPE IF EXISTS adjustment_class;
