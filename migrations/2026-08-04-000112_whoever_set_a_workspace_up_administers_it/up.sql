-- Migration 112: whoever set a workspace up administers it. D192.
--
-- `person_tenant.role` has been free text that nothing read (Q176). A backup
-- holds every row of the business and its sign-ins, so taking one is the
-- first thing that needs a role: `administrator`. Setup now writes it for the
-- person who sets the workspace up; here, each workspace's earliest member
-- still in it is that person.
--
-- The rest of the vocabulary, and a screen to give the role to someone else,
-- are still Q176's.

UPDATE person_tenant pt
   SET role = 'administrator'
  FROM (SELECT DISTINCT ON (tenant_id) person_id, tenant_id
          FROM person_tenant
         WHERE left_at IS NULL
         ORDER BY tenant_id, joined_at, person_id) first
 WHERE pt.person_id = first.person_id
   AND pt.tenant_id = first.tenant_id;

COMMENT ON COLUMN person_tenant.role IS
    '`administrator` may take a backup (D192); any other value is a member '
    'without that. The vocabulary is Q176''s. Migration 112.';

-- **Asked through a definer, like every identity read** (migrations 70, 87).
-- The application holds no privilege on `person_tenant`; this answers one
-- question, the caller's own role in the workspace the transaction names, and
-- nothing about anyone else.
CREATE FUNCTION role_in_tenant(p_person uuid)
    RETURNS text
    LANGUAGE sql
    STABLE
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
    SELECT pt.role
      FROM person_tenant pt
     WHERE pt.person_id = p_person
       AND pt.tenant_id = current_tenant()
       AND pt.left_at IS NULL
$$;

COMMENT ON FUNCTION role_in_tenant(uuid) IS
    'A person''s role in the current tenant, or null when not a member. D192, '
    'migration 112.';

ALTER FUNCTION role_in_tenant(uuid) OWNER TO spork_mediation_owner;
REVOKE EXECUTE ON FUNCTION role_in_tenant(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION role_in_tenant(uuid) TO spork_app;
