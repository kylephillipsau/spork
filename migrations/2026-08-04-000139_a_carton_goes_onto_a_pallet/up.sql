-- Migration 139: a carton goes onto a pallet. D246, D247.
--
-- Containment was there (D24, D166) and nothing used it. To build pallets on,
-- three things more:
--
-- - **Where on the pallet it went**: a `contained` event can say the spot,
--   from the front left corner of the parent's deck (front being where the
--   forks go in) and whether it is turned a quarter round. All or nothing,
--   and only on a containment. A breakdown pallet built through the day is
--   planned round the spots already taken (D248).
-- - **A pallet goes as one, or is broken down** (`package.breaks_down`): its
--   parcels delivered each on their own from the carrier's depot. What is
--   delivered, labelled, booked and despatched follows from it (D247).
-- - **Its parent now**, from its events rather than the lagging fold, so a
--   loop or a fourth level is refused at the act that would make it.

ALTER TABLE package_event
    ADD COLUMN at_x_mm integer,
    ADD COLUMN at_y_mm integer,
    ADD COLUMN at_z_mm integer,
    ADD COLUMN turned boolean,
    ADD CONSTRAINT package_event_spot_ck CHECK (
        num_nonnulls(at_x_mm, at_y_mm, at_z_mm, turned) IN (0, 4)
        AND (at_x_mm IS NULL OR kind = 'contained')
        AND (at_x_mm IS NULL OR (at_x_mm >= 0 AND at_y_mm >= 0 AND at_z_mm >= 0)));

COMMENT ON COLUMN package_event.at_x_mm IS
    'Where on its parent a contained package went: across from the front left '
    'corner of the deck, front being where the forks go in. With at_y_mm (back '
    'from the front), at_z_mm (up from the deck) and turned (a quarter turn). '
    'D246, migration 139.';

ALTER TABLE package
    ADD COLUMN breaks_down boolean NOT NULL DEFAULT false;
COMMENT ON COLUMN package.breaks_down IS
    'A pallet or skid broken down at the carrier''s depot, its parcels each '
    'delivered on their own, rather than delivered as one. D247, migration 139.';
GRANT INSERT (breaks_down), UPDATE (breaks_down) ON package TO spork_app;

-- A package's parent now: the parent its newest placement names, or NULL
-- when that placement is at a location. The fold's own rule (J6), live.
CREATE FUNCTION package_parent_now(p_package uuid)
RETURNS uuid
LANGUAGE sql STABLE AS $$
    SELECT parent_package_id FROM package_event
     WHERE package_id = p_package
       AND asserts_placement
     ORDER BY occurred_at DESC, recorded_at DESC, id DESC
     LIMIT 1
$$;

COMMENT ON FUNCTION package_parent_now(uuid) IS
    'The package a package is in or on now, from its newest placement event; '
    'NULL when that placement is at a location. D246.';
