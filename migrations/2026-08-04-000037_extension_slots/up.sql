-- Migration 37: the ceiling is data, so there is no number to disagree with.
--
-- D80. D26 was adopted on 2026-08-01 and D36 settled its ceilings on 2026-08-03,
-- and neither was built. Three invariants have been waiting: J21, J39, J40.
--
-- This is the **declaration layer** — the tables a schema compiler must claim
-- against before it runs any DDL. The compiler itself is not here, and section 5
-- says what else is not.

-- ---------------------------------------------------------------------------
-- 1. Why the ceiling is rows rather than a number
-- ---------------------------------------------------------------------------
--
-- D36's argument, which is narrower and stronger than "undefined behaviour is
-- bad": **by the time a job runs there is no action left that is not worse than
-- the violation.** The tenant declared, the compiler ran DDL, the table exists
-- and rows are landing in it. Tomorrow's job can drop the table, destroying
-- tenant data in a model whose generated tables carry `ON DELETE RESTRICT`
-- precisely so evidence is never destroyed as a side effect — or it can raise a
-- finding and change nothing, which makes the ceiling a note.
--
-- So the ceiling holds before the DDL or it does not hold. `CREATE TABLE` is
-- transactional in Postgres, so a failed claim inside the materialisation
-- transaction rolls the DDL back with it and "mid-declaration" is not a state
-- that exists.
--
-- And it cannot be a constant. D25 forbids validation triggers by name, a `CHECK`
-- cannot hold a subquery, and a counter column is a projection with no fact
-- behind it. **A number in a `CHECK` and a number in a plan description are two
-- representations that will eventually disagree**, and this design has one:
-- issued rows. Raising a tenant's ceiling is inserting slots, which is a platform
-- act with a row and a timestamp behind it rather than a migration.

CREATE TYPE extension_slot_kind AS ENUM ('record_scheme', 'metric');

CREATE TABLE extension_slot (
    tenant_id     uuid NOT NULL REFERENCES tenant(id),
    kind          extension_slot_kind NOT NULL,
    ordinal       integer NOT NULL,

    -- NULL is free. The claim is by **key**, not by row: D36 separates the two
    -- ceilings that "50 schemes per tenant" was conflating, because D26's
    -- evolution rule mints a new table per version and the old one stays. Under
    -- the row reading a tenant with ten well-maintained schemes is punished for
    -- maintaining them.
    claimed_key   text,
    claimed_at    timestamptz,
    withdrawn_at  timestamptz,

    PRIMARY KEY (tenant_id, kind, ordinal),
    CONSTRAINT extension_slot_key_once UNIQUE (tenant_id, kind, claimed_key),
    CONSTRAINT extension_slot_claim_ck
        CHECK ((claimed_key IS NULL) = (claimed_at IS NULL)),
    CONSTRAINT extension_slot_ordinal_ck CHECK (ordinal > 0)
);

COMMENT ON TABLE extension_slot IS
    'The extension ceiling, as issued rows rather than a constant. Platform-owned '
    'like number_range: a tenant may read its slots and may not issue itself more. '
    'D26, D36.';
COMMENT ON COLUMN extension_slot.claimed_key IS
    'The scheme key holding this slot. Every version of that key shares it, so '
    'the boundary counts distinct schemes -- which is what it was always about. '
    'D36.';
COMMENT ON COLUMN extension_slot.withdrawn_at IS
    'A slot the platform has taken out of issue. Withdrawing cannot delete a '
    'claimed slot: the foreign key from record_scheme would refuse, and cascading '
    'would mean deleting tenant tables to enforce a billing change. D36.';

-- ---------------------------------------------------------------------------
-- 2. record_scheme -- a code generator whose input happens to live in a row
-- ---------------------------------------------------------------------------
--
-- D19's nullable-tenant shape: `tenant_id NULL` is a scheme we ship. A shipped
-- scheme consumes no tenant's ceiling, which is why the slot reference is
-- nullable and why J39 is about tenant schemes.

CREATE TYPE record_scheme_provenance AS ENUM
    ('fact', 'intention', 'assertion', 'finding');
CREATE TYPE record_scheme_role AS ENUM ('reference', 'grouping');
CREATE TYPE record_scheme_cardinality AS ENUM ('one', 'many');
CREATE TYPE record_scheme_source AS ENUM ('shipped', 'tenant', 'plugin');
CREATE TYPE record_scheme_state AS ENUM
    ('declared', 'materialising', 'materialised', 'retired');

CREATE TABLE record_scheme (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id       uuid REFERENCES tenant(id),
    key             text NOT NULL,
    version         integer NOT NULL,

    provenance      record_scheme_provenance NOT NULL,
    role            record_scheme_role NOT NULL,
    -- A core entity name. The set of legal values is the code-side table
    -- registry, which does not exist here and is why two other invariants are
    -- pending on it -- so this is text and the compiler is what refuses an
    -- unknown one.
    attaches_to     text NOT NULL,
    cardinality     record_scheme_cardinality NOT NULL,

    physical_table  text,
    manifest_source bytea,
    manifest_hash   bytea,

    source          record_scheme_source NOT NULL,
    state           record_scheme_state NOT NULL DEFAULT 'declared',
    materialised_at timestamptz,
    created_by_id   uuid REFERENCES person(id),

    -- The claiming slot. `slot_kind` is a stored generated constant so the
    -- foreign key can be composite without the caller restating it -- the same
    -- device D21's typed bodies use to pin a body to its kind.
    slot_ordinal    integer,
    slot_kind       extension_slot_kind GENERATED ALWAYS AS
                        ('record_scheme'::extension_slot_kind) STORED,

    CONSTRAINT record_scheme_version_key UNIQUE (tenant_id, key, version),
    CONSTRAINT record_scheme_tenant_key UNIQUE (id, tenant_id),
    CONSTRAINT record_scheme_slot_fk
        FOREIGN KEY (tenant_id, slot_kind, slot_ordinal)
        REFERENCES extension_slot (tenant_id, kind, ordinal),
    -- A tenant scheme holds a slot; a shipped one has no tenant whose ceiling it
    -- could consume. J39 checks the first half against the data.
    CONSTRAINT record_scheme_slot_ck
        CHECK ((tenant_id IS NULL) = (slot_ordinal IS NULL)),
    CONSTRAINT record_scheme_source_ck
        CHECK ((source = 'shipped') = (tenant_id IS NULL)),
    CONSTRAINT record_scheme_materialised_ck
        CHECK ((state = 'materialised') = (materialised_at IS NOT NULL)),
    CONSTRAINT record_scheme_physical_table_ck
        CHECK (physical_table IS NULL OR physical_table ~ '^ext_[a-z0-9_]+$'),
    CONSTRAINT record_scheme_version_ck CHECK (version > 0)
);

COMMENT ON TABLE record_scheme IS
    'A code generator whose input happens to live in a row, not an attribute '
    'store. The generated table gets real columns, real foreign keys with ON '
    'DELETE RESTRICT, RLS with FORCE and grants derived from provenance. D26.';
COMMENT ON COLUMN record_scheme.physical_table IS
    'Immutable once set. The name is the compiler''s output and the join target '
    'for every "show me this tenant''s facts" query, which is a generated query '
    'rather than a hunt. D26.';

CREATE UNIQUE INDEX record_scheme_physical_table_idx
    ON record_scheme (physical_table) WHERE physical_table IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 3. record_scheme_field -- the compiled symbol table, rows rather than JSON
-- ---------------------------------------------------------------------------
--
-- **The 60-field ceiling is a constant here, and D36 states the test between the
-- two shapes rather than leaving it to taste:**
--
--   "A ceiling that is per-tenant and commercially variable is issued slots. A
--    ceiling that is per-parent and fixed by design is an ordinal with a CHECK."
--
-- Sixty fields is a statement about what one table should hold before it wants to
-- be two, and that is the same for every tenant on every plan. The 61st field has
-- nowhere to go, declaratively and with no trigger.

CREATE TYPE record_scheme_field_type AS ENUM
    ('integer', 'text', 'boolean', 'date', 'timestamptz',
     'quantity', 'money_minor', 'enum', 'ref', 'attachment');

CREATE TABLE record_scheme_field (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    record_scheme_id uuid NOT NULL,
    tenant_id        uuid,

    ordinal          integer NOT NULL,
    column_name      text NOT NULL,
    label            text,

    field_type       record_scheme_field_type NOT NULL,
    unit_id          uuid REFERENCES unit(id),
    currency         text,
    enum_values      text[],
    ref_entity       text,

    required         boolean NOT NULL DEFAULT false,
    min_value        numeric,
    max_value        numeric,

    CONSTRAINT record_scheme_field_scheme_fk
        FOREIGN KEY (record_scheme_id, tenant_id)
        REFERENCES record_scheme (id, tenant_id),
    CONSTRAINT record_scheme_field_ordinal_key UNIQUE (record_scheme_id, ordinal),
    CONSTRAINT record_scheme_field_column_key UNIQUE (record_scheme_id, column_name),
    -- D36. Sixty, and the 61st has nowhere to go.
    CONSTRAINT record_scheme_field_ordinal_ck CHECK (ordinal BETWEEN 1 AND 60),
    CONSTRAINT record_scheme_field_column_name_ck
        CHECK (column_name ~ '^[a-z][a-z0-9_]*$'),
    -- D26: "CHECK (parameter presence matches field_type)". Each parameter is
    -- required by exactly the type that generates it and forbidden elsewhere,
    -- so a quantity without a unit cannot be declared and then compiled into a
    -- column whose numbers mean nothing.
    CONSTRAINT record_scheme_field_unit_ck
        CHECK ((field_type = 'quantity') = (unit_id IS NOT NULL)),
    CONSTRAINT record_scheme_field_currency_ck
        CHECK ((field_type = 'money_minor') = (currency IS NOT NULL)),
    CONSTRAINT record_scheme_field_enum_ck
        CHECK ((field_type = 'enum')
               = (enum_values IS NOT NULL AND cardinality(enum_values) > 0)),
    CONSTRAINT record_scheme_field_ref_ck
        CHECK ((field_type = 'ref') = (ref_entity IS NOT NULL)),
    CONSTRAINT record_scheme_field_range_ck
        CHECK (min_value IS NULL OR max_value IS NULL OR min_value <= max_value)
);

COMMENT ON TABLE record_scheme_field IS
    'The compiled symbol table: rows, not JSON. Each row generates a real column '
    'with a real type, and the parameter CHECKs are what stop a declaration that '
    'cannot compile into one. D26, D36.';

CREATE INDEX record_scheme_field_scheme_idx ON record_scheme_field (record_scheme_id);

-- ---------------------------------------------------------------------------
-- 4. The claim, which is the whole enforcement
-- ---------------------------------------------------------------------------
--
-- D36's query, unchanged, with the concurrency argument it states:
--
--   "Two transactions claiming simultaneously both target the lowest free
--    ordinal; one blocks on the row lock, then re-evaluates its predicate under
--    READ COMMITTED, finds `claimed_key` no longer null, updates zero rows and
--    moves to the next ordinal or hits the ceiling honestly. No lost update, no
--    double claim, no advisory lock."
--
-- **Zero rows returned is the ceiling** — a defined, testable outcome rather than
-- an error class. NULL is that outcome here, and the caller decides whether a
-- full ceiling is an error or a prompt to buy more.
--
-- Re-claiming the same key returns the slot it already holds, so declaring
-- version N+1 of a scheme costs nothing: every version of a key shares one slot.

CREATE FUNCTION extension_slot_claim(
        p_tenant uuid, p_kind extension_slot_kind, p_key text)
    RETURNS integer
    LANGUAGE plpgsql
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    held integer;
BEGIN
    SELECT ordinal INTO held FROM extension_slot
     WHERE tenant_id = p_tenant AND kind = p_kind AND claimed_key = p_key;
    IF held IS NOT NULL THEN
        RETURN held;
    END IF;

    UPDATE extension_slot SET claimed_key = p_key, claimed_at = now()
     WHERE (tenant_id, kind, ordinal) = (
         SELECT tenant_id, kind, ordinal FROM extension_slot
          WHERE tenant_id = p_tenant AND kind = p_kind
            AND claimed_key IS NULL AND withdrawn_at IS NULL
          ORDER BY ordinal LIMIT 1)
    RETURNING ordinal INTO held;

    RETURN held;
END
$$;

COMMENT ON FUNCTION extension_slot_claim(uuid, extension_slot_kind, text) IS
    'Claims the lowest free slot for a key, or returns the one that key already '
    'holds, or NULL when the ceiling is reached. NULL is the ceiling and is a '
    'defined outcome rather than an error class. D36.';

-- ---------------------------------------------------------------------------
-- 5. RLS and grants
-- ---------------------------------------------------------------------------

ALTER TABLE extension_slot ENABLE ROW LEVEL SECURITY;
ALTER TABLE extension_slot FORCE ROW LEVEL SECURITY;
CREATE POLICY extension_slot_tenant_scoped ON extension_slot
    USING (tenant_id = current_tenant());

-- D19's shared-reference shape, and D55's pair: read what is ours or shipped,
-- write only what is ours -- and only the platform writes a shipped row.
ALTER TABLE record_scheme ENABLE ROW LEVEL SECURITY;
ALTER TABLE record_scheme FORCE ROW LEVEL SECURITY;
CREATE POLICY record_scheme_shared_read ON record_scheme FOR SELECT
    USING (tenant_id IS NULL OR tenant_id = current_tenant());
CREATE POLICY record_scheme_own_write ON record_scheme
    USING (tenant_id = current_tenant() OR (tenant_id IS NULL AND is_platform()))
    WITH CHECK (tenant_id = current_tenant() OR (tenant_id IS NULL AND is_platform()));

ALTER TABLE record_scheme_field ENABLE ROW LEVEL SECURITY;
ALTER TABLE record_scheme_field FORCE ROW LEVEL SECURITY;
CREATE POLICY record_scheme_field_shared_read ON record_scheme_field FOR SELECT
    USING (tenant_id IS NULL OR tenant_id = current_tenant());
CREATE POLICY record_scheme_field_own_write ON record_scheme_field
    USING (tenant_id = current_tenant() OR (tenant_id IS NULL AND is_platform()))
    WITH CHECK (tenant_id = current_tenant() OR (tenant_id IS NULL AND is_platform()));

-- **A tenant may read its ceiling and may not raise it.** Issuing slots is a
-- platform act, which is the whole point of making the ceiling data.
GRANT SELECT ON extension_slot TO nylonite_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON extension_slot TO nylonite_platform;

-- Declaring a scheme runs DDL, so it is the compiler's act and the compiler is
-- platform. The application reads what has been declared.
GRANT SELECT ON record_scheme, record_scheme_field TO nylonite_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON record_scheme, record_scheme_field
    TO nylonite_platform;

GRANT EXECUTE ON FUNCTION extension_slot_claim(uuid, extension_slot_kind, text)
    TO nylonite_platform;

-- ---------------------------------------------------------------------------
-- 6. What this does not build
-- ---------------------------------------------------------------------------
--
-- **The compiler.** Nothing here runs `CREATE TABLE`. D36's whole argument is
-- that the claim must happen inside the materialisation transaction, and that
-- transaction lives in application code that does not exist. What is built is the
-- thing it must claim against, which is the half that cannot be added afterwards.
--
-- **Slot release.** D36: *"a slot is released only when every table its scheme
-- ever materialised has been archived under D31."* D31's archival is not built —
-- nothing records that a table was archived — so a release function could not
-- check its own precondition, and one that cannot is worse than none. **J40 stays
-- pending on `retention_floor`** rather than being answered by a function that
-- would always say yes.
--
-- **The outbox and `event_subscription`**, which are the other third of D26, and
-- **tenant metrics**, which are the second slot kind. The `metric` kind exists in
-- the enum because the ceiling is per-kind and issuing metric slots is the same
-- act; nothing claims one yet.
--
-- **`plugin_id`.** D26 lists it on `record_scheme` and there is no plugin
-- registry, so the column is absent rather than unfillable — the lesson S16 and
-- S43 taught D78 an hour ago.
