-- Migration 105: a side can look like its opposite. D183.
--
-- Many cartons are printed the same front and back, and the same on both
-- ends. Photographing the back of one of those is a second photograph of the
-- front. So the back can be said to look like the front: a photograph row for
-- the back, in the same look, that points at the front's photograph and
-- carries its bytes. Its cut is the front's cut, so it is never cut on its
-- own and never waits in the crop queue (D181).

ALTER TABLE observation_image
    ADD COLUMN same_as_id uuid,
    ADD CONSTRAINT observation_image_same_as_fk
        FOREIGN KEY (same_as_id, tenant_id) REFERENCES observation_image(id, tenant_id),
    ADD CONSTRAINT observation_image_same_as_ck CHECK (same_as_id IS NULL OR same_as_id <> id);

COMMENT ON COLUMN observation_image.same_as_id IS
    'This side looks like that photograph''s side, said rather than taken: its '
    'bytes and its cut are that photograph''s. Migration 105, D183.';
