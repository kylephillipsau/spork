-- Migration 96: where things are. D173.
--
-- A site's layout is **places**: boxes drawn inside other boxes, positioned
-- relative to their parent rather than measured. A place can hold a grid, and
-- every bin sits in one cell of one place. Nothing here is in millimetres.
--
-- # Relationships exact, positions approximate
--
-- Which bay comes next and which level is above are cell numbers, and they are
-- as exact as the bin labels on the racks. Where a place is drawn is somebody's
-- honest estimate, in cells of its parent. Nothing stores a position relative
-- to the whole site: it is composed from the chain of parents when it is read,
-- so moving a building moves everything in it and no copy can fall out of step.
--
-- # Why the six millimetre columns go
--
-- `location` has carried `x_mm` … `height_mm` since migration 1 and nothing has
-- ever written them. They promise a survey nobody will do, and a bin's place is
-- now its cell. A measured estimate, if one ever exists, belongs on the place
-- that holds the bin, as a better guess at the same drawing.

CREATE TABLE place (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id    uuid NOT NULL,
    site_id      uuid NOT NULL,
    -- NULL: it stands directly on the site.
    parent_id    uuid,
    name         text NOT NULL,
    -- Walk-through (a room, a walkway, a reserved area) or not (racking, a
    -- wall, a column). The only thing about a place the system needs to know
    -- to route around it; what it is called is up to the people who use it.
    solid        boolean NOT NULL DEFAULT false,

    -- Its box, in cells of its parent, from the parent's corner. z is up.
    x            double precision NOT NULL DEFAULT 0,
    y            double precision NOT NULL DEFAULT 0,
    z            double precision NOT NULL DEFAULT 0,
    length       double precision NOT NULL,
    depth        double precision NOT NULL,
    height       double precision NOT NULL,
    turn         smallint NOT NULL DEFAULT 0,
    -- A footprint that is not the rectangle: x, y pairs in its own cells.
    outline      double precision[],

    -- Its grid: bays along its length, levels up its height, rows into its
    -- depth, and how many bins share a bay at each level.
    bays         integer NOT NULL DEFAULT 1,
    levels       integer NOT NULL DEFAULT 1,
    rows         integer NOT NULL DEFAULT 1,
    positions    integer[],

    -- How its bins are named, and how the numbers on its labels run.
    bin_pattern  text,
    first_bay    integer NOT NULL DEFAULT 1,
    bay_step     integer NOT NULL DEFAULT 1,
    first_level  integer NOT NULL DEFAULT 1,

    CONSTRAINT place_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id),
    -- S51's and S37's targets, for a child's parent and a bin's place.
    CONSTRAINT place_tenant_key UNIQUE (id, tenant_id),
    CONSTRAINT place_site_key UNIQUE (id, site_id),
    CONSTRAINT place_parent_tenant_fk FOREIGN KEY (parent_id, tenant_id)
        REFERENCES place(id, tenant_id),
    -- A place inside another site's building would be drawn in the wrong one.
    CONSTRAINT place_parent_site_fk FOREIGN KEY (parent_id, site_id)
        REFERENCES place(id, site_id),
    CONSTRAINT place_not_its_own_parent_ck CHECK (parent_id <> id),
    -- Two places of one name side by side make a breadcrumb that cannot say
    -- which one you are in.
    CONSTRAINT place_name_key UNIQUE NULLS NOT DISTINCT (tenant_id, site_id, parent_id, name),
    CONSTRAINT place_name_ck CHECK (btrim(name) <> ''),

    -- Finite, and NaN excluded: in Postgres NaN sorts above infinity.
    CONSTRAINT place_box_ck CHECK (
        x > '-Infinity' AND x < 'Infinity'
        AND y > '-Infinity' AND y < 'Infinity'
        AND z > '-Infinity' AND z < 'Infinity'
        AND length > 0 AND length < 'Infinity'
        AND depth > 0 AND depth < 'Infinity'
        AND height > 0 AND height < 'Infinity'),
    CONSTRAINT place_turn_ck CHECK (turn >= 0 AND turn < 360),
    CONSTRAINT place_outline_ck CHECK (
        outline IS NULL
        OR (array_ndims(outline) = 1
            AND array_position(outline, NULL) IS NULL
            AND cardinality(outline) >= 6
            AND cardinality(outline) % 2 = 0)),
    -- A grid is rows and columns. An L-shaped run of shelving is two runs.
    CONSTRAINT place_grid_is_rectangular_ck CHECK (
        outline IS NULL OR (bays = 1 AND levels = 1 AND rows = 1)),
    CONSTRAINT place_grid_ck CHECK (bays >= 1 AND levels >= 1 AND rows >= 1),
    -- `0 < ALL (...)` is NULL, and so passes, when an element is NULL.
    CONSTRAINT place_positions_ck CHECK (
        positions IS NULL
        OR (array_ndims(positions) = 1
            AND array_position(positions, NULL) IS NULL
            AND cardinality(positions) = levels
            AND 0 < ALL (positions))),
    CONSTRAINT place_numbering_ck CHECK (
        bay_step <> 0 AND first_bay >= 0 AND first_level >= 0
        AND first_bay + (bays - 1) * bay_step >= 0),
    CONSTRAINT place_pattern_ck CHECK (bin_pattern IS NULL OR bin_pattern LIKE '%{%}%')
);

COMMENT ON TABLE place IS
    'REFERENCE, tenant-scoped. A box drawn inside another place, or on the '
    'site: a building, a room, a mezzanine, a rack, a reserved area, a column. '
    'Positioned relative to its parent, in cells, never measured. A place can '
    'hold a grid, and every bin sits in one of its cells. D173.';
COMMENT ON COLUMN place.solid IS
    'False for somewhere you can walk (a room, a walkway, a reserved area), '
    'true for somewhere you cannot (racking, a wall, a column). The name says '
    'what it is; this says how to get around it. D173.';
COMMENT ON COLUMN place.x IS
    'With y and z: its corner, in cells of its parent, from the parent''s '
    'corner. An estimate: drawn, not measured. D173.';
COMMENT ON COLUMN place.turn IS
    'Degrees counter-clockwise about its corner. At 0 its length runs along the '
    'parent''s x and its front, the side you face it from, is its low-y edge. '
    'D173.';
COMMENT ON COLUMN place.outline IS
    'Its footprint when it is not the rectangle: x, y pairs in its own cells, '
    'in order around the edge. An L-shaped building, a notch, an angled wall. '
    'D173.';
COMMENT ON COLUMN place.positions IS
    'How many bins share each bay at each level, lowest first. NULL is one. '
    'D173.';
COMMENT ON COLUMN place.bin_pattern IS
    'How its bins are named: literal text with {bay}, {level}, {row} and '
    '{position}, each optionally padded ({bay:02}) or lettered ({level:A}). '
    'Bins whose codes it produces drop into their cells. D173.';
COMMENT ON COLUMN place.bay_step IS
    'The difference between one bay''s label and the next: 2 when odd bays face '
    'one aisle and even the other, negative when they count down. D173.';

ALTER TABLE place ENABLE ROW LEVEL SECURITY;
ALTER TABLE place FORCE ROW LEVEL SECURITY;
CREATE POLICY place_tenant_scoped ON place
    USING (tenant_id = current_tenant());

CREATE INDEX place_parent_idx ON place (parent_id) WHERE parent_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- A bin's cell
-- ---------------------------------------------------------------------------

-- First statement that adds a column, so the server tests' schema probe asks
-- for this one.
ALTER TABLE location ADD COLUMN place_id uuid;
ALTER TABLE location ADD COLUMN slot_bay integer;
ALTER TABLE location ADD COLUMN slot_level integer;
ALTER TABLE location ADD COLUMN slot_row integer;
ALTER TABLE location ADD COLUMN slot_position integer;

ALTER TABLE location
    ADD CONSTRAINT location_place_tenant_fk FOREIGN KEY (place_id, tenant_id)
        REFERENCES place(id, tenant_id),
    -- S37's reason: a bin in another site's rack is a bin in the wrong building.
    ADD CONSTRAINT location_place_site_fk FOREIGN KEY (place_id, site_id)
        REFERENCES place(id, site_id),
    -- A cell is whole or absent: a bay without a level is not somewhere.
    ADD CONSTRAINT location_slot_whole_ck CHECK (
        num_nonnulls(place_id, slot_bay, slot_level, slot_row, slot_position) IN (0, 5)),
    ADD CONSTRAINT location_slot_ck CHECK (
        slot_bay >= 1 AND slot_level >= 1 AND slot_row >= 1 AND slot_position >= 1);

-- One bin to a cell. Which bin is where is the exact half of the layout, and
-- two in one cell would make a pick list that cannot say which to take.
CREATE UNIQUE INDEX location_slot_key
    ON location (place_id, slot_bay, slot_level, slot_row, slot_position)
    WHERE place_id IS NOT NULL;

COMMENT ON COLUMN location.place_id IS
    'The place whose grid holds this bin, with slot_bay, slot_level, slot_row '
    'and slot_position: its cell, counted from 1. NULL is not on the layout '
    'yet, which J77 reports once its site has one. D173.';

ALTER TABLE location
    DROP COLUMN x_mm,
    DROP COLUMN y_mm,
    DROP COLUMN z_mm,
    DROP COLUMN length_mm,
    DROP COLUMN width_mm,
    DROP COLUMN height_mm;

-- ---------------------------------------------------------------------------
-- Grants (D25)
-- ---------------------------------------------------------------------------

GRANT SELECT, INSERT, UPDATE, DELETE ON place TO spork_app;
GRANT SELECT ON place TO spork_platform, spork_scheduler;
