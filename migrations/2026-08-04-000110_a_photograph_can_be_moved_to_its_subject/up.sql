-- Migration 110: a photograph filed against the wrong thing is moved. D190.
--
-- On a phone it is easy to photograph a box against the wrong card: the
-- family's carton when it was one size's, the each when it was the carton.
-- A photograph is a fact of the look that took it and is never edited, so a
-- move files it again (its bytes, under a look of the right subject) and
-- records here that the first filing was moved, and to which. A photograph
-- that was moved is shown nowhere: not on the item, not in the crop queue,
-- not as a picture.

CREATE TABLE observation_image_move (
    id                    uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id             uuid NOT NULL REFERENCES tenant(id),
    observation_image_id  uuid NOT NULL,
    moved_to_image_id     uuid NOT NULL,
    client_event_id       uuid NOT NULL,
    recorded_by_id        uuid NOT NULL REFERENCES person(id),
    recorded_at           timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT observation_image_move_from_fk
        FOREIGN KEY (observation_image_id, tenant_id) REFERENCES observation_image(id, tenant_id),
    CONSTRAINT observation_image_move_to_fk
        FOREIGN KEY (moved_to_image_id, tenant_id) REFERENCES observation_image(id, tenant_id),
    CONSTRAINT observation_image_move_not_itself_ck CHECK (observation_image_id <> moved_to_image_id),
    -- Moved once: a photograph moved again is its new filing moved.
    CONSTRAINT observation_image_move_once_key UNIQUE (observation_image_id),
    CONSTRAINT observation_image_move_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT observation_image_move_event_key UNIQUE (tenant_id, client_event_id)
);

COMMENT ON TABLE observation_image_move IS
    'A photograph filed against the wrong subject, moved: filed again under the '
    'right one, and this first filing shown nowhere. D190.';

ALTER TABLE observation_image_move ENABLE ROW LEVEL SECURITY;
ALTER TABLE observation_image_move FORCE ROW LEVEL SECURITY;
CREATE POLICY observation_image_move_tenant_scoped ON observation_image_move
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON observation_image_move TO spork_app;
GRANT SELECT ON observation_image_move TO spork_projection_owner;
