-- Migration 8: the policy resolver.
--
-- D13 drew the line: the model holds the inputs, the policy belongs to a
-- manager. D22 built the mechanism, and the mechanism is a scope lattice with
-- most-specific-wins resolution rather than a rules engine.
--
-- The distinction is not stylistic. A rules engine stores logic as rows: a
-- field name, an operator, a value, an action. This stores only *where* a policy
-- applies and *what* its value is. The entire matching language has cardinality
-- one, "is this node an ancestor-or-self of that node", evaluated over closure
-- tables. No <, no LIKE, no IN, no boolean connectives anywhere in the data.
-- That is why this is not a rules engine, and it is greppable, which S11 does.

-- ---------------------------------------------------------------------------
-- The Counterparty dimension needs a taxonomy (D22)
-- ---------------------------------------------------------------------------

CREATE TABLE party_class (
    id        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL REFERENCES tenant(id),
    parent_id uuid REFERENCES party_class(id),
    code      text NOT NULL,
    name      text NOT NULL,
    CONSTRAINT party_class_tenant_code_key UNIQUE (tenant_id, code),
    CONSTRAINT party_class_tenant_key UNIQUE (id, tenant_id),
    CONSTRAINT party_class_not_own_parent_ck CHECK (parent_id <> id)
);

CREATE TABLE party_class_closure (
    tenant_id     uuid NOT NULL,
    ancestor_id   uuid NOT NULL,
    descendant_id uuid NOT NULL,
    depth         integer NOT NULL,
    PRIMARY KEY (ancestor_id, descendant_id),
    CONSTRAINT party_class_closure_ancestor_fk FOREIGN KEY (ancestor_id, tenant_id)
        REFERENCES party_class(id, tenant_id),
    CONSTRAINT party_class_closure_descendant_fk FOREIGN KEY (descendant_id, tenant_id)
        REFERENCES party_class(id, tenant_id),
    CONSTRAINT party_class_closure_depth_ck CHECK (depth >= 0)
);

ALTER TABLE party ADD COLUMN party_class_id uuid;
ALTER TABLE party ADD CONSTRAINT party_class_fk
    FOREIGN KEY (party_class_id, tenant_id) REFERENCES party_class(id, tenant_id);

ALTER TABLE party_class ENABLE ROW LEVEL SECURITY;
ALTER TABLE party_class FORCE ROW LEVEL SECURITY;
CREATE POLICY party_class_tenant_scoped ON party_class
    USING (tenant_id = current_tenant());
ALTER TABLE party_class_closure ENABLE ROW LEVEL SECURITY;
ALTER TABLE party_class_closure FORCE ROW LEVEL SECURITY;
CREATE POLICY party_class_closure_tenant_scoped ON party_class_closure
    USING (tenant_id = current_tenant());

GRANT SELECT, INSERT, UPDATE, DELETE ON party_class TO spork_app;
GRANT SELECT ON party_class_closure TO spork_app;

COMMENT ON TABLE party_class_closure IS
    'PROJECTION of party_class. The closure is what makes the matching language '
    'cardinality one: ancestor-or-self is a lookup, not a recursion.';

-- ---------------------------------------------------------------------------
-- The kind vocabulary (D22, S13)
-- ---------------------------------------------------------------------------

-- S13 asserts three sets are identical: this enum, the %_policy table set, and
-- the Rust PolicyKind registry. So the enum holds exactly the kinds whose value
-- tables exist, and adding a kind means adding all three in one commit. Ten
-- kinds are named across the decisions; three have value tables today, and the
-- rest arrive with their consumers.
CREATE TYPE policy_kind AS ENUM ('allocation', 'receiving', 'shelf_life');

-- ---------------------------------------------------------------------------
-- Where a policy applies (D22)
-- ---------------------------------------------------------------------------

CREATE TABLE policy_binding (
    id        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid REFERENCES tenant(id),   -- NULL = platform-shipped default
    kind      policy_kind NOT NULL,

    -- A scope is a conjunction. The columns are independent nullable axes and
    -- NULL means "any".
    item_class_id  uuid,
    item_id        uuid REFERENCES item(id),
    party_class_id uuid,
    party_id       uuid REFERENCES party(id),
    site_id        uuid REFERENCES site(id),
    zone_id        uuid REFERENCES zone(id),
    owner_party_id uuid REFERENCES party(id),
    metric_id      uuid REFERENCES metric(id),

    supersedes_id  uuid REFERENCES policy_binding(id),
    note           text,
    created_at     timestamptz NOT NULL DEFAULT now(),
    created_by_id  uuid REFERENCES person(id),

    -- S12: there is deliberately NO num_nonnulls CHECK here. One would reverse
    -- the semantics and forbid the all-NULL scope, which is exactly the
    -- platform-shipped default that shipped defaults and clamping require.
    -- The absence is the design, so the invariant asserts it.

    -- NULLS NOT DISTINCT, so two bindings cannot claim the same scope. Without
    -- it every NULL axis would compare unequal and the resolver would find two
    -- winners at the same specificity with no way to choose.
    CONSTRAINT policy_binding_scope_key UNIQUE NULLS NOT DISTINCT
        (tenant_id, kind, item_class_id, item_id, party_class_id, party_id,
         site_id, zone_id, owner_party_id, metric_id),
    CONSTRAINT policy_binding_kind_key UNIQUE (id, kind)
);

-- The resolver's only scan.
CREATE INDEX policy_binding_resolve_idx ON policy_binding (tenant_id, kind);

COMMENT ON TABLE policy_binding IS
    'CONFIGURATION. Where a policy applies. The scope is immutable: a change of '
    'scope is a new binding superseding the old, so a resolution made last March '
    'is still explainable. D22.';

-- Four fixed values that code branches on, so an enum under D33's test rather
-- than text. It is also named change_kind rather than action because S11's
-- denylist forbids a policy column called action: the denylist is aimed at
-- rule-engine actions, and a column that has to be explained is a column worth
-- renaming.
CREATE TYPE policy_change_kind AS ENUM
    ('created', 'revalued', 'retired', 'reinstated');

CREATE TABLE policy_change (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         uuid REFERENCES tenant(id),
    occurred_at       timestamptz NOT NULL DEFAULT now(),
    recorded_at       timestamptz NOT NULL DEFAULT now(),
    policy_binding_id uuid NOT NULL REFERENCES policy_binding(id),
    kind              policy_kind NOT NULL,

    -- D25: every fact table carries client_event_id. A policy edit is an act
    -- with an actor, a device and an idempotency key like any other, and
    -- exempting it because it comes from an admin screen rather than a handheld
    -- is the kind of carve-out this model keeps refusing. S19 caught its
    -- absence.
    tenant_scope_id   uuid,
    client_event_id   uuid NOT NULL,
    change_kind       policy_change_kind NOT NULL,
    -- A weight change with no reason is how tuning becomes superstition.
    reason            text NOT NULL,
    recorded_by_id    uuid REFERENCES person(id),
    authorised_by_id  uuid REFERENCES person(id),
    CONSTRAINT policy_change_reason_ck CHECK (length(btrim(reason)) > 0),
    CONSTRAINT policy_change_client_event_fk
        FOREIGN KEY (tenant_scope_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id)
);

COMMENT ON TABLE policy_change IS
    'FACT. Append-only, mandatory reason. J13 anti-joins this against every '
    'policy value version in both directions.';

-- ---------------------------------------------------------------------------
-- The value tables (D22, S14, S16)
-- ---------------------------------------------------------------------------

-- Every %_policy table has the same three things: a CHECK pinning its kind, a
-- composite FK to (policy_binding.id, kind) so a value cannot attach to a
-- binding of another kind, and an effective-range exclusion so one scope has one
-- value at any instant. S14 asserts the shape from one template.
--
-- Note what is absent from all of them: no field name, no operator, no
-- comparator, no expression. S11 greps for exactly those column names, and the
-- register records that the first draft of this design failed that check on a
-- column called band_axis.

CREATE TABLE allocation_policy (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    policy_binding_id uuid NOT NULL,
    kind              policy_kind NOT NULL GENERATED ALWAYS AS ('allocation') STORED,
    effective         tstzrange NOT NULL DEFAULT tstzrange(now(), NULL, '[)'),

    -- Values, not logic. The scoring function stays in Rust; these are its
    -- weights, and D13's line is that we ship defaults rather than hard-coded
    -- behaviour.
    weight_rotation   integer NOT NULL DEFAULT 0,
    weight_travel     integer NOT NULL DEFAULT 0,
    weight_consolidation integer NOT NULL DEFAULT 0,
    allow_partial     boolean NOT NULL DEFAULT true,

    CONSTRAINT allocation_policy_kind_ck CHECK (kind = 'allocation'),
    CONSTRAINT allocation_policy_binding_fk FOREIGN KEY (policy_binding_id, kind)
        REFERENCES policy_binding(id, kind),
    CONSTRAINT allocation_policy_no_overlap
        EXCLUDE USING gist (policy_binding_id WITH =, effective WITH &&)
);

CREATE TABLE receiving_policy (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    policy_binding_id uuid NOT NULL,
    kind              policy_kind NOT NULL GENERATED ALWAYS AS ('receiving') STORED,
    effective         tstzrange NOT NULL DEFAULT tstzrange(now(), NULL, '[)'),

    default_status_id uuid REFERENCES inventory_status(id),
    -- D8's clock, populated from here per the supply-side amendment.
    respond_by_hours  integer,
    require_lot       boolean NOT NULL DEFAULT false,
    tolerance_over_pct numeric,
    tolerance_under_pct numeric,

    CONSTRAINT receiving_policy_kind_ck CHECK (kind = 'receiving'),
    CONSTRAINT receiving_policy_binding_fk FOREIGN KEY (policy_binding_id, kind)
        REFERENCES policy_binding(id, kind),
    CONSTRAINT receiving_policy_no_overlap
        EXCLUDE USING gist (policy_binding_id WITH =, effective WITH &&)
);

CREATE TABLE shelf_life_policy (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    policy_binding_id uuid NOT NULL,
    kind              policy_kind NOT NULL GENERATED ALWAYS AS ('shelf_life') STORED,
    effective         tstzrange NOT NULL DEFAULT tstzrange(now(), NULL, '[)'),

    -- D14 removed customer.min_shelf_life_days because a column on the customer
    -- cannot express "different requirements by category". This can, because the
    -- scope carries both the customer and the item class.
    min_shelf_life_days integer,
    min_shelf_life_pct  numeric,

    CONSTRAINT shelf_life_policy_kind_ck CHECK (kind = 'shelf_life'),
    CONSTRAINT shelf_life_policy_binding_fk FOREIGN KEY (policy_binding_id, kind)
        REFERENCES policy_binding(id, kind),
    CONSTRAINT shelf_life_policy_no_overlap
        EXCLUDE USING gist (policy_binding_id WITH =, effective WITH &&)
);

ALTER TABLE policy_binding ENABLE ROW LEVEL SECURITY;
ALTER TABLE policy_binding FORCE ROW LEVEL SECURITY;
CREATE POLICY policy_binding_shared_reference ON policy_binding
    USING (tenant_id IS NULL OR tenant_id = current_tenant());

ALTER TABLE policy_change ENABLE ROW LEVEL SECURITY;
ALTER TABLE policy_change FORCE ROW LEVEL SECURITY;
CREATE POLICY policy_change_shared_reference ON policy_change
    USING (tenant_id IS NULL OR tenant_id = current_tenant());

GRANT SELECT, INSERT, UPDATE ON policy_binding TO spork_app;
GRANT SELECT, INSERT ON policy_change TO spork_app;
GRANT SELECT, INSERT, UPDATE ON allocation_policy, receiving_policy, shelf_life_policy
    TO spork_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON policy_binding,
    allocation_policy, receiving_policy, shelf_life_policy TO spork_platform;
-- S6: policy_change is a fact. There is no verb for changing what happened, and
-- that holds for the platform role too. The bulk grant that swept it in with the
-- configuration tables was the mistake.
GRANT SELECT, INSERT ON policy_change TO spork_platform;

-- ---------------------------------------------------------------------------
-- The resolver (D22)
-- ---------------------------------------------------------------------------

-- Most-specific-wins over a depth vector compared lexicographically.
--
-- Specificity is a vector, not a number. Collapsing a componentwise comparison
-- to one integer lets a large count in a low-weight component beat a small count
-- in a high-weight one, which is why CSS has no specificity column after twenty
-- years of proof.
--
-- Tenancy is not a declarable dimension: it is the mandatory first component of
-- every depth vector, so a tenant's binding always beats a platform-shipped one.
-- Without that, a per-kind order ranking Product above Tenancy would let our
-- default outrank a tenant's own configuration, which is a correctness hole
-- rather than a support surface. S15 asserts it is index 0.
--
-- The per-kind precedence order after tenancy is declared in the Rust registry.
-- This function implements the default order; a kind wanting a different one
-- passes its permutation rather than getting its own function.
CREATE FUNCTION resolve_policy_binding(
        p_tenant  uuid,
        p_kind    policy_kind,
        p_item    uuid DEFAULT NULL,
        p_party   uuid DEFAULT NULL,
        p_site    uuid DEFAULT NULL,
        p_zone    uuid DEFAULT NULL,
        p_owner   uuid DEFAULT NULL,
        p_metric  uuid DEFAULT NULL)
    RETURNS uuid
    LANGUAGE sql STABLE
    AS $$
    SELECT b.id
      FROM policy_binding b
      LEFT JOIN item_classification ic
             ON ic.tenant_id = p_tenant AND ic.item_id = p_item
      LEFT JOIN party pc ON pc.id = p_party
     WHERE b.kind = p_kind
       AND (b.tenant_id IS NULL OR b.tenant_id = p_tenant)
       -- Every set axis must be at-or-above the request on that axis. An unset
       -- axis matches anything, which is what NULL means here.
       AND (b.item_id IS NULL OR b.item_id = p_item)
       AND (b.item_class_id IS NULL OR EXISTS (
             SELECT 1 FROM item_class_closure c
              WHERE c.ancestor_id = b.item_class_id
                AND c.descendant_id = ic.item_class_id))
       AND (b.party_id IS NULL OR b.party_id = p_party)
       AND (b.party_class_id IS NULL OR EXISTS (
             SELECT 1 FROM party_class_closure c
              WHERE c.ancestor_id = b.party_class_id
                AND c.descendant_id = pc.party_class_id))
       AND (b.site_id IS NULL OR b.site_id = p_site)
       AND (b.zone_id IS NULL OR b.zone_id = p_zone)
       AND (b.owner_party_id IS NULL OR b.owner_party_id = p_owner)
       AND (b.metric_id IS NULL OR b.metric_id = p_metric)
     ORDER BY
       -- Index 0, always: a tenant's binding beats ours.
       (b.tenant_id IS NOT NULL) DESC,
       -- Product, most specific first.
       (b.item_id IS NOT NULL) DESC, (b.item_class_id IS NOT NULL) DESC,
       -- Counterparty, then Space, then Ownership, then Metric.
       (b.party_id IS NOT NULL) DESC, (b.party_class_id IS NOT NULL) DESC,
       (b.zone_id IS NOT NULL) DESC, (b.site_id IS NOT NULL) DESC,
       (b.owner_party_id IS NOT NULL) DESC,
       (b.metric_id IS NOT NULL) DESC
     LIMIT 1
$$;

COMMENT ON FUNCTION resolve_policy_binding IS
    'Most-specific-wins over the scope lattice. Returns the winning binding, or '
    'NULL when not even a platform default exists, which is a configuration gap '
    'rather than an error. D22.';

-- The explain view D22 asks for: which bindings matched, in the order the
-- resolver considered them. A manager who cannot see why a policy resolved the
-- way it did will eventually add a column to make it obvious.
CREATE VIEW policy_binding_scope AS
    SELECT b.id, b.tenant_id, b.kind,
           concat_ws(', ',
               CASE WHEN b.tenant_id IS NULL THEN 'platform default' END,
               CASE WHEN b.item_id IS NOT NULL THEN 'item' END,
               CASE WHEN b.item_class_id IS NOT NULL THEN 'item class' END,
               CASE WHEN b.party_id IS NOT NULL THEN 'party' END,
               CASE WHEN b.party_class_id IS NOT NULL THEN 'party class' END,
               CASE WHEN b.zone_id IS NOT NULL THEN 'zone' END,
               CASE WHEN b.site_id IS NOT NULL THEN 'site' END,
               CASE WHEN b.owner_party_id IS NOT NULL THEN 'owner' END,
               CASE WHEN b.metric_id IS NOT NULL THEN 'metric' END) AS scope,
           b.note
      FROM policy_binding b;

GRANT SELECT ON policy_binding_scope TO spork_app;
