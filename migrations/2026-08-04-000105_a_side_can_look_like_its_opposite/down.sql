-- Migration 105 down: no side is said to look like another.
--
-- The rows that said so go; the photographs they pointed at stay.

DELETE FROM observation_image_cut WHERE observation_image_id IN
    (SELECT id FROM observation_image WHERE same_as_id IS NOT NULL);
DELETE FROM observation_image WHERE same_as_id IS NOT NULL;
ALTER TABLE observation_image
    DROP CONSTRAINT IF EXISTS observation_image_same_as_ck,
    DROP CONSTRAINT IF EXISTS observation_image_same_as_fk,
    DROP COLUMN IF EXISTS same_as_id;
