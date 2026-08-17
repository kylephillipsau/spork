-- Migration 72: one registry row per item subject, including an each.
--
-- `observable` says of itself that *"a partial unique index per arm makes the
-- second observer of a pallet find the first one's row rather than mint a
-- rival."* The item arm does not do that, and could not have: it is keyed on
-- four columns and `item_packing_config_id` is null for the one level that does
-- not need a case pack to be a definite thing.
--
--     CREATE UNIQUE INDEX observable_item_idx ON observable
--         (tenant_id, item_id, packaging_level, item_packing_config_id)
--         WHERE item_id IS NOT NULL;
--
-- Under the default NULLS DISTINCT, two rows both naming (alpha, gloves, each,
-- null) do not conflict, because in SQL those nulls are not equal to each other.
-- So the index reads as though it enforces one row per subject and enforces one
-- row per subject *except at `each`* -- and `each` is the level everything a
-- person picks up is measured at.
--
-- Nothing has hit it because nothing has ever written an item observation: the
-- `/observations` handler says in as many words that item subjects are
-- deliberately absent because no screen needed one. A prepack list is item
-- dimensions by the thousand, so one does now, and an `ON CONFLICT` against this
-- index would have silently minted a rival subject on every line.
--
-- **NULLS NOT DISTINCT is the fix rather than a sentinel uuid.** The null is
-- correct and load-bearing -- `observable_item_config_ck` requires it to be
-- absent at `each` and present above -- so the index should treat two absences
-- as the same absence, which is what this says.

DROP INDEX observable_item_idx;

CREATE UNIQUE INDEX observable_item_idx ON observable
    (tenant_id, item_id, packaging_level, item_packing_config_id)
    NULLS NOT DISTINCT
    WHERE item_id IS NOT NULL;

COMMENT ON INDEX observable_item_idx IS
    'One registry row per item subject. NULLS NOT DISTINCT because '
    'item_packing_config_id is legitimately absent at the each level, and two '
    'absences are the same subject rather than two of them. Migration 72.';

-- ---------------------------------------------------------------------------
-- Centimetres, because that is the unit a carton is quoted in
-- ---------------------------------------------------------------------------
--
-- Migration 7 shipped mm, m and in. A prepack list gives a carton as 45 by 34 by
-- 41, and without cm the only ways to load that are to convert it before it
-- reaches the database -- which is what `entered_value` and `entered_unit_id`
-- exist to make unnecessary -- or to record 45 as though it were millimetres.
--
-- Exact rational, never a float: a centimetre is 10/1 millimetres and it is that
-- exactly. S22 is unaffected, which is about the *canonical* unit of each
-- dimension having factor 1/1; mm keeps that job.

INSERT INTO unit (id, dimension_id, code, ucum_code, factor_num, factor_den,
                  offset_num, offset_den)
SELECT 'f1000000-0000-0000-0000-000000000015', d.id, 'cm', 'cm', 10, 1, 0, 1
  FROM dimension d WHERE d.code = 'length';

-- A seed that joins can insert nothing and report success. Migration 7's own
-- metric seed did exactly that once, so this one says so out loud.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM unit WHERE code = 'cm') THEN
        RAISE EXCEPTION 'the cm seed inserted nothing: is there a length dimension?';
    END IF;
END
$$;
