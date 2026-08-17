-- Reverse of 2026-08-04-000004_containment.

DROP FUNCTION IF EXISTS projection_stock_resolve_locations(uuid);
DROP FUNCTION IF EXISTS projection_package_rebuild(uuid);

DELETE FROM projection_rebuild WHERE table_name = 'package';

ALTER TABLE package
    DROP COLUMN IF EXISTS identifier_kind,
    DROP COLUMN IF EXISTS depth,
    DROP COLUMN IF EXISTS status,
    DROP COLUMN IF EXISTS resolved_location_id,
    DROP COLUMN IF EXISTS location_id,
    DROP COLUMN IF EXISTS parent_package_id;

DROP TABLE IF EXISTS package_event;
