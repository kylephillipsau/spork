-- Reverse of 2026-08-04-000003_projection.
--
-- Roles are left alone, as in migration 1: they may own objects from later
-- migrations, and dropping one out from under them fails less legibly than the
-- leftover role it was avoiding.

DROP FUNCTION IF EXISTS projection_stock_rebuild(uuid);
DROP TABLE IF EXISTS stock_allocation;
DROP TABLE IF EXISTS projection_rebuild;
