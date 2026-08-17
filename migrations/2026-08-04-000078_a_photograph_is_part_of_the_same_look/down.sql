-- Migration 78 down: the pictures stop being recordable.
--
-- The rows go and the files do not: `crate::images` writes content-addressed
-- blobs to a volume this migration has no reach into, and deleting them from
-- here would be a schema change removing data it does not own. What is left
-- behind is a directory of unreferenced files, which is exactly the shape D30's
-- reaper exists for and is safer than the alternative.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM observation_image;
    IF n > 0 THEN
        RAISE NOTICE 'reversing migration 78 drops % image record(s); the files they address remain on disk and become unreferenced', n;
    END IF;
END
$$;

DROP TABLE observation_image;
