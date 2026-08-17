-- Migration 67 down: presets go back to claiming fixed dimensions without
-- saying what they are.

DO $$
DECLARE
    n bigint;
BEGIN
    SELECT count(*) INTO n FROM package_type
     WHERE num_nonnulls(length_mm, width_mm, height_mm) > 0;
    IF n > 0 THEN
        RAISE NOTICE 'reversing migration 67 destroys the nominal size of % preset(s): '
                     'dimensions_fixed goes back to being a flag with nothing behind it', n;
    END IF;
END $$;

-- The two shipped standards go with the columns that made them worth shipping.
-- A package built against one keeps its own dimensions, which are observations
-- and are not stored here — so what is lost is which preset it was built to,
-- not what it measured.
DO $$
DECLARE
    n bigint;
BEGIN
    SELECT count(*) INTO n FROM package
     WHERE package_type_id IN ('9a7e0000-0000-0000-0000-000000000001',
                               '9a7e0000-0000-0000-0000-000000000002');
    IF n > 0 THEN
        UPDATE package SET package_type_id = NULL
         WHERE package_type_id IN ('9a7e0000-0000-0000-0000-000000000001',
                                   '9a7e0000-0000-0000-0000-000000000002');
        RAISE NOTICE 'reversing migration 67 unlinks % package(s) from the shipped preset '
                     'they were built to: their own measurements survive, the standard '
                     'they were meant to match does not', n;
    END IF;
END $$;

DELETE FROM package_type
 WHERE id IN ('9a7e0000-0000-0000-0000-000000000001',
              '9a7e0000-0000-0000-0000-000000000002');

ALTER TABLE package_type
    DROP CONSTRAINT IF EXISTS package_type_fixed_states_dimensions_ck,
    DROP CONSTRAINT IF EXISTS package_type_dimensions_positive_ck,
    DROP COLUMN IF EXISTS length_mm,
    DROP COLUMN IF EXISTS width_mm,
    DROP COLUMN IF EXISTS height_mm;
