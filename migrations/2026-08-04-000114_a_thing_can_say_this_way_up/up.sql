-- Migration 114: a thing can say this way up. D200.
--
-- The pack bench's suggestion lays everything on its biggest side (D195): a
-- box of bottles goes in on its back, and a carton marked "this way up" goes
-- in on its side. Whether a thing must stay the way up it stands is said of a
-- subject, as what it ships as is (D196): the newest saying wins, a variant
-- takes its item's carton's and an item's carton its family's carton's.
-- Unsaid, any way up will do, which is what the suggestion has assumed.

CREATE TABLE subject_upright (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),

    -- The subject, as `subject_shipping` names one.
    item_id          uuid,
    item_style_id    uuid,
    lot_id           uuid,
    item_part_id     uuid,
    packaging_level  packaging_level,

    -- It stays the way up it stands: turned round, never onto its side.
    upright          boolean NOT NULL,

    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid NOT NULL REFERENCES person(id),
    recorded_at      timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT subject_upright_one_arm_ck
        CHECK (num_nonnulls(item_id, item_style_id, lot_id, item_part_id) = 1),
    CONSTRAINT subject_upright_level_ck
        CHECK ((packaging_level IS NOT NULL) = (item_id IS NOT NULL OR item_style_id IS NOT NULL)),
    CONSTRAINT subject_upright_item_fk
        FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT subject_upright_style_fk
        FOREIGN KEY (item_style_id, tenant_id) REFERENCES item_style(id, tenant_id),
    CONSTRAINT subject_upright_lot_fk
        FOREIGN KEY (lot_id, tenant_id) REFERENCES lot(id, tenant_id),
    CONSTRAINT subject_upright_part_fk
        FOREIGN KEY (item_part_id, tenant_id) REFERENCES item_part(id, tenant_id),
    CONSTRAINT subject_upright_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT subject_upright_event_key UNIQUE (tenant_id, client_event_id)
);

COMMENT ON TABLE subject_upright IS
    'Whether a subject must stay the way up it stands. The newest saying wins. '
    'D200, migration 114.';

CREATE INDEX subject_upright_item_idx ON subject_upright (item_id, packaging_level, recorded_at DESC) WHERE item_id IS NOT NULL;
CREATE INDEX subject_upright_style_idx ON subject_upright (item_style_id, packaging_level, recorded_at DESC) WHERE item_style_id IS NOT NULL;
CREATE INDEX subject_upright_lot_idx ON subject_upright (lot_id, recorded_at DESC) WHERE lot_id IS NOT NULL;
CREATE INDEX subject_upright_part_idx ON subject_upright (item_part_id, recorded_at DESC) WHERE item_part_id IS NOT NULL;

ALTER TABLE subject_upright ENABLE ROW LEVEL SECURITY;
ALTER TABLE subject_upright FORCE ROW LEVEL SECURITY;
CREATE POLICY subject_upright_tenant_scoped ON subject_upright
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON subject_upright TO spork_app;

-- Whether a subject stays the way up it stands, and whose saying it is: its
-- own; a variant's item's carton's (`item`); an item's carton's or a
-- variant's family's carton's (`style`); and `default`, any way up, when
-- nobody has said. `ships_as_is`'s inheritance.
CREATE FUNCTION keeps_upright(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (upright boolean, source text)
LANGUAGE sql STABLE AS $$
    WITH asked (rank, source, item_id, item_style_id, lot_id, item_part_id, level) AS (
        SELECT 0, 'own', p_item, p_style, p_lot, p_part, p_level
        UNION ALL
        SELECT 1, 'item', l.item_id, NULL::uuid, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM lot l
         WHERE l.id = p_lot
        UNION ALL
        SELECT 2, 'style', NULL::uuid, i.style_id, NULL::uuid, NULL::uuid, 'carton'::packaging_level
          FROM item i
         WHERE i.style_id IS NOT NULL
           AND ((i.id = p_item AND p_level = 'carton')
                OR i.id = (SELECT l.item_id FROM lot l WHERE l.id = p_lot))
    ),
    said AS (
        SELECT su.upright, a.source
          FROM asked a
          JOIN subject_upright su
            ON su.item_id IS NOT DISTINCT FROM a.item_id
           AND su.item_style_id IS NOT DISTINCT FROM a.item_style_id
           AND su.lot_id IS NOT DISTINCT FROM a.lot_id
           AND su.item_part_id IS NOT DISTINCT FROM a.item_part_id
           AND su.packaging_level IS NOT DISTINCT FROM a.level
         ORDER BY a.rank, su.recorded_at DESC, su.id DESC
         LIMIT 1
    )
    SELECT upright, source FROM said
    UNION ALL
    SELECT false, 'default' WHERE NOT EXISTS (SELECT 1 FROM said)
$$;

COMMENT ON FUNCTION keeps_upright(uuid, uuid, uuid, uuid, packaging_level) IS
    'Whether a subject stays the way up it stands, own before inherited before '
    'the default, and whose saying it is. D200.';
