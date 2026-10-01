-- Migration 100: a photograph is cut to the face it is of. D176.
--
-- A photograph of a carton's front is the front and everything around it: the
-- shelf, the next carton, the floor, the photographer's boots, and the front
-- itself leaning away because the phone was above it. Drawn on a box, that is
-- a box dressed in its surroundings. What the box wants is the face alone,
-- straight on, at its own proportions.
--
-- # The cut is a second picture, and the photograph stays
--
-- Somebody marks the face's four corners on the photograph and the client
-- straightens what is inside them into a rectangle. The result is a picture of
-- its own, stored like any other (a content address, D132), and the photograph
-- it was cut from is untouched: it is the record of what was in front of the
-- camera, and a cut marked badly is cut again from it.
--
-- # Who cut it is not who took it
--
-- A photograph hangs off the look that took it, which says who and when. A cut
-- can be made later, by somebody else, at a desk: the corners are that
-- person's judgement, not the photographer's. So a cut is an act of its own,
-- with its own `client_event` (D5, and so its own retry identity) and its own
-- person, and it points at the photograph rather than at the look.
--
-- # The corners
--
-- Eight numbers: the face's top-left, top-right, bottom-right and bottom-left
-- corners, x then y, each a fraction of the photograph's width or height as it
-- is shown the right way up. Fractions, so the corners mean the same thing at
-- any size the photograph is drawn. The order says which way up the face is:
-- the first corner is its top-left, whatever corner of the photograph it falls
-- in, so a top photographed sideways is turned by naming its corners in a
-- different order rather than by a second field.
--
-- # Facts only
--
-- Like a photograph, a cut is never edited. Cutting again is another row, and
-- the newest cut of a photograph is the one shown.

-- The tenant travels in the key (S51): a cut may only point at a photograph of
-- its own tenant, which needs the pair to be a key.
ALTER TABLE observation_image
    ADD CONSTRAINT observation_image_id_tenant_key UNIQUE (id, tenant_id);

CREATE TABLE observation_image_cut (
    id                   uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id            uuid NOT NULL REFERENCES tenant(id),

    -- The photograph it was cut from.
    observation_image_id uuid NOT NULL,

    -- The act of cutting: its retry identity, and who did it. The person is
    -- also on the client_event; it is here too, as `observation_event` carries
    -- it, so "who cut this" is a column and not a join.
    client_event_id      uuid NOT NULL,
    recorded_by_id       uuid NOT NULL REFERENCES person(id),

    -- Top-left, top-right, bottom-right, bottom-left of the face; x then y;
    -- fractions of the photograph shown the right way up.
    corners              double precision[] NOT NULL,

    -- The straightened face, behind a content address like any photograph.
    digest               text NOT NULL,
    mime                 text NOT NULL,
    byte_count           bigint NOT NULL,
    width_px             integer,
    height_px            integer,

    recorded_at          timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT observation_image_cut_corners_ck
        CHECK (array_length(corners, 1) = 8
               AND array_ndims(corners) = 1
               AND 0 <= ALL (corners) AND 1 >= ALL (corners)),
    CONSTRAINT observation_image_cut_digest_ck
        CHECK (digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT observation_image_cut_bytes_ck
        CHECK (byte_count > 0),
    CONSTRAINT observation_image_cut_pixels_ck
        CHECK ((width_px IS NULL) = (height_px IS NULL)
               AND (width_px IS NULL OR (width_px > 0 AND height_px > 0))),

    CONSTRAINT observation_image_cut_image_fk
        FOREIGN KEY (observation_image_id, tenant_id)
        REFERENCES observation_image(id, tenant_id),
    CONSTRAINT observation_image_cut_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),
    -- One act, one cut: a retried press finds the cut it already made.
    CONSTRAINT observation_image_cut_event_key
        UNIQUE (tenant_id, client_event_id)
);

COMMENT ON TABLE observation_image_cut IS
    'A photograph cut to the face it is of and straightened: the four corners '
    'somebody marked, and the picture that came out. The photograph is kept; '
    'the newest cut of it is the one shown. D176.';
COMMENT ON COLUMN observation_image_cut.corners IS
    'Top-left, top-right, bottom-right, bottom-left of the face, x then y, as '
    'fractions of the photograph shown the right way up. The order says which '
    'way up the face is.';

-- The newest cut of a photograph.
CREATE INDEX observation_image_cut_image_idx
    ON observation_image_cut (observation_image_id, recorded_at DESC, id DESC);

-- Every row that names a digest, for the reaper and for serving the bytes.
CREATE INDEX observation_image_cut_digest_idx
    ON observation_image_cut (tenant_id, digest);

ALTER TABLE observation_image_cut ENABLE ROW LEVEL SECURITY;
ALTER TABLE observation_image_cut FORCE ROW LEVEL SECURITY;
CREATE POLICY observation_image_cut_tenant_scoped ON observation_image_cut
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- Fact: INSERT only for the app (S6). Cutting again is another row.
GRANT SELECT, INSERT ON observation_image_cut TO spork_app;
GRANT SELECT ON observation_image_cut TO spork_platform;
GRANT SELECT ON observation_image_cut TO spork_projection_owner;
GRANT SELECT ON observation_image_cut TO spork_scheduler;
