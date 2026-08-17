-- Reverse of 2026-08-04-000033_taxonomy_history.
--
-- Creation acts go, and with them the base state that made the taxonomy
-- reconstructible. The classes keep the parentage the fold left on them, which is
-- the present shape and only the present shape -- the state migration 33 was
-- written to end.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM policy_change
     WHERE change_kind = 'created'
       AND (item_class_id IS NOT NULL OR party_class_id IS NOT NULL);
    IF n > 0 THEN
        RAISE NOTICE 'reversing D76 destroys % class origin(s): the taxonomy stops being '
                     'reconstructible before its first move', n;
    END IF;
END $$;

DROP FUNCTION IF EXISTS item_class_closure_as_at(uuid, timestamptz, timestamptz);
DROP FUNCTION IF EXISTS party_class_closure_as_at(uuid, timestamptz, timestamptz);

DELETE FROM policy_change
 WHERE change_kind = 'created'
   AND (item_class_id IS NOT NULL OR party_class_id IS NOT NULL);

CREATE OR REPLACE FUNCTION projection_taxonomy_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
    n bigint;
    total bigint := 0;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH folded AS (
        SELECT DISTINCT ON (item_class_id)
               item_class_id, new_item_parent_id AS new_parent_id
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind = 'reparented'
           AND item_class_id IS NOT NULL
         ORDER BY item_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE item_class c
           SET parent_id = f.new_parent_id
          FROM folded f
         WHERE c.id = f.item_class_id AND c.tenant_id = p_tenant
           AND c.parent_id IS DISTINCT FROM f.new_parent_id
        RETURNING c.id)
    SELECT count(*) INTO touched FROM updated;
    total := total + touched;

    WITH folded AS (
        SELECT DISTINCT ON (party_class_id)
               party_class_id, new_party_parent_id AS new_parent_id
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind = 'reparented'
           AND party_class_id IS NOT NULL
         ORDER BY party_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE party_class c
           SET parent_id = f.new_parent_id
          FROM folded f
         WHERE c.id = f.party_class_id AND c.tenant_id = p_tenant
           AND c.parent_id IS DISTINCT FROM f.new_parent_id
        RETURNING c.id)
    SELECT count(*) INTO n FROM updated;
    total := total + n;

    WITH folded AS (
        SELECT DISTINCT ON (item_class_id)
               item_class_id,
               CASE WHEN change_kind = 'retired' THEN occurred_at END AS retired_at
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('retired', 'reinstated')
           AND item_class_id IS NOT NULL
         ORDER BY item_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE item_class c
           SET retired_at = f.retired_at
          FROM folded f
         WHERE c.id = f.item_class_id AND c.tenant_id = p_tenant
           AND c.retired_at IS DISTINCT FROM f.retired_at
        RETURNING c.id)
    SELECT count(*) INTO n FROM updated;
    total := total + n;

    WITH folded AS (
        SELECT DISTINCT ON (party_class_id)
               party_class_id,
               CASE WHEN change_kind = 'retired' THEN occurred_at END AS retired_at
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('retired', 'reinstated')
           AND party_class_id IS NOT NULL
         ORDER BY party_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE party_class c
           SET retired_at = f.retired_at
          FROM folded f
         WHERE c.id = f.party_class_id AND c.tenant_id = p_tenant
           AND c.retired_at IS DISTINCT FROM f.retired_at
        RETURNING c.id)
    SELECT count(*) INTO n FROM updated;
    total := total + n;

    RETURN total;
END
$$;

ALTER FUNCTION projection_taxonomy_rebuild(uuid) OWNER TO nylonite_projection_owner;

ALTER TABLE policy_change
    DROP CONSTRAINT IF EXISTS policy_change_parent_only_on_placement_ck;

ALTER TABLE policy_change
    ADD CONSTRAINT policy_change_parent_only_on_reparent_ck
        CHECK ((new_item_parent_id IS NULL AND new_party_parent_id IS NULL)
               OR change_kind = 'reparented');
