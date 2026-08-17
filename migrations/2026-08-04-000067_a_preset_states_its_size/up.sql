-- Migration 67: a preset states its size.
--
-- `package_type` has recorded since migration 9 *whether* a preset's dimensions
-- are fixed, and never what they are:
--
--     -- A box's dimensions are fixed. A pallet's height is not, which is why
--     -- this is a flag rather than an assumption.
--     dimensions_fixed    boolean NOT NULL DEFAULT true,
--
-- The observation is right and the flag is the wrong half to keep on its own. It
-- says "do not ask the operator for these" while giving nobody the numbers not to
-- ask for, so the only place a small box's size can live is the screen — which is
-- the walkthrough's own complaint about the system being replaced:
--
--     Package presets are the useful domain data. The prepack list … with
--     default dimensions and package type PAL … should be first-class in the
--     replacement, not buried in a NetSuite tab.
--
-- **A specification, deliberately not an observation.** The other candidate was
-- `observation`, which is where every other dimension in this model lives, and it
-- was refused: nobody measured "small box" as a class. An observation carries a
-- method, an instrument and a moment, and `observation_current` is read by J12
-- and J18 as *what a physical thing currently measures*. A preset's size is what
-- the thing is supposed to be, and mixing the two would make one table answer two
-- questions — which is D53's catch, and D99's, arriving here.
--
-- The measured pallet and the preset it was built on can then disagree, and that
-- difference is the point: it is the freight variance question `dimensions_source`
-- was added for.

ALTER TABLE package_type
    ADD COLUMN length_mm integer,
    ADD COLUMN width_mm  integer,
    ADD COLUMN height_mm integer,

    -- **The flag now has to mean something.** A preset claiming fixed dimensions
    -- and stating none is the flag without the fact, and the failure is silent:
    -- the screen does not ask, and nothing fills in. A pallet is the other case
    -- and stays expressible — `dimensions_fixed = false` with a length and width
    -- and no height is exactly a pallet whose stack height varies.
    ADD CONSTRAINT package_type_fixed_states_dimensions_ck
        CHECK (NOT dimensions_fixed
               OR num_nonnulls(length_mm, width_mm, height_mm) = 3),

    ADD CONSTRAINT package_type_dimensions_positive_ck
        CHECK ((length_mm IS NULL OR length_mm > 0)
           AND (width_mm  IS NULL OR width_mm  > 0)
           AND (height_mm IS NULL OR height_mm > 0));

COMMENT ON COLUMN package_type.length_mm IS
    'The preset''s nominal length in millimetres. A specification rather than a '
    'measurement: what this kind of package is meant to be, against which an '
    'observation of an actual carton may disagree. Migration 67.';
COMMENT ON COLUMN package_type.width_mm IS
    'Nominal width in millimetres. See length_mm. Migration 67.';
COMMENT ON COLUMN package_type.height_mm IS
    'Nominal height in millimetres, NULL where the height genuinely varies — a '
    'stacked pallet, which is what dimensions_fixed = false records. Migration 67.';

-- ---------------------------------------------------------------------------
-- The two that are actually standards
-- ---------------------------------------------------------------------------
--
-- Shipped shared (`tenant_id IS NULL`) because they are not this warehouse's
-- choices. **The Australian standard pallet is 1165 by 1165 millimetres**, which
-- is a national standard rather than a preference, and the same argument
-- migration 65 makes for the platform receiving ceiling applies: a default that
-- every deployment needs belongs with the schema rather than in a fixture.
--
-- Height is NULL on both and `dimensions_fixed` is false, because a stacked
-- pallet's height is the one thing the walkthrough says genuinely varies:
-- *"For pallets, the fields typically changed are weight and height."*
--
-- Box sizes are not standards and are not here. They are this business's, and
-- they live in `seed.sql` as tenant-owned rows — which is also the only way the
-- app could write them, since the write policy lets a tenant create its own and
-- reserves the shared arm for the platform role.

INSERT INTO package_type (
        id, tenant_id, name, carrier_package_code, dimensions_fixed,
        length_mm, width_mm, height_mm, tare_weight_g, reusable, max_payload_g)
VALUES
    ('9a7e0000-0000-0000-0000-000000000001', NULL, 'PALLET', 'PAL', false,
     1165, 1165, NULL, 25000, true, 1000000),
    ('9a7e0000-0000-0000-0000-000000000002', NULL, 'SKID', 'SKD', false,
     1165, 1165, NULL, 15000, true, 700000);
