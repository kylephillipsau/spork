-- Reverse of 2026-08-04-000017_shared_write_guard.
--
-- Restores the single permissive policy per table, which is the shape that let
-- any tenant write the shared catalogue. Reversible means reversible; the
-- migration that reintroduces a hole is still the migration that reverses this
-- one, and pretending otherwise would make the pair untestable.

DO $$
DECLARE
    r record;
BEGIN
    FOR r IN
        SELECT c.relname AS tbl
          FROM pg_policy p
          JOIN pg_class c ON c.oid = p.polrelid
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'public'
           AND p.polname = c.relname || '_shared_read'
    LOOP
        EXECUTE format('DROP POLICY %I ON %I', r.tbl || '_shared_read', r.tbl);
        EXECUTE format('DROP POLICY %I ON %I', r.tbl || '_own_write', r.tbl);
        EXECUTE format(
            'CREATE POLICY %I ON %I
                 USING (tenant_id IS NULL OR tenant_id = current_tenant())',
            r.tbl || '_shared_reference', r.tbl);
    END LOOP;
END
$$;

DROP FUNCTION IF EXISTS is_platform();
