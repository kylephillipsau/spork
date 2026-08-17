-- Back to the table grant, which is what `issue_session` reads without the
-- function.
GRANT SELECT ON person_tenant TO nylonite_app;

REVOKE SELECT ON tenant FROM nylonite_mediation_owner;
DROP FUNCTION IF EXISTS memberships_of(uuid);
