-- Reverse of 2026-08-04-000122_not_where_netsuite_lists_it.
--
-- `ALTER TYPE ... ADD VALUE` has no inverse in Postgres, so `discrepancy_kind`
-- keeps `not_in_listed_bin` and `found_in_unlisted_bin` after this runs: the
-- residue migrations 30, 40 and 52 documented, survivable because the up
-- migration adds them with IF NOT EXISTS. The findings already raised stay.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM discrepancy
     WHERE kind::text IN ('not_in_listed_bin', 'found_in_unlisted_bin');
    IF n > 0 THEN
        RAISE NOTICE 'reversing D215 leaves % finding(s) of a kind nothing raises any more', n;
    END IF;
END $$;
