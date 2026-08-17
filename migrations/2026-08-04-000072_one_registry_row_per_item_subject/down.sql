-- Migration 72 down: the item arm goes back to letting two `each` subjects exist.

DROP INDEX IF EXISTS observable_item_idx;

CREATE UNIQUE INDEX observable_item_idx ON observable
    (tenant_id, item_id, packaging_level, item_packing_config_id)
    WHERE item_id IS NOT NULL;

-- Removing a unit removes the ability to quote back what somebody wrote down.
--
-- `observation.entered_unit_id` is ON DELETE NO ACTION, so the delete below
-- would fail rather than orphan. Migration 65 set the precedent for this shape:
-- clear the references, and say out loud what reversing costs before doing it.
-- The canonical `value_numeric` survives untouched — that is the whole reason
-- Principle 5 stores it — so nothing becomes wrong, but a carton whose supplier
-- quoted it as 45 can no longer be shown as 45.
DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM observation
     WHERE entered_unit_id = 'f1000000-0000-0000-0000-000000000015';
    IF n > 0 THEN
        UPDATE observation SET entered_value = NULL, entered_unit_id = NULL
         WHERE entered_unit_id = 'f1000000-0000-0000-0000-000000000015';
        RAISE NOTICE 'reversing migration 72 discards the entered form of % observation(s): '
                     'the canonical millimetres survive, the centimetres somebody '
                     'wrote down do not', n;
    END IF;
END $$;

DELETE FROM unit WHERE id = 'f1000000-0000-0000-0000-000000000015';
