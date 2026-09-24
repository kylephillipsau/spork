-- Migration 78: a photograph is part of the same look at the same box.
--
-- Capturing what a thing measures is already modelled — `observation_event`
-- says who looked, when, on which device and by what method, and `measurement`
-- hangs the figures off it. What nothing could record is the other thing an
-- operator produces while standing there: a picture of the object.
--
-- **This is not a new concept and it is deliberately not a new event.** A
-- photograph of a carton is another artefact of one act of examining it, so it
-- hangs off the observation event that already carries the who, the when, the
-- device and the method. One capture session is one event with numbers and
-- images on it, which makes "the photographs and the figures came from the same
-- look at the same box" a property the schema can answer rather than a hope.
--
-- Giving photographs their own event would have made that unanswerable: two
-- rows with two timestamps and nothing joining them but proximity, which is the
-- shape D10 rejected for movements and D44 recognised again later.
--
-- # The bytes are not in here
--
-- The row holds a **content address** — a SHA-256 of the image — plus its type
-- and size, and the bytes live behind `crate::images`. Three reasons, and the
-- first is the one that decides it:
--
-- D39 says every integration is a capability with a working default. A
-- filesystem volume needs nothing external, so a deployment running no object
-- store loses no function; an S3 or R2 implementation is the same seam later.
--
-- Second, a content address deduplicates by construction. Photographing the
-- same unchanged carton twice writes one file, and a retake that changes
-- nothing costs nothing.
--
-- Third, multi-megabyte blobs in a table are multi-megabyte blobs in the WAL,
-- in every replica and in every `pg_dump`. The figures in this database are
-- small and the photographs are not, and mixing them makes the small ones
-- expensive to move.

CREATE TABLE observation_image (
    id                   uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id            uuid NOT NULL REFERENCES tenant(id),

    -- The act of looking. Not nullable: an image with no event is an image
    -- nobody can say who took, when, or with what.
    --
    -- **The tenant travels in the key**, per S51. A plain reference to the id
    -- alone lets a row point at another tenant's observation: RLS filters what
    -- a query *reads* and says nothing about what a row may *point at*, which
    -- is the hole D55 found open since migration 1. `observation_event` already
    -- offers `(id, tenant_id)` for exactly this, and the structural check
    -- caught this row's absence of it before the migration had run twice.
    observation_event_id uuid NOT NULL,

    -- Which side of the object. `label` is the seventh because it is the one
    -- that is not a geometric face and is the most useful of the lot: it is the
    -- carrier label, the barcode and the lot code, which is what D28's
    -- unresolvable-scan question eventually needs a picture of.
    face                 text NOT NULL,

    -- SHA-256, lower-case hex. The address of the bytes, and the reason a
    -- retake of an unchanged carton costs nothing.
    digest               text NOT NULL,
    mime                 text NOT NULL,
    byte_count           bigint NOT NULL,

    -- What the pixels are, when the encoder said. Nullable because a format we
    -- do not parse is still a photograph worth keeping.
    width_px             integer,
    height_px            integer,

    -- Valid time from the device, transaction time from the server: the same
    -- pair `observation_event` and `stock_movement` carry, for the same reason.
    captured_at          timestamptz NOT NULL,
    recorded_at          timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT observation_image_face_ck
        CHECK (face IN ('front','back','left','right','top','bottom','label')),
    CONSTRAINT observation_image_digest_ck
        CHECK (digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT observation_image_bytes_ck
        CHECK (byte_count > 0),
    CONSTRAINT observation_image_pixels_ck
        CHECK ((width_px IS NULL) = (height_px IS NULL)
               AND (width_px IS NULL OR (width_px > 0 AND height_px > 0))),

    -- **No uniqueness on (event, face), and that is a correction.**
    --
    -- The first draft made a retake replace its predecessor, which needed
    -- `ON CONFLICT DO UPDATE`, which needs UPDATE on a table that grants
    -- INSERT and SELECT. The grant was right and the design was wrong: this is
    -- a fact table, facts are only ever added to, and a blurred first attempt
    -- is a thing that happened.
    --
    -- So a retake is a new row and the current picture of a face is a fold —
    -- latest by `captured_at`, then `recorded_at`, then `id`. That is the same
    -- winning-row rule `package_event` already uses to say where a carton is,
    -- and reaching for it here rather than inventing a second one is the point.
    CONSTRAINT observation_image_id_positive CHECK (byte_count > 0),

    CONSTRAINT observation_image_event_fk
        FOREIGN KEY (observation_event_id, tenant_id)
        REFERENCES observation_event(id, tenant_id)
);

COMMENT ON TABLE observation_image IS
    'A photograph taken during an observation. Hangs off the event rather than '
    'carrying its own, so the pictures and the figures provably came from one '
    'look at one object. Bytes live behind a content address (D39).';

-- The fold: the current picture of a face, newest first.
CREATE INDEX observation_image_face_idx
    ON observation_image (observation_event_id, face,
                          captured_at DESC, recorded_at DESC, id DESC);

-- Every row that names a digest, for the reaper and for a store audit: the
-- files on disk and the rows here are two sides of one set, and D30's reference
-- question is asked of whichever is longer.
CREATE INDEX observation_image_digest_idx
    ON observation_image (tenant_id, digest);

ALTER TABLE observation_image ENABLE ROW LEVEL SECURITY;
ALTER TABLE observation_image FORCE ROW LEVEL SECURITY;
CREATE POLICY observation_image_tenant_scoped ON observation_image
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- Fact: INSERT only for the app (S6). A photograph of what was in front of
-- somebody is not editable afterwards; a better picture is another row, and
-- which one is current is a question the fold answers.
GRANT SELECT, INSERT ON observation_image TO spork_app;
GRANT SELECT ON observation_image TO spork_platform;
GRANT SELECT ON observation_image TO spork_projection_owner;
GRANT SELECT ON observation_image TO spork_scheduler;
