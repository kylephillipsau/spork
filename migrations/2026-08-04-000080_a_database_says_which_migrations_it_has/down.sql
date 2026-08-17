-- Migration 80 down: the database stops saying what it has.
--
-- Reversing this loses the record of which migrations are applied, and there is
-- no way to recompute it — inferring a version from artefacts is exactly what
-- this table exists to replace. A database that goes back through here needs
-- `scripts/migrate.sh --baseline` before it can be deployed to again.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM schema_migration;
    IF n > 0 THEN
        RAISE NOTICE 'reversing migration 80 forgets % applied migration(s); run scripts/migrate.sh --baseline before deploying to this database again', n;
    END IF;
END
$$;

DROP TABLE schema_migration;
