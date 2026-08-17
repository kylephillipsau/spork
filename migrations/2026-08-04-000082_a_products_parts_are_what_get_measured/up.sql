-- Migration 82: a product's parts are what get measured when the product has
-- no shape.
--
-- D139. The Oates lobby pan set is one sellable code, `PN 165918`, and two
-- physical objects: a moulded pan and a 1200mm aluminium handle. They have
-- different weights, different sizes, and they go into a box independently of
-- each other. **The set has no bounding box.** The one you would measure is
-- whatever the pair happened to make on the bench, and it is mostly air.
--
-- D138 gave the each an honest way to say so — `not_applicable`, recorded rather
-- than left blank. This is the other half: the sizes have to live somewhere, and
-- there was nowhere to put them, because every measurable subject in this
-- database is an item, a style, a package, a lot, a location or a unit somebody
-- asserted, and a handle is none of those.
--
-- # The sixth arm, which is what the registry is for
--
-- D23 put the subject union on `observable` rather than on the fact tables so
-- that widening it is "one column on a table of about 10^5 rows and zero change
-- to anything holding 10^7", and said the remaining arms would "each arrive as
-- one column in the migration that creates its target". Migration 73 was the
-- first. This is the second, and it is the same shape for the same reason: a
-- style is measured and not sold, and a part is measured and not sold.
--
-- # Why a part is not an item
--
-- The obvious build makes the pan and the handle `item` rows and joins them with
-- a composition table. It is how most systems do it, and it is wrong here today.
--
-- An `item` is a thing that is stocked, counted, allocated, picked and ordered.
-- Minting two of them for one product creates stock cells nobody counts, a
-- catalogue with codes that cannot be sold, and an immediate question with no
-- good answer: does receiving ten sets put ten sets on hand, or ten pans and ten
-- handles? **A part is a measurable constituent of a sellable thing, not a
-- sellable thing**, and that is a smaller claim that happens to be true.
--
-- **The trigger for promoting parts to items is stated so nobody has to guess
-- it:** the day a part is stocked, picked or sold on its own — a replacement
-- handle going out alone, a pan counted separately in a stocktake — it has
-- stopped being a part and become an item, and this table stops being the right
-- home. That change is a migration, and it should be, because it is a change in
-- what the thing is.
--
-- Recorded plainly because it departs from how the case was described to me:
-- these were called "two separate items". They are two separate *things*, and
-- they are one item until one of them moves on its own.
--
-- # No packaging level
--
-- `observable`'s item arm carries a level because a carton of boots and a pair
-- of boots are different subjects. A part has no level: there is one handle, and
-- until handles arrive in a carton of handles there is no such object to
-- measure. `observable_item_level_ck` already ties the level to the item and
-- style arms only, so this needs no change and gets none — the constraint says
-- the right thing about an arm written after it.
--
-- The trigger, again stated: parts arriving in their own cartons. That is the
-- same event as the promotion trigger above, and it is not a coincidence.
--
-- # What cartonisation gets, and what it does not
--
-- Two parts with real sizes, and a parent that says it has no box. What it does
-- not get is a rule for arranging them, because nothing is packing anything yet.
-- Building a nesting model now would be building against no caller. D67 declined
-- partitioning on the same ground.

CREATE TABLE item_part (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id    uuid NOT NULL REFERENCES tenant(id),
    item_id      uuid NOT NULL,
    code         text NOT NULL,
    label        text NOT NULL,
    -- How many of this part are in one of the parent. A set with two identical
    -- brackets is one part row with `quantity_per = 2`, not two rows: they are
    -- the same object measured once, and measuring one of them twice is the
    -- lie D108 exists to prevent, one level down.
    quantity_per integer NOT NULL DEFAULT 1,
    ordinal      integer NOT NULL,
    CONSTRAINT item_part_item_fk FOREIGN KEY (item_id, tenant_id)
        REFERENCES item (id, tenant_id),
    CONSTRAINT item_part_quantity_ck CHECK (quantity_per > 0),
    CONSTRAINT item_part_code_key UNIQUE (tenant_id, item_id, code),
    CONSTRAINT item_part_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON TABLE item_part IS
    'A physical constituent of a sellable item that is measured on its own -- a '
    'pan and its handle under one code. Not an item: a part is not stocked, '
    'counted, allocated or picked, and the day one of those becomes true it has '
    'become an item and this row has stopped being the right home. D139, '
    'migration 82.';

CREATE INDEX item_part_item_idx ON item_part (tenant_id, item_id);

ALTER TABLE item_part ENABLE ROW LEVEL SECURITY;
ALTER TABLE item_part FORCE ROW LEVEL SECURITY;
CREATE POLICY item_part_tenant_scoped ON item_part
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());
GRANT SELECT, INSERT, UPDATE, DELETE ON item_part TO nylonite_app;

-- ---------------------------------------------------------------------------
-- The sixth arm
-- ---------------------------------------------------------------------------

ALTER TABLE observable
    ADD COLUMN item_part_id uuid,
    ADD CONSTRAINT observable_item_part_fk
        FOREIGN KEY (item_part_id, tenant_id)
        REFERENCES item_part (id, tenant_id);

-- Every arm, not the ones this migration happens to care about. Migration 73
-- learned that restating the constraint from an older migration's text silently
-- drops the arms added since, and the fixture caught it.
ALTER TABLE observable DROP CONSTRAINT observable_one_arm_ck;
ALTER TABLE observable ADD CONSTRAINT observable_one_arm_ck
    CHECK (num_nonnulls(item_id, item_style_id, item_part_id, package_id, lot_id,
                        location_id, asserted_unit_id, asserted_unit_content_id) = 1);

-- A stored generated expression cannot be altered in place.
ALTER TABLE observable DROP COLUMN kind;
ALTER TABLE observable ADD COLUMN kind text GENERATED ALWAYS AS (
    CASE WHEN item_id IS NOT NULL THEN 'item'
         WHEN item_style_id IS NOT NULL THEN 'item_style'
         WHEN item_part_id IS NOT NULL THEN 'item_part'
         WHEN package_id IS NOT NULL THEN 'package'
         WHEN lot_id IS NOT NULL THEN 'lot'
         WHEN location_id IS NOT NULL THEN 'location'
         WHEN asserted_unit_id IS NOT NULL THEN 'asserted_unit'
         WHEN asserted_unit_content_id IS NOT NULL
             THEN 'asserted_unit_content' END) STORED;

CREATE UNIQUE INDEX observable_item_part_idx ON observable (tenant_id, item_part_id)
    WHERE item_part_id IS NOT NULL;

-- S45: a column the application cannot write is a column nothing can fill.
GRANT INSERT (item_part_id), UPDATE (item_part_id) ON observable TO nylonite_app;

-- The metrics that may be asserted about a part are the ones that may be
-- asserted about an item: it is the same physical claim about a smaller object.
UPDATE metric
   SET applies_to = applies_to || ARRAY['item_part']
 WHERE reserved AND 'item' = ANY(applies_to);

DO $$
DECLARE n integer;
BEGIN
    SELECT count(*) INTO n FROM metric WHERE 'item_part' = ANY(applies_to);
    IF n <> 6 THEN
        RAISE EXCEPTION 'expected 6 metrics to widen to item_part, got %', n;
    END IF;
END
$$;
