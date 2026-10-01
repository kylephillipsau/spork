-- Migration 100 down: photographs are no longer cut to their faces.
--
-- The cuts go and their files do not, as with migration 78: the bytes live on
-- a volume this migration has no reach into, and become unreferenced for the
-- reaper. The photographs they were cut from are untouched.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM observation_image_cut;
    IF n > 0 THEN
        RAISE NOTICE 'reversing migration 100 drops % cut(s); the photographs they were cut from remain, and the cut files become unreferenced', n;
    END IF;
END
$$;

DROP TABLE observation_image_cut;
ALTER TABLE observation_image DROP CONSTRAINT observation_image_id_tenant_key;
