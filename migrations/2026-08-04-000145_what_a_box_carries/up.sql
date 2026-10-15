-- Migration 145: what a thing takes on top, what a box carries, and the
-- warehouse's conditions. D249, docs/packing-plan.md (M4).
--
-- A thing takes weight unless somebody says otherwise: up to so much, or
-- nothing. It is said of a subject and inherited as "this way up" is (D200).
-- A box type has two limits: on its walls, estimated from its size and board
-- unless a test on the floor says, and on its lid, 10 kg to start. What the
-- estimate allows for is the warehouse's: its humidity, how long a load
-- stands, how it is stacked, the gaps in a pallet's deck and its handling,
-- one setting for the workspace. A carton is solid or hollow, said when it is
-- sealed: a solid one's lid carries what its walls do.

-- ---------------------------------------------------------------------------
-- Whose saying it is, asked once
-- ---------------------------------------------------------------------------

-- The subjects a saying about one is looked for on, nearest first: its own; a
-- variant's item's carton (`item`); an item's carton's or a variant's
-- family's carton (`style`). Four functions wrote this out each; a fifth reads
-- it from here, and so now do they.
CREATE FUNCTION subject_asked(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (rank integer, source text, item_id uuid, item_style_id uuid,
               lot_id uuid, item_part_id uuid, level packaging_level)
LANGUAGE sql STABLE AS $$
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
$$;

COMMENT ON FUNCTION subject_asked(uuid, uuid, uuid, uuid, packaging_level) IS
    'The subjects a saying about one is inherited from, nearest first, and whose '
    'each is: own, item, style. D191, D196, D200, D239, D249.';

CREATE OR REPLACE FUNCTION packed_in(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (packaging_type text, source text)
LANGUAGE sql STABLE AS $$
    SELECT sp.packaging_type, a.source
      FROM subject_asked(p_item, p_style, p_lot, p_part, p_level) a
      JOIN subject_packaging sp
        ON sp.item_id IS NOT DISTINCT FROM a.item_id
       AND sp.item_style_id IS NOT DISTINCT FROM a.item_style_id
       AND sp.lot_id IS NOT DISTINCT FROM a.lot_id
       AND sp.item_part_id IS NOT DISTINCT FROM a.item_part_id
       AND sp.packaging_level IS NOT DISTINCT FROM a.level
     ORDER BY a.rank, sp.recorded_at DESC, sp.id DESC
     LIMIT 1
$$;

CREATE OR REPLACE FUNCTION ships_as_is(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (as_it_is boolean, source text)
LANGUAGE sql STABLE AS $$
    WITH said AS (
        SELECT ss.as_it_is, a.source
          FROM subject_asked(p_item, p_style, p_lot, p_part, p_level) a
          JOIN subject_shipping ss
            ON ss.item_id IS NOT DISTINCT FROM a.item_id
           AND ss.item_style_id IS NOT DISTINCT FROM a.item_style_id
           AND ss.lot_id IS NOT DISTINCT FROM a.lot_id
           AND ss.item_part_id IS NOT DISTINCT FROM a.item_part_id
           AND ss.packaging_level IS NOT DISTINCT FROM a.level
         ORDER BY a.rank, ss.recorded_at DESC, ss.id DESC
         LIMIT 1
    )
    SELECT as_it_is, source FROM said
    UNION ALL
    -- A part has no level, and a comparison with nothing is nothing, not false.
    SELECT p_lot IS NOT NULL OR coalesce(p_level = 'carton', false), 'default'
     WHERE NOT EXISTS (SELECT 1 FROM said)
$$;

CREATE OR REPLACE FUNCTION keeps_upright(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (upright boolean, source text)
LANGUAGE sql STABLE AS $$
    WITH said AS (
        SELECT su.upright, a.source
          FROM subject_asked(p_item, p_style, p_lot, p_part, p_level) a
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

CREATE OR REPLACE FUNCTION is_box(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT CASE
               WHEN t.code IS NULL OR t.six_sided THEN true
               WHEN t.round THEN false
               ELSE coalesce((
                   SELECT ss.box_shaped
                     FROM subject_asked(p_item, p_style, p_lot, p_part, p_level) a
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

-- ---------------------------------------------------------------------------
-- What a thing takes on top
-- ---------------------------------------------------------------------------

CREATE TABLE subject_load (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),

    -- The subject, as `subject_shipping` names one.
    item_id          uuid,
    item_style_id    uuid,
    lot_id           uuid,
    item_part_id     uuid,
    packaging_level  packaging_level,

    -- The most it takes on top, in grams: none at all at nought, and no limit
    -- when empty, which is what "takes weight" means said of it.
    most_on_top_g    bigint,

    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid NOT NULL REFERENCES person(id),
    recorded_at      timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT subject_load_one_arm_ck
        CHECK (num_nonnulls(item_id, item_style_id, lot_id, item_part_id) = 1),
    CONSTRAINT subject_load_level_ck
        CHECK ((packaging_level IS NOT NULL) = (item_id IS NOT NULL OR item_style_id IS NOT NULL)),
    CONSTRAINT subject_load_most_ck CHECK (most_on_top_g >= 0),
    CONSTRAINT subject_load_item_fk
        FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT subject_load_style_fk
        FOREIGN KEY (item_style_id, tenant_id) REFERENCES item_style(id, tenant_id),
    CONSTRAINT subject_load_lot_fk
        FOREIGN KEY (lot_id, tenant_id) REFERENCES lot(id, tenant_id),
    CONSTRAINT subject_load_part_fk
        FOREIGN KEY (item_part_id, tenant_id) REFERENCES item_part(id, tenant_id),
    CONSTRAINT subject_load_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT subject_load_event_key UNIQUE (tenant_id, client_event_id)
);

COMMENT ON TABLE subject_load IS
    'What a subject takes on top: up to so many grams, nothing, or, empty, no '
    'limit. The newest saying wins. D249, migration 145.';

CREATE INDEX subject_load_item_idx ON subject_load (item_id, packaging_level, recorded_at DESC) WHERE item_id IS NOT NULL;
CREATE INDEX subject_load_style_idx ON subject_load (item_style_id, packaging_level, recorded_at DESC) WHERE item_style_id IS NOT NULL;
CREATE INDEX subject_load_lot_idx ON subject_load (lot_id, recorded_at DESC) WHERE lot_id IS NOT NULL;
CREATE INDEX subject_load_part_idx ON subject_load (item_part_id, recorded_at DESC) WHERE item_part_id IS NOT NULL;

ALTER TABLE subject_load ENABLE ROW LEVEL SECURITY;
ALTER TABLE subject_load FORCE ROW LEVEL SECURITY;
CREATE POLICY subject_load_tenant_scoped ON subject_load
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON subject_load TO spork_app;

-- The most a subject takes on top, and whose saying it is; `default`, with no
-- limit, when nobody has said: a thing takes weight unless it says otherwise.
CREATE FUNCTION takes_on_top(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (most_on_top_g bigint, source text)
LANGUAGE sql STABLE AS $$
    WITH said AS (
        SELECT sl.most_on_top_g, a.source
          FROM subject_asked(p_item, p_style, p_lot, p_part, p_level) a
          JOIN subject_load sl
            ON sl.item_id IS NOT DISTINCT FROM a.item_id
           AND sl.item_style_id IS NOT DISTINCT FROM a.item_style_id
           AND sl.lot_id IS NOT DISTINCT FROM a.lot_id
           AND sl.item_part_id IS NOT DISTINCT FROM a.item_part_id
           AND sl.packaging_level IS NOT DISTINCT FROM a.level
         ORDER BY a.rank, sl.recorded_at DESC, sl.id DESC
         LIMIT 1
    )
    SELECT most_on_top_g, source FROM said
    UNION ALL
    SELECT NULL::bigint, 'default' WHERE NOT EXISTS (SELECT 1 FROM said)
$$;

COMMENT ON FUNCTION takes_on_top(uuid, uuid, uuid, uuid, packaging_level) IS
    'The most a subject takes on top, own before inherited before the default of '
    'no limit, and whose saying it is. D249.';

-- ---------------------------------------------------------------------------
-- What a box carries
-- ---------------------------------------------------------------------------

ALTER TABLE package_type
    -- Its board, as stamped: single or double wall, and the edge crush in
    -- pounds an inch, "32 ECT". Unsaid, single-wall 32 ECT is assumed.
    ADD COLUMN board_walls smallint,
    ADD COLUMN board_ect   smallint,
    -- What its walls carry, from a test on the floor; unsaid, it is estimated
    -- from its size and board under the warehouse's conditions.
    ADD COLUMN wall_load_g bigint,
    -- What its lid carries when it is hollow; unsaid, 10 kg.
    ADD COLUMN lid_load_g  bigint,
    ADD CONSTRAINT package_type_board_ck
        CHECK (num_nonnulls(board_walls, board_ect) IN (0, 2)
               AND board_walls IN (1, 2) AND board_ect BETWEEN 10 AND 200),
    ADD CONSTRAINT package_type_load_ck
        CHECK (wall_load_g > 0 AND lid_load_g >= 0);

COMMENT ON COLUMN package_type.board_walls IS
    'D249. Single (1) or double (2) wall board, with board_ect; both or neither.';
COMMENT ON COLUMN package_type.board_ect IS
    'D249. The board''s edge crush, in pounds an inch, as stamped: 32 ECT.';
COMMENT ON COLUMN package_type.wall_load_g IS
    'D249. What its walls carry, set from a test; empty, it is estimated.';
COMMENT ON COLUMN package_type.lid_load_g IS
    'D249. What its lid carries when hollow; empty, 10 kg.';

-- As migration 141 has it, and a cut's board and limits taken from its full
-- box unless it says its own (D252).
DROP VIEW package_type_effective;
CREATE VIEW package_type_effective
    WITH (security_invoker = true) AS
SELECT t.id, t.tenant_id, t.name, t.carrier_package_code, t.dimensions_fixed,
       t.length_mm, t.width_mm, t.height_mm,
       coalesce(t.tare_weight_g, f.tare_weight_g) AS tare_weight_g,
       coalesce(t.max_payload_g, f.max_payload_g) AS max_payload_g,
       t.suggested, t.reusable, t.effective_from,
       t.cut_from_id,
       f.name AS cut_from_name,
       f.height_mm AS full_height_mm,
       -- A pallet or a skid carries cartons rather than holding goods.
       coalesce(t.carrier_package_code, '') IN ('PAL', 'SKI', 'SKD') AS carries,
       -- A cut, or a box with cuts: its height is one of a set list, so the
       -- type says how tall a carton of it is.
       (t.cut_from_id IS NOT NULL
        OR EXISTS (SELECT 1 FROM package_type c WHERE c.cut_from_id = t.id)) AS height_is_set,
       -- Whose photographs it wears: a cut is drawn with its full box's.
       coalesce(t.cut_from_id, t.id) AS pictured_as,
       CASE WHEN t.board_walls IS NOT NULL THEN t.board_walls ELSE f.board_walls END AS board_walls,
       CASE WHEN t.board_walls IS NOT NULL THEN t.board_ect ELSE f.board_ect END AS board_ect,
       coalesce(t.wall_load_g, f.wall_load_g) AS wall_load_g,
       coalesce(t.lid_load_g, f.lid_load_g) AS lid_load_g
  FROM package_type t
  LEFT JOIN package_type f ON f.id = t.cut_from_id;

COMMENT ON VIEW package_type_effective IS
    'D252, D249. A box type with a cut''s empty weight, limit, board and loads '
    'taken from its full box unless it says its own, whether it carries cartons, '
    'whether its height is set, and whose photographs it wears.';

GRANT SELECT ON package_type_effective TO spork_app;

-- ---------------------------------------------------------------------------
-- The workspace's settings, and the warehouse's conditions first
-- ---------------------------------------------------------------------------

-- What is said of a whole workspace rather than of a site or a thing. A row
-- once something is said; until then every setting is its default, which the
-- server keeps (`carrying::Conditions::default`).
CREATE TABLE workspace_setting (
    tenant_id        uuid PRIMARY KEY REFERENCES tenant(id),

    -- The warehouse's conditions, which the estimate of what a box's walls
    -- carry allows for (D249): the Fibre Box Association's multipliers.
    -- The relative humidity most days, in per cent.
    humidity_pct     smallint NOT NULL,
    -- How long a load stands before it goes, in days.
    days_under_load  smallint NOT NULL,
    -- How the cartons are stacked, most of the time.
    stacking         text NOT NULL,
    -- A pallet's deck has gaps between its boards.
    deck_gaps        boolean NOT NULL,
    -- How the cartons are handled.
    handling         text NOT NULL,
    -- What the walls must carry over what is on them: the road (ASTM D4169).
    road_factor      smallint NOT NULL,

    updated_at       timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT workspace_setting_humidity_ck CHECK (humidity_pct IN (50, 60, 70, 80, 90)),
    CONSTRAINT workspace_setting_days_ck CHECK (days_under_load IN (1, 10, 30, 90, 180)),
    CONSTRAINT workspace_setting_stacking_ck CHECK (stacking IN ('columns', 'misaligned', 'interlocked')),
    CONSTRAINT workspace_setting_handling_ck CHECK (handling IN ('careful', 'some_rough', 'rough')),
    CONSTRAINT workspace_setting_road_ck CHECK (road_factor BETWEEN 1 AND 10)
);

COMMENT ON TABLE workspace_setting IS
    'What a workspace says of itself as a whole: the warehouse''s conditions '
    '(D249). One row once said; the defaults until then. Migration 145.';

ALTER TABLE workspace_setting ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspace_setting FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_setting_tenant_scoped ON workspace_setting
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- Settings, not facts: changed in place.
GRANT SELECT, INSERT, UPDATE ON workspace_setting TO spork_app;

-- ---------------------------------------------------------------------------
-- Solid or hollow, said at sealing
-- ---------------------------------------------------------------------------

ALTER TABLE package_event
    ADD COLUMN hollow boolean,
    ADD CONSTRAINT package_event_hollow_ck CHECK (hollow IS NULL OR kind = 'sealed');

COMMENT ON COLUMN package_event.hollow IS
    'D249. Said with a seal: the carton is hollow, so its lid carries what the '
    'box''s lid does, or solid, and it carries what its walls do. Empty, unsaid.';

GRANT SELECT (hollow), INSERT (hollow) ON package_event TO spork_app;
