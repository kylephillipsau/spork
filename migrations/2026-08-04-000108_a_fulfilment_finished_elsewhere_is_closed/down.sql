-- Migration 108 down: nothing records a fulfilment finished elsewhere.
--
-- The fulfilments closed that way are open again, as they were before.

ALTER TABLE fulfilment
    DROP CONSTRAINT IF EXISTS fulfilment_closed_elsewhere_pair_ck,
    DROP CONSTRAINT IF EXISTS fulfilment_closed_elsewhere_ck,
    DROP COLUMN IF EXISTS closed_elsewhere_at,
    DROP COLUMN IF EXISTS closed_elsewhere;
