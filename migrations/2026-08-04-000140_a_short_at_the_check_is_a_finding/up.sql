-- Migration 140: a short at the check is a finding. D244, docs/packing-plan.md
-- (settled 3).
--
-- The packer's count is recorded and the plan packs what is there (D244); the
-- shortfall goes into the findings queue against the line, for whoever picked
-- it, here or in NetSuite. The finding names the check that found it in a
-- typed column, as a receipt line's does (migration 66): one more source arm.

ALTER TABLE pack_check
    ADD CONSTRAINT pack_check_tenant_key UNIQUE (id, tenant_id);

ALTER TABLE discrepancy
    ADD COLUMN pack_check_id uuid,
    ADD CONSTRAINT discrepancy_pack_check_fk
        FOREIGN KEY (pack_check_id, tenant_id) REFERENCES pack_check(id, tenant_id);

ALTER TABLE discrepancy
    DROP CONSTRAINT discrepancy_source_ck;
ALTER TABLE discrepancy
    ADD CONSTRAINT discrepancy_source_ck
        CHECK (num_nonnulls(
            stock_movement_id,
            package_event_id,
            stock_count_id,
            goods_receipt_line_id,
            pack_check_id
        ) <= 1);

CREATE INDEX discrepancy_pack_check_idx
    ON discrepancy (pack_check_id)
    WHERE pack_check_id IS NOT NULL;

COMMENT ON COLUMN discrepancy.pack_check_id IS
    'Source arm: the check at the pack bench that found the line short. Typed, '
    'as migration 66 typed the receipt line. D244, migration 140.';

GRANT INSERT (pack_check_id), SELECT (pack_check_id)
    ON discrepancy TO spork_app;
GRANT INSERT (pack_check_id), SELECT (pack_check_id)
    ON discrepancy TO spork_mediation_owner;
GRANT INSERT (pack_check_id), SELECT (pack_check_id)
    ON discrepancy TO spork_projection_owner;
GRANT INSERT (pack_check_id), SELECT (pack_check_id)
    ON discrepancy TO spork_scheduler;
