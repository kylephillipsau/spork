-- Reverse of 2026-08-04-000005_containment_intervals.

DROP FUNCTION IF EXISTS projection_package_stamp(uuid);
DROP FUNCTION IF EXISTS projection_package_containment_rebuild(uuid);

DELETE FROM projection_rebuild WHERE table_name = 'package_containment';
DELETE FROM projection_rebuild
 WHERE table_name = 'package'
   AND column_name IN ('placement_event_id', 'placement_occurred_at');

DROP TABLE IF EXISTS package_containment;

ALTER TABLE package
    DROP COLUMN IF EXISTS placement_occurred_at,
    DROP COLUMN IF EXISTS placement_event_id;

-- btree_gist is left installed: other tables will want the same interval idiom,
-- and dropping a shared extension in one migration's down is how the next
-- migration's up fails.
