-- Migration 96: where the racks stand. D173.
--
-- `location` has had `x_mm`, `y_mm`, `z_mm`, `length_mm`, `width_mm` and
-- `height_mm` since migration 1, and nothing has ever written them. Coordinates
-- for thousands of bins will not be typed in one at a time; they come from a
-- description of each rack, which is a few dozen rows a person can check.
--
-- # A rack generates its bins' boxes
--
-- A rack is one run of bays along one aisle face. Expanding it gives every
-- slot's code (from `code_template`) and box (from the origin, rotation, bay
-- widths and level heights). A slot whose code is a bin on file is placed; a slot
-- whose code is not is reported by the import and not created, because the bin
-- list says what exists and the rack only says where.
--
-- # A slot is not stored
--
-- It is a function of its rack row. Storing it would be a second copy of the
-- rack to drift from the first. What is stored is the result on the bin, with
-- `geometry_source` saying where it came from, so a measured box is never
-- overwritten by a drawing of what the rack should be.
--
-- # Quarter turns only
--
-- Every bin stays an axis-aligned box, so the six columns mean one thing and the
-- view's picking stays a ray against boxes. D173 says why waiting is cheap.
--
-- # Arrays, not child tables
--
-- Bay widths, level heights and positions per level are short ordered lists that
-- mean nothing apart from their rack and are always read whole. The CHECKs below
-- hold what SQL can: one dimension, no NULL element (`0 < ALL (...)` is NULL,
-- and therefore passes, when an element is NULL), lengths that agree. That the
-- levels ascend is held by the importer, which is the only writer.

CREATE TABLE rack (
    id             uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id      uuid NOT NULL,
    site_id        uuid NOT NULL,
    code           text NOT NULL,
    aisle          text NOT NULL,

    x_mm           integer NOT NULL,
    y_mm           integer NOT NULL,
    rotation       smallint NOT NULL,
    depth_mm       integer NOT NULL,
    height_mm      integer NOT NULL,
    upright_mm     integer NOT NULL DEFAULT 0,

    first_bay      integer NOT NULL,
    bay_step       integer NOT NULL DEFAULT 1,
    bay_widths_mm  integer[] NOT NULL,
    first_level    integer NOT NULL DEFAULT 1,
    level_z_mm     integer[] NOT NULL,
    positions      integer[] NOT NULL,
    code_template  text NOT NULL,

    active         boolean NOT NULL DEFAULT true,

    CONSTRAINT rack_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id),
    CONSTRAINT rack_tenant_site_code_key UNIQUE (tenant_id, site_id, code),
    -- S51's and S37's targets, for `location.rack_id`.
    CONSTRAINT rack_tenant_key UNIQUE (id, tenant_id),
    CONSTRAINT rack_site_key UNIQUE (id, site_id),

    CONSTRAINT rack_rotation_ck CHECK (rotation IN (0, 90, 180, 270)),
    CONSTRAINT rack_extent_ck CHECK (depth_mm > 0 AND height_mm > 0 AND upright_mm >= 0),
    CONSTRAINT rack_bays_ck CHECK (
        array_ndims(bay_widths_mm) = 1
        AND array_position(bay_widths_mm, NULL) IS NULL
        AND 0 < ALL (bay_widths_mm)),
    -- Every bay's label is a number a code can carry, whichever way they run.
    CONSTRAINT rack_bay_numbers_ck CHECK (
        bay_step <> 0 AND first_bay >= 0
        AND first_bay + (cardinality(bay_widths_mm) - 1) * bay_step >= 0),
    CONSTRAINT rack_levels_ck CHECK (
        array_ndims(level_z_mm) = 1
        AND array_position(level_z_mm, NULL) IS NULL
        AND level_z_mm[1] >= 0
        AND level_z_mm[cardinality(level_z_mm)] < height_mm
        AND first_level >= 0),
    CONSTRAINT rack_positions_ck CHECK (
        array_ndims(positions) = 1
        AND array_position(positions, NULL) IS NULL
        AND cardinality(positions) = cardinality(level_z_mm)
        AND 0 < ALL (positions)),
    CONSTRAINT rack_template_ck CHECK (
        code_template LIKE '%{bay%' AND code_template LIKE '%{level%')
);

COMMENT ON TABLE rack IS
    'REFERENCE, tenant-scoped. One run of bays along one aisle face, placed on '
    'its site''s plan. Its bins'' codes and boxes are generated from it and are '
    'not stored as slots: a slot is a function of this row. D173.';
COMMENT ON COLUMN rack.aisle IS
    'The aisle every code this rack generates decomposes to. The importer refuses '
    'a template that disagrees, because J77 reads bins by aisle. D173.';
COMMENT ON COLUMN rack.x_mm IS
    'With y_mm: the corner at the left end of the rack''s front, standing in the '
    'aisle facing it, on the site plan in millimetres. D173.';
COMMENT ON COLUMN rack.rotation IS
    'Quarter turns counter-clockwise, in degrees. At 0 the rack runs along +x and '
    'its depth along +y, so it is faced from lower y. D173.';
COMMENT ON COLUMN rack.upright_mm IS
    'The frame between bays, and at each end. Bay widths are clear widths. D173.';
COMMENT ON COLUMN rack.bay_step IS
    'The difference between one bay''s number and the next along the rack. 2 when '
    'odd bays face one side of the aisle and even the other; negative when they '
    'count down left to right. Never 0. D173.';
COMMENT ON COLUMN rack.level_z_mm IS
    'The height of each level''s floor, lowest first. A level reaches to the next '
    'one, and the top level to height_mm. Ascending, held by the importer. D173.';
COMMENT ON COLUMN rack.positions IS
    'How many bins share each bay at each level, one entry per level, splitting '
    'the bay''s width evenly. D173.';
COMMENT ON COLUMN rack.code_template IS
    'How a slot is named: {aisle}, {bay}, {level} and {position}, each optionally '
    'zero-padded ({bay:02}) or, for a level or position, lettered ({level:A}, 1 '
    'is A). D173.';

ALTER TABLE rack ENABLE ROW LEVEL SECURITY;
ALTER TABLE rack FORCE ROW LEVEL SECURITY;
CREATE POLICY rack_tenant_scoped ON rack
    USING (tenant_id = current_tenant());

-- ---------------------------------------------------------------------------
-- A bin's box, and where it came from
-- ---------------------------------------------------------------------------

-- First after the tables, so the schema probe in the server's tests asks for
-- this column.
ALTER TABLE location ADD COLUMN rack_id uuid;
ALTER TABLE location ADD COLUMN geometry_source text;

ALTER TABLE location
    ADD CONSTRAINT location_rack_tenant_fk FOREIGN KEY (rack_id, tenant_id)
        REFERENCES rack(id, tenant_id),
    -- S37's reason: a bin on another site's rack would be drawn in the wrong
    -- building.
    ADD CONSTRAINT location_rack_site_fk FOREIGN KEY (rack_id, site_id)
        REFERENCES rack(id, site_id),
    -- A box is whole or absent. Five of six cannot be drawn, and would leave
    -- `geometry_source` describing a box that is not there.
    ADD CONSTRAINT location_geometry_whole_ck CHECK (
        num_nonnulls(x_mm, y_mm, z_mm, length_mm, width_mm, height_mm) IN (0, 6)),
    ADD CONSTRAINT location_geometry_extent_ck CHECK (
        length_mm > 0 AND width_mm > 0 AND height_mm > 0),
    -- A template box needs the rack it was generated from.
    ADD CONSTRAINT location_geometry_source_ck CHECK (
        geometry_source IN ('template', 'survey', 'manual')
        AND (geometry_source <> 'template' OR rack_id IS NOT NULL)),
    ADD CONSTRAINT location_geometry_sourced_ck CHECK (
        (geometry_source IS NULL) = (x_mm IS NULL));

COMMENT ON COLUMN location.rack_id IS
    'The rack whose slot this bin is, when one is. Set whether or not the box '
    'came from the rack: a surveyed bin is still on its rack. D173.';
COMMENT ON COLUMN location.geometry_source IS
    'Where the box came from: template (generated from rack_id), survey '
    '(measured) or manual (entered). A generator writes only over NULL or '
    'template. NULL exactly when there is no box. D173.';
COMMENT ON COLUMN location.x_mm IS
    'With y_mm and z_mm: the corner of the bin''s box nearest the site plan''s '
    'origin, in millimetres; x right, y up the page, z up from the floor. D173.';
COMMENT ON COLUMN location.length_mm IS
    'The box''s extent along x, not its frontage. D173.';
COMMENT ON COLUMN location.width_mm IS
    'The box''s extent along y, not its depth. D173.';
COMMENT ON COLUMN location.height_mm IS
    'The box''s extent along z. D173.';

CREATE INDEX location_rack_idx ON location (rack_id) WHERE rack_id IS NOT NULL;
-- J76 walks each site's boxes along x.
CREATE INDEX location_geometry_idx ON location (tenant_id, site_id, x_mm)
    WHERE x_mm IS NOT NULL;

-- ---------------------------------------------------------------------------
-- Floor areas
-- ---------------------------------------------------------------------------

-- A floor area can name the location it is, and it must be one of its own
-- site's (S37), which needs this key to point at.
ALTER TABLE location ADD CONSTRAINT location_site_key UNIQUE (id, site_id);

CREATE TABLE floor_area (
    id           uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id    uuid NOT NULL,
    site_id      uuid NOT NULL,
    code         text NOT NULL,
    kind         text NOT NULL,
    outline_mm   integer[] NOT NULL,
    height_mm    integer,
    location_id  uuid,

    CONSTRAINT floor_area_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id),
    CONSTRAINT floor_area_location_tenant_fk FOREIGN KEY (location_id, tenant_id)
        REFERENCES location(id, tenant_id),
    CONSTRAINT floor_area_location_site_fk FOREIGN KEY (location_id, site_id)
        REFERENCES location(id, site_id),
    CONSTRAINT floor_area_tenant_site_code_key UNIQUE (tenant_id, site_id, code),
    CONSTRAINT floor_area_kind_ck CHECK (kind IN
        ('dock', 'staging', 'packing', 'walkway', 'wall', 'floor_stack', 'office',
         'restricted')),
    -- x, y pairs: at least a triangle. That its edges do not cross is held by the
    -- importer.
    CONSTRAINT floor_area_outline_ck CHECK (
        array_ndims(outline_mm) = 1
        AND array_position(outline_mm, NULL) IS NULL
        AND cardinality(outline_mm) >= 6
        AND cardinality(outline_mm) % 2 = 0),
    CONSTRAINT floor_area_height_ck CHECK (height_mm IS NULL OR height_mm > 0)
);

COMMENT ON TABLE floor_area IS
    'REFERENCE, tenant-scoped. A polygon on a site''s plan that is not racking: '
    'docks, staging, walkways, walls, floor stacks. Optionally the location it '
    'is, so what is held there can be drawn there. D173.';
COMMENT ON COLUMN floor_area.outline_mm IS
    'x, y pairs in millimetres on the site plan, in order around the edge, not '
    'repeating the first point. D173.';
COMMENT ON COLUMN floor_area.height_mm IS
    'How tall, for what is drawn standing: a wall, a floor stack. NULL is flat. '
    'D173.';

ALTER TABLE floor_area ENABLE ROW LEVEL SECURITY;
ALTER TABLE floor_area FORCE ROW LEVEL SECURITY;
CREATE POLICY floor_area_tenant_scoped ON floor_area
    USING (tenant_id = current_tenant());

-- ---------------------------------------------------------------------------
-- Grants (D25)
-- ---------------------------------------------------------------------------

GRANT SELECT, INSERT, UPDATE, DELETE ON rack, floor_area TO spork_app;
GRANT SELECT ON rack, floor_area TO spork_platform, spork_scheduler;
