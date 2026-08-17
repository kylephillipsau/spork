-- Migration 83 down: a record stops being able to point at what supports it.
--
-- The looks survive and so do the photographs — they hang off
-- `observation_event`, which this does not touch. What goes is the statement
-- that one of them was offered in support of a particular finding, receipt line
-- or count, and there is nowhere else that was ever recorded.
--
-- **`ALTER TYPE ... ADD VALUE` has no inverse**, as migration 30 found and
-- wrote down: an enum label cannot be dropped once added, because rows
-- elsewhere may already be typed by it and the catalogue keeps no reference
-- count. So `observation_method` keeps `photographed` after this runs. `up.sql`
-- adds it with `IF NOT EXISTS`, so re-applying after a down is clean.
--
-- Any `observation_event` already carrying that method would fail the type's
-- own constraint if the label could be removed, which is the second reason not
-- to try.

DO $$
DECLARE links bigint; looks bigint;
BEGIN
    SELECT count(*) INTO links FROM evidence;
    SELECT count(*) INTO looks FROM observation_event WHERE method = 'photographed';
    IF links > 0 OR looks > 0 THEN
        RAISE NOTICE 'reversing D140 discards % evidence link(s): % photographic look(s) survive with their pictures, and what each one was offered in support of does not', links, looks;
    END IF;
END
$$;

DROP TABLE evidence;
