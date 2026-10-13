-- Migration 141: a box is cut down to set heights, and box types are
-- photographed. D252.
--
-- The warehouse's boxes are cut at the corners and folded down to set heights.
-- Each height is a box of its own, cut from a full box: the full box's length
-- and width, its own height, and its own name for the booking. A row for each
-- height means the suggestion, the bench and the booking use the cuts with
-- nothing new to learn.

-- ---------------------------------------------------------------------------
-- A cut, and the box it is cut from
-- ---------------------------------------------------------------------------

-- The footprint travels with the reference: a cut's length and width are its
-- full box's, and stay so when the full box is measured again.
ALTER TABLE package_type
    ADD CONSTRAINT package_type_footprint_key UNIQUE (id, length_mm, width_mm);

ALTER TABLE package_type
    ADD COLUMN cut_from_id uuid,
    ADD CONSTRAINT package_type_cut_from_fk
        FOREIGN KEY (cut_from_id, length_mm, width_mm)
        REFERENCES package_type (id, length_mm, width_mm)
        ON UPDATE CASCADE,
    ADD CONSTRAINT package_type_cut_not_itself_ck
        CHECK (cut_from_id IS DISTINCT FROM id),
    -- Sized, so the reference above is always checked: MATCH SIMPLE checks
    -- nothing while a column of it is null.
    ADD CONSTRAINT package_type_cut_is_sized_ck
        CHECK (cut_from_id IS NULL OR (dimensions_fixed AND height_mm IS NOT NULL));

CREATE UNIQUE INDEX package_type_one_cut_a_height
    ON package_type (cut_from_id, height_mm)
    WHERE cut_from_id IS NOT NULL;

COMMENT ON COLUMN package_type.cut_from_id IS
    'D252. The full box this one is cut down from: its length and width, its own '
    'height. Its empty weight and limit are the full box''s unless it says its own. '
    'One level: a cut is not cut again (J82).';

-- What a box type says, a cut's gaps filled from its full box: the one reading
-- the bench, the suggestion and the booking share.
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
       coalesce(t.cut_from_id, t.id) AS pictured_as
  FROM package_type t
  LEFT JOIN package_type f ON f.id = t.cut_from_id;

COMMENT ON VIEW package_type_effective IS
    'D252. A box type with a cut''s empty weight and limit taken from its full box '
    'unless it says its own, whether it carries cartons, whether its height is '
    'set, and whose photographs it wears.';

GRANT SELECT ON package_type_effective TO spork_app;

-- ---------------------------------------------------------------------------
-- The package-type arm, declared by migration 7
-- ---------------------------------------------------------------------------

-- A box type the workspace owns: one the platform ships is nobody's to
-- photograph, and its null tenant matches no observable's.
ALTER TABLE package_type
    ADD CONSTRAINT package_type_tenant_key UNIQUE (id, tenant_id);

ALTER TABLE observable
    ADD COLUMN package_type_id uuid,
    ADD CONSTRAINT observable_package_type_fk
        FOREIGN KEY (package_type_id, tenant_id)
        REFERENCES package_type (id, tenant_id);

-- Every arm, as migration 82 says.
ALTER TABLE observable DROP CONSTRAINT observable_one_arm_ck;
ALTER TABLE observable ADD CONSTRAINT observable_one_arm_ck
    CHECK (num_nonnulls(item_id, item_style_id, item_part_id, package_id, lot_id,
                        location_id, asserted_unit_id, asserted_unit_content_id,
                        package_type_id) = 1);

ALTER TABLE observable DROP COLUMN kind;
ALTER TABLE observable ADD COLUMN kind text GENERATED ALWAYS AS (
    CASE WHEN item_id IS NOT NULL THEN 'item'
         WHEN item_style_id IS NOT NULL THEN 'item_style'
         WHEN item_part_id IS NOT NULL THEN 'item_part'
         WHEN package_id IS NOT NULL THEN 'package'
         WHEN lot_id IS NOT NULL THEN 'lot'
         WHEN location_id IS NOT NULL THEN 'location'
         WHEN asserted_unit_id IS NOT NULL THEN 'asserted_unit'
         WHEN asserted_unit_content_id IS NOT NULL THEN 'asserted_unit_content'
         WHEN package_type_id IS NOT NULL THEN 'package_type' END) STORED;

CREATE UNIQUE INDEX observable_package_type_idx ON observable (tenant_id, package_type_id)
    WHERE package_type_id IS NOT NULL;

COMMENT ON COLUMN observable.package_type_id IS
    'D252. A box type, photographed so a carton of it is drawn as it looks. '
    'Photographs only: nothing is measured of a box type here.';

-- S45: a column the application cannot write is a column nothing can fill.
GRANT INSERT (package_type_id), UPDATE (package_type_id) ON observable TO spork_app;
