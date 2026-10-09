-- Migration 134: a round thing is wrapped in its photographs. D240.
--
-- D213 measured a bucket as the shape it is (across the rim, across the base,
-- its height, the straight band under the rim) and left drawing it for later.
-- Its side has no faces to cut a photograph to (D191). Instead, at a
-- computer, the camera behind each photograph of its side is found by
-- fitting the measured shape to it, and the side is read off each photograph
-- where it faced the camera: four quarter turns, laid side by side and
-- blended, are its whole label, as one picture to wrap round it. Its lid and
-- base are straightened into discs.
--
-- # What is kept
--
-- The pictures made, each a row of its own with its type and size, as a box's
-- drawing is (D186): the side, and the lid and the base where they were
-- photographed. And the photographs it was made from, so a retaken side is
-- seen to want wrapping again. Said of a subject as what it is packed in is
-- (D191), the newest wrapping winning.
--
-- The item's drawing for lists (D186) is still `box_picture`: a round thing's
-- is drawn from its wrap, and names the wrap's three pictures as a box's
-- names its three cut faces.

CREATE TABLE round_wrap (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),

    -- The subject, as `subject_packaging` names one.
    item_id          uuid,
    item_style_id    uuid,
    lot_id           uuid,
    item_part_id     uuid,
    packaging_level  packaging_level,

    -- The photographs it was made from.
    made_from        uuid[] NOT NULL,

    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid NOT NULL REFERENCES person(id),
    recorded_at      timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT round_wrap_one_arm_ck
        CHECK (num_nonnulls(item_id, item_style_id, lot_id, item_part_id) = 1),
    CONSTRAINT round_wrap_level_ck
        CHECK ((packaging_level IS NOT NULL) = (item_id IS NOT NULL OR item_style_id IS NOT NULL)),
    CONSTRAINT round_wrap_made_from_ck CHECK (cardinality(made_from) >= 1),
    CONSTRAINT round_wrap_item_fk
        FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT round_wrap_style_fk
        FOREIGN KEY (item_style_id, tenant_id) REFERENCES item_style(id, tenant_id),
    CONSTRAINT round_wrap_lot_fk
        FOREIGN KEY (lot_id, tenant_id) REFERENCES lot(id, tenant_id),
    CONSTRAINT round_wrap_part_fk
        FOREIGN KEY (item_part_id, tenant_id) REFERENCES item_part(id, tenant_id),
    CONSTRAINT round_wrap_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT round_wrap_event_key UNIQUE (tenant_id, client_event_id),
    CONSTRAINT round_wrap_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON TABLE round_wrap IS
    'A round thing wrapped in its photographs, and what it was made from. '
    'The newest wrapping wins. D240, migration 134.';

CREATE INDEX round_wrap_item_idx ON round_wrap (item_id, packaging_level, recorded_at DESC) WHERE item_id IS NOT NULL;
CREATE INDEX round_wrap_style_idx ON round_wrap (item_style_id, packaging_level, recorded_at DESC) WHERE item_style_id IS NOT NULL;
CREATE INDEX round_wrap_lot_idx ON round_wrap (lot_id, recorded_at DESC) WHERE lot_id IS NOT NULL;
CREATE INDEX round_wrap_part_idx ON round_wrap (item_part_id, recorded_at DESC) WHERE item_part_id IS NOT NULL;

ALTER TABLE round_wrap ENABLE ROW LEVEL SECURITY;
ALTER TABLE round_wrap FORCE ROW LEVEL SECURITY;
CREATE POLICY round_wrap_tenant_scoped ON round_wrap
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON round_wrap TO spork_app;

-- A wrapping's pictures: its side unwrapped, once round from the back with
-- the front in the middle and the rim at the top; its lid and its base as
-- discs, the photograph's bottom edge toward the front.
CREATE TABLE round_wrap_picture (
    id             uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id      uuid NOT NULL REFERENCES tenant(id),
    round_wrap_id  uuid NOT NULL,
    part           text NOT NULL,
    digest         text NOT NULL,
    mime           text NOT NULL,
    byte_count     bigint NOT NULL,
    width_px       integer,
    height_px      integer,

    CONSTRAINT round_wrap_picture_part_ck CHECK (part IN ('side', 'lid', 'base')),
    CONSTRAINT round_wrap_picture_digest_ck CHECK (digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT round_wrap_picture_bytes_ck CHECK (byte_count > 0),
    CONSTRAINT round_wrap_picture_wrap_fk
        FOREIGN KEY (round_wrap_id, tenant_id) REFERENCES round_wrap(id, tenant_id),
    CONSTRAINT round_wrap_picture_part_key UNIQUE (round_wrap_id, part)
);

COMMENT ON TABLE round_wrap_picture IS
    'A round thing''s wrapping''s pictures: its side, lid and base. D240, migration 134.';

CREATE INDEX round_wrap_picture_digest_idx ON round_wrap_picture (tenant_id, digest);

ALTER TABLE round_wrap_picture ENABLE ROW LEVEL SECURITY;
ALTER TABLE round_wrap_picture FORCE ROW LEVEL SECURITY;
CREATE POLICY round_wrap_picture_tenant_scoped ON round_wrap_picture
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON round_wrap_picture TO spork_app;

-- A subject's newest wrapping, its own only: a photograph is of the thing it
-- was taken of. Nothing wrapped, no row. Invoker's rights.
CREATE FUNCTION wrap_of(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (side text, lid text, base text, made_from uuid[])
LANGUAGE sql STABLE AS $$
    SELECT (SELECT p.digest FROM round_wrap_picture p WHERE p.round_wrap_id = w.id AND p.part = 'side'),
           (SELECT p.digest FROM round_wrap_picture p WHERE p.round_wrap_id = w.id AND p.part = 'lid'),
           (SELECT p.digest FROM round_wrap_picture p WHERE p.round_wrap_id = w.id AND p.part = 'base'),
           w.made_from
      FROM round_wrap w
     WHERE w.item_id IS NOT DISTINCT FROM p_item
       AND w.item_style_id IS NOT DISTINCT FROM p_style
       AND w.lot_id IS NOT DISTINCT FROM p_lot
       AND w.item_part_id IS NOT DISTINCT FROM p_part
       AND w.packaging_level IS NOT DISTINCT FROM p_level
     ORDER BY w.recorded_at DESC, w.id DESC
     LIMIT 1
$$;

COMMENT ON FUNCTION wrap_of(uuid, uuid, uuid, uuid, packaging_level) IS
    'A subject''s newest wrapping in its photographs. D240.';
