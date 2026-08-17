-- Migration 75 down: a person authenticates with something the server also knows.
--
-- Every passkey goes. There is nowhere else to put a public key, and a person
-- who had only a passkey has no way in afterwards — which the NOTICE says,
-- because restoring the NOT NULL below would fail on exactly those rows.

DO $$
DECLARE keys bigint; passwordless bigint;
BEGIN
    SELECT count(*) INTO keys FROM person_passkey;
    SELECT count(*) INTO passwordless FROM person_credential WHERE phc IS NULL;
    IF keys > 0 THEN
        RAISE NOTICE 'reversing migration 75 discards % passkey(s)', keys;
    END IF;
    IF passwordless > 0 THEN
        RAISE NOTICE '% person(s) have no password and will have no way to sign in; '
                     'their credential rows are removed', passwordless;
    END IF;
END $$;

DELETE FROM person_credential WHERE phc IS NULL;
ALTER TABLE person_credential ALTER COLUMN phc SET NOT NULL;

DROP FUNCTION IF EXISTS passkey_disable(uuid, uuid);
DROP FUNCTION IF EXISTS passkey_record_use(uuid, bigint, boolean);
DROP FUNCTION IF EXISTS passkey_by_credential(bytea);
DROP FUNCTION IF EXISTS passkeys_for_person(uuid);
DROP FUNCTION IF EXISTS passkey_register(uuid, bytea, text, uuid, bigint, boolean,
    boolean, text[], text);
DROP FUNCTION IF EXISTS webauthn_challenge_claim(uuid, text);
DROP FUNCTION IF EXISTS webauthn_challenge_open(uuid, text, text, interval);

DROP TABLE IF EXISTS webauthn_challenge;
DROP TABLE IF EXISTS person_passkey;
