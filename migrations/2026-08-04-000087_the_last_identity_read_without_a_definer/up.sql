-- The membership read joins the pattern migration 70 established for the rest.
--
-- # What went looking, and what it found
--
-- J73 arrived in the previous migration to answer a narrow question: a table
-- with a `tenant_id` and row level security switched off had reached the schema
-- and no check had noticed. Written to look at every tenant-scoped table rather
-- than only the projected ones, it reported three. `reported_stock` was the one
-- it was written for. `session` was a false alarm with a real answer -- the
-- application holds no privilege on it at all, so it is closed by grant and
-- needs no policy, which is now what the check itself asks.
--
-- `person_tenant` was neither. It carries a `tenant_id`, has no policy, and
-- `spork_app` holds `SELECT` on it. With a tenant set, the application role
-- can read every membership row of every other tenant: which people work for
-- which company, across the whole deployment. That is not a large secret and it
-- is not nothing, and it is the one identity table the pattern skipped.
--
-- # Why it cannot simply have a policy
--
-- The obvious repair is a tenant-scoped policy, and it does not work here.
-- `issue_session` reads memberships to decide *which* tenant a session names,
-- which is by definition before there is one. `current_tenant()` is null there
-- and migration 1 fixed the meaning of that null: every tenant-scoped policy
-- treats it as matching nothing. A policy would return zero memberships and
-- nobody would ever sign in.
--
-- The other tempting repair is a policy that opens when the setting is unset.
-- That inverts the one convention the whole tenancy boundary rests on, and it
-- fails in the direction that hurts: any path that forgot to set the tenant
-- would silently read everybody's rows. A rule whose failure mode is silent
-- disclosure is worse than no rule, because it reads like a rule.
--
-- # So it joins the others
--
-- Migration 70's design is that the identity tables are unreachable and the
-- definer functions are the interface, and every other identity read already
-- goes through one -- `credential_for_login`, `session_resolve`,
-- `passkeys_for_person`, `passkey_disable`. This read is the only one that did
-- not. Nothing new is being invented here; the exception is being removed.
--
-- The function is deliberately no narrower than the grant it replaces. The
-- application could already ask this of any person, so answering it through a
-- definer widens nothing -- it just makes the whole table stop being readable
-- for anything else. `issue_session` calls it about a person who has proved who
-- they are one statement earlier.

CREATE FUNCTION memberships_of(p_person uuid)
    RETURNS TABLE (tenant_id uuid, name text)
    LANGUAGE sql
    STABLE
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
    SELECT pt.tenant_id, t.name
      FROM person_tenant pt
      JOIN tenant t ON t.id = pt.tenant_id
     WHERE pt.person_id = p_person AND pt.left_at IS NULL
     ORDER BY t.name
$$;

COMMENT ON FUNCTION memberships_of(uuid) IS
    'The tenants a person is a current member of, by name. The only interface '
    'to person_tenant the application has, because the choice it feeds is made '
    'before a tenant is set and no policy can be written for that moment. '
    'Migration 87.';

ALTER FUNCTION memberships_of(uuid) OWNER TO spork_mediation_owner;
REVOKE EXECUTE ON FUNCTION memberships_of(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION memberships_of(uuid) TO spork_app;

-- **The owner needs what its function reads**, which is the sentence migration
-- 70 already wrote for `person` and `person_tenant`. The join adds `tenant`.
GRANT SELECT ON tenant TO spork_mediation_owner;

-- And the grant this replaces. Migration 70 gave it with a reason -- "so a
-- sign-on can offer the choice when there is more than one" -- and the reason
-- survives; it is the whole table being readable to satisfy it that does not.
REVOKE SELECT ON person_tenant FROM spork_app;
