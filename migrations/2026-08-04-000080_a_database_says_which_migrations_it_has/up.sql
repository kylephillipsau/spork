-- Migration 80: a database says which migrations it has.
--
-- Seventy-nine migrations and nothing recorded which of them had been applied.
-- That was survivable while every deployment was one laptop and one person who
-- remembered, and it stopped being survivable twice in one session:
--
--   * migration 78 was missing from the compose database, `docker compose up -d
--     --build` restarted happily against it, the server logged a clean start,
--     and the only reason it was noticed is that somebody went looking.
--   * the second time, 79, the order was got right on purpose — and getting it
--     right was a thing a person had to remember rather than a thing the deploy
--     could not get wrong.
--
-- **Nothing here checks a schema at boot.** The fix is ordering rather than
-- assertion: `scripts/migrate.sh` runs to completion before the server starts,
-- so a binary ahead of its schema is not a state the system can be in. This
-- table is what makes "run only what is pending" answerable at all.
--
-- # Why this is not derivable
--
-- The obvious alternative is to probe for artefacts — does `item_barcode`
-- exist, does this constraint carry its sentinels — and infer a version. That
-- is what was done by hand, it took several rounds, and one of the probes was
-- for a table that has never existed in any migration, which answers false
-- exactly like a missing one. A list of names is not clever and cannot be wrong
-- in that way.

CREATE TABLE schema_migration (
    -- The directory name, verbatim: `2026-08-04-000080_a_database_says…`. Not a
    -- number, because the number is a prefix of the name and a name is the thing
    -- the filesystem actually orders by — the same ordering `verify-migrations.sh`
    -- has always relied on.
    name        text PRIMARY KEY,

    -- When it went on. Transaction time only: a migration has no valid time
    -- distinct from when it ran, unlike every fact table in this schema.
    applied_at  timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE schema_migration IS
    'Which migrations this database has, by directory name. Written only by '
    'scripts/migrate.sh, one row per migration, in the same transaction as the '
    'migration itself. Not a fact table and not tenant-scoped: the schema is a '
    'property of the database rather than of anybody in it.';

-- ---------------------------------------------------------------------------
-- No RLS, and no grants to the application roles
-- ---------------------------------------------------------------------------
--
-- Every other table here is tenant-scoped because it holds somebody's data.
-- This holds the shape of the database itself, which belongs to no tenant and
-- is the same for all of them — so `current_tenant()` has nothing to say about
-- it and a policy would be a policy over a constant.
--
-- The application roles get nothing at all. `nylonite_app` has no reason to read
-- which migrations exist and every reason not to be able to write it: a role
-- that can insert into this table can make a pending migration look applied,
-- which is the one lie that would defeat the whole mechanism. The migrator
-- connects as the superuser that owns the schema, and it is the only writer.

-- ---------------------------------------------------------------------------
-- This migration does not record itself
-- ---------------------------------------------------------------------------
--
-- It could — `INSERT INTO schema_migration VALUES ('2026-08-04-000080_…')` — and
-- then this file would carry its own name as a string, which is the kind of
-- duplication that survives a rename and starts lying. `migrate.sh` inserts the
-- row for whatever directory it just applied, including this one, in the same
-- transaction. One writer, one rule, no special case.
--
-- A database built by `verify-migrations.sh` therefore has this table and no
-- rows in it, which is correct: that harness applies every file to a throwaway
-- database and is deliberately not a deploy.
