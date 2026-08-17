-- Migration 85 down: nobody can change their own password again.
--
-- Nothing to preserve — these are three functions and no data — but worth
-- saying what reversing costs, because it is not nothing: a deployment whose
-- password was chosen at setup goes back to having no way to change it, and the
-- passwords already changed through here stay changed. Reversing is not a
-- rollback of anybody's credential.

DROP FUNCTION IF EXISTS session_revoke_others(uuid, bytea);
DROP FUNCTION IF EXISTS credential_change_password(uuid, text, text);
DROP FUNCTION IF EXISTS credential_phc(uuid);
