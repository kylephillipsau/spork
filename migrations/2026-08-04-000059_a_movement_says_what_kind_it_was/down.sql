-- Migration 59 down: reason is free text again. Question 143 reopens on this
-- column; the rest of its classification in D105 is prose and needs no reverse.

ALTER TABLE stock_movement
    DROP CONSTRAINT IF EXISTS stock_movement_reason_ck;

COMMENT ON COLUMN stock_movement.reason IS NULL;
