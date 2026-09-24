-- Reverse of 2026-08-04-000053_a_pick_says_which_line_it_served.
--
-- A pick can again record that stock left a bin without recording which commitment
-- it served, so outbound progress has no key to group by and can only be folded from
-- the allocation state machine. Question 165's first half reopens.
--
-- The cause CHECK returns to its one-argument form, which is always true.

REVOKE INSERT (fulfilment_line_id) ON stock_movement FROM spork_app;

DROP INDEX IF EXISTS stock_movement_fulfilment_line_idx;

ALTER TABLE stock_movement
    DROP CONSTRAINT IF EXISTS stock_movement_cause_ck;

ALTER TABLE stock_movement
    DROP CONSTRAINT IF EXISTS stock_movement_fulfilment_line_fk;

ALTER TABLE stock_movement
    DROP COLUMN IF EXISTS fulfilment_line_id;

-- Migration 21's constraint, restored verbatim.
ALTER TABLE stock_movement
    ADD CONSTRAINT stock_movement_cause_ck
        CHECK (num_nonnulls(goods_receipt_line_id) <= 1);
