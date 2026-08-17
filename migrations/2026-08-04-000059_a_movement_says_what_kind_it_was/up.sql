-- Migration 59: a movement says what kind of movement it was.
--
-- D105, settling question 143. The only schema change is a CHECK on
-- `stock_movement.reason`, which has been `text NOT NULL` with no vocabulary at
-- all since migration 2 — worse than the `stock_allocation.state` instance 143 was
-- raised about, which at least listed seven values.
--
-- D99 and D100 deliberately fold progress by shape, never by reason, and that
-- stays. The trap this closes is the next author who reaches for `reason = 'pick'`
-- in a fold: the column is no longer an unconstrained free string.
--
-- The set is every value the fixtures and walks already write. `putaway` is in
-- the year generator; the rest are in seed and the suite. Adding a value is a
-- migration, the same cost as adding one to any other CHECK vocabulary.

ALTER TABLE stock_movement
    ADD CONSTRAINT stock_movement_reason_ck
        CHECK (reason IN (
            'receipt',
            'putaway',
            'pick',
            'despatch',
            'move',
            'adjustment'
        ));

COMMENT ON COLUMN stock_movement.reason IS
    'What kind of movement this was: receipt, putaway, pick, despatch, move or '
    'adjustment. Fixed vocabulary (D105). Not a fold discriminator — outbound '
    'progress reads shape, not this column (D99, D100, J68). Distinct from '
    'adjustment_reason_id, which says why an adjustment was recorded (D47).';
