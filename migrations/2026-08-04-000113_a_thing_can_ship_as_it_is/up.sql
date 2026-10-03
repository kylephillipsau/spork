-- Migration 113: a thing can ship as it is, and a box says whether to suggest it. D196.
--
-- The pack bench's suggestion (D195) packed three rolls of wipers, each in its
-- own box, into a shovel box. Two things were missing.
--
-- # A box says whether to suggest it
--
-- A shovel box is the smallest box that takes three rolls, and nobody would
-- send wipers in one. Whether a box is offered by the suggestion is the
-- workspace's to say, box by box. Every box is still one a packer can choose.
--
-- # A thing says whether it ships as it is
--
-- A roll that comes in its own box can go to the carrier in it. Whether a
-- thing can is not its packaging type: a retail box (`BX`) is not always fit
-- to travel alone, and a case (`CS`) can still go into a bigger box with
-- others. So it is its own fact, said of a subject as `subject_packaging` is
-- (D191), the newest saying winning, a variant taking its item's carton's and
-- an item's carton its family's carton's. Nothing said: a carton ships as it
-- is, which is what migration 98 made of every whole carton, and an each or
-- an inner pack goes into a box.
--
-- # A package can be one of a thing as it is
--
-- Migration 98 let a package be a product's own carton, through its case
-- pack. A roll with no case pack, or an inner pack, had no way to be a
-- package of its own. A package now names the item and the level it is one
-- of; a case pack stays alongside for a carton or an inner, which is where
-- the count in it comes from. Its size and weight are still read from the
-- item's measurements at that level, never copied here.

ALTER TABLE package_type
    ADD COLUMN suggested boolean NOT NULL DEFAULT true;

COMMENT ON COLUMN package_type.suggested IS
    'Whether the pack bench''s suggestion may choose this box. Any box can still '
    'be chosen by hand. D196, migration 113.';

CREATE TABLE subject_shipping (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),

    -- The subject, as `subject_packaging` names one.
    item_id          uuid,
    item_style_id    uuid,
    lot_id           uuid,
    item_part_id     uuid,
    packaging_level  packaging_level,

    -- It goes to the carrier as it is, rather than into a box.
    as_it_is         boolean NOT NULL,

    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid NOT NULL REFERENCES person(id),
    recorded_at      timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT subject_shipping_one_arm_ck
        CHECK (num_nonnulls(item_id, item_style_id, lot_id, item_part_id) = 1),
    CONSTRAINT subject_shipping_level_ck
        CHECK ((packaging_level IS NOT NULL) = (item_id IS NOT NULL OR item_style_id IS NOT NULL)),
    CONSTRAINT subject_shipping_item_fk
        FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT subject_shipping_style_fk
        FOREIGN KEY (item_style_id, tenant_id) REFERENCES item_style(id, tenant_id),
    CONSTRAINT subject_shipping_lot_fk
        FOREIGN KEY (lot_id, tenant_id) REFERENCES lot(id, tenant_id),
    CONSTRAINT subject_shipping_part_fk
        FOREIGN KEY (item_part_id, tenant_id) REFERENCES item_part(id, tenant_id),
    CONSTRAINT subject_shipping_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT subject_shipping_event_key UNIQUE (tenant_id, client_event_id)
);

COMMENT ON TABLE subject_shipping IS
    'Whether a subject ships as it is rather than in a box. The newest saying '
    'wins. D196, migration 113.';

CREATE INDEX subject_shipping_item_idx ON subject_shipping (item_id, packaging_level, recorded_at DESC) WHERE item_id IS NOT NULL;
CREATE INDEX subject_shipping_style_idx ON subject_shipping (item_style_id, packaging_level, recorded_at DESC) WHERE item_style_id IS NOT NULL;
CREATE INDEX subject_shipping_lot_idx ON subject_shipping (lot_id, recorded_at DESC) WHERE lot_id IS NOT NULL;
CREATE INDEX subject_shipping_part_idx ON subject_shipping (item_part_id, recorded_at DESC) WHERE item_part_id IS NOT NULL;

ALTER TABLE subject_shipping ENABLE ROW LEVEL SECURITY;
ALTER TABLE subject_shipping FORCE ROW LEVEL SECURITY;
CREATE POLICY subject_shipping_tenant_scoped ON subject_shipping
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON subject_shipping TO spork_app;

-- Whether a subject ships as it is, and whose saying it is: its own; for a
-- variant, its item's carton's (`item`); for an item's carton or a variant,
-- its family's carton's (`style`); and when nobody has said, `default`: a
-- carton or a variant (which stands for a carton) does, anything else does
-- not. `packed_in`'s inheritance, so the two facts are read alike.
CREATE FUNCTION ships_as_is(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (as_it_is boolean, source text)
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
        SELECT ss.as_it_is, a.source
          FROM asked a
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

COMMENT ON FUNCTION ships_as_is(uuid, uuid, uuid, uuid, packaging_level) IS
    'Whether a subject ships as it is, own before inherited before the default, '
    'and whose saying it is. D196.';

ALTER TABLE package
    ADD COLUMN own_item_id uuid,
    ADD COLUMN own_level packaging_level,
    ADD CONSTRAINT package_own_item_fk
        FOREIGN KEY (own_item_id, tenant_id) REFERENCES item(id, tenant_id),
    ADD CONSTRAINT package_own_level_ck
        CHECK ((own_item_id IS NULL) = (own_level IS NULL)),
    -- A case pack is a count of the item the package is one of: a carton or
    -- an inner pack, never an each and never a box type.
    ADD CONSTRAINT package_own_case_ck
        CHECK (item_packing_config_id IS NULL
               OR (own_item_id IS NOT NULL AND own_level IN ('carton', 'inner')));

-- Every product's own carton so far is one of its item, as a carton.
UPDATE package p
   SET own_item_id = c.item_id, own_level = 'carton'
  FROM item_packing_config c
 WHERE c.id = p.item_packing_config_id AND p.own_item_id IS NULL;

ALTER TABLE package DROP CONSTRAINT package_box_or_own_carton_ck;
ALTER TABLE package ADD CONSTRAINT package_box_or_own_ck
    CHECK (num_nonnulls(package_type_id, own_item_id) <= 1);

COMMENT ON COLUMN package.own_item_id IS
    'This package is one of this item as it is, at own_level, rather than a box '
    'type. Its size and weight are read from the item''s measurements at that level. '
    'D196, migration 113.';

GRANT INSERT (own_item_id, own_level) ON package TO spork_app;
