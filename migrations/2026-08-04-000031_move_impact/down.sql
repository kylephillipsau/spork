-- Reverse of 2026-08-04-000031_move_impact.
--
-- Restores D72's impact functions as they were, including the definition D73
-- found wrong -- a down migration returns the schema to a state, not to a better
-- state.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM policy_change WHERE affected_binding_count IS NOT NULL;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D73 destroys the blast radius recorded against % move(s): '
                     'it cannot be recomputed, because the taxonomy it was measured '
                     'against has already moved', n;
    END IF;
END $$;

ALTER TABLE policy_change
    DROP CONSTRAINT IF EXISTS policy_change_affected_count_ck;
ALTER TABLE policy_change
    DROP COLUMN IF EXISTS affected_binding_count;

DROP FUNCTION IF EXISTS item_class_move_impact(uuid, uuid, timestamptz);
DROP FUNCTION IF EXISTS party_class_move_impact(uuid, uuid, timestamptz);
DROP FUNCTION IF EXISTS item_class_move_affected(uuid, uuid, timestamptz);
DROP FUNCTION IF EXISTS party_class_move_affected(uuid, uuid, timestamptz);

CREATE FUNCTION item_class_move_impact(p_class_id uuid, p_new_parent_id uuid)
    RETURNS bigint
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    WITH old_anc AS (
        SELECT ancestor_id AS id FROM item_class_closure
         WHERE descendant_id = p_class_id AND ancestor_id <> p_class_id),
    new_anc AS (
        SELECT ancestor_id AS id FROM item_class_closure
         WHERE descendant_id = p_new_parent_id),
    gained AS (SELECT id FROM new_anc EXCEPT SELECT id FROM old_anc),
    lost   AS (SELECT id FROM old_anc EXCEPT SELECT id FROM new_anc)
    SELECT count(*) FROM policy_binding b
     WHERE b.item_class_id IN (SELECT id FROM gained UNION SELECT id FROM lost)
$$;

CREATE FUNCTION party_class_move_impact(p_class_id uuid, p_new_parent_id uuid)
    RETURNS bigint
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    WITH old_anc AS (
        SELECT ancestor_id AS id FROM party_class_closure
         WHERE descendant_id = p_class_id AND ancestor_id <> p_class_id),
    new_anc AS (
        SELECT ancestor_id AS id FROM party_class_closure
         WHERE descendant_id = p_new_parent_id),
    gained AS (SELECT id FROM new_anc EXCEPT SELECT id FROM old_anc),
    lost   AS (SELECT id FROM old_anc EXCEPT SELECT id FROM new_anc)
    SELECT count(*) FROM policy_binding b
     WHERE b.party_class_id IN (SELECT id FROM gained UNION SELECT id FROM lost)
$$;

COMMENT ON FUNCTION item_class_move_impact(uuid, uuid) IS
    'How many bindings start or stop reaching this subtree if the class moves to '
    'the given parent -- the symmetric difference of the old and new ancestor '
    'chains. NULL parent means the root. Question 78 asked for this number on the '
    'screen; it is derived so the screen cannot get it wrong. D72.';
COMMENT ON FUNCTION party_class_move_impact(uuid, uuid) IS
    'The same for the counterparty taxonomy. Separate from the item form because '
    'one function taking an untyped id answered 0 for the taxonomy it could not '
    'see. D72.';

GRANT EXECUTE ON FUNCTION item_class_move_impact(uuid, uuid),
    party_class_move_impact(uuid, uuid)
    TO nylonite_app, nylonite_platform, nylonite_scheduler, nylonite_projection_owner;
