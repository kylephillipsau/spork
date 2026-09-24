-- Back to the table grant, which is what `issue_session` reads without the
-- function.
GRANT SELECT ON person_tenant TO spork_app;

REVOKE SELECT ON tenant FROM spork_mediation_owner;
DROP FUNCTION IF EXISTS memberships_of(uuid);
