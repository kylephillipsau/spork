-- Migration 75: a key the person holds.
--
-- Migration 70 gave the server a way to answer "who is this" from a password.
-- A password is a secret the server also knows, so a stolen digest is a stolen
-- identity, and a person on a dock types it into a shared handheld in front of
-- whoever is standing there.
--
-- A passkey is a private key that never leaves the authenticator. The server
-- holds a public key, verifies a signature, and has nothing worth stealing. That
-- matters more here than in most places: D11's non-repudiable floor says
-- *"whatever else is claimed, we always know which person recorded this"*, and
-- a shared password on a shared device is exactly how that stops being true.
--
-- # Not a row in `person_credential`
--
-- That table is keyed by `person_id` alone: one person, one secret. A person has
-- as many passkeys as they have devices, and revoking the phone must not revoke
-- the laptop. Different cardinality, different shape — a public key and a
-- counter rather than a PHC string — so a separate table rather than a widened
-- one and a nullable column for every field that only one kind uses.
--
-- # Where the opaque part is, and why it is not jsonb
--
-- S10 forbids `jsonb` model-wide, and `verifier_state` here is `text`. That is
-- not a way around the rule, so the reasoning is written down.
--
-- The columns below are the model: which authenticator, how many times it has
-- signed, whether the key is backed up, when it was last used. Those are facts
-- the business reasons about and they are typed. `verifier_state` is the
-- verifying library's own serialisation of the credential, stored because the
-- verification must round-trip it *exactly* — reconstructing a COSE public key
-- from decomposed columns and hoping it re-encodes byte-identically is how a
-- signature check starts failing on a library upgrade, and a signature check
-- that fails open is worse than one that is hard to query.
--
-- So: an implementation detail of one verifier, named as such, and nothing in
-- the model reads it. If it were jsonb it would invite queries into it, which is
-- the thing Principle 3 is protecting against.

CREATE TABLE person_passkey (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    person_id       uuid NOT NULL REFERENCES person(id),

    -- The authenticator's own identifier for this credential. Globally unique:
    -- it is what an assertion names, and two rows claiming one id would make
    -- "whose key signed this" unanswerable.
    credential_id   bytea NOT NULL,

    -- What the person calls it. "The blue YubiKey", "my phone".
    label           text,

    -- Which model of authenticator, as the vendor declares it. Nullable because
    -- a platform authenticator may report all zeroes deliberately.
    aaguid          uuid,

    -- **A cloned authenticator is detectable and this is how.** The counter only
    -- ever rises; a signature arriving with a counter at or below the stored one
    -- means two devices hold the same key. Some authenticators do not implement
    -- it and report zero forever, which is why this is evidence rather than a
    -- constraint.
    sign_count      bigint NOT NULL DEFAULT 0,

    -- Whether the key can leave the device, and whether it currently has. A
    -- synced passkey is a different security story from one bound to hardware,
    -- and an operator ought to be able to see which they are trusting.
    backup_eligible boolean,
    backup_state    boolean,

    transports      text[],

    -- See the note above. Opaque, one reader, never queried into.
    verifier_state  text NOT NULL,

    created_at      timestamptz NOT NULL DEFAULT now(),
    last_used_at    timestamptz,
    -- Revoked rather than deleted: a key that signed movements last month is
    -- part of how those movements are attributed, and deleting it makes an
    -- answered question unanswerable. D11.
    disabled_at     timestamptz,

    CONSTRAINT person_passkey_sign_count_ck CHECK (sign_count >= 0)
);

CREATE UNIQUE INDEX person_passkey_credential_idx ON person_passkey (credential_id);
CREATE INDEX person_passkey_person_idx ON person_passkey (person_id)
    WHERE disabled_at IS NULL;

COMMENT ON TABLE person_passkey IS
    'A public key a person authenticates with. Many per person, one per device. '
    'Revoked and never deleted: a key that signed movements is part of how those '
    'movements are attributed. D11, migration 75.';

COMMENT ON COLUMN person_passkey.verifier_state IS
    'The verifying library''s serialisation of this credential, round-tripped '
    'byte-exactly. An implementation detail of one reader and never queried '
    'into: text rather than jsonb precisely so it does not invite that. '
    'Principle 3, S10.';

-- ---------------------------------------------------------------------------
-- The challenge, which is the whole of the replay defence
-- ---------------------------------------------------------------------------
--
-- A WebAuthn ceremony is two round trips: the server issues a challenge, the
-- authenticator signs it. **The state between them must live on the server.**
-- Handing it to the browser in a cookie and taking it back means trusting the
-- party being authenticated to remember what they were asked, which is not
-- authentication.
--
-- Single use, and short-lived. `consumed_at` rather than a delete so that a
-- replay attempt is a row that already has a timestamp, rather than a row that
-- is missing and indistinguishable from one that never existed.

CREATE TABLE webauthn_challenge (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),

    -- Absent for a discoverable-credential sign-in, where the whole point is
    -- that the server does not know who is at the keyboard until they answer.
    person_id   uuid REFERENCES person(id),

    kind        text NOT NULL,
    CONSTRAINT webauthn_challenge_kind_ck CHECK (kind IN ('registration', 'authentication')),

    state       text NOT NULL,

    created_at  timestamptz NOT NULL DEFAULT now(),
    expires_at  timestamptz NOT NULL,
    consumed_at timestamptz,

    CONSTRAINT webauthn_challenge_expires_ck CHECK (expires_at > created_at),
    -- A registration ceremony always knows who it is for; an authentication one
    -- may not.
    CONSTRAINT webauthn_challenge_person_ck
        CHECK (kind <> 'registration' OR person_id IS NOT NULL)
);

CREATE INDEX webauthn_challenge_sweep_idx ON webauthn_challenge (expires_at)
    WHERE consumed_at IS NULL;

COMMENT ON TABLE webauthn_challenge IS
    'One in-flight WebAuthn ceremony. Server-side because handing the state to '
    'the browser means trusting the party being authenticated to remember what '
    'they were asked. Single use: consumed_at rather than a delete, so a replay '
    'is a row with a timestamp rather than a row that is missing. Migration 75.';

-- ---------------------------------------------------------------------------
-- The functions that are the only way in
-- ---------------------------------------------------------------------------
--
-- D89's pattern, and migration 70's: no login role holds SELECT on either table,
-- and every path in is a definer owned by `spork_mediation_owner` with a
-- pinned `search_path`. `person` is global under D19, so a passkey readable by
-- the application role would be readable by every tenant's connection.

CREATE FUNCTION webauthn_challenge_open(
    p_person_id uuid, p_kind text, p_state text, p_ttl interval)
RETURNS uuid
LANGUAGE sql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
    INSERT INTO webauthn_challenge (person_id, kind, state, expires_at)
    VALUES (p_person_id, p_kind, p_state, now() + p_ttl)
    RETURNING id;
$$;

-- **Claims the challenge and returns it in one statement.** Two statements would
-- let two requests both read an unconsumed row and both proceed, which is the
-- replay this table exists to stop.
CREATE FUNCTION webauthn_challenge_claim(p_id uuid, p_kind text)
RETURNS TABLE (person_id uuid, state text)
LANGUAGE sql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
    UPDATE webauthn_challenge
       SET consumed_at = now()
     WHERE id = p_id
       AND kind = p_kind
       AND consumed_at IS NULL
       AND expires_at > now()
    RETURNING webauthn_challenge.person_id, webauthn_challenge.state;
$$;

CREATE FUNCTION passkey_register(
    p_person_id uuid, p_credential_id bytea, p_label text, p_aaguid uuid,
    p_sign_count bigint, p_backup_eligible boolean, p_backup_state boolean,
    p_transports text[], p_verifier_state text)
RETURNS uuid
LANGUAGE sql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
    INSERT INTO person_passkey (person_id, credential_id, label, aaguid,
        sign_count, backup_eligible, backup_state, transports, verifier_state)
    VALUES (p_person_id, p_credential_id, p_label, p_aaguid, p_sign_count,
            p_backup_eligible, p_backup_state, p_transports, p_verifier_state)
    RETURNING id;
$$;

-- Every live key for a person, for the allow-list an authentication ceremony
-- offers. Returns the opaque state because that is what the verifier needs.
CREATE FUNCTION passkeys_for_person(p_person_id uuid)
RETURNS TABLE (id uuid, verifier_state text)
LANGUAGE sql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
    SELECT person_passkey.id, person_passkey.verifier_state
      FROM person_passkey
     WHERE person_passkey.person_id = p_person_id
       AND person_passkey.disabled_at IS NULL
     ORDER BY person_passkey.created_at;
$$;

-- The other direction: an assertion names a credential and the server has to
-- find out whose it is. This is the discoverable-credential path, and it is why
-- `credential_id` is globally unique.
CREATE FUNCTION passkey_by_credential(p_credential_id bytea)
RETURNS TABLE (id uuid, person_id uuid, verifier_state text)
LANGUAGE sql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
    SELECT person_passkey.id, person_passkey.person_id, person_passkey.verifier_state
      FROM person_passkey
     WHERE person_passkey.credential_id = p_credential_id
       AND person_passkey.disabled_at IS NULL;
$$;

-- **The counter is the clone detector, so it is written on every use.** The
-- caller passes what the assertion claimed; this refuses to move it backwards,
-- and reports whether it moved at all so the application can raise a finding.
CREATE FUNCTION passkey_record_use(p_id uuid, p_sign_count bigint, p_backup_state boolean)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
DECLARE went_backwards boolean;
BEGIN
    SELECT p_sign_count <= sign_count AND p_sign_count > 0 AND sign_count > 0
      INTO went_backwards
      FROM person_passkey WHERE id = p_id;

    UPDATE person_passkey
       SET sign_count = greatest(sign_count, p_sign_count),
           backup_state = coalesce(p_backup_state, backup_state),
           last_used_at = now()
     WHERE id = p_id;

    RETURN coalesce(went_backwards, false);
END;
$$;

CREATE FUNCTION passkey_disable(p_id uuid, p_person_id uuid)
RETURNS boolean
LANGUAGE sql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
    UPDATE person_passkey SET disabled_at = now()
     WHERE id = p_id AND person_id = p_person_id AND disabled_at IS NULL
    RETURNING true;
$$;

ALTER FUNCTION webauthn_challenge_open(uuid, text, text, interval)
    OWNER TO spork_mediation_owner;
ALTER FUNCTION webauthn_challenge_claim(uuid, text) OWNER TO spork_mediation_owner;
ALTER FUNCTION passkey_register(uuid, bytea, text, uuid, bigint, boolean, boolean,
    text[], text) OWNER TO spork_mediation_owner;
ALTER FUNCTION passkeys_for_person(uuid) OWNER TO spork_mediation_owner;
ALTER FUNCTION passkey_by_credential(bytea) OWNER TO spork_mediation_owner;
ALTER FUNCTION passkey_record_use(uuid, bigint, boolean) OWNER TO spork_mediation_owner;
ALTER FUNCTION passkey_disable(uuid, uuid) OWNER TO spork_mediation_owner;

GRANT SELECT, INSERT, UPDATE ON person_passkey TO spork_mediation_owner;
GRANT SELECT, INSERT, UPDATE ON webauthn_challenge TO spork_mediation_owner;

-- **PUBLIC first, and this was nearly missed.** Postgres grants EXECUTE on a
-- new function to PUBLIC by default, so a definer that bypasses row-level
-- security and reads credentials is callable by every role until this runs.
-- J37 reported all seven of these before the commit, which is what that check
-- is for; migration 70 set the pattern and this did not follow it.
REVOKE EXECUTE ON FUNCTION webauthn_challenge_open(uuid, text, text, interval) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION webauthn_challenge_claim(uuid, text) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION passkey_register(uuid, bytea, text, uuid, bigint, boolean,
    boolean, text[], text) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION passkeys_for_person(uuid) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION passkey_by_credential(bytea) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION passkey_record_use(uuid, bigint, boolean) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION passkey_disable(uuid, uuid) FROM PUBLIC;

GRANT EXECUTE ON FUNCTION webauthn_challenge_open(uuid, text, text, interval) TO spork_app;
GRANT EXECUTE ON FUNCTION webauthn_challenge_claim(uuid, text) TO spork_app;
GRANT EXECUTE ON FUNCTION passkey_register(uuid, bytea, text, uuid, bigint, boolean,
    boolean, text[], text) TO spork_app;
GRANT EXECUTE ON FUNCTION passkeys_for_person(uuid) TO spork_app;
GRANT EXECUTE ON FUNCTION passkey_by_credential(bytea) TO spork_app;
GRANT EXECUTE ON FUNCTION passkey_record_use(uuid, bigint, boolean) TO spork_app;
GRANT EXECUTE ON FUNCTION passkey_disable(uuid, uuid) TO spork_app;

-- A person may hold a passkey and no password, which is the point.
ALTER TABLE person_credential ALTER COLUMN phc DROP NOT NULL;
COMMENT ON COLUMN person_credential.phc IS
    'A PHC string, or NULL for a person who authenticates only with a passkey. '
    'Nullable since migration 75: passwordless is the destination, and a row '
    'that exists to hold a lockout counter should not need a digest to do it.';
