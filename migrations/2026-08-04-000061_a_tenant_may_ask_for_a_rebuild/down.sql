-- Migration 61 down: app may no longer request a rebuild; dirty set goes.

DROP FUNCTION IF EXISTS projection_refresh_tenant(uuid);
DROP FUNCTION IF EXISTS projection_run_dirty();
DROP FUNCTION IF EXISTS projection_mark_dirty(uuid, text);

ALTER TABLE projection_freshness DROP COLUMN IF EXISTS last_on_demand_at;

DROP TABLE IF EXISTS projection_dirty;
