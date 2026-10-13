-- Migration 139 down: a containment says no spot, and every pallet goes as one.

DROP FUNCTION package_parent_now(uuid);
ALTER TABLE package DROP COLUMN breaks_down;
ALTER TABLE package_event
    DROP CONSTRAINT package_event_spot_ck,
    DROP COLUMN turned,
    DROP COLUMN at_z_mm,
    DROP COLUMN at_y_mm,
    DROP COLUMN at_x_mm;
