-- Migration 7: the second spine table.
--
-- The architecture names two tables that carry everything. Migration 2 built the
-- first: a quantity of a product changed hands. This is the second: something was
-- measured, along with who measured it, how, and how much it should be trusted.
--
-- Dimensions, weights, temperatures, quality grades, carrier delivery estimates
-- and quantities a supplier claims to have sent all go in the same place. That
-- is what makes integration tractable, because the same fact arriving by EDI, a
-- supplier's website, a spreadsheet, a dock scale or somebody typing it produces
-- identical records apart from where it came from.

-- ---------------------------------------------------------------------------
-- Packing configuration, which the item arm of the subject registry needs
-- ---------------------------------------------------------------------------

-- Tenant-scoped always, per D19: a carton is only a definite physical object
-- relative to a case pack, and one tenant's case pack must not rewrite another's
-- scan arithmetic. Versioned by effective_from so a consignment shipped last
-- year can still explain its own dimensions.
CREATE TABLE item_packing_config (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         uuid NOT NULL REFERENCES tenant(id),
    item_id           uuid NOT NULL REFERENCES item(id),
    units_per_inner   integer,
    inners_per_carton integer,
    cartons_per_layer integer,     -- the classic Ti
    layers_per_pallet integer,     -- the classic Hi
    effective_from    date NOT NULL DEFAULT CURRENT_DATE,
    CONSTRAINT item_packing_config_tenant_key UNIQUE (id, tenant_id)
);

ALTER TABLE item_packing_config ENABLE ROW LEVEL SECURITY;
ALTER TABLE item_packing_config FORCE ROW LEVEL SECURITY;
CREATE POLICY item_packing_config_tenant_scoped ON item_packing_config
    USING (tenant_id = current_tenant());
GRANT SELECT, INSERT, UPDATE, DELETE ON item_packing_config TO nylonite_app;

-- ---------------------------------------------------------------------------
-- The subject registry (D23)
-- ---------------------------------------------------------------------------

CREATE TYPE packaging_level AS ENUM ('each', 'inner', 'carton', 'layer', 'pallet');

-- The only place the subject set widens.
--
-- Typed subject FKs, but on the registry rather than on the fact. That is the
-- whole trick: adding "we now observe pallet-pooling accounts" is one column on
-- a table of about 10^5 rows and zero change to anything holding 10^7. The fact
-- tables are permanently stable in shape, and an investigation can never
-- dead-end because every arm is a real foreign key.
--
-- D23's discriminated-union rule licenses the exclusive arms here: they are
-- alternative identities of one referent, and nobody will ever discover an
-- observation about no thing, or about two things at once.
CREATE TABLE observable (
    id        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL REFERENCES tenant(id),

    -- The item arm is three columns, not one. A carton is only a definite
    -- physical object relative to a case pack, and because item_packing_config
    -- is versioned, a corrected case pack cannot silently rewrite the dimensions
    -- of cartons shipped last year. packaging_level is a subject qualifier and
    -- never a unit: a carton is not commensurable with a millimetre.
    item_id                uuid REFERENCES item(id),
    packaging_level        packaging_level,
    item_packing_config_id uuid REFERENCES item_packing_config(id),

    package_id  uuid REFERENCES package(id),
    lot_id      uuid REFERENCES lot(id),
    location_id uuid REFERENCES location(id),

    -- package_type, consignment, device, vehicle_arrival, asserted_unit and
    -- asserted_unit_content are the remaining declared arms. Each arrives as one
    -- column in the migration that creates its target, which is the point of
    -- putting the union here rather than on the fact.

    kind text GENERATED ALWAYS AS (
        CASE WHEN item_id IS NOT NULL THEN 'item'
             WHEN package_id IS NOT NULL THEN 'package'
             WHEN lot_id IS NOT NULL THEN 'lot'
             WHEN location_id IS NOT NULL THEN 'location' END) STORED,

    CONSTRAINT observable_one_arm_ck
        CHECK (num_nonnulls(item_id, package_id, lot_id, location_id) = 1),
    CONSTRAINT observable_item_level_ck
        CHECK ((item_id IS NOT NULL) = (packaging_level IS NOT NULL)),
    -- A carton needs a case pack to be a definite thing. An each does not.
    CONSTRAINT observable_item_config_ck
        CHECK (item_id IS NULL OR packaging_level = 'each'
               OR item_packing_config_id IS NOT NULL),
    CONSTRAINT observable_tenant_key UNIQUE (id, tenant_id)
);

-- One partial unique index per arm, so a subject has exactly one registry row.
CREATE UNIQUE INDEX observable_item_idx ON observable
    (tenant_id, item_id, packaging_level, item_packing_config_id)
    WHERE item_id IS NOT NULL;
CREATE UNIQUE INDEX observable_package_idx ON observable (tenant_id, package_id)
    WHERE package_id IS NOT NULL;
CREATE UNIQUE INDEX observable_lot_idx ON observable (tenant_id, lot_id)
    WHERE lot_id IS NOT NULL;
CREATE UNIQUE INDEX observable_location_idx ON observable (tenant_id, location_id)
    WHERE location_id IS NOT NULL;

ALTER TABLE observable ENABLE ROW LEVEL SECURITY;
ALTER TABLE observable FORCE ROW LEVEL SECURITY;
CREATE POLICY observable_tenant_scoped ON observable
    USING (tenant_id = current_tenant());
GRANT SELECT, INSERT, UPDATE, DELETE ON observable TO nylonite_app;

-- ---------------------------------------------------------------------------
-- The metric vocabulary (D23)
-- ---------------------------------------------------------------------------

CREATE TYPE metric_result_kind AS ENUM
    ('quantity', 'instant', 'code', 'boolean', 'text');

-- Why this is not a custom-field framework, which is the objection it will
-- always attract. EAV defers *type* decisions to runtime; its signature is one
-- value text column, an arbitrary attribute name, an untyped subject, and a
-- schema that cannot be read. This has none of them. The subject is a foreign
-- key. The value is one of five typed columns chosen by the metric's declared
-- result_kind and enforced per row by a composite FK plus a CHECK. The unit is
-- enforced commensurable by a second composite FK, so recording a length in
-- grams is a constraint violation rather than a code-review finding. Nothing
-- queryable is in JSONB, and a metric cannot add a column elsewhere or make the
-- system branch.
--
-- The result types are code; only the vocabulary is data.
CREATE TABLE metric (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id    uuid REFERENCES tenant(id),   -- NULL = shipped by us (D19)
    code         text NOT NULL,
    label        text NOT NULL,
    result_kind  metric_result_kind NOT NULL,
    dimension_id uuid REFERENCES dimension(id),
    reserved     boolean NOT NULL DEFAULT false,
    applies_to   text[],            -- which observable arms are legal
    higher_is_better boolean,

    -- aggregation is deliberately absent. last/min/max/mean is a per-row data
    -- value selecting which fold a projection performs, and that is semantics,
    -- which belongs in the Rust type. Same argument that refused a combine
    -- column in D22. higher_is_better stays because it is a display and
    -- scorecard hint rather than a fold.

    CONSTRAINT metric_quantity_has_dimension_ck
        CHECK ((result_kind = 'quantity') = (dimension_id IS NOT NULL)),
    -- Only we may reserve a code, because reserved codes are the ones
    -- application code is allowed to name.
    CONSTRAINT metric_reserved_is_ours_ck CHECK (NOT reserved OR tenant_id IS NULL),
    CONSTRAINT metric_code_key UNIQUE NULLS NOT DISTINCT (tenant_id, code),
    CONSTRAINT metric_result_kind_key UNIQUE (id, result_kind),
    CONSTRAINT metric_dimension_key UNIQUE (id, dimension_id)
);

CREATE TABLE metric_code (
    id        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    metric_id uuid NOT NULL REFERENCES metric(id),
    code      text NOT NULL,
    label     text NOT NULL,
    ordinal   integer NOT NULL,
    CONSTRAINT metric_code_metric_key UNIQUE (id, metric_id),
    CONSTRAINT metric_code_unique UNIQUE (metric_id, code)
);

ALTER TABLE metric ENABLE ROW LEVEL SECURITY;
ALTER TABLE metric FORCE ROW LEVEL SECURITY;
CREATE POLICY metric_shared_reference ON metric
    USING (tenant_id IS NULL OR tenant_id = current_tenant());
GRANT SELECT, INSERT, UPDATE, DELETE ON metric TO nylonite_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON metric TO nylonite_platform;
GRANT SELECT ON metric_code TO nylonite_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON metric_code TO nylonite_platform;

-- ---------------------------------------------------------------------------
-- The facts (D23)
-- ---------------------------------------------------------------------------

CREATE TYPE observation_method AS ENUM
    ('instrument', 'scan', 'keyed', 'derived', 'estimated', 'transcribed', 'asserted');
CREATE TYPE ingestion_channel AS ENUM
    ('edi', 'portal', 'csv', 'email', 'api', 'keyed', 'scale', 'scanner', 'derived');

CREATE TABLE observation_event (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     uuid NOT NULL REFERENCES tenant(id),
    client_event_id uuid NOT NULL,
    observable_id uuid NOT NULL REFERENCES observable(id),

    observed_at   timestamptz NOT NULL,   -- valid time, device clock (D5)
    recorded_at   timestamptz NOT NULL DEFAULT now(),   -- transaction time

    device_id           uuid,   -- the RECORDING device, unconditional (D11)
    instrument_device_id uuid,  -- the MEASURING instrument

    recorded_by_id uuid REFERENCES person(id),
    automation_key text,
    authorised_by_id uuid REFERENCES person(id),
    work_session_id  uuid,

    -- Provenance is three deliberately uncorrelated columns. A carrier re-weigh
    -- is (carrier, instrument). A supplier ASN is (supplier, asserted). Our own
    -- eyeball is (NULL, estimated). The old single enum could express the first
    -- and third only by having a value per combination, which is why it ran out.
    asserted_by_party_id uuid REFERENCES party(id),   -- NULL = us
    method            observation_method NOT NULL,
    ingestion_channel ingestion_channel NOT NULL,

    derived_from_event_id uuid REFERENCES observation_event(id),

    CONSTRAINT observation_event_actor_ck
        CHECK (num_nonnulls(recorded_by_id, automation_key) = 1),
    CONSTRAINT observation_event_instrument_ck
        CHECK (instrument_device_id IS NULL OR method IN ('instrument', 'scan')),
    CONSTRAINT observation_event_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),

    -- Composite targets so observation can denormalise these and have the
    -- database enforce that the copy agrees.
    CONSTRAINT observation_event_observable_key UNIQUE (id, observable_id),
    CONSTRAINT observation_event_observed_at_key UNIQUE (id, observed_at),
    CONSTRAINT observation_event_tenant_key UNIQUE (id, tenant_id),
    CONSTRAINT observation_event_client_event_key UNIQUE (id, client_event_id)
);

COMMENT ON TABLE observation_event IS
    'FACT. The act of measuring. One act may produce several results, which is '
    'why the results are a separate table. D23.';

CREATE TABLE observation (
    id                   uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id            uuid NOT NULL,
    observation_event_id uuid NOT NULL REFERENCES observation_event(id),

    -- Denormalised and composite-FK'd, so the copy cannot disagree.
    observable_id uuid NOT NULL,
    observed_at   timestamptz NOT NULL,

    -- D25: every fact table carries client_event_id as a plain FK, not a unique
    -- one, because one physical act produces many facts. A cubing scan writes
    -- one event and four observations, all sharing the handheld's identifier.
    -- Denormalised from the event and composite-FK'd back to it, so the copy
    -- cannot disagree. S19 caught its absence.
    client_event_id uuid NOT NULL,
    metric_id     uuid NOT NULL,
    result_kind   metric_result_kind NOT NULL,
    dimension_id  uuid,

    -- ALWAYS the dimension's canonical unit. There is no unit column, which
    -- makes non-canonical storage structurally unrepresentable rather than
    -- merely discouraged.
    value_numeric  bigint,
    value_instant  timestamptz,
    value_code_id  uuid,
    value_boolean  boolean,
    value_text     text,

    -- Separate from dimension_id because an ETA has no dimension but "plus or
    -- minus two hours" is a real answer.
    uncertainty_dimension_id uuid REFERENCES dimension(id),
    uncertainty_numeric      bigint,

    absent_reason text,   -- not_measured | not_applicable | unreadable | retracted

    -- What the counterparty actually gave us, before conversion. The canonical
    -- value is what we compute with; this is what we can quote back.
    entered_value  numeric,
    entered_unit_id uuid,
    confidence     smallint,

    corrects_observation_id uuid REFERENCES observation(id),   -- was never true
    retracts_observation_id uuid REFERENCES observation(id),   -- should not exist

    FOREIGN KEY (observation_event_id, observable_id)
        REFERENCES observation_event(id, observable_id),
    FOREIGN KEY (observation_event_id, observed_at)
        REFERENCES observation_event(id, observed_at),
    FOREIGN KEY (observation_event_id, tenant_id)
        REFERENCES observation_event(id, tenant_id),
    FOREIGN KEY (observation_event_id, client_event_id)
        REFERENCES observation_event(id, client_event_id),
    FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),

    -- The value column must match the metric's declared result kind.
    FOREIGN KEY (metric_id, result_kind) REFERENCES metric(id, result_kind),
    -- And the dimension must be the metric's, so a length cannot be recorded in
    -- grams. A constraint violation rather than a code-review finding.
    FOREIGN KEY (metric_id, dimension_id) REFERENCES metric(id, dimension_id),
    FOREIGN KEY (entered_unit_id, dimension_id) REFERENCES unit(id, dimension_id),
    FOREIGN KEY (value_code_id, metric_id) REFERENCES metric_code(id, metric_id),

    CONSTRAINT observation_absent_reason_ck CHECK (absent_reason IS NULL
        OR absent_reason IN ('not_measured','not_applicable','unreadable','retracted')),
    -- Exactly one value, or an explicit reason there is none. A silent null is
    -- indistinguishable from a measurement nobody took.
    CONSTRAINT observation_one_value_ck CHECK (
        num_nonnulls(value_numeric, value_instant, value_code_id,
                     value_boolean, value_text) = 1
        OR absent_reason IS NOT NULL)
);

COMMENT ON TABLE observation IS
    'FACT. One result. Append-only and never UPDATEd: a correction is a new row '
    'pointing at the one it corrects. D23.';

COMMENT ON COLUMN observation.value_numeric IS
    'Always in the dimension''s canonical unit. There is no unit column, so a '
    'value stored in anything else cannot be represented. Principle 5.';

CREATE INDEX observation_subject_idx
    ON observation (tenant_id, observable_id, metric_id, observed_at DESC);

ALTER TABLE observation_event ENABLE ROW LEVEL SECURITY;
ALTER TABLE observation_event FORCE ROW LEVEL SECURITY;
CREATE POLICY observation_event_tenant_scoped ON observation_event
    USING (tenant_id = current_tenant());

ALTER TABLE observation ENABLE ROW LEVEL SECURITY;
ALTER TABLE observation FORCE ROW LEVEL SECURITY;
CREATE POLICY observation_tenant_scoped ON observation
    USING (tenant_id = current_tenant());

-- S6: facts take INSERT and SELECT and nothing else.
GRANT SELECT, INSERT ON observation_event, observation TO nylonite_app;

-- ---------------------------------------------------------------------------
-- The reserved seed (D23, S21)
-- ---------------------------------------------------------------------------

-- S21 asserts every metric-code literal in application code appears here with
-- reserved = true and tenant_id IS NULL, which is a grep in CI. The day someone
-- writes if metric.code == "customer_special_thing", the build fails.
--
-- Gross, net and tare are three metrics rather than one with a modifier. GS1
-- settled it: AI 310n is net weight and AI 330n is gross. A single weight metric
-- is genuinely ambiguous, and gross versus net is exactly what a carrier
-- re-weigh surfaces.
-- The dimensions and units themselves, which migration 1 created a home for and
-- never filled. Canonical units are the identity by construction, which S22
-- asserts: a canonical unit with a factor other than 1/1 would mean every stored
-- value is in something other than what every reader assumes.
--
-- Wrapped in a transaction because dimension and unit reference each other and
-- the canonical FK is DEFERRABLE INITIALLY DEFERRED. Outside a transaction each
-- statement is its own, and the deferred check fires with nothing to point at.
BEGIN;

INSERT INTO dimension (id, code, canonical_unit_id) VALUES
    ('d1000000-0000-0000-0000-000000000001', 'mass',        'f1000000-0000-0000-0000-000000000001'),
    ('d1000000-0000-0000-0000-000000000002', 'length',      'f1000000-0000-0000-0000-000000000002'),
    ('d1000000-0000-0000-0000-000000000003', 'temperature', 'f1000000-0000-0000-0000-000000000003'),
    ('d1000000-0000-0000-0000-000000000004', 'count',       'f1000000-0000-0000-0000-000000000004')
ON CONFLICT (code) DO NOTHING;

INSERT INTO unit (id, dimension_id, code, ucum_code, factor_num, factor_den, offset_num, offset_den) VALUES
    -- Canonical: factor 1/1, offset 0. Principle 5, asserted by S22.
    ('f1000000-0000-0000-0000-000000000001', 'd1000000-0000-0000-0000-000000000001', 'g',  'g',   1, 1, 0, 1),
    ('f1000000-0000-0000-0000-000000000002', 'd1000000-0000-0000-0000-000000000002', 'mm', 'mm',  1, 1, 0, 1),
    ('f1000000-0000-0000-0000-000000000003', 'd1000000-0000-0000-0000-000000000003', 'mK', 'mK',  1, 1, 0, 1),
    ('f1000000-0000-0000-0000-000000000004', 'd1000000-0000-0000-0000-000000000004', 'ea', '1',   1, 1, 0, 1),
    -- Exact rationals, never floats. An inch is 254/10 mm and it is that exactly.
    ('f1000000-0000-0000-0000-000000000011', 'd1000000-0000-0000-0000-000000000001', 'kg', 'kg', 1000, 1, 0, 1),
    ('f1000000-0000-0000-0000-000000000012', 'd1000000-0000-0000-0000-000000000002', 'm',  'm',  1000, 1, 0, 1),
    ('f1000000-0000-0000-0000-000000000013', 'd1000000-0000-0000-0000-000000000002', 'in', '[in_i]', 254, 10, 0, 1),
    -- Affine: degC = mK, scaled by 1000 and offset by 273150 mK.
    ('f1000000-0000-0000-0000-000000000014', 'd1000000-0000-0000-0000-000000000003', 'Cel', 'Cel', 1000, 1, 273150, 1)
ON CONFLICT (dimension_id, code) DO NOTHING;

COMMIT;

INSERT INTO metric (tenant_id, code, label, result_kind, dimension_id, reserved, applies_to)
SELECT NULL, m.code, m.label, 'quantity'::metric_result_kind, d.id, true, m.applies
  FROM (VALUES
        ('gross_weight', 'Gross weight', 'mass',   ARRAY['item','package']),
        ('net_weight',   'Net weight',   'mass',   ARRAY['item','package']),
        ('tare_weight',  'Tare weight',  'mass',   ARRAY['item','package']),
        ('length',       'Length',       'length', ARRAY['item','package']),
        ('width',        'Width',        'length', ARRAY['item','package']),
        ('height',       'Height',       'length', ARRAY['item','package']),
        ('temperature',  'Temperature',  'temperature', ARRAY['lot','location','package'])
       ) AS m(code, label, dim, applies)
  JOIN dimension d ON d.code = m.dim;

-- A seed that joins can insert nothing and report success, which is how this one
-- first behaved: the dimensions it joins to did not exist, so seven reserved
-- metrics silently became zero and the migration still said it applied. The same
-- vacuity the invariant register warns about, in a migration rather than a check.
DO $$
DECLARE n integer;
BEGIN
    SELECT count(*) INTO n FROM metric WHERE reserved;
    IF n <> 7 THEN
        RAISE EXCEPTION 'reserved metric seed inserted % rows, expected 7', n;
    END IF;
END
$$;
