-- Migration 133: a wrapped pack can be box-shaped. D239.
--
-- D191 made whether a thing is a box a question of what it is packed in: a
-- six-sided type (a case, a box) is photographed side by side, cut to its
-- faces and drawn; anything else is photographed as a thing. A brush in
-- shrink-wrap has no sides, and that is still right. But ten scouring pads
-- wrapped in film are a block with six flat sides, and a band round a stack
-- of boxes leaves the stack a box. The wrapping says what it is made of, not
-- its shape.
--
-- # Said of a subject, where its packaging leaves it open
--
-- Whether it is box-shaped is said of a subject as what it is packed in is
-- (`subject_packaging`), the newest saying winning, a variant taking its
-- item's carton's and an item's carton its family's carton's. It only
-- decides anything for a type with no shape of its own: a six-sided type is
-- a box whatever is said, a round one (a bucket, a roll) never is, and a
-- subject nobody has said is packed in anything is taken to be a box, as it
-- has been since D176.
--
-- # One rule, read everywhere
--
-- The item page, Photos to crop, the drawing and the list's picture each
-- asked `packaging_type.six_sided` themselves. They ask `is_box` now.

CREATE TABLE subject_shape (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),

    -- The subject, as `subject_packaging` names one.
    item_id          uuid,
    item_style_id    uuid,
    lot_id           uuid,
    item_part_id     uuid,
    packaging_level  packaging_level,

    -- Its sides are flat: photographed side by side, cut to its faces, drawn.
    box_shaped       boolean NOT NULL,

    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid NOT NULL REFERENCES person(id),
    recorded_at      timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT subject_shape_one_arm_ck
        CHECK (num_nonnulls(item_id, item_style_id, lot_id, item_part_id) = 1),
    CONSTRAINT subject_shape_level_ck
        CHECK ((packaging_level IS NOT NULL) = (item_id IS NOT NULL OR item_style_id IS NOT NULL)),
    CONSTRAINT subject_shape_item_fk
        FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT subject_shape_style_fk
        FOREIGN KEY (item_style_id, tenant_id) REFERENCES item_style(id, tenant_id),
    CONSTRAINT subject_shape_lot_fk
        FOREIGN KEY (lot_id, tenant_id) REFERENCES lot(id, tenant_id),
    CONSTRAINT subject_shape_part_fk
        FOREIGN KEY (item_part_id, tenant_id) REFERENCES item_part(id, tenant_id),
    CONSTRAINT subject_shape_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT subject_shape_event_key UNIQUE (tenant_id, client_event_id)
);

COMMENT ON TABLE subject_shape IS
    'Whether a subject packed in something with no shape of its own is box-shaped. '
    'The newest saying wins. D239, migration 133.';

CREATE INDEX subject_shape_item_idx ON subject_shape (item_id, packaging_level, recorded_at DESC) WHERE item_id IS NOT NULL;
CREATE INDEX subject_shape_style_idx ON subject_shape (item_style_id, packaging_level, recorded_at DESC) WHERE item_style_id IS NOT NULL;
CREATE INDEX subject_shape_lot_idx ON subject_shape (lot_id, recorded_at DESC) WHERE lot_id IS NOT NULL;
CREATE INDEX subject_shape_part_idx ON subject_shape (item_part_id, recorded_at DESC) WHERE item_part_id IS NOT NULL;

ALTER TABLE subject_shape ENABLE ROW LEVEL SECURITY;
ALTER TABLE subject_shape FORCE ROW LEVEL SECURITY;
CREATE POLICY subject_shape_tenant_scoped ON subject_shape
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON subject_shape TO spork_app;

-- Whether a subject is a box: photographed side by side, cut to its faces and
-- drawn. Packed in nothing said, or in a six-sided type, it is; in a round
-- type it is not; in any other, it is when the newest saying of its shape,
-- own before inherited (`packed_in`'s inheritance), says so. Invoker's
-- rights, so the tenant's policy applies.
CREATE FUNCTION is_box(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS boolean
LANGUAGE sql STABLE AS $$
    WITH asked (rank, item_id, item_style_id, lot_id, item_part_id, level) AS (
        SELECT 0, p_item, p_style, p_lot, p_part, p_level
        UNION ALL
        SELECT 1, l.item_id, NULL::uuid, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM lot l
         WHERE l.id = p_lot
        UNION ALL
        SELECT 2, NULL::uuid, i.style_id, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM item i
         WHERE i.style_id IS NOT NULL
           AND ((i.id = p_item AND p_level = 'carton')
                OR i.id = (SELECT l.item_id FROM lot l WHERE l.id = p_lot))
    )
    SELECT CASE
               WHEN t.code IS NULL OR t.six_sided THEN true
               WHEN t.round THEN false
               ELSE coalesce((
                   SELECT ss.box_shaped
                     FROM asked a
                     JOIN subject_shape ss
                       ON ss.item_id IS NOT DISTINCT FROM a.item_id
                      AND ss.item_style_id IS NOT DISTINCT FROM a.item_style_id
                      AND ss.lot_id IS NOT DISTINCT FROM a.lot_id
                      AND ss.item_part_id IS NOT DISTINCT FROM a.item_part_id
                      AND ss.packaging_level IS NOT DISTINCT FROM a.level
                    ORDER BY a.rank, ss.recorded_at DESC, ss.id DESC
                    LIMIT 1), false)
           END
      FROM (SELECT 1) one
      LEFT JOIN LATERAL packed_in(p_item, p_style, p_lot, p_part, p_level) pk ON true
      LEFT JOIN packaging_type t ON t.code = pk.packaging_type
$$;

COMMENT ON FUNCTION is_box(uuid, uuid, uuid, uuid, packaging_level) IS
    'Whether a subject is a box: photographed side by side, cut to its faces and drawn. '
    'Its packaging type''s shape, or for a type with none, what was said of it. D191, D239.';
