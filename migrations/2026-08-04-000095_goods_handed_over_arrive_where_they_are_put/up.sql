-- Migration 95: goods handed over arrive where they are put (D172).
--
-- Migration 94 records that another system says goods were picked. This is the
-- moment they reach this system's ledger: the packer takes them from the
-- picker and puts them down, on the staging spot or straight into a carton.
-- That is an arrival, not a pick: nothing here held them before, so the from
-- side is empty, exactly as a receipt's is, and the to side is wherever they
-- went.
--
-- # Why it is not counted as picked
--
-- picked_quantity folds movements out of storage (D166). An arrival leaves no
-- storage, so it is not counted there, and it should not be: the picking
-- happened elsewhere and external_picked_quantity already says so. packed and
-- despatched fold by shape and need nothing new: a handover into a carton that
-- is sealed is packed, and the despatch that takes it out is despatched.
--
-- # Why it names the report
--
-- A movement from nowhere is the shape of a receipt, and a receipt names what
-- it received against. This names the external_pick it is the goods of, so a
-- handover cannot be written for a line nobody reported picked, and so the
-- amount handed over can be held to the level reported (checked as a finding,
-- not refused: a count made at the bench outranks a report from a screen).
--
-- # Whose goods
--
-- An arrival needs an owner, and nothing in the schema says who owns goods a
-- site holds by default -- question 26 is open and D169 declined to invent one.
-- So a site says it: site.owner_party_id. A handover at a site that has not
-- said is refused with a message saying what to set, rather than guessing.

-- ---------------------------------------------------------------------------
-- 1. The vocabulary
-- ---------------------------------------------------------------------------

ALTER TABLE stock_movement DROP CONSTRAINT stock_movement_reason_ck;
ALTER TABLE stock_movement
    ADD CONSTRAINT stock_movement_reason_ck
        CHECK (reason IN (
            'receipt',
            'putaway',
            'pick',
            'despatch',
            'move',
            'adjustment',
            'handover'
        ));

COMMENT ON COLUMN stock_movement.reason IS
    'What kind of movement this was: receipt, putaway, pick, despatch, move, '
    'adjustment or handover. Fixed vocabulary (D105). Not a fold discriminator -- '
    'outbound progress reads shape, not this column (D99, D100, J68). Distinct '
    'from adjustment_reason_id, which says why an adjustment was recorded (D47). '
    'handover: goods picked elsewhere reaching this ledger (D172).';

-- ---------------------------------------------------------------------------
-- 2. A handover names the report it is the goods of
-- ---------------------------------------------------------------------------

ALTER TABLE external_pick ADD CONSTRAINT external_pick_tenant_key UNIQUE (id, tenant_id);

ALTER TABLE stock_movement
    ADD COLUMN external_pick_id uuid,
    ADD CONSTRAINT stock_movement_external_pick_fk
        FOREIGN KEY (external_pick_id, tenant_id) REFERENCES external_pick(id, tenant_id),
    -- A movement naming a report is an arrival for a line: no from side, a
    -- line named, and called what it is.
    ADD CONSTRAINT stock_movement_handover_shape_ck
        CHECK (external_pick_id IS NULL
               OR (fulfilment_line_id IS NOT NULL
                   AND from_location_id IS NULL AND from_package_id IS NULL
                   AND reason = 'handover'));

CREATE INDEX stock_movement_external_pick_idx
    ON stock_movement (external_pick_id) WHERE external_pick_id IS NOT NULL;

COMMENT ON COLUMN stock_movement.external_pick_id IS
    'The external_pick this movement is the goods of: a handover, the arrival of '
    'goods another system picked. Set only on an arrival that names a line. D172.';

GRANT INSERT (external_pick_id) ON stock_movement TO spork_app;

-- ---------------------------------------------------------------------------
-- 3. Whose goods a site holds
-- ---------------------------------------------------------------------------

ALTER TABLE site
    ADD COLUMN owner_party_id uuid,
    ADD CONSTRAINT site_owner_party_fk
        FOREIGN KEY (owner_party_id, tenant_id) REFERENCES party(id, tenant_id);

COMMENT ON COLUMN site.owner_party_id IS
    'Who owns goods this site holds when nothing else says: the owner a handover '
    'records (D172). Nullable, and a handover at a site without one is refused '
    'rather than defaulted, for D169''s reason. Question 26 is still open for '
    'the general case.';
