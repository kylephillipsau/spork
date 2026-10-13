-- Migration 140 down: a short at the check is no longer a finding.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM discrepancy WHERE pack_check_id IS NOT NULL;
    IF n > 0 THEN
        RAISE NOTICE 'reversing migration 140 drops the check behind % finding(s)', n;
    END IF;
END $$;

ALTER TABLE discrepancy DROP CONSTRAINT discrepancy_source_ck;
DROP INDEX IF EXISTS discrepancy_pack_check_idx;
ALTER TABLE discrepancy DROP CONSTRAINT discrepancy_pack_check_fk;
ALTER TABLE discrepancy DROP COLUMN pack_check_id;
ALTER TABLE discrepancy
    ADD CONSTRAINT discrepancy_source_ck
        CHECK (num_nonnulls(
            stock_movement_id,
            package_event_id,
            stock_count_id,
            goods_receipt_line_id
        ) <= 1);

ALTER TABLE pack_check DROP CONSTRAINT pack_check_tenant_key;
