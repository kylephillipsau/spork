-- Migration 107: an item is pictured as its box. D186.
--
-- A list of items is read by their pictures, and a front alone is a flat
-- rectangle that looks like every other carton. Once a box's front, right and
-- top have been cut to their faces (D176), a computer draws the box from them
-- at an angle, three faces at once, at its measured proportions, and keeps the
-- drawing: a small picture the list shows instead of the front.
--
-- # Drawn, and from what
--
-- A drawing is made by somebody's computer from cuts somebody checked, so it
-- carries its act (D5) and its person like a cut does, and the three cuts it
-- was drawn from, by their content addresses: a face cut again is a drawing
-- out of date, and the next computer to see it draws it again. Facts only;
-- the newest drawing of an item is the one shown.

CREATE TABLE box_picture (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),
    item_id          uuid NOT NULL,

    digest           text NOT NULL,
    mime             text NOT NULL,
    byte_count       bigint NOT NULL,
    width_px         integer,
    height_px        integer,

    -- The front, right and top it was drawn from: the cuts' content addresses.
    made_from        text[] NOT NULL,

    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid NOT NULL REFERENCES person(id),
    recorded_at      timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT box_picture_digest_ck CHECK (digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT box_picture_bytes_ck CHECK (byte_count > 0),
    CONSTRAINT box_picture_made_from_ck CHECK (array_length(made_from, 1) = 3),
    CONSTRAINT box_picture_item_fk FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT box_picture_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT box_picture_event_key UNIQUE (tenant_id, client_event_id)
);

COMMENT ON TABLE box_picture IS
    'An item drawn as its box from three cut faces (front, right, top), at an '
    'angle: the picture a list shows. The newest is shown. D186.';

CREATE INDEX box_picture_item_idx ON box_picture (item_id, recorded_at DESC, id DESC);
CREATE INDEX box_picture_digest_idx ON box_picture (tenant_id, digest);

ALTER TABLE box_picture ENABLE ROW LEVEL SECURITY;
ALTER TABLE box_picture FORCE ROW LEVEL SECURITY;
CREATE POLICY box_picture_tenant_scoped ON box_picture
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON box_picture TO spork_app;
