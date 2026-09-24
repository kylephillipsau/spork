-- Migration 22: what a re-pointed allocation remembers.
--
-- D62. D24 states the receipt handover in one sentence and the schema has been
-- missing a third of it:
--
--   "At receipt, in the same transaction as the movements, allocations are
--    re-pointed at the new stock_id, bound_at is stamped, and
--    origin_expected_supply_id is retained so 'this unit was cross-docked against
--    Coles PO 88421' stays answerable."
--
-- `stock_id` and `bound_at` exist. `origin_expected_supply_id` does not, so the
-- moment an allocation moves from a promise to the stock that promise became,
-- the promise it came from is gone. Which purchase order a cross-docked unit was
-- committed against stops being answerable at exactly the point somebody starts
-- asking.
--
-- J58 caught the missing handover from the other side in migration 21: a fully
-- received promise still carrying claims. This is the column the handover needs
-- to leave behind when it finally runs.

ALTER TABLE stock_allocation
    ADD COLUMN origin_expected_supply_id uuid REFERENCES expected_supply(id);

COMMENT ON COLUMN stock_allocation.origin_expected_supply_id IS
    'The promise this allocation was made against before receipt re-pointed it at '
    'stock. Retained rather than moved: expected_supply_id says what is claimed '
    'now, this says what it was claimed against. D24, D62.';

-- Not ON DELETE RESTRICT, unlike `expected_supply_id`. That one restricts because
-- a live claim on a promise makes the promise un-removable; this is a historical
-- reference held by a claim that has already moved on, and letting it block a
-- deletion would make the memory of a handover more binding than the handover.
--
-- It also means the two columns are read differently and should not be confused:
-- `expected_supply_id` is a claim, `origin_expected_supply_id` is provenance.

ALTER TABLE stock_allocation
    -- A claim is against a promise or against stock, never both, and the origin
    -- is only meaningful once it is no longer the claim. Naming the same row in
    -- both columns would say the handover both happened and did not.
    ADD CONSTRAINT stock_allocation_origin_ck
        CHECK (origin_expected_supply_id IS NULL
               OR origin_expected_supply_id IS DISTINCT FROM expected_supply_id),
    -- Provenance without a handover is a claim that never moved. If the origin is
    -- set, the claim is on stock now.
    ADD CONSTRAINT stock_allocation_origin_implies_stock_ck
        CHECK (origin_expected_supply_id IS NULL OR stock_id IS NOT NULL);

CREATE INDEX stock_allocation_origin_idx
    ON stock_allocation (origin_expected_supply_id)
    WHERE origin_expected_supply_id IS NOT NULL;

GRANT INSERT (origin_expected_supply_id), UPDATE (origin_expected_supply_id)
    ON stock_allocation TO spork_app;
