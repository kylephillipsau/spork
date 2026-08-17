-- Migration 74 down: a pick list goes back to being ordered by whatever the bin
-- code happens to sort as.

DROP INDEX IF EXISTS location_pick_sequence_idx;
ALTER TABLE location DROP CONSTRAINT IF EXISTS location_pick_sequence_ck;
ALTER TABLE location DROP COLUMN IF EXISTS pick_sequence;
