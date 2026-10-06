-- Migration 126 down: lines no longer say which kit they are part of.

REVOKE INSERT (kit_line_id), UPDATE (kit_line_id) ON order_line FROM spork_app;
DROP INDEX IF EXISTS order_line_kit_idx;
ALTER TABLE order_line DROP CONSTRAINT IF EXISTS order_line_kit_not_itself_ck;
ALTER TABLE order_line DROP CONSTRAINT IF EXISTS order_line_kit_fk;
ALTER TABLE order_line DROP CONSTRAINT IF EXISTS order_line_kit_tenant_fk;
ALTER TABLE order_line DROP COLUMN IF EXISTS kit_line_id;
