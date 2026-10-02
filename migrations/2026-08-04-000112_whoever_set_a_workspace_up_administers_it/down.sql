-- Migration 112 down: nobody administers a workspace.

DROP FUNCTION role_in_tenant(uuid);

UPDATE person_tenant SET role = 'operator' WHERE role = 'administrator';

COMMENT ON COLUMN person_tenant.role IS NULL;
