-- Migration 1: the reference layer.
--
-- Ordered first by docs/migration-1.md because everything else has a foreign key
-- into it and none of it depends on anything else. Nothing here holds an
-- observation, an intention or an assertion; those come later and land on top.
--
-- Three rules govern every table below and they come from decisions rather than
-- from taste:
--
--   D18/D19  tenancy has exactly three shapes. Global tables carry no tenant_id.
--            Shared reference carries a nullable one where NULL means shared.
--            Everything operational carries tenant_id NOT NULL.
--   D25      the application role never holds UPDATE on a projection column and
--            never holds anything on a fact table. Enforced by grant, not by
--            convention.
--   S8       every RLS-protected table gets FORCE ROW LEVEL SECURITY, so the
--            owner is not exempt from its own policy.

-- ---------------------------------------------------------------------------
-- Roles and the tenancy function
-- ---------------------------------------------------------------------------

-- Created idempotently: a migration that fails on a second database because a
-- role already exists is a migration nobody can run twice.
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'spork_app') THEN
        CREATE ROLE spork_app NOLOGIN;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'spork_platform') THEN
        CREATE ROLE spork_platform NOLOGIN;
    END IF;
END
$$;

-- The tenant of the current session. RLS policies read this and nothing else.
-- STABLE rather than IMMUTABLE because it varies per session, and marked
-- LEAKPROOF-free deliberately: it is not, and claiming otherwise would let the
-- planner push it past a policy.
CREATE FUNCTION current_tenant() RETURNS uuid
    LANGUAGE sql STABLE
    AS $$ SELECT nullif(current_setting('spork.tenant_id', true), '')::uuid $$;

COMMENT ON FUNCTION current_tenant() IS
    'The session tenant, from the spork.tenant_id GUC. NULL outside a tenant '
    'context, which every tenant-scoped policy treats as matching nothing.';

-- ---------------------------------------------------------------------------
-- Tenancy root (D18)
-- ---------------------------------------------------------------------------

CREATE TABLE tenant (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name        text NOT NULL,
    slug        text NOT NULL,
    active      boolean NOT NULL DEFAULT true,
    created_at  timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT tenant_slug_key UNIQUE (slug)
);

COMMENT ON TABLE tenant IS
    'REFERENCE, platform-owned. The tenancy root, so it carries no tenant_id of '
    'its own. D18.';

-- ---------------------------------------------------------------------------
-- People span tenants (D19)
-- ---------------------------------------------------------------------------

-- Shape 1: global. No tenant_id at all, because one person may work for two
-- tenants and duplicating them would make "who did this" ambiguous across a
-- boundary it is supposed to be answerable across.
CREATE TABLE person (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name text NOT NULL,
    email        text,
    active       boolean NOT NULL DEFAULT true,
    created_at   timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT person_email_key UNIQUE (email)
);

COMMENT ON TABLE person IS
    'REFERENCE, global. No tenant_id: membership is person_tenant. The facts a '
    'person appears on are tenant-scoped, so cross-tenant visibility of their '
    'activity falls out of RLS rather than needing a rule. D19.';

CREATE TABLE person_tenant (
    person_id  uuid NOT NULL REFERENCES person(id),
    tenant_id  uuid NOT NULL REFERENCES tenant(id),
    role       text NOT NULL,
    joined_at  timestamptz NOT NULL DEFAULT now(),
    left_at    timestamptz,
    PRIMARY KEY (person_id, tenant_id),
    CONSTRAINT person_tenant_interval_ck CHECK (left_at IS NULL OR left_at >= joined_at)
);

-- ---------------------------------------------------------------------------
-- Units and dimensions (D23, principle 5)
-- ---------------------------------------------------------------------------

-- Shape 1: global, shipped by us, never tenant-scoped. A metre is a metre.
CREATE TABLE dimension (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    code              text NOT NULL,
    canonical_unit_id uuid NOT NULL,          -- FK added below: mutual reference
    CONSTRAINT dimension_code_key UNIQUE (code),
    CONSTRAINT dimension_canonical_key UNIQUE (id, canonical_unit_id)
);

CREATE TABLE unit (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    dimension_id    uuid NOT NULL REFERENCES dimension(id),
    code            text NOT NULL,
    ucum_code       text,
    uncefact_code   text,

    -- Exact rational conversion, never a float. An inch is 254/10 mm and it is
    -- that exactly. Principle 5.
    factor_num      bigint NOT NULL,
    factor_den      bigint NOT NULL,
    offset_num      bigint NOT NULL DEFAULT 0,
    offset_den      bigint NOT NULL DEFAULT 1,
    display_decimals smallint NOT NULL DEFAULT 0,

    CONSTRAINT unit_dimension_code_key UNIQUE (dimension_id, code),
    CONSTRAINT unit_dimension_key UNIQUE (id, dimension_id),
    CONSTRAINT unit_factor_den_ck CHECK (factor_den <> 0),
    CONSTRAINT unit_offset_den_ck CHECK (offset_den <> 0)
);

ALTER TABLE dimension
    ADD CONSTRAINT dimension_canonical_unit_fk
    FOREIGN KEY (canonical_unit_id, id) REFERENCES unit(id, dimension_id)
    DEFERRABLE INITIALLY DEFERRED;

COMMENT ON CONSTRAINT dimension_canonical_unit_fk ON dimension IS
    'Composite and deferrable. Composite so a dimension cannot nominate a unit '
    'of some other dimension as canonical. Deferrable because dimension and unit '
    'reference each other and one of the two inserts has to come first.';

COMMENT ON COLUMN unit.factor_num IS
    'S22 asserts every canonical unit has factor 1/1 and offset 0. Non-canonical '
    'storage is structurally unrepresentable elsewhere: observation.value_numeric '
    'is always in the dimension canonical unit and carries no unit column.';

-- ---------------------------------------------------------------------------
-- Sites, zones and locations (D18, D46)
-- ---------------------------------------------------------------------------

-- Shape 3: tenant-scoped.
CREATE TABLE site (
    id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  uuid NOT NULL REFERENCES tenant(id),
    name       text NOT NULL,
    code       text NOT NULL,
    timezone   text NOT NULL,                 -- IANA name, e.g. Australia/Melbourne
    active     boolean NOT NULL DEFAULT true,
    CONSTRAINT site_tenant_code_key UNIQUE (tenant_id, code),
    -- Composite target so children can prove they share their parent's tenant.
    CONSTRAINT site_tenant_key UNIQUE (id, tenant_id)
);

-- D46. Identity, membership and a name. Everything else a zone might carry is
-- either a property of a location or a policy bound to the zone, which is what
-- D22 built the Space dimension for.
CREATE TABLE zone (
    id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  uuid NOT NULL,
    site_id    uuid NOT NULL,
    code       text NOT NULL,
    name       text NOT NULL,
    active     boolean NOT NULL DEFAULT true,

    CONSTRAINT zone_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id),
    CONSTRAINT zone_tenant_site_code_key UNIQUE (tenant_id, site_id, code),
    -- S37's target. Without it the composite FK on location has nothing to point at.
    CONSTRAINT zone_site_key UNIQUE (id, site_id)
);

COMMENT ON TABLE zone IS
    'REFERENCE, tenant-scoped. A name for a set of locations. Temperature, '
    'priority and putaway preference are policies bound to this zone, never '
    'columns on it. D46.';

COMMENT ON COLUMN zone.tenant_id IS
    'Denormalised through site on purpose: D19 shape 3 wants it NOT NULL, and '
    'J14 is a local join with it and a two-hop join without. The composite FK '
    'makes disagreement with the site unrepresentable. D46.';

CREATE TABLE location (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      uuid NOT NULL,
    site_id        uuid NOT NULL,
    zone_id        uuid,                      -- nullable: a dock belongs to no zone
    code           text NOT NULL,

    -- Parsed, not just a code string, because the router and the map both need
    -- the components and re-parsing a string in a query is how that gets slow.
    aisle          text,
    bay            text,
    level          text,
    position       text,

    x_mm           integer,
    y_mm           integer,
    z_mm           integer,
    length_mm      integer,
    width_mm       integer,
    height_mm      integer,
    max_weight_g   bigint,

    kind           text NOT NULL,
    active         boolean NOT NULL DEFAULT true,

    CONSTRAINT location_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id),
    -- S37. A plain zone_id FK would let a location sit in another site's zone,
    -- and every zone-scoped policy resolution for it would then silently return
    -- that other site's answer. D46.
    CONSTRAINT location_zone_fk FOREIGN KEY (zone_id, site_id)
        REFERENCES zone(id, site_id),
    CONSTRAINT location_tenant_site_code_key UNIQUE (tenant_id, site_id, code),
    CONSTRAINT location_kind_ck CHECK (kind IN
        ('pick_face', 'bulk', 'staging', 'dock', 'overflow'))
);

CREATE INDEX location_zone_idx ON location (zone_id) WHERE zone_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- Catalogue (D19, D22, D33)
-- ---------------------------------------------------------------------------

-- Shape 2: shared reference. NULL tenant_id means the shared catalogue, which
-- only the platform role may write. A shared item is thin by construction:
-- identity and intrinsic properties only, because a tenant's measurements and
-- packing configs cannot be shared without one tenant's corrections rewriting
-- another's. D19.
CREATE TABLE item (
    id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           uuid REFERENCES tenant(id),   -- NULL = shared
    code                text NOT NULL,
    description         text NOT NULL,
    base_unit_id        uuid NOT NULL REFERENCES unit(id),
    active              boolean NOT NULL DEFAULT true,

    -- Intrinsic: what the thing is, therefore shareable.
    dangerous_goods_class text,
    un_number           text,
    packing_group       text,
    temperature_class   text,
    stackable           boolean NOT NULL DEFAULT true,
    max_stack_height_mm integer,
    this_way_up         boolean NOT NULL DEFAULT false,

    -- D33. Mutable, because an item can start being lot-tracked, and historical
    -- movements made before that change must not fail the assertion.
    tracking            text NOT NULL DEFAULT 'none',
    tracking_effective_from date,

    CONSTRAINT item_tracking_ck CHECK (tracking IN ('none', 'lot', 'serial')),
    CONSTRAINT item_tracking_from_ck
        CHECK (tracking = 'none' OR tracking_effective_from IS NOT NULL),
    CONSTRAINT item_code_key UNIQUE NULLS NOT DISTINCT (tenant_id, code)
);

COMMENT ON COLUMN item.tracking_effective_from IS
    'D33. Without this, switching an item to lot-tracked retroactively flags '
    'every movement it ever had.';

-- Per-tenant rooted tree. D22's Product dimension is the only one carrying an
-- ancestors level, because item taxonomies genuinely nest.
CREATE TABLE item_class (
    id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  uuid NOT NULL REFERENCES tenant(id),
    parent_id  uuid,
    code       text NOT NULL,
    name       text NOT NULL,
    CONSTRAINT item_class_parent_fk FOREIGN KEY (parent_id) REFERENCES item_class(id),
    CONSTRAINT item_class_tenant_code_key UNIQUE (tenant_id, code),
    CONSTRAINT item_class_tenant_key UNIQUE (id, tenant_id),
    CONSTRAINT item_class_not_own_parent_ck CHECK (parent_id <> id)
);

-- The closure table is what makes D22's matching language cardinality one:
-- "is this node an ancestor-or-self of that node" is a lookup, not a recursion.
-- No <, no LIKE, no IN, which is why the resolver is not a rules engine.
CREATE TABLE item_class_closure (
    tenant_id   uuid NOT NULL,
    ancestor_id uuid NOT NULL,
    descendant_id uuid NOT NULL,
    depth       integer NOT NULL,
    PRIMARY KEY (ancestor_id, descendant_id),
    CONSTRAINT item_class_closure_ancestor_fk FOREIGN KEY (ancestor_id, tenant_id)
        REFERENCES item_class(id, tenant_id),
    CONSTRAINT item_class_closure_descendant_fk FOREIGN KEY (descendant_id, tenant_id)
        REFERENCES item_class(id, tenant_id),
    CONSTRAINT item_class_closure_depth_ck CHECK (depth >= 0)
);

CREATE INDEX item_class_closure_descendant_idx
    ON item_class_closure (descendant_id, depth);

COMMENT ON TABLE item_class_closure IS
    'PROJECTION of item_class. Maintained by a named function under D35, never '
    'written by the application role.';

-- Classification is an association, not a column on item: item.item_class_id
-- NOT NULL would break D19, because a shared item cannot carry a mandatory FK
-- into one tenant's private taxonomy. Single parentage is preserved by the
-- primary key, which is what D22's tie-freedom proof rests on.
CREATE TABLE item_classification (
    tenant_id     uuid NOT NULL,
    item_id       uuid NOT NULL REFERENCES item(id),
    item_class_id uuid NOT NULL,
    PRIMARY KEY (tenant_id, item_id),
    CONSTRAINT item_classification_class_fk FOREIGN KEY (item_class_id, tenant_id)
        REFERENCES item_class(id, tenant_id)
);

-- ---------------------------------------------------------------------------
-- Row level security (D18, D19, S8, S9)
-- ---------------------------------------------------------------------------

-- Shape 1, global: person, person_tenant, dimension, unit and tenant carry no
-- policy. They are readable by every tenant because they contain nothing
-- tenant-specific.

-- Shape 2, shared reference: readable when shared or ours.
ALTER TABLE item ENABLE ROW LEVEL SECURITY;
ALTER TABLE item FORCE ROW LEVEL SECURITY;
CREATE POLICY item_shared_reference ON item
    USING (tenant_id IS NULL OR tenant_id = current_tenant());

-- Shape 3, tenant-scoped.
ALTER TABLE site ENABLE ROW LEVEL SECURITY;
ALTER TABLE site FORCE ROW LEVEL SECURITY;
CREATE POLICY site_tenant_scoped ON site
    USING (tenant_id = current_tenant());

ALTER TABLE zone ENABLE ROW LEVEL SECURITY;
ALTER TABLE zone FORCE ROW LEVEL SECURITY;
CREATE POLICY zone_tenant_scoped ON zone
    USING (tenant_id = current_tenant());

ALTER TABLE location ENABLE ROW LEVEL SECURITY;
ALTER TABLE location FORCE ROW LEVEL SECURITY;
CREATE POLICY location_tenant_scoped ON location
    USING (tenant_id = current_tenant());

ALTER TABLE item_class ENABLE ROW LEVEL SECURITY;
ALTER TABLE item_class FORCE ROW LEVEL SECURITY;
CREATE POLICY item_class_tenant_scoped ON item_class
    USING (tenant_id = current_tenant());

ALTER TABLE item_class_closure ENABLE ROW LEVEL SECURITY;
ALTER TABLE item_class_closure FORCE ROW LEVEL SECURITY;
CREATE POLICY item_class_closure_tenant_scoped ON item_class_closure
    USING (tenant_id = current_tenant());

ALTER TABLE item_classification ENABLE ROW LEVEL SECURITY;
ALTER TABLE item_classification FORCE ROW LEVEL SECURITY;
CREATE POLICY item_classification_tenant_scoped ON item_classification
    USING (tenant_id = current_tenant());

-- ---------------------------------------------------------------------------
-- Grants (D25)
-- ---------------------------------------------------------------------------

GRANT USAGE ON SCHEMA public TO spork_app, spork_platform;

-- Reference data the application may maintain within its own tenant. RLS
-- decides which rows; the grant decides which verbs.
GRANT SELECT, INSERT, UPDATE, DELETE ON
    site, zone, location, item_class, item_classification
    TO spork_app;

-- Shared rows belong to the platform. The application may read the shared
-- catalogue and write only its own rows, which RLS enforces per row; the
-- platform role is what writes tenant_id IS NULL.
GRANT SELECT, INSERT, UPDATE, DELETE ON item TO spork_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON item TO spork_platform;

GRANT SELECT ON tenant, person, person_tenant, dimension, unit TO spork_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON
    tenant, person, person_tenant, dimension, unit
    TO spork_platform;

-- The closure is a projection. J36 asserts no login role holds INSERT, UPDATE
-- or DELETE on a projection, so the application reads it and the maintainer
-- function writes it.
GRANT SELECT ON item_class_closure TO spork_app;

GRANT EXECUTE ON FUNCTION current_tenant() TO spork_app, spork_platform;
