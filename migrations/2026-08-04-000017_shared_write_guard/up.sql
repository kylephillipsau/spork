-- Migration 17: the shared catalogue was readable by everyone and writable by anyone.
--
-- Migration 1 states the rule in its own grants section:
--
--   "Shared rows belong to the platform. The application may read the shared
--    catalogue and write only its own rows, which RLS enforces per row; the
--    platform role is what writes tenant_id IS NULL."
--
-- RLS did not enforce it. The sentence has been in the oldest file in the
-- repository since the first migration, describing a guarantee the mechanism
-- never provided, which is the failure this project has now found in schema
-- comments, in projection markers, in pending reasons and in its own counts.
--
-- ---------------------------------------------------------------------------
-- What was actually true
-- ---------------------------------------------------------------------------
--
-- Shape 2 was one policy carrying one expression:
--
--   CREATE POLICY item_shared_reference ON item
--       USING (tenant_id IS NULL OR tenant_id = current_tenant());
--
-- Postgres uses the USING expression as the WITH CHECK when none is given, so
-- that clause authorised writes as well as reads. Verified against the live
-- schema as spork_app with a tenant context set:
--
--   INSERT INTO source_channel (tenant_id, ...) VALUES (NULL, 'evil_shared', ...)
--   -> INSERT 0 1
--   UPDATE source_channel SET name = 'hijacked' WHERE code = 'manual'
--   -> UPDATE 1
--
-- Any tenant could mint rows visible to every other tenant, and rewrite the
-- platform-shipped ones. The reach is the shared catalogue itself: `item`
-- (D19's thin shared item), `metric` (D23's vocabulary, whose platform codes S21
-- reserves), `policy_binding` (a tenant-authored binding that resolves for
-- everybody, which is precisely what J14 was written to catch after the fact),
-- and the freight and reason vocabularies.
--
-- S9 could not see it. It asserts exactly three RLS shapes exist and reads
-- `polqual`, the USING expression — which was correct. The read side was never
-- the problem, and nothing looked at the write side because the write side was
-- never written down.
--
-- ---------------------------------------------------------------------------
-- Why an added WITH CHECK is not enough
-- ---------------------------------------------------------------------------
--
-- DELETE has no WITH CHECK. It is governed entirely by USING, so a policy that
-- admits shared rows for reading admits them for deletion however carefully its
-- WITH CHECK is written. The application holds DELETE on these tables and needs
-- it, for its own rows.
--
-- So the shape splits in two, which is what it always meant:
--
--   <table>_shared_read   FOR SELECT   shared rows and ours
--   <table>_own_write     FOR ALL      ours, or shared if we are the platform
--
-- Permissive policies are OR'd per command. SELECT sees both and gets the union.
-- INSERT, UPDATE and DELETE see only the write policy, so a shared row is
-- unreachable by all three unless the caller is the platform.

-- ---------------------------------------------------------------------------
-- Who the platform is
-- ---------------------------------------------------------------------------
--
-- `spork_platform` is not a superuser and does not carry BYPASSRLS, so it is
-- subject to these policies like anything else and needs an arm that admits it.
-- Migrations run as the owner, which is a superuser and bypasses RLS entirely,
-- so seeding shared rows from a migration is unaffected either way.
--
-- STABLE and not SECURITY DEFINER: it reports on the caller, so running it as
-- anybody else would make it answer the wrong question.

CREATE FUNCTION is_platform() RETURNS boolean
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$ SELECT pg_has_role(current_user, 'spork_platform', 'MEMBER') $$;

COMMENT ON FUNCTION is_platform() IS
    'Whether the caller may write shared rows. The platform ships the catalogue; '
    'a tenant may read it and may not edit it. D19, D55.';

GRANT EXECUTE ON FUNCTION is_platform() TO spork_app, spork_platform,
    spork_scheduler, spork_projection_owner;

-- ---------------------------------------------------------------------------
-- The split, derived rather than listed
-- ---------------------------------------------------------------------------
--
-- Every policy whose USING is the shared-reference expression, found from the
-- catalogue rather than from a list written here. A list would be the thing this
-- migration exists to stop trusting: it would be correct today and silently
-- short by one the next time somebody adds a shared table.

DO $$
DECLARE
    r record;
    split_count integer := 0;
BEGIN
    FOR r IN
        SELECT c.relname AS tbl, p.polname AS pol
          FROM pg_policy p
          JOIN pg_class c ON c.oid = p.polrelid
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'public'
           AND pg_get_expr(p.polqual, p.polrelid) =
               '((tenant_id IS NULL) OR (tenant_id = current_tenant()))'
    LOOP
        EXECUTE format('DROP POLICY %I ON %I', r.pol, r.tbl);

        EXECUTE format(
            'CREATE POLICY %I ON %I FOR SELECT
                 USING (tenant_id IS NULL OR tenant_id = current_tenant())',
            r.tbl || '_shared_read', r.tbl);

        EXECUTE format(
            'CREATE POLICY %I ON %I FOR ALL
                 USING (tenant_id = current_tenant()
                        OR (tenant_id IS NULL AND is_platform()))
                 WITH CHECK (tenant_id = current_tenant()
                        OR (tenant_id IS NULL AND is_platform()))',
            r.tbl || '_own_write', r.tbl);

        split_count := split_count + 1;
    END LOOP;

    RAISE NOTICE 'split % shared-reference table(s) into a read policy and a write policy', split_count;
END
$$;

-- ---------------------------------------------------------------------------
-- What this does not change
-- ---------------------------------------------------------------------------
--
-- The tenant-scoped shape is untouched and was already correct: its USING is
-- `tenant_id = current_tenant()`, which becomes the same WITH CHECK, so a write
-- naming another tenant is rejected. Confirmed before this migration was
-- written, because a fix aimed at the wrong shape is worse than none.
--
-- The global shape carries no policy and holds nothing tenant-specific.
--
-- Grants are unchanged. The application still holds INSERT, UPDATE and DELETE on
-- the shared-reference tables, and still needs them, for its own rows. **This is
-- the division D25 draws everywhere else**: the grant decides which verbs, the
-- policy decides which rows, and the mistake here was asking the grant to carry
-- a distinction only the policy can make.
