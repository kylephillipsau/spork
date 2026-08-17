-- Migration 82 down: a product's parts stop being things that can be measured.
--
-- The observations against them go with the observables that name them, and
-- there is nowhere else those sizes could have been recorded — the whole reason
-- for the arm is that a handle is not an item, a style, a package, a lot or a
-- location.

DO $$
DECLARE parts bigint; observed bigint;
BEGIN
    SELECT count(*) INTO parts FROM item_part;
    SELECT count(*) INTO observed
      FROM observation o JOIN observable ob ON ob.id = o.observable_id
     WHERE ob.item_part_id IS NOT NULL;
    IF parts > 0 OR observed > 0 THEN
        RAISE NOTICE 'reversing D139 destroys % part(s) and % observation(s) about them, and no other subject can hold those sizes', parts, observed;
    END IF;
END
$$;

DELETE FROM observation o
 USING observable ob
 WHERE ob.id = o.observable_id AND ob.item_part_id IS NOT NULL;

DELETE FROM observation_event e
 USING observable ob
 WHERE ob.id = e.observable_id AND ob.item_part_id IS NOT NULL;

DELETE FROM observable WHERE item_part_id IS NOT NULL;

UPDATE metric
   SET applies_to = array_remove(applies_to, 'item_part')
 WHERE 'item_part' = ANY(applies_to);

DROP INDEX observable_item_part_idx;

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

ALTER TABLE observable DROP CONSTRAINT observable_one_arm_ck;
ALTER TABLE observable ADD CONSTRAINT observable_one_arm_ck
    CHECK (num_nonnulls(item_id, item_style_id, package_id, lot_id, location_id,
                        asserted_unit_id, asserted_unit_content_id) = 1);

ALTER TABLE observable DROP COLUMN item_part_id;

DROP TABLE item_part;
