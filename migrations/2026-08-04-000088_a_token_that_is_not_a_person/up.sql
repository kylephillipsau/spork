-- A credential for a program, and it is a capability rather than a role.
--
-- # Why a session token is the wrong thing to hand a loader
--
-- Bearer tokens already work: `Authorization: Bearer` and the `__Host-` cookie
-- name the same `session` row, which is D5's handheld story. So the shortest
-- path to importing over HTTPS is to sign in and use that token, and it is the
-- wrong path for three reasons that are all about the token outliving the act.
--
-- A session is eight hours by absolute lifetime and thirty minutes idle. An
-- import is not a sitting: `reported_stock` is replace-on-reload by design, so
-- loading it is a thing that happens again next week and the week after. A
-- credential that has to be re-minted by typing a password is a credential
-- somebody eventually leaves lying in a script.
--
-- A session also carries a person's whole reach. Handing a loader Kyle's
-- session hands it every endpoint Kyle has, and revoking it signs Kyle out of
-- the floor. One credential doing two jobs cannot be withdrawn from one of them.
--
-- # Scope without inventing authorisation
--
-- `auth.rs` says plainly what is deliberately absent: **no authorisation**.
-- `person_tenant.role` is an undefined text column, and question 176 holds the
-- role vocabulary precisely so that nobody invents one in passing. A token
-- scoped by permissions would be exactly that invention.
--
-- So this is not scoped by permission. It is scoped by **kind**, which is the
-- shape D142 already established: the setup token authorises one endpoint and
-- nothing else, and it needs no notion of what a role may do. There are now
-- three credential kinds and each one *is* its scope --- a session reaches
-- everything its person reaches, a setup token reaches `POST /setup`, an import
-- token reaches the import endpoints. A capability answers "what may this
-- token do" by being the answer, rather than by consulting a table nobody has
-- designed yet.
--
-- # The table is unreachable, which is now checked rather than intended
--
-- Migration 70's shape: the application holds no privilege on the table at all
-- and the definers below are the only interface. That is deliberate, and as of
-- migration 87 it is also *audited* --- J73 asks Postgres whether the
-- application can reach any table carrying a `tenant_id`, and exempts only the
-- ones it cannot. So this table needs no policy, and the day somebody grants
-- the application a column of it the check fires and says so.
--
-- Unlike `person_tenant` there is no chicken-and-egg here: a token names its
-- tenant on the row, so nothing has to be read before a tenant is known.

CREATE TABLE api_token (
    id             uuid PRIMARY KEY DEFAULT uuidv7(),

    -- The digest, never the token. Same reasoning as `session.token_sha256`:
    -- nothing ever needs to read it back, so nothing stores something that can
    -- be read back.
    token_sha256   bytea NOT NULL,

    tenant_id      uuid NOT NULL REFERENCES tenant(id),

    -- **Who is answerable for it**, which is not the same as who uses it. A
    -- program has no standing under D11; the person who minted this does, and
    -- that is the name on it.
    created_by_id  uuid NOT NULL REFERENCES person(id),

    -- What it is for, in words, because a list of digests is not a thing anybody
    -- can audit. `NOT NULL` and non-empty: an unlabelled token is one nobody
    -- will dare revoke.
    label          text NOT NULL,

    created_at     timestamptz NOT NULL DEFAULT now(),
    expires_at     timestamptz NOT NULL,
    last_used_at   timestamptz,
    revoked_at     timestamptz,

    CONSTRAINT api_token_digest_key UNIQUE (token_sha256),
    CONSTRAINT api_token_label_ck CHECK (btrim(label) <> ''),
    CONSTRAINT api_token_expires_after_start_ck CHECK (expires_at > created_at)
);

COMMENT ON TABLE api_token IS
    'A bearer credential held by a program rather than a person. Scoped by kind '
    'rather than by permission: it reaches the import endpoints and nothing '
    'else. Unreachable to spork_app; the definers are the interface. D158.';

-- ---------------------------------------------------------------------------
-- The interface
-- ---------------------------------------------------------------------------

-- Mint one. Called on a session, so a person is always the one asking.
CREATE FUNCTION api_token_open(
        p_tenant     uuid,
        p_created_by uuid,
        p_label      text,
        p_digest     bytea,
        p_lifetime   interval)
    RETURNS uuid
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    t_id uuid;
BEGIN
    -- The same membership test `session_open` makes, for the same reason: a
    -- token for a tenant its minter does not belong to would be a way to reach
    -- another company's data by asking politely.
    IF NOT EXISTS (
        SELECT 1 FROM person_tenant
         WHERE person_id = p_created_by AND tenant_id = p_tenant AND left_at IS NULL)
    THEN
        RAISE EXCEPTION 'person % is not a current member of tenant %', p_created_by, p_tenant;
    END IF;

    INSERT INTO api_token (token_sha256, tenant_id, created_by_id, label, expires_at)
    VALUES (p_digest, p_tenant, p_created_by, btrim(p_label), now() + p_lifetime)
    RETURNING id INTO t_id;
    RETURN t_id;
END
$$;

-- Resolve one, and touch the clock.
--
-- No idle timeout, unlike `session_resolve`. A loader that runs monthly is not
-- an abandoned terminal, and the thing an idle timeout protects against --- a
-- screen left open on a warehouse floor --- has no analogue here. The absolute
-- expiry is the whole of the limit, and it is on the row.
CREATE FUNCTION api_token_resolve(p_digest bytea)
    RETURNS TABLE (token_id uuid, tenant_id uuid, created_by_id uuid, label text)
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
BEGIN
    RETURN QUERY
    UPDATE api_token t
       SET last_used_at = now()
     WHERE t.token_sha256 = p_digest
       AND t.revoked_at IS NULL
       AND t.expires_at > now()
       -- **Membership is rechecked on every call**, not only at minting. A
       -- token minted by somebody who has since left is a credential belonging
       -- to nobody, and `session_resolve` refuses the same case.
       AND EXISTS (SELECT 1 FROM person_tenant pt
                    WHERE pt.person_id = t.created_by_id
                      AND pt.tenant_id = t.tenant_id
                      AND pt.left_at IS NULL)
    RETURNING t.id, t.tenant_id, t.created_by_id, t.label;
END
$$;

-- Withdraw one. Scoped to a tenant so that holding an id is not enough.
CREATE FUNCTION api_token_revoke(p_id uuid, p_tenant uuid)
    RETURNS boolean
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    hit boolean;
BEGIN
    UPDATE api_token
       SET revoked_at = now()
     WHERE id = p_id AND tenant_id = p_tenant AND revoked_at IS NULL;
    GET DIAGNOSTICS hit = ROW_COUNT;
    RETURN hit;
END
$$;

-- List them, without the digests, so a person can see what is outstanding.
CREATE FUNCTION api_tokens_for_tenant(p_tenant uuid)
    RETURNS TABLE (id uuid, label text, created_by_id uuid, created_at timestamptz,
                   expires_at timestamptz, last_used_at timestamptz, revoked_at timestamptz)
    LANGUAGE sql
    STABLE
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
    SELECT t.id, t.label, t.created_by_id, t.created_at,
           t.expires_at, t.last_used_at, t.revoked_at
      FROM api_token t
     WHERE t.tenant_id = p_tenant
     ORDER BY t.created_at DESC
$$;

-- ---------------------------------------------------------------------------
-- Privileges
-- ---------------------------------------------------------------------------
--
-- J37 wants a pinned `search_path` and no EXECUTE to PUBLIC; S53 wants an owner
-- that is neither SUPERUSER nor BYPASSRLS, which `spork_mediation_owner`
-- already satisfies. **No table grant to `spork_app` at any point**, which
-- is what makes J73 exempt this table and what makes the exemption meaningful.

ALTER FUNCTION api_token_open(uuid, uuid, text, bytea, interval)
    OWNER TO spork_mediation_owner;
ALTER FUNCTION api_token_resolve(bytea) OWNER TO spork_mediation_owner;
ALTER FUNCTION api_token_revoke(uuid, uuid) OWNER TO spork_mediation_owner;
ALTER FUNCTION api_tokens_for_tenant(uuid) OWNER TO spork_mediation_owner;

REVOKE EXECUTE ON FUNCTION api_token_open(uuid, uuid, text, bytea, interval) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION api_token_resolve(bytea) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION api_token_revoke(uuid, uuid) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION api_tokens_for_tenant(uuid) FROM PUBLIC;

GRANT EXECUTE ON FUNCTION api_token_open(uuid, uuid, text, bytea, interval) TO spork_app;
GRANT EXECUTE ON FUNCTION api_token_resolve(bytea) TO spork_app;
GRANT EXECUTE ON FUNCTION api_token_revoke(uuid, uuid) TO spork_app;
GRANT EXECUTE ON FUNCTION api_tokens_for_tenant(uuid) TO spork_app;

-- The owner needs what its functions touch. `person_tenant` it already has,
-- from migration 70.
GRANT SELECT, INSERT, UPDATE ON api_token TO spork_mediation_owner;

-- Provisioning is the platform's business too, on the same terms as credentials.
GRANT SELECT ON api_token TO spork_platform;
