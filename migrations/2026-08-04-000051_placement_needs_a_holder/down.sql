-- Reverse of 2026-08-04-000051_placement_needs_a_holder.
--
-- A `created` event may again say a package exists without saying where, which the
-- containment fold cannot represent and raises on rather than reporting.

ALTER TABLE package_event
    DROP CONSTRAINT IF EXISTS package_event_placement_needs_holder_ck;
