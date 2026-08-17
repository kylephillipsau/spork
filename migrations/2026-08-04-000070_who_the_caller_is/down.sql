-- Migration 70 down: the server stops being able to say who the caller is, and
-- `recorded_by_id` goes back to being a number the request asserts.

DO $$
DECLARE
    n bigint;
BEGIN
    SELECT count(*) INTO n FROM session WHERE revoked_at IS NULL AND expires_at > now();
    IF n > 0 THEN
        RAISE NOTICE 'reversing migration 70 ends % live session(s): everyone signed in is '
                     'signed out, and nothing can verify a password afterwards', n;
    END IF;
END $$;

DROP FUNCTION IF EXISTS session_revoke(bytea);
DROP FUNCTION IF EXISTS session_resolve(bytea, interval);
DROP FUNCTION IF EXISTS session_open(uuid, uuid, uuid, bytea, interval, uuid, text);
DROP FUNCTION IF EXISTS credential_record_attempt(uuid, boolean);
DROP FUNCTION IF EXISTS credential_for_login(text);

DROP TABLE IF EXISTS session;
DROP TABLE IF EXISTS person_credential;

REVOKE SELECT ON person_tenant FROM nylonite_app;
REVOKE SELECT ON person, person_tenant FROM nylonite_mediation_owner;
