-- Reverse of 2026-08-04-000030_taxonomy_change.
--
-- One thing here does not reverse, and saying so is the point of the comment:
-- **`ALTER TYPE ... ADD VALUE` has no inverse in Postgres.** An enum label cannot
-- be dropped once added, because rows elsewhere may already be typed by it and the
-- catalogue keeps no reference count. So `policy_change_kind` keeps `reparented`
-- after this runs.
--
-- That is survivable in the direction that matters: `up.sql` adds the label with
-- `IF NOT EXISTS`, so a re-apply after a down is clean. It is worth naming because
-- the down/up cycle is a *test* here (phase 4 of verify-migrations.sh runs it with
-- the fixture loaded), and a test that quietly leaves residue teaches the wrong
-- lesson about what reversible means.

-- ---------------------------------------------------------------------------
-- 1. The rows this migration made expressible
-- ---------------------------------------------------------------------------
--
-- A row with a taxonomy subject has no policy_binding_id and no kind, which is
-- exactly what the CHECK constraints below are about to forbid again. They have
-- to go before NOT NULL comes back, and going means the taxonomy loses the record
-- of its own edits -- the shape itself survives, because it is sitting in
-- parent_id where the fold left it.
--
-- **Keyed on the subject rather than on the change kind**, which is the correction
-- D74 forced. This first read `change_kind = 'reparented'`, and that was right
-- only while re-parenting was the sole taxonomy act; D74 pointed `retired` and
-- `reinstated` at a class as well, and those rows are equally unrepresentable in
-- the schema this reverts to. The subject is what migration 30 made possible, so
-- the subject is what its reversal must remove. Phase 4 of verify-migrations.sh
-- found it, which is the phase D56 added for exactly this.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM policy_change
     WHERE item_class_id IS NOT NULL OR party_class_id IS NOT NULL;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D72 destroys % taxonomy act(s): the shape survives in '
                     'parent_id, the actor, moment and reason do not', n;
    END IF;
END $$;

DELETE FROM policy_change
 WHERE item_class_id IS NOT NULL OR party_class_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 2. parent_id stops being a projection
-- ---------------------------------------------------------------------------

DELETE FROM projection_rebuild
 WHERE function_name = 'projection_taxonomy_rebuild';

DELETE FROM projection_step
 WHERE function_name = 'projection_taxonomy_rebuild';

DROP FUNCTION IF EXISTS projection_taxonomy_rebuild(uuid);
DROP FUNCTION IF EXISTS item_class_move_impact(uuid, uuid);
DROP FUNCTION IF EXISTS party_class_move_impact(uuid, uuid);

COMMENT ON COLUMN item_class.parent_id IS NULL;
COMMENT ON COLUMN party_class.parent_id IS NULL;

-- ---------------------------------------------------------------------------
-- 3. The application gets its write back
-- ---------------------------------------------------------------------------
--
-- Column-level grants and table-level grants are separate entries in the
-- catalogue, so the column grants have to be revoked by name rather than
-- overwritten by the table grant that follows -- the lesson J36 taught when a
-- surviving table-level DELETE kept `parent_id` writable through a REVOKE that
-- looked complete.

REVOKE INSERT (id, tenant_id, parent_id, code, name), UPDATE (code, name)
    ON item_class FROM spork_app;
REVOKE INSERT (id, tenant_id, parent_id, code, name), UPDATE (code, name)
    ON party_class FROM spork_app;

GRANT INSERT, UPDATE, DELETE ON item_class TO spork_app;
GRANT INSERT, UPDATE, DELETE ON party_class TO spork_app;

REVOKE SELECT, UPDATE ON item_class, party_class FROM spork_projection_owner;

-- ---------------------------------------------------------------------------
-- 4. policy_change goes back to being about bindings only
-- ---------------------------------------------------------------------------

DROP INDEX IF EXISTS policy_change_item_class_idx;
DROP INDEX IF EXISTS policy_change_party_class_idx;

ALTER TABLE policy_change
    DROP CONSTRAINT IF EXISTS policy_change_subject_ck,
    DROP CONSTRAINT IF EXISTS policy_change_kind_pairs_ck,
    DROP CONSTRAINT IF EXISTS policy_change_reparent_ck,
    DROP CONSTRAINT IF EXISTS policy_change_parent_only_on_reparent_ck,
    DROP CONSTRAINT IF EXISTS policy_change_parent_matches_subject_ck,
    DROP CONSTRAINT IF EXISTS policy_change_not_own_parent_ck;

ALTER TABLE policy_change
    DROP COLUMN IF EXISTS item_class_id,
    DROP COLUMN IF EXISTS party_class_id,
    DROP COLUMN IF EXISTS new_item_parent_id,
    DROP COLUMN IF EXISTS new_party_parent_id;

ALTER TABLE policy_change ALTER COLUMN policy_binding_id SET NOT NULL;
ALTER TABLE policy_change ALTER COLUMN kind SET NOT NULL;
