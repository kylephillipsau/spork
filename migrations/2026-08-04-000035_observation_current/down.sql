-- Reverse of 2026-08-04-000035_observation_current.
--
-- The projection is derived and costs nothing to lose. The acceptances are not:
-- each one is a recorded decision to adopt a counterparty's number as ours, and
-- no rebuild can reconstruct who agreed to it or why.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM observation_acceptance;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D78 destroys % acceptance(s): the observations survive, '
                     'our decision to trust them does not', n;
    END IF;
END $$;

DELETE FROM projection_rebuild
 WHERE function_name = 'projection_observation_current_rebuild';
DELETE FROM projection_step
 WHERE function_name = 'projection_observation_current_rebuild';
DROP FUNCTION IF EXISTS projection_observation_current_rebuild(uuid);

DROP TABLE IF EXISTS observation_current;
DROP TABLE IF EXISTS observation_acceptance;

ALTER TABLE observation DROP CONSTRAINT IF EXISTS observation_tenant_key;

-- The registry narrows again, and **that makes some observations unrepresentable**:
-- a measurement whose subject is a declared pallet has nowhere to point once the
-- arm is gone. Deleted rather than orphaned, which is the same choice migration
-- 30's reversal makes about re-parenting acts, and it is worth a notice because a
-- supplier's declared weight is evidence.
DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM observable
     WHERE asserted_unit_id IS NOT NULL OR asserted_unit_content_id IS NOT NULL;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D78 destroys the observations about % declared unit(s): '
                     'the claims survive, what anyone measured about them does not', n;
    END IF;
END $$;

DELETE FROM observation
 WHERE observable_id IN (SELECT id FROM observable
                          WHERE asserted_unit_id IS NOT NULL
                             OR asserted_unit_content_id IS NOT NULL);
DELETE FROM observation_event
 WHERE observable_id IN (SELECT id FROM observable
                          WHERE asserted_unit_id IS NOT NULL
                             OR asserted_unit_content_id IS NOT NULL);
DELETE FROM observable
 WHERE asserted_unit_id IS NOT NULL OR asserted_unit_content_id IS NOT NULL;

-- The registry narrows again, and the generated column goes back with it.
DROP INDEX IF EXISTS observable_asserted_unit_idx;
DROP INDEX IF EXISTS observable_asserted_unit_content_idx;

ALTER TABLE observable DROP CONSTRAINT IF EXISTS observable_one_arm_ck;
ALTER TABLE observable DROP COLUMN IF EXISTS kind;
ALTER TABLE observable
    DROP CONSTRAINT IF EXISTS observable_asserted_unit_fk,
    DROP CONSTRAINT IF EXISTS observable_asserted_unit_content_fk;
ALTER TABLE observable
    DROP COLUMN IF EXISTS asserted_unit_id,
    DROP COLUMN IF EXISTS asserted_unit_content_id;

ALTER TABLE observable ADD COLUMN kind text GENERATED ALWAYS AS (
    CASE WHEN item_id IS NOT NULL THEN 'item'
         WHEN package_id IS NOT NULL THEN 'package'
         WHEN lot_id IS NOT NULL THEN 'lot'
         WHEN location_id IS NOT NULL THEN 'location' END) STORED;

ALTER TABLE observable
    ADD CONSTRAINT observable_one_arm_ck
        CHECK (num_nonnulls(item_id, package_id, lot_id, location_id) = 1);
