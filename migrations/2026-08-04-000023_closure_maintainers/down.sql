-- Reverse of 2026-08-04-000023_closure_maintainers.
--
-- The closure rows stay. They are facts recomputed rather than rows created, and
-- reversing the maintainer does not make the taxonomy untrue -- it makes it
-- unmaintained, which is the state this migration found and is the honest thing
-- to return to.

DELETE FROM projection_rebuild
 WHERE function_name IN ('projection_item_class_closure_rebuild',
                         'projection_party_class_closure_rebuild');

COMMENT ON TABLE item_class_closure IS
    'PROJECTION of item_class. Never written by the application role, which is '
    'enforced. The named function under D35 does not exist yet: question 140.';
COMMENT ON TABLE party_class_closure IS
    'PROJECTION of party_class. The closure is what makes the matching language '
    'cardinality one: ancestor-or-self is a lookup, not a recursion.';

COMMENT ON COLUMN item_class_closure.tenant_id IS
    '@projection(pending) of item_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN item_class_closure.ancestor_id IS
    '@projection(pending) of item_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN item_class_closure.descendant_id IS
    '@projection(pending) of item_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN item_class_closure.depth IS
    '@projection(pending) of item_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN party_class_closure.tenant_id IS
    '@projection(pending) of party_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN party_class_closure.ancestor_id IS
    '@projection(pending) of party_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN party_class_closure.descendant_id IS
    '@projection(pending) of party_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN party_class_closure.depth IS
    '@projection(pending) of party_class; no maintainer exists. Question 140.';

REVOKE SELECT ON item_class, party_class FROM nylonite_projection_owner;
REVOKE SELECT, INSERT, UPDATE, DELETE ON item_class_closure, party_class_closure
    FROM nylonite_projection_owner;

DROP FUNCTION IF EXISTS projection_party_class_closure_rebuild(uuid);
DROP FUNCTION IF EXISTS projection_item_class_closure_rebuild(uuid);
