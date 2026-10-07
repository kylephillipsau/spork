-- Reverse of 2026-08-04-000130_a_list_is_renamed_and_put_away.
--
-- A list is never edited again. Its history goes; what each list is now, its
-- name and items, stays as it was left.

DROP TABLE IF EXISTS item_list_change;
REVOKE UPDATE (name, removed_at) ON item_list FROM spork_app;
REVOKE DELETE ON item_list_entry FROM spork_app;
ALTER TABLE item_list DROP COLUMN IF EXISTS removed_at;
