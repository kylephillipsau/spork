-- Migration 65 down: the platform receiving default goes back to being fixture
-- data, and a deployment without the fixture can no longer receive.
--
-- Two tables may name the version being removed -- `goods_receipt_line` records
-- which policy governed a disposition (D87, J63) and `expected_supply` records
-- which shaped a promise (D24). Both are ON DELETE NO ACTION, so the delete would
-- fail rather than orphan, and the NOTICE says what reversing costs before it
-- clears them: J63's audit question, "why was this line accepted", stops being
-- answerable for every line this version decided.

DO $$
DECLARE
    lines    bigint;
    promises bigint;
BEGIN
    SELECT count(*) INTO lines FROM goods_receipt_line
     WHERE receiving_policy_id = '4ec00000-0000-0000-0000-000000000001';
    SELECT count(*) INTO promises FROM expected_supply
     WHERE receiving_policy_id = '4ec00000-0000-0000-0000-000000000001';

    IF lines > 0 OR promises > 0 THEN
        UPDATE goods_receipt_line SET receiving_policy_id = NULL
         WHERE receiving_policy_id = '4ec00000-0000-0000-0000-000000000001';
        UPDATE expected_supply SET receiving_policy_id = NULL
         WHERE receiving_policy_id = '4ec00000-0000-0000-0000-000000000001';
        RAISE NOTICE 'reversing migration 65 destroys the governing policy on % receipt line(s) '
                     'and % promise(s): why they were decided as they were becomes '
                     'unanswerable', lines, promises;
    END IF;
END $$;

DELETE FROM receiving_policy
 WHERE id = '4ec00000-0000-0000-0000-000000000001';

DELETE FROM policy_binding
 WHERE id = 'b0000000-0000-0000-0000-000000000005';
