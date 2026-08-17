-- Migration 73 down: a measurement can only name a SKU again.
--
-- Observations against a style have nowhere to go: writing them onto every
-- variant is exactly the claim this migration exists to avoid making. So they
-- are removed with the subject, and the NOTICE says how many facts that costs.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM observation o
     WHERE EXISTS (SELECT 1 FROM observable s
                    WHERE s.id = o.observable_id AND s.item_style_id IS NOT NULL);
    IF n > 0 THEN
        RAISE NOTICE 'reversing migration 73 discards % observation(s) about a style: '
                     'they cannot be written onto the variants without claiming '
                     'each one was measured', n;
    END IF;
END $$;

DELETE FROM observation_current
 WHERE observable_id IN (SELECT id FROM observable WHERE item_style_id IS NOT NULL);
DELETE FROM observation
 WHERE observable_id IN (SELECT id FROM observable WHERE item_style_id IS NOT NULL);
DELETE FROM observation_event
 WHERE observable_id IN (SELECT id FROM observable WHERE item_style_id IS NOT NULL);
DELETE FROM observable WHERE item_style_id IS NOT NULL;

UPDATE metric
   SET applies_to = array_remove(applies_to, 'item_style')
 WHERE 'item_style' = ANY(applies_to);

DROP INDEX IF EXISTS observable_item_style_idx;

ALTER TABLE observable DROP COLUMN kind;
ALTER TABLE observable ADD COLUMN kind text GENERATED ALWAYS AS (
    CASE WHEN item_id IS NOT NULL THEN 'item'
         WHEN package_id IS NOT NULL THEN 'package'
         WHEN lot_id IS NOT NULL THEN 'lot'
         WHEN location_id IS NOT NULL THEN 'location'
         WHEN asserted_unit_id IS NOT NULL THEN 'asserted_unit'
         WHEN asserted_unit_content_id IS NOT NULL
             THEN 'asserted_unit_content' END) STORED;

ALTER TABLE observable DROP CONSTRAINT observable_item_config_ck;
ALTER TABLE observable ADD CONSTRAINT observable_item_config_ck
    CHECK (item_id IS NULL OR packaging_level = 'each'
           OR item_packing_config_id IS NOT NULL);

ALTER TABLE observable DROP CONSTRAINT observable_item_level_ck;
ALTER TABLE observable ADD CONSTRAINT observable_item_level_ck
    CHECK ((item_id IS NOT NULL) = (packaging_level IS NOT NULL));

ALTER TABLE observable DROP CONSTRAINT observable_one_arm_ck;
ALTER TABLE observable ADD CONSTRAINT observable_one_arm_ck
    CHECK (num_nonnulls(item_id, package_id, lot_id, location_id,
                        asserted_unit_id, asserted_unit_content_id) = 1);

ALTER TABLE observable
    DROP CONSTRAINT observable_item_style_fk,
    DROP COLUMN item_style_id;

DROP INDEX IF EXISTS item_style_idx;
ALTER TABLE item DROP CONSTRAINT item_style_fk, DROP COLUMN style_id;

DROP POLICY IF EXISTS item_style_tenant_scoped ON item_style;
DROP TABLE IF EXISTS item_style;
