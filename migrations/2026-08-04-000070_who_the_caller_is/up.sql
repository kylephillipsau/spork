-- Migration 70: who the caller is.
--
-- Question 171, and **D11 decided this in 2026-08-02 and nothing built it**:
--
--   `recorded_by_id` comes from the authenticated session, is never
--   client-supplied, and is never editable. That is the non-repudiable floor:
--   whatever else is claimed, we always know which person, on which device,
--   recorded this.
--
-- Until now it arrived in the request body, so the non-repudiable floor was a
-- number the caller typed. This migration builds the two tables that let the
-- server answer "who is this" for itself, and the next commit makes every write
-- path read the answer from here rather than from the request.
--
-- **Authentication only.** `person_tenant.role` is an undefined text column and
-- stays that way: membership decides which tenants a person may act for, and
-- nothing else. A role vocabulary is a business decision about who may do what,
-- and inventing one here would be the kind of undesigned language D22 refuses
-- everywhere else. Question 176 carries it.
--
-- **No `work_session` yet.** D11 has a crew declared at sign-on rather than
-- inferred, with append-only membership so "who was on this team when that
-- movement happened" stays answerable. That is the picking story, which is
-- upstream of this one, and question 177 carries it.

-- ---------------------------------------------------------------------------
-- What a person knows
-- ---------------------------------------------------------------------------
--
-- **Deliberately not a column on `person`.** D19 makes `person` global with no
-- row-level security at all -- *"one person may work for two tenants"* -- so
-- every tenant's connection can read every row of it. A password hash there
-- would be readable by all of them. Separate table, and no login role holds
-- SELECT on it: the only way in is the function below.

CREATE TABLE person_credential (
    person_id       uuid PRIMARY KEY REFERENCES person(id),

    -- The vocabulary is closed and starts with one member. A handheld will want
    -- a PIN and that is a row here, not a second table.
    kind            text NOT NULL DEFAULT 'password',
    CONSTRAINT person_credential_kind_ck CHECK (kind IN ('password', 'pin')),

    -- A PHC string: algorithm, parameters and salt travel with the digest, so
    -- the cost can be raised later without a flag day. Argon2id at OWASP's
    -- m=19456, t=2, p=1.
    phc             text NOT NULL,

    -- OWASP counts failures against the account rather than the source address,
    -- because an attacker has more addresses than we have accounts.
    failed_attempts integer NOT NULL DEFAULT 0,
    locked_until    timestamptz,

    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT person_credential_attempts_ck CHECK (failed_attempts >= 0)
);

COMMENT ON TABLE person_credential IS
    'What a person knows, kept off `person` because D19 makes that table global '
    'and unprotected by row-level security. No login role may read this: '
    'authentication goes through session_open(), which is a definer. D11, Q171.';

-- ---------------------------------------------------------------------------
-- What a person is currently doing
-- ---------------------------------------------------------------------------
--
-- The token itself is **never stored**. What is stored is its SHA-256, so a
-- database backup that leaks does not hand over live sessions -- the same
-- argument as for a password, one layer out. A session id is a bearer
-- credential and this table is where the bearer part is kept honest.
--
-- A session names its tenant because `person` does not have one: D19 puts
-- membership in `person_tenant`, so a person belonging to two tenants must say
-- which one they are acting for. It is not inferable and the session is where
-- the answer belongs.

CREATE TABLE session (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    token_sha256    bytea NOT NULL,

    person_id       uuid NOT NULL REFERENCES person(id),
    tenant_id       uuid NOT NULL REFERENCES tenant(id),
    -- Where they are working. Every act needs one for `client_event.site_id`.
    site_id         uuid,

    created_at      timestamptz NOT NULL DEFAULT now(),
    -- Idle timeout is measured from here; the absolute one from `created_at`.
    last_seen_at    timestamptz NOT NULL DEFAULT now(),
    expires_at      timestamptz NOT NULL,
    revoked_at      timestamptz,

    -- D11's other two attribution columns, so a session can carry them when the
    -- client knows them. `device_id` is D27's recording device.
    device_id       uuid,
    user_agent      text,

    CONSTRAINT session_expires_after_start_ck CHECK (expires_at > created_at),
    CONSTRAINT session_site_fk
        FOREIGN KEY (site_id, tenant_id) REFERENCES site(id, tenant_id)
);

-- Unique so a token names at most one session, and the lookup is a single
-- index probe rather than a scan.
CREATE UNIQUE INDEX session_token_idx ON session (token_sha256);
CREATE INDEX session_person_idx ON session (person_id, created_at DESC);

COMMENT ON TABLE session IS
    'A live sign-on: which person, acting for which tenant, at which site. The '
    'token is stored as a SHA-256 and never in the clear. Names its tenant '
    'because D19 makes `person` global and membership `person_tenant`, so which '
    'tenant a person is acting for cannot be inferred. D11, Q171.';

-- ---------------------------------------------------------------------------
-- The two functions that are the only way in
-- ---------------------------------------------------------------------------
--
-- **Definers, on D89's pattern and for D89's reason.** The application needs to
-- verify a password and resolve a token, and it needs neither the ability to
-- read every credential nor to enumerate every session. So the tables are
-- unreadable to it and these are the whole interface: a fixed query shape, one
-- row out, owned by a role that is neither SUPERUSER nor BYPASSRLS (S53).
--
-- Argon2 cannot run in Postgres, so `session_open` takes the digest the
-- application has already verified rather than the password. It is the
-- bookkeeping around the check, not the check.

CREATE FUNCTION credential_for_login(p_email text)
    RETURNS TABLE (person_id uuid, phc text, locked_until timestamptz, active boolean)
    LANGUAGE sql STABLE
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
    SELECT p.id, c.phc, c.locked_until, p.active
      FROM person p
      JOIN person_credential c ON c.person_id = p.id
     WHERE lower(p.email) = lower(p_email)
$$;

COMMENT ON FUNCTION credential_for_login(text) IS
    'The one query shape that reaches a credential. Returns no row for an '
    'unknown address, which the caller must answer identically to a wrong '
    'password -- OWASP requires one generic message for wrong password, missing '
    'account and locked account alike. Q171.';

-- Records the outcome. Success clears the counter; failure advances it and locks
-- the account at the threshold. **Counted per account**, per OWASP: an attacker
-- has more source addresses than we have accounts.
CREATE FUNCTION credential_record_attempt(p_person uuid, p_succeeded boolean)
    RETURNS void
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
BEGIN
    IF p_succeeded THEN
        UPDATE person_credential
           SET failed_attempts = 0, locked_until = NULL, updated_at = now()
         WHERE person_id = p_person;
    ELSE
        UPDATE person_credential
           SET failed_attempts = failed_attempts + 1,
               -- Ten is the top of OWASP's five-to-ten band. Fifteen minutes is
               -- long enough to make guessing pointless and short enough that a
               -- packer who fumbled twice is not stood down for the shift.
               locked_until = CASE WHEN failed_attempts + 1 >= 10
                                   THEN now() + interval '15 minutes' END,
               updated_at = now()
         WHERE person_id = p_person;
    END IF;
END
$$;

-- Open a session for a person who has already been verified, against a tenant
-- they are currently a member of. Membership is checked here rather than in the
-- application, because it is the sentence that decides whose data they see.
CREATE FUNCTION session_open(
        p_person     uuid,
        p_tenant     uuid,
        p_site       uuid,
        p_token_hash bytea,
        p_lifetime   interval,
        p_device     uuid,
        p_user_agent text)
    RETURNS uuid
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    s_id uuid;
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM person_tenant
         WHERE person_id = p_person AND tenant_id = p_tenant AND left_at IS NULL)
    THEN
        RAISE EXCEPTION 'person % is not a current member of tenant %', p_person, p_tenant;
    END IF;

    INSERT INTO session (token_sha256, person_id, tenant_id, site_id,
                         expires_at, device_id, user_agent)
    VALUES (p_token_hash, p_person, p_tenant, p_site,
            now() + p_lifetime, p_device, p_user_agent)
    RETURNING id INTO s_id;
    RETURN s_id;
END
$$;

-- Resolve a token to the caller it names, and touch the idle clock.
--
-- Both timeouts are enforced here rather than by the caller: OWASP is explicit
-- that they are server-side, and a check the application could forget is a check
-- somebody eventually forgets. Returns no row for expired, revoked, unknown or
-- lapsed-membership, which the caller answers as one unauthenticated.
CREATE FUNCTION session_resolve(p_token_hash bytea, p_idle interval)
    RETURNS TABLE (session_id uuid, person_id uuid, tenant_id uuid, site_id uuid)
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
BEGIN
    RETURN QUERY
    UPDATE session s
       SET last_seen_at = now()
     WHERE s.token_sha256 = p_token_hash
       AND s.revoked_at IS NULL
       AND s.expires_at > now()
       AND s.last_seen_at > now() - p_idle
       AND EXISTS (SELECT 1 FROM person_tenant pt
                    WHERE pt.person_id = s.person_id
                      AND pt.tenant_id = s.tenant_id
                      AND pt.left_at IS NULL)
    RETURNING s.id, s.person_id, s.tenant_id, s.site_id;
END
$$;

CREATE FUNCTION session_revoke(p_token_hash bytea)
    RETURNS boolean
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    n integer;
BEGIN
    UPDATE session SET revoked_at = now()
     WHERE token_sha256 = p_token_hash AND revoked_at IS NULL;
    GET DIAGNOSTICS n = ROW_COUNT;
    RETURN n = 1;
END
$$;

-- ---------------------------------------------------------------------------
-- Privileges
-- ---------------------------------------------------------------------------
--
-- The tables are unreachable and the functions are the interface. J37 wants
-- every definer to have `search_path` pinned and no EXECUTE to PUBLIC, which is
-- what the revokes below are; S53 wants the owner to be neither SUPERUSER nor
-- BYPASSRLS, which `nylonite_mediation_owner` already satisfies.

ALTER FUNCTION credential_for_login(text) OWNER TO nylonite_mediation_owner;
ALTER FUNCTION credential_record_attempt(uuid, boolean) OWNER TO nylonite_mediation_owner;
ALTER FUNCTION session_open(uuid, uuid, uuid, bytea, interval, uuid, text)
    OWNER TO nylonite_mediation_owner;
ALTER FUNCTION session_resolve(bytea, interval) OWNER TO nylonite_mediation_owner;
ALTER FUNCTION session_revoke(bytea) OWNER TO nylonite_mediation_owner;

REVOKE EXECUTE ON FUNCTION credential_for_login(text) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION credential_record_attempt(uuid, boolean) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION session_open(uuid, uuid, uuid, bytea, interval, uuid, text)
    FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION session_resolve(bytea, interval) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION session_revoke(bytea) FROM PUBLIC;

GRANT EXECUTE ON FUNCTION credential_for_login(text) TO nylonite_app;
GRANT EXECUTE ON FUNCTION credential_record_attempt(uuid, boolean) TO nylonite_app;
GRANT EXECUTE ON FUNCTION session_open(uuid, uuid, uuid, bytea, interval, uuid, text)
    TO nylonite_app;
GRANT EXECUTE ON FUNCTION session_resolve(bytea, interval) TO nylonite_app;
GRANT EXECUTE ON FUNCTION session_revoke(bytea) TO nylonite_app;

-- **The owner needs what its functions read.** A definer runs its body as its
-- owner, so `nylonite_mediation_owner` -- which owns nothing else and logs in
-- nowhere -- must be able to reach `person`, `person_tenant` and the two tables
-- above. Without this the functions parse, deploy, and fail at the first call
-- with `permission denied for table person`, which is what happened.
GRANT SELECT ON person, person_tenant TO nylonite_mediation_owner;
GRANT SELECT, UPDATE ON person_credential TO nylonite_mediation_owner;
GRANT SELECT, INSERT, UPDATE ON session TO nylonite_mediation_owner;

-- Provisioning a credential is the platform's job, not the floor's.
GRANT SELECT, INSERT, UPDATE ON person_credential TO nylonite_platform;
GRANT SELECT ON session TO nylonite_platform;

-- The app may read which tenants a person belongs to, so a sign-on can offer the
-- choice when there is more than one.
GRANT SELECT ON person_tenant TO nylonite_app;
