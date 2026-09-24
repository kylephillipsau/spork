-- Migration 85: a password the person can change.
--
-- D142 shipped the way *into* a deployment and said plainly what it did not
-- ship:
--
--   **Still owed.** There is no change-password path: `spork_app` cannot
--   write `person_credential` at all, so it wants a `SECURITY DEFINER` function
--   scoped to the caller's own row.
--
-- Until now the password chosen at setup was the password forever, which on a
-- deployment reachable from the internet is not a gap in convenience — it is
-- the fixture password committed in this repository, still live.
--
-- Three functions, on migration 70's pattern and for migration 70's reason: the
-- tables stay unreadable and unwritable to every login role, and a fixed set of
-- query shapes is the whole interface.

-- ---------------------------------------------------------------------------
-- Reading one's own credential, to check the old password
-- ---------------------------------------------------------------------------
--
-- Argon2 cannot run in Postgres, so verifying the current password happens in
-- the application and this hands it the digest to verify against. It is
-- deliberately **by person rather than by email**: a session names a person, and
-- routing a change through `credential_for_login` would mean looking an address
-- up in order to act on the row a session already identifies.
--
-- `locked_until` comes back with it because the lockout applies here too. A
-- stolen session guessing at the current password is the exact attack the
-- counter exists for, and an endpoint that ignores it is a way around it.

CREATE FUNCTION credential_phc(p_person uuid)
    RETURNS TABLE (phc text, locked_until timestamptz)
    LANGUAGE sql STABLE
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
    SELECT c.phc, c.locked_until
      FROM person_credential c
     WHERE c.person_id = p_person
$$;

COMMENT ON FUNCTION credential_phc(uuid) IS
    'The digest for one person, so the application can verify a password it '
    'already has a session for. By person and not by address, because a session '
    'names a person. D142, migration 85.';

-- ---------------------------------------------------------------------------
-- Writing it back, only over the digest that was read
-- ---------------------------------------------------------------------------
--
-- **A compare-and-set rather than a setter**, and the `p_expected_phc` argument
-- is doing three jobs at once:
--
-- * it closes the read-verify-write race, so two changes racing cannot have the
--   second silently overwrite the first's password with one derived from a
--   credential that no longer exists;
-- * it refuses a passkey-only row for free, because migration 75 made `phc`
--   nullable and `NULL = anything` is never true — a person with no password
--   cannot have one set by a path that claims to be *changing* one;
-- * and it narrows what the application can do. A bare
--   `credential_set_password(person, phc)` would let the app write anybody's
--   password; this one only lets it write a row whose current digest it has
--   already been given, which is the mediation argument migration 70 is built
--   on rather than a restatement of the check above it.
--
-- The counter and the lock are cleared on success. Changing a password is the
-- remedy for having been locked out, so leaving the lock in place would mean
-- doing the right thing and still being unable to sign in.

CREATE FUNCTION credential_change_password(
        p_person       uuid,
        p_expected_phc text,
        p_new_phc      text)
    RETURNS boolean
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    n integer;
BEGIN
    UPDATE person_credential
       SET phc = p_new_phc,
           failed_attempts = 0,
           locked_until = NULL,
           updated_at = now()
     WHERE person_id = p_person
       AND phc = p_expected_phc;
    GET DIAGNOSTICS n = ROW_COUNT;
    RETURN n = 1;
END
$$;

COMMENT ON FUNCTION credential_change_password(uuid, text, text) IS
    'Replace one person''s password digest, but only over the digest the caller '
    'was given. Compare-and-set so a race cannot lose a change, so a '
    'passkey-only row (NULL phc) is refused, and so the application never holds '
    'the ability to write a credential it has not read. D142, migration 85.';

-- ---------------------------------------------------------------------------
-- And the sessions that were opened with the old one
-- ---------------------------------------------------------------------------
--
-- OWASP is explicit: changing a password invalidates every other session. The
-- reason is the case that makes change-password worth having — somebody who
-- believes a session has been stolen — and a change that leaves the thief signed
-- in has not helped them.
--
-- The current session survives, identified by its own digest, because signing
-- somebody out of the screen they just used is a way of teaching them not to
-- use it. Returns how many died so the caller can say.

CREATE FUNCTION session_revoke_others(p_person uuid, p_keep bytea)
    RETURNS integer
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    n integer;
BEGIN
    UPDATE session
       SET revoked_at = now()
     WHERE person_id = p_person
       AND revoked_at IS NULL
       AND token_sha256 IS DISTINCT FROM p_keep;
    GET DIAGNOSTICS n = ROW_COUNT;
    RETURN n;
END
$$;

COMMENT ON FUNCTION session_revoke_others(uuid, bytea) IS
    'Revoke every live session for a person except the one presenting p_keep. '
    'What makes a password change worth doing for somebody who thinks a session '
    'was stolen. D142, migration 85.';

-- ---------------------------------------------------------------------------
-- Privileges
-- ---------------------------------------------------------------------------
--
-- J37 wants every definer to have `search_path` pinned and no EXECUTE to
-- PUBLIC — Postgres grants EXECUTE to PUBLIC on a new function by default, so
-- without these revokes a definer that reaches credentials is callable by every
-- role. S53 wants an owner that is neither SUPERUSER nor BYPASSRLS, which
-- `spork_mediation_owner` already is.
--
-- **No table grants below**, and that is not an oversight: migration 70 already
-- gave the owner SELECT, UPDATE on `person_credential` and SELECT, INSERT,
-- UPDATE on `session`, which is everything these three bodies touch.

ALTER FUNCTION credential_phc(uuid) OWNER TO spork_mediation_owner;
ALTER FUNCTION credential_change_password(uuid, text, text)
    OWNER TO spork_mediation_owner;
ALTER FUNCTION session_revoke_others(uuid, bytea) OWNER TO spork_mediation_owner;

REVOKE EXECUTE ON FUNCTION credential_phc(uuid) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION credential_change_password(uuid, text, text) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION session_revoke_others(uuid, bytea) FROM PUBLIC;

GRANT EXECUTE ON FUNCTION credential_phc(uuid) TO spork_app;
GRANT EXECUTE ON FUNCTION credential_change_password(uuid, text, text) TO spork_app;
GRANT EXECUTE ON FUNCTION session_revoke_others(uuid, bytea) TO spork_app;
