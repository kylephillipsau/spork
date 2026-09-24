-- Migration 73: a style is what gets measured, and a SKU inherits it.
--
-- The prepack list measures `SKU-0180`. There is no such item. The catalogue
-- sells `SKU-0180-S`, `-M`, `-L` and `-XL`, and one carton spec covers all four
-- — 58 measured rows standing for 252 sellable codes. The gumboot is the same
-- shape: `STY-7720` in thirteen sizes, `-03` through `-13`, one box.
--
-- **The codes are invented and the counts are not.** 58 and 252 are what the
-- real file holds; the two codes stand in for a customer's, which do not belong
-- in this repository. What they stand for — a style with sized variants sharing
-- one carton — is the whole of what this migration is about.
--
-- **Writing the style's numbers onto every variant would be a lie about how many
-- measurements exist.** Four rows claiming to be measured when one carton was
-- put on a scale, and no way afterwards to tell which one that was — or to
-- record that size 13 turned out not to fit. That is the shape of error this
-- project keeps finding: a fact copied to where it is convenient, and then
-- indistinguishable from a fact observed there.
--
-- So the style is a thing that can be observed, and a SKU that has no
-- measurement of its own inherits its style's.
--
-- # Most specific wins, which is not a new idea here
--
-- D22 resolves policy by walking a lattice and taking the most specific match.
-- This is that principle at one level: an observation against the SKU beats an
-- observation against its style, because somebody measured *this* one. The
-- resolution is two rows deep and needs no closure table, which is why this is a
-- nullable FK and not a second taxonomy.
--
-- # Why not `item_class`
--
-- The taxonomy already exists, with a closure table and a resolver over it. A
-- style is not a class. `item_class` answers *what kind of thing is this* and
-- feeds D22's policy lattice; a style answers *which codes share a carton*.
-- Filing 58 styles as classes would put packaging facts in the structure that
-- decides receiving tolerance and shelf life, and every future policy binding
-- would have to be written to avoid them. Two different questions, two
-- structures.
--
-- # Nullable, and no backfill
--
-- Most items are not part of a style, and an item whose style is unknown is the
-- ordinary case rather than an error. Nothing here infers a style from a code
-- prefix: `SKU-0180-L` looking like a variant of `SKU-0180` is a convention this
-- database has never been told about, and guessing it in a migration would make
-- it true by assertion. The loader proposes; a person decides.

CREATE TABLE item_style (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id   uuid NOT NULL REFERENCES tenant(id),
    code        text NOT NULL,
    description text,
    CONSTRAINT item_style_code_key UNIQUE (tenant_id, code),
    CONSTRAINT item_style_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON TABLE item_style IS
    'A group of item codes that share physical packaging -- one carton spec '
    'across every size of a garment or boot. Not a taxonomy: item_class answers '
    'what kind of thing this is and feeds D22''s policy lattice, and this '
    'answers which codes ship in the same box. Migration 73.';

ALTER TABLE item_style ENABLE ROW LEVEL SECURITY;
ALTER TABLE item_style FORCE ROW LEVEL SECURITY;
CREATE POLICY item_style_tenant_scoped ON item_style
    USING (tenant_id = current_tenant());
GRANT SELECT, INSERT, UPDATE, DELETE ON item_style TO spork_app;

ALTER TABLE item
    ADD COLUMN style_id uuid,
    ADD CONSTRAINT item_style_fk FOREIGN KEY (style_id, tenant_id)
        REFERENCES item_style (id, tenant_id);

COMMENT ON COLUMN item.style_id IS
    'The style whose measurements this code inherits when it has none of its '
    'own. Nullable: most items belong to no style, and an unknown style is the '
    'ordinary case. Migration 73.';

CREATE INDEX item_style_idx ON item (tenant_id, style_id) WHERE style_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- The fourth arm
-- ---------------------------------------------------------------------------
--
-- `observable` said of itself that the remaining arms "each arrive as one column
-- in the migration that creates its target, which is the point of putting the
-- union here rather than on the fact." This is that, for the first time.
--
-- The level constraints widen from "item" to "item or style", because a style is
-- measured at a packaging level exactly as an item is: a carton of SKU-0180 is a
-- definite object relative to a case pack, and a single one is not.

ALTER TABLE observable
    ADD COLUMN item_style_id uuid,
    ADD CONSTRAINT observable_item_style_fk
        FOREIGN KEY (item_style_id, tenant_id)
        REFERENCES item_style (id, tenant_id);

-- **Every arm, not the four this table was born with.** `asserted_unit` and
-- `asserted_unit_content` arrived in later migrations, exactly as the original
-- comment said they would. Restating the constraint from migration 7's text
-- would silently drop them, and the fixture caught it: a row naming an asserted
-- unit stopped satisfying a check that had never stopped being true.
ALTER TABLE observable DROP CONSTRAINT observable_one_arm_ck;
ALTER TABLE observable ADD CONSTRAINT observable_one_arm_ck
    CHECK (num_nonnulls(item_id, item_style_id, package_id, lot_id, location_id,
                        asserted_unit_id, asserted_unit_content_id) = 1);

ALTER TABLE observable DROP CONSTRAINT observable_item_level_ck;
ALTER TABLE observable ADD CONSTRAINT observable_item_level_ck
    CHECK ((item_id IS NOT NULL OR item_style_id IS NOT NULL)
           = (packaging_level IS NOT NULL));

ALTER TABLE observable DROP CONSTRAINT observable_item_config_ck;
ALTER TABLE observable ADD CONSTRAINT observable_item_config_ck
    CHECK ((item_id IS NULL AND item_style_id IS NULL)
           OR packaging_level = 'each'
           OR item_packing_config_id IS NOT NULL);

-- The generated column has to be rebuilt rather than altered: a stored
-- generated expression cannot be changed in place.
ALTER TABLE observable DROP COLUMN kind;
ALTER TABLE observable ADD COLUMN kind text GENERATED ALWAYS AS (
    CASE WHEN item_id IS NOT NULL THEN 'item'
         WHEN item_style_id IS NOT NULL THEN 'item_style'
         WHEN package_id IS NOT NULL THEN 'package'
         WHEN lot_id IS NOT NULL THEN 'lot'
         WHEN location_id IS NOT NULL THEN 'location'
         WHEN asserted_unit_id IS NOT NULL THEN 'asserted_unit'
         WHEN asserted_unit_content_id IS NOT NULL
             THEN 'asserted_unit_content' END) STORED;

-- NULLS NOT DISTINCT for migration 72's reason: the packing config is
-- legitimately absent at `each`, and two absences are the same subject.
CREATE UNIQUE INDEX observable_item_style_idx ON observable
    (tenant_id, item_style_id, packaging_level, item_packing_config_id)
    NULLS NOT DISTINCT
    WHERE item_style_id IS NOT NULL;

-- S45: a column the application cannot write is a column nothing can fill.
GRANT INSERT (item_style_id), UPDATE (item_style_id) ON observable TO spork_app;
GRANT INSERT (style_id), UPDATE (style_id) ON item TO spork_app;

-- The metrics that may be asserted about a style are the ones that may be
-- asserted about an item: it is the same physical claim, made one level up.
UPDATE metric
   SET applies_to = applies_to || ARRAY['item_style']
 WHERE reserved AND 'item' = ANY(applies_to);

DO $$
DECLARE n integer;
BEGIN
    SELECT count(*) INTO n FROM metric WHERE 'item_style' = ANY(applies_to);
    IF n <> 6 THEN
        RAISE EXCEPTION 'expected 6 metrics to widen to item_style, got %', n;
    END IF;
END
$$;
