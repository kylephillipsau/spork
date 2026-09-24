-- Migration 92: a device that stays signed in.
--
-- The session cookie had no `Max-Age`, so it died with the browser and every
-- morning began at the sign-in page. Two changes, one policy each:
--
-- * **Each session carries its own idle limit.** `session_resolve` compared
--   every session against one interval the application passed in, so a
--   "keep me signed in" session could not be told apart from a shared bench.
--   Now `session_open` records the limit and `session_resolve` reads it. The
--   application still chooses the numbers (auth.rs); the database enforces
--   whichever was chosen, so a timeout is still not something a handler can
--   forget.
--
-- * **A long-lived token is rotated.** A remembered session lasts thirty days,
--   and a token that never changes for thirty days is thirty days of use for
--   anybody who copies it. `session_rotate` swaps the digest in place, keeping
--   the row (the same person, site and absolute expiry) and remembering the
--   previous digest for a short grace, so two tabs that raced on the old
--   cookie are not both signed out.

ALTER TABLE session
    ADD COLUMN idle                  interval    NOT NULL DEFAULT interval '30 minutes',
    ADD COLUMN remembered            boolean     NOT NULL DEFAULT false,
    ADD COLUMN previous_token_sha256 bytea,
    ADD COLUMN rotated_at            timestamptz,
    ADD CONSTRAINT session_idle_positive CHECK (idle > interval '0'),
    ADD CONSTRAINT session_rotation_pairs
        CHECK ((previous_token_sha256 IS NULL) = (rotated_at IS NULL));

COMMENT ON COLUMN session.idle IS
    'How long this session may go unused. Chosen at sign-on and enforced by session_resolve.';
COMMENT ON COLUMN session.remembered IS
    'The person asked to stay signed in on this device. Carried to the session a site change opens.';
COMMENT ON COLUMN session.previous_token_sha256 IS
    'The digest this session answered to before its last rotation; accepted for two minutes after rotated_at.';

CREATE UNIQUE INDEX session_previous_token_idx ON session (previous_token_sha256)
    WHERE previous_token_sha256 IS NOT NULL;

-- ---------------------------------------------------------------------------
-- Opening: the idle limit and the choice are recorded with the session.
-- ---------------------------------------------------------------------------

DROP FUNCTION session_open(uuid, uuid, uuid, bytea, interval, uuid, text);

CREATE FUNCTION session_open(
        p_person     uuid,
        p_tenant     uuid,
        p_site       uuid,
        p_token_hash bytea,
        p_lifetime   interval,
        p_idle       interval,
        p_remembered boolean,
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
                         expires_at, idle, remembered, device_id, user_agent)
    VALUES (p_token_hash, p_person, p_tenant, p_site,
            now() + p_lifetime, p_idle, p_remembered, p_device, p_user_agent)
    RETURNING id INTO s_id;
    RETURN s_id;
END
$$;

-- ---------------------------------------------------------------------------
-- Resolving: each session against its own limit, and the previous digest for
-- two minutes after a rotation.
-- ---------------------------------------------------------------------------

DROP FUNCTION session_resolve(bytea, interval);

CREATE FUNCTION session_resolve(p_token_hash bytea)
    RETURNS TABLE (session_id uuid, person_id uuid, tenant_id uuid, site_id uuid,
                   remembered boolean, issued_at timestamptz, expires_at timestamptz)
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
BEGIN
    RETURN QUERY
    UPDATE session s
       SET last_seen_at = now()
     WHERE (s.token_sha256 = p_token_hash
            OR (s.previous_token_sha256 = p_token_hash
                AND s.rotated_at > now() - interval '2 minutes'))
       AND s.revoked_at IS NULL
       AND s.expires_at > now()
       AND s.last_seen_at > now() - s.idle
       AND EXISTS (SELECT 1 FROM person_tenant pt
                    WHERE pt.person_id = s.person_id
                      AND pt.tenant_id = s.tenant_id
                      AND pt.left_at IS NULL)
    RETURNING s.id, s.person_id, s.tenant_id, s.site_id,
              s.remembered, coalesce(s.rotated_at, s.created_at), s.expires_at;
END
$$;

-- ---------------------------------------------------------------------------
-- Rotating: a new digest for the same session, once it is `p_after` old.
-- Only the current digest may rotate, so a race on the old cookie rotates once.
-- ---------------------------------------------------------------------------

CREATE FUNCTION session_rotate(p_token_hash bytea, p_new_hash bytea, p_after interval)
    RETURNS boolean
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    n integer;
BEGIN
    UPDATE session
       SET previous_token_sha256 = token_sha256,
           token_sha256 = p_new_hash,
           rotated_at = now()
     WHERE token_sha256 = p_token_hash
       AND revoked_at IS NULL
       AND expires_at > now()
       AND coalesce(rotated_at, created_at) < now() - p_after;
    GET DIAGNOSTICS n = ROW_COUNT;
    RETURN n = 1;
END
$$;

-- ---------------------------------------------------------------------------
-- Revoking: a token inside its grace names the session too. Signing out with
-- it must end the session, and "sign out everywhere else" must not end the
-- one it was asked from.
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION session_revoke(p_token_hash bytea)
    RETURNS boolean
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    n integer;
BEGIN
    UPDATE session SET revoked_at = now()
     WHERE (token_sha256 = p_token_hash OR previous_token_sha256 = p_token_hash)
       AND revoked_at IS NULL;
    GET DIAGNOSTICS n = ROW_COUNT;
    RETURN n = 1;
END
$$;

CREATE OR REPLACE FUNCTION session_revoke_others(p_person uuid, p_keep bytea)
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
       AND token_sha256 IS DISTINCT FROM p_keep
       AND previous_token_sha256 IS DISTINCT FROM p_keep;
    GET DIAGNOSTICS n = ROW_COUNT;
    RETURN n;
END
$$;

-- ---------------------------------------------------------------------------
-- Privileges, as migration 70 set them for the functions these replace.
-- ---------------------------------------------------------------------------

ALTER FUNCTION session_open(uuid, uuid, uuid, bytea, interval, interval, boolean, uuid, text)
    OWNER TO spork_mediation_owner;
ALTER FUNCTION session_resolve(bytea) OWNER TO spork_mediation_owner;
ALTER FUNCTION session_rotate(bytea, bytea, interval) OWNER TO spork_mediation_owner;

REVOKE EXECUTE ON FUNCTION session_open(uuid, uuid, uuid, bytea, interval, interval, boolean, uuid, text)
    FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION session_resolve(bytea) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION session_rotate(bytea, bytea, interval) FROM PUBLIC;

GRANT EXECUTE ON FUNCTION session_open(uuid, uuid, uuid, bytea, interval, interval, boolean, uuid, text)
    TO spork_app;
GRANT EXECUTE ON FUNCTION session_resolve(bytea) TO spork_app;
GRANT EXECUTE ON FUNCTION session_rotate(bytea, bytea, interval) TO spork_app;
