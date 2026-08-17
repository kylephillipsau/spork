-- Reverse of 2026-08-04-000042_receipt_decision.
--
-- The reference goes, and with it the only record of which tolerances a line was
-- accepted against. No rebuild reconstructs it: that is the whole reason it is
-- stored.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM goods_receipt_line WHERE receiving_policy_id IS NOT NULL;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D87 destroys the governing policy on % receipt line(s): '
                     'why they were accepted becomes unanswerable again', n;
    END IF;
END $$;

DROP INDEX IF EXISTS goods_receipt_line_receiving_policy_idx;
ALTER TABLE goods_receipt_line DROP COLUMN IF EXISTS receiving_policy_id;
