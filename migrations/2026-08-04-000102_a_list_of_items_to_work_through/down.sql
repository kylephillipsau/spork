-- Migration 102 down: no lists of items.
--
-- The lists go. The items on them, and everything recorded about those items
-- while the lists were worked, are untouched.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM item_list;
    IF n > 0 THEN
        RAISE NOTICE 'reversing migration 102 drops % list(s) of items; the items and their figures remain', n;
    END IF;
END
$$;

DROP TABLE item_list_entry;
DROP TABLE item_list;
