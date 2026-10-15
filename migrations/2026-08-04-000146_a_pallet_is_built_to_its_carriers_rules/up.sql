-- Migration 146: a carrier's rules are settings, and a pallet is built to
-- them. D250, docs/packing-plan.md (settled 9, M4).
--
-- How a pallet must be built is a named set of build rules, each of a fixed
-- list, a must or a preference with a weight, or off. A carrier names the set
-- its breakdown pallets are built to and the set its pallets that go as one
-- are built to; one carrier is the workspace's usual one. A pallet is built
-- for a carrier, not a service, so a service names no rules of its own. A pallet is started under a carrier and keeps that carrier's rules as
-- they stood then: a set is saved as a new version each time, and the pallet
-- names the version. The planner and the checks read rules, never a carrier.

-- ---------------------------------------------------------------------------
-- A carrier's services are its workspace's, as the carrier is
-- ---------------------------------------------------------------------------

-- Any workspace could change any carrier's services: the table had no tenant
-- and no policy. A service is its carrier's workspace's, or the platform's
-- with a platform carrier.
ALTER TABLE carrier
    ADD CONSTRAINT carrier_tenant_key UNIQUE (id, tenant_id);

ALTER TABLE carrier_service
    ADD COLUMN tenant_id uuid REFERENCES tenant(id);
UPDATE carrier_service s SET tenant_id = c.tenant_id FROM carrier c WHERE c.id = s.carrier_id;
-- A platform service's empty tenant leaves this unchecked (MATCH SIMPLE), and
-- the write policy leaves a platform service to the platform.
ALTER TABLE carrier_service
    ADD CONSTRAINT carrier_service_carrier_tenant_fk
        FOREIGN KEY (carrier_id, tenant_id) REFERENCES carrier (id, tenant_id);

ALTER TABLE carrier_service ENABLE ROW LEVEL SECURITY;
ALTER TABLE carrier_service FORCE ROW LEVEL SECURITY;
CREATE POLICY carrier_service_shared_read ON carrier_service
    FOR SELECT
    USING (tenant_id IS NULL OR tenant_id = current_tenant());
CREATE POLICY carrier_service_own_write ON carrier_service
    USING (tenant_id = current_tenant() OR (tenant_id IS NULL AND is_platform()))
    WITH CHECK (tenant_id = current_tenant() OR (tenant_id IS NULL AND is_platform()));

-- ---------------------------------------------------------------------------
-- Build rules
-- ---------------------------------------------------------------------------

-- The fixed list (D250). A new kind of rule is code, because the planner has
-- to understand it; a new carrier, or one that changes what it allows, is a
-- change of settings.
CREATE TYPE build_rule_kind AS ENUM (
    'height',           -- the most the load stands, in mm, with its base or from the deck
    'weight',           -- the most the pallet and its load weigh, in grams
    'overhang_sides',   -- the most a carton stands out left or right, either side of the forks, in mm
    'overhang_ends',    -- the most it stands out at the front, where the forks go in, or the back, in mm
    'labels',           -- each delivered parcel's label on a side facing out, hidden behind nothing
    'support',          -- the least share of each box's base resting on something, per thousand
    'heavy',            -- a parcel over so many grams is marked heavy
    'loose',            -- a parcel over so many grams goes on a pallet
    'together',         -- each order's parcels next to one another
    'columns',          -- corners over corners where the boxes allow
    'low_and_centred',  -- the heavy and weight-taking low, the weight over the middle
    'flat_top'          -- as few heights on top as it can
);

CREATE TYPE rule_strength AS ENUM ('must', 'prefer', 'off');

CREATE TABLE build_rules (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id   uuid NOT NULL REFERENCES tenant(id),
    name        text NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT build_rules_tenant_key UNIQUE (id, tenant_id),
    CONSTRAINT build_rules_name_key UNIQUE (tenant_id, name),
    CONSTRAINT build_rules_name_ck CHECK (btrim(name) <> '')
);

COMMENT ON TABLE build_rules IS
    'D250. A named set of build rules: how a pallet must be built. Carriers point '
    'at sets; the rules themselves are each version''s.';

-- The rules of a set as they stood from when they were saved. Never changed:
-- saving a set is a new version, so a pallet started under one keeps it.
CREATE TABLE build_rules_version (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id       uuid NOT NULL REFERENCES tenant(id),
    build_rules_id  uuid NOT NULL,
    saved_by_id     uuid NOT NULL REFERENCES person(id),
    saved_at        timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT build_rules_version_tenant_key UNIQUE (id, tenant_id),
    CONSTRAINT build_rules_version_set_fk
        FOREIGN KEY (build_rules_id, tenant_id) REFERENCES build_rules (id, tenant_id)
);

COMMENT ON TABLE build_rules_version IS
    'D250. A set of build rules as saved: the newest is the set now, and a pallet '
    'keeps the one it was started under.';

CREATE INDEX build_rules_version_set_idx ON build_rules_version (build_rules_id, saved_at DESC, id DESC);

CREATE TABLE build_rule (
    tenant_id   uuid NOT NULL REFERENCES tenant(id),
    version_id  uuid NOT NULL,
    kind        build_rule_kind NOT NULL,
    strength    rule_strength NOT NULL,
    -- How much a preference counts against the others, one to three.
    weight      smallint,
    -- Its figure, in the unit its kind says: millimetres for height and
    -- overhang, grams for weight, heavy and loose, per thousand for support.
    amount      bigint,
    -- Height: counted with the pallet's own base, or from its deck.
    with_base   boolean,
    -- Labels: a label on top of the top layer counts as facing out.
    top_counts  boolean,

    PRIMARY KEY (version_id, kind),
    CONSTRAINT build_rule_version_fk
        FOREIGN KEY (version_id, tenant_id) REFERENCES build_rules_version (id, tenant_id),
    CONSTRAINT build_rule_weight_ck
        CHECK ((strength = 'prefer') = (weight IS NOT NULL) AND weight BETWEEN 1 AND 3),
    CONSTRAINT build_rule_amount_ck
        CHECK ((kind IN ('height', 'weight', 'overhang_sides', 'overhang_ends', 'support', 'heavy', 'loose'))
               = (amount IS NOT NULL)
               AND amount >= 0
               AND (kind <> 'support' OR amount <= 1000)),
    CONSTRAINT build_rule_with_base_ck CHECK ((kind = 'height') = (with_base IS NOT NULL)),
    CONSTRAINT build_rule_top_counts_ck CHECK ((kind = 'labels') = (top_counts IS NOT NULL)),
    -- Only a preference: nothing can be lowest and flattest absolutely.
    CONSTRAINT build_rule_prefer_only_ck
        CHECK (kind NOT IN ('low_and_centred', 'flat_top') OR strength <> 'must')
);

COMMENT ON TABLE build_rule IS
    'D250. One rule of a version of a set: its kind, must, prefer (with a weight) '
    'or off, and its figure.';

ALTER TABLE build_rules ENABLE ROW LEVEL SECURITY;
ALTER TABLE build_rules FORCE ROW LEVEL SECURITY;
CREATE POLICY build_rules_tenant_scoped ON build_rules
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

ALTER TABLE build_rules_version ENABLE ROW LEVEL SECURITY;
ALTER TABLE build_rules_version FORCE ROW LEVEL SECURITY;
CREATE POLICY build_rules_version_tenant_scoped ON build_rules_version
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

ALTER TABLE build_rule ENABLE ROW LEVEL SECURITY;
ALTER TABLE build_rule FORCE ROW LEVEL SECURITY;
CREATE POLICY build_rule_tenant_scoped ON build_rule
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- A set is renamed in place; a version and its rules are never changed.
GRANT SELECT, INSERT, UPDATE (name) ON build_rules TO spork_app;
GRANT SELECT, INSERT ON build_rules_version TO spork_app;
GRANT SELECT, INSERT ON build_rule TO spork_app;

-- The newest version of each set: the set as it is now.
CREATE VIEW build_rules_now
    WITH (security_invoker = true) AS
SELECT DISTINCT ON (v.build_rules_id) v.build_rules_id, v.id AS version_id, v.saved_at, v.saved_by_id
  FROM build_rules_version v
 ORDER BY v.build_rules_id, v.saved_at DESC, v.id DESC;

COMMENT ON VIEW build_rules_now IS 'D250. Each set of build rules'' newest version.';

GRANT SELECT ON build_rules_now TO spork_app;

-- ---------------------------------------------------------------------------
-- Which rules a carrier's pallets are built to, and the usual carrier
-- ---------------------------------------------------------------------------

ALTER TABLE carrier
    -- Its breakdown pallets', each carton delivered on its own (D247).
    ADD COLUMN breakdown_rules_id uuid,
    -- Its pallets that go as one.
    ADD COLUMN as_one_rules_id uuid,
    ADD CONSTRAINT carrier_breakdown_rules_fk
        FOREIGN KEY (breakdown_rules_id, tenant_id) REFERENCES build_rules (id, tenant_id),
    ADD CONSTRAINT carrier_as_one_rules_fk
        FOREIGN KEY (as_one_rules_id, tenant_id) REFERENCES build_rules (id, tenant_id),
    -- A platform carrier names no workspace's rules: MATCH SIMPLE would check
    -- nothing with its tenant empty.
    ADD CONSTRAINT carrier_rules_own_ck
        CHECK (tenant_id IS NOT NULL OR num_nonnulls(breakdown_rules_id, as_one_rules_id) = 0);

COMMENT ON COLUMN carrier.breakdown_rules_id IS
    'D250. The build rules its breakdown pallets are built to.';
COMMENT ON COLUMN carrier.as_one_rules_id IS
    'D250. The build rules its pallets that go as one are built to.';

ALTER TABLE workspace_setting
    -- The carrier a pallet is started under unless the board is told another
    -- (settled 9).
    ADD COLUMN usual_carrier_id uuid REFERENCES carrier (id);

-- ---------------------------------------------------------------------------
-- A pallet is built for a carrier, to its rules as they stood
-- ---------------------------------------------------------------------------

ALTER TABLE package
    ADD COLUMN carrier_id uuid REFERENCES carrier (id),
    ADD COLUMN build_rules_version_id uuid,
    ADD CONSTRAINT package_build_rules_fk
        FOREIGN KEY (build_rules_version_id, tenant_id) REFERENCES build_rules_version (id, tenant_id);

COMMENT ON COLUMN package.carrier_id IS
    'D250, settled 9. The carrier a pallet is built for, said when it is made.';
COMMENT ON COLUMN package.build_rules_version_id IS
    'D250. The rules a pallet is built to, as they stood when it was made: its '
    'carrier''s for a pallet of its kind. Empty when the carrier named none.';

GRANT SELECT (carrier_id, build_rules_version_id), INSERT (carrier_id, build_rules_version_id)
    ON package TO spork_app;

-- A planned pallet built for another carrier than the usual one.
ALTER TABLE pack_plan_parcel
    ADD COLUMN carrier_id uuid REFERENCES carrier (id);
