-- Migration 95 down: goods picked elsewhere no longer reach the ledger.
--
-- Handover movements are facts and are not deleted by a down migration; one
-- that has any refuses at the CHECK below, which is the right outcome.

ALTER TABLE site DROP CONSTRAINT IF EXISTS site_owner_party_fk;
ALTER TABLE site DROP COLUMN IF EXISTS owner_party_id;

DROP INDEX IF EXISTS stock_movement_external_pick_idx;
ALTER TABLE stock_movement DROP CONSTRAINT IF EXISTS stock_movement_handover_shape_ck;
ALTER TABLE stock_movement DROP CONSTRAINT IF EXISTS stock_movement_external_pick_fk;
ALTER TABLE stock_movement DROP COLUMN IF EXISTS external_pick_id;

ALTER TABLE external_pick DROP CONSTRAINT IF EXISTS external_pick_tenant_key;

ALTER TABLE stock_movement DROP CONSTRAINT stock_movement_reason_ck;
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
