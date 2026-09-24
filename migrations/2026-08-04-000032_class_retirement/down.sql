-- Reverse of 2026-08-04-000032_class_retirement.
--
-- Restores D72's two-column fold. The retirement acts themselves are ordinary
-- `policy_change` rows and survive; what goes is the projection they were folded
-- onto, so a retired class comes back indistinguishable from a live one.

DO $$
DECLARE n bigint;
BEGIN
    SELECT (SELECT count(*) FROM item_class  WHERE retired_at IS NOT NULL)
         + (SELECT count(*) FROM party_class WHERE retired_at IS NOT NULL)
      INTO n;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D74 returns % retired class(es) to the pickers: the acts '
                     'survive in policy_change, the state folded from them does not', n;
    END IF;
END $$;

DROP FUNCTION IF EXISTS item_class_retire_impact(uuid);
DROP FUNCTION IF EXISTS party_class_retire_impact(uuid);

DELETE FROM projection_rebuild
 WHERE function_name = 'projection_taxonomy_rebuild'
   AND column_name = 'retired_at';

CREATE OR REPLACE FUNCTION projection_taxonomy_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
    n bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

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

    RETURN touched + n;
END
$$;

ALTER FUNCTION projection_taxonomy_rebuild(uuid) OWNER TO spork_projection_owner;

ALTER TABLE item_class  DROP COLUMN IF EXISTS retired_at;
ALTER TABLE party_class DROP COLUMN IF EXISTS retired_at;
