-- Migration 77 down: the two roles lose the schema again.
--
-- This restores a schema in which every projection maintainer is broken, which
-- is what the state before this migration was. A down migration returns the
-- database to the shape it had, defects included.

REVOKE USAGE ON SCHEMA public FROM spork_projection_owner, spork_scheduler;
