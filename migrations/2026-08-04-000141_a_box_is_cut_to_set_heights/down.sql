-- Migration 141 down: boxes are not cut, and box types are not photographed.
--
-- The photographs of box types go with the observables that name them: no
-- other subject is a box type. Cut boxes stay as boxes of their own, unlinked,
-- as the half box was before.

DO $$
DECLARE photos bigint; cuts bigint;
BEGIN
    SELECT count(*) INTO photos
      FROM observation_image oi
      JOIN observation_event e ON e.id = oi.observation_event_id
      JOIN observable ob ON ob.id = e.observable_id
     WHERE ob.package_type_id IS NOT NULL;
    SELECT count(*) INTO cuts FROM package_type WHERE cut_from_id IS NOT NULL;
    IF photos > 0 OR cuts > 0 THEN
        RAISE NOTICE 'reversing D252 destroys % photograph(s) of box types and unlinks % cut box(es) from their full box', photos, cuts;
    END IF;
END
$$;

CREATE TEMPORARY TABLE box_type_event AS
SELECT e.id
  FROM observation_event e
  JOIN observable ob ON ob.id = e.observable_id
 WHERE ob.package_type_id IS NOT NULL;

DELETE FROM observation_image_move mv
 USING observation_image oi
 WHERE oi.observation_event_id IN (SELECT id FROM box_type_event)
   AND oi.id IN (mv.observation_image_id, mv.moved_to_image_id);
DELETE FROM observation_image_cut c
 USING observation_image oi
 WHERE oi.id = c.observation_image_id
   AND oi.observation_event_id IN (SELECT id FROM box_type_event);
DELETE FROM observation_image
 WHERE observation_event_id IN (SELECT id FROM box_type_event);
DELETE FROM evidence WHERE observation_event_id IN (SELECT id FROM box_type_event);
DELETE FROM observation WHERE observation_event_id IN (SELECT id FROM box_type_event);
DELETE FROM observation_current oc
 USING observable ob
 WHERE ob.id = oc.observable_id AND ob.package_type_id IS NOT NULL;
DELETE FROM observation_event WHERE id IN (SELECT id FROM box_type_event);
DELETE FROM observable WHERE package_type_id IS NOT NULL;
DROP TABLE box_type_event;

DROP INDEX observable_package_type_idx;

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

ALTER TABLE observable DROP CONSTRAINT observable_one_arm_ck;
ALTER TABLE observable ADD CONSTRAINT observable_one_arm_ck
    CHECK (num_nonnulls(item_id, item_style_id, item_part_id, package_id, lot_id,
                        location_id, asserted_unit_id, asserted_unit_content_id) = 1);

ALTER TABLE observable DROP COLUMN package_type_id;
ALTER TABLE package_type DROP CONSTRAINT package_type_tenant_key;

DROP VIEW package_type_effective;

DROP INDEX package_type_one_cut_a_height;
ALTER TABLE package_type DROP CONSTRAINT package_type_cut_is_sized_ck;
ALTER TABLE package_type DROP CONSTRAINT package_type_cut_not_itself_ck;
ALTER TABLE package_type DROP CONSTRAINT package_type_cut_from_fk;
ALTER TABLE package_type DROP COLUMN cut_from_id;
ALTER TABLE package_type DROP CONSTRAINT package_type_footprint_key;
