-- Migration 71 down: a fulfilment goes back to being named by its order.

DROP INDEX IF EXISTS fulfilment_reference_idx;
ALTER TABLE fulfilment DROP COLUMN IF EXISTS reference;
