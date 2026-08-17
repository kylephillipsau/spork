-- Migration 84 down: `detail` stops being a thing a photograph can be of.
--
-- Any picture already filed that way has to go, because the CHECK coming back
-- would refuse it — and there is no side of the object it could honestly be
-- refiled as, which is the whole reason the word was added.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM observation_image WHERE face = 'detail';
    IF n > 0 THEN
        RAISE NOTICE 'reversing migration 84 destroys % evidence photograph(s): there is no side of the object they could be refiled as, which is why the word existed', n;
    END IF;
END
$$;

DELETE FROM observation_image WHERE face = 'detail';

ALTER TABLE observation_image DROP CONSTRAINT observation_image_face_ck;
ALTER TABLE observation_image ADD CONSTRAINT observation_image_face_ck
    CHECK (face IN ('front', 'back', 'left', 'right', 'top', 'bottom', 'label'));
