-- Migration 92 down: one idle limit for every session again, and no rotation.
--
-- Migration 70's session_open, session_resolve and session_revoke and
-- migration 85's session_revoke_others, verbatim. A session that was rotated
-- keeps its current digest; the previous one stops working.

DROP FUNCTION session_rotate(bytea, bytea, interval);
DROP FUNCTION session_resolve(bytea);
DROP FUNCTION session_open(uuid, uuid, uuid, bytea, interval, interval, boolean, uuid, text);

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
     WHERE token_sha256 = p_token_hash AND revoked_at IS NULL;
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
       AND token_sha256 IS DISTINCT FROM p_keep;
    GET DIAGNOSTICS n = ROW_COUNT;
    RETURN n;
END
$$;

ALTER FUNCTION session_open(uuid, uuid, uuid, bytea, interval, uuid, text)
    OWNER TO spork_mediation_owner;
ALTER FUNCTION session_resolve(bytea, interval) OWNER TO spork_mediation_owner;
REVOKE EXECUTE ON FUNCTION session_open(uuid, uuid, uuid, bytea, interval, uuid, text)
    FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION session_resolve(bytea, interval) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION session_open(uuid, uuid, uuid, bytea, interval, uuid, text)
    TO spork_app;
GRANT EXECUTE ON FUNCTION session_resolve(bytea, interval) TO spork_app;

DROP INDEX session_previous_token_idx;
ALTER TABLE session
    DROP CONSTRAINT session_rotation_pairs,
    DROP CONSTRAINT session_idle_positive,
    DROP COLUMN rotated_at,
    DROP COLUMN previous_token_sha256,
    DROP COLUMN remembered,
    DROP COLUMN idle;
