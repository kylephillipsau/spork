-- Migration 2: the spine.
--
-- stock_movement is the ledger and stock is the fold of it. Everything else in
-- the system is downstream of these two tables, which is why the architecture
-- calls them the spine and why the design spent D4, D12, D20 and D24 arriving at
-- the key they share.
--
-- Scope. This migration adds the ledger and the reference rows it cannot exist
-- without: client_event for idempotency, inventory_status, lot, a minimal party
-- for ownership, and a minimal package so the containment arm has a target.
-- package_event's fold and stock_allocation are migration 3. The line is drawn
-- where it is because stock and stock_movement have to land together: a ledger
-- with no projection is unreadable and a projection with no ledger is a lie.

-- ---------------------------------------------------------------------------
-- Idempotency (D5)
-- ---------------------------------------------------------------------------

-- Sending the same entry twice changes nothing and entries may arrive in any
-- order. The handheld generates the identifier, so a network dropout is a few
-- late arrivals rather than a separate offline mode with its own code to
-- maintain.
CREATE TABLE client_event (
    tenant_id       uuid NOT NULL REFERENCES tenant(id),
    client_event_id uuid NOT NULL,
    site_id         uuid,
    device_id       uuid,
    work_session_id uuid,

    -- D11. Exactly one of a person or an automation, never both and never
    -- neither, because "who did this" has to have an answer.
    recorded_by_id  uuid REFERENCES person(id),
    automation_key  text,

    app_version     text,
    submitted_at    timestamptz NOT NULL,   -- the device clock
    received_at     timestamptz NOT NULL DEFAULT now(),   -- ours

    PRIMARY KEY (tenant_id, client_event_id),
    CONSTRAINT client_event_actor_ck
        CHECK (num_nonnulls(recorded_by_id, automation_key) = 1),
    CONSTRAINT client_event_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id)
);

COMMENT ON TABLE client_event IS
    'FACT. Unpartitioned by necessity: every fact table references it, and a '
    'partitioned primary key would degrade replay detection to per-partition. D5.';

-- ---------------------------------------------------------------------------
-- The dimensions of the stock key that do not exist yet (D4, D14, D32)
-- ---------------------------------------------------------------------------

-- D4. Status joins the key rather than being a location, which is what every
-- competitor's quarantine-as-a-location fudge gets wrong.
CREATE TABLE inventory_status (
    id                          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id                   uuid REFERENCES tenant(id),   -- NULL = shipped
    code                        text NOT NULL,
    name                        text NOT NULL,
    is_available_for_allocation boolean NOT NULL DEFAULT true,
    CONSTRAINT inventory_status_code_key UNIQUE NULLS NOT DISTINCT (tenant_id, code)
);

COMMENT ON COLUMN inventory_status.is_available_for_allocation IS
    'S25: never joined on the availability path. Availability indexes carry '
    'status_id in the key instead, so the hot query never reaches this table.';

-- D32. One party for identity, roles as relationships. Minimal here: ownership
-- is the only role the spine needs, and the role table arrives with inbound.
CREATE TABLE party (
    id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  uuid NOT NULL REFERENCES tenant(id),
    name       text NOT NULL,
    code       text NOT NULL,
    CONSTRAINT party_tenant_code_key UNIQUE (tenant_id, code),
    CONSTRAINT party_tenant_key UNIQUE (id, tenant_id)
);

-- D14. Lot is a real entity, and D33 made its absence a finding rather than a
-- rejection, so nothing here is NOT NULL that the floor might not know.
CREATE TABLE lot (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   uuid NOT NULL REFERENCES tenant(id),
    item_id     uuid NOT NULL REFERENCES item(id),
    code        text NOT NULL,
    expiry_date date,
    best_before_date date,
    CONSTRAINT lot_tenant_item_code_key UNIQUE (tenant_id, item_id, code),
    CONSTRAINT lot_tenant_key UNIQUE (id, tenant_id)
);

-- D6/D24, minimal. Containment is a fold over package_event, which arrives in
-- migration 3; what the spine needs is something for holder_package_id to point
-- at. No parent_package_id and no location_id here on purpose: D24 demoted both
-- to projections of the event stream and adding them now would create exactly
-- the second writable representation it removed.
CREATE TABLE package (
    id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  uuid NOT NULL REFERENCES tenant(id),
    barcode    text,
    sscc       char(18),
    CONSTRAINT package_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON COLUMN package.sscc IS
    'CHAR(18), never bigint: leading zeros are significant. No UNIQUE, because '
    'GS1 permits reuse after twelve months, suppliers ignore it, and D5 forbids '
    'rejecting a true observation. A duplicate resolves to the existing package '
    'and raises a containment_conflict. D29.';

-- ---------------------------------------------------------------------------
-- The ledger (D5, D10, D11, D20, D24)
-- ---------------------------------------------------------------------------

CREATE TABLE stock_movement (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       uuid NOT NULL REFERENCES tenant(id),
    client_event_id uuid NOT NULL,

    item_id         uuid NOT NULL REFERENCES item(id),
    quantity        bigint NOT NULL,
    catch_weight_g  bigint,

    -- S1. Every stock key column except tenant_id and item_id appears here as a
    -- from_/to_ pair. Every movement subtracts from one place and adds to
    -- another, and a receipt or a despatch simply leaves one side null.
    from_location_id uuid REFERENCES location(id),
    from_package_id  uuid REFERENCES package(id),
    from_lot_id      uuid REFERENCES lot(id),
    from_status_id   uuid REFERENCES inventory_status(id),
    from_owner_id    uuid REFERENCES party(id),

    to_location_id   uuid REFERENCES location(id),
    to_package_id    uuid REFERENCES package(id),
    to_lot_id        uuid REFERENCES lot(id),
    to_status_id     uuid REFERENCES inventory_status(id),
    to_owner_id      uuid REFERENCES party(id),

    lot_id uuid GENERATED ALWAYS AS (COALESCE(to_lot_id, from_lot_id)) STORED,

    reason          text NOT NULL,
    occurred_at     timestamptz NOT NULL,   -- device clock, D5
    recorded_at     timestamptz NOT NULL DEFAULT now(),
    recorded_by_id  uuid REFERENCES person(id),
    automation_key  text,
    authorised_by_id uuid REFERENCES person(id),

    CONSTRAINT stock_movement_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),

    CONSTRAINT stock_movement_quantity_ck CHECK (quantity > 0),
    CONSTRAINT stock_movement_actor_ck
        CHECK (num_nonnulls(recorded_by_id, automation_key) = 1),

    -- A holder is a location or a package, never both.
    CONSTRAINT stock_movement_from_holder_ck
        CHECK (num_nonnulls(from_location_id, from_package_id) <= 1),
    CONSTRAINT stock_movement_to_holder_ck
        CHECK (num_nonnulls(to_location_id, to_package_id) <= 1),

    -- A populated side carries the WHOLE key, not just a holder. This matters
    -- more than it looks: under NULLS NOT DISTINCT a null owner is a different
    -- cell from the site's entity, so an omitted column would not error, it
    -- would silently fork the balance.
    CONSTRAINT stock_movement_from_whole_key_ck
        CHECK (num_nonnulls(from_location_id, from_package_id) = 0
               OR (from_status_id IS NOT NULL AND from_owner_id IS NOT NULL)),
    CONSTRAINT stock_movement_to_whole_key_ck
        CHECK (num_nonnulls(to_location_id, to_package_id) = 0
               OR (to_status_id IS NOT NULL AND to_owner_id IS NOT NULL)),

    -- A movement that changes nothing is not a movement.
    CONSTRAINT stock_movement_distinct_sides_ck CHECK (
        ROW(from_location_id, from_package_id, from_lot_id, from_status_id, from_owner_id)
        IS DISTINCT FROM
        ROW(to_location_id, to_package_id, to_lot_id, to_status_id, to_owner_id)),

    -- At least one side must exist, or the row references nothing at all.
    CONSTRAINT stock_movement_some_side_ck CHECK (
        num_nonnulls(from_location_id, from_package_id,
                     to_location_id, to_package_id) > 0)
);

COMMENT ON TABLE stock_movement IS
    'FACT. Append-only: no UPDATE and no DELETE is granted to the application '
    'role, which is S6 and is a grant rather than a convention. D5.';

CREATE INDEX stock_movement_client_event_idx
    ON stock_movement (tenant_id, client_event_id);
CREATE INDEX stock_movement_item_idx ON stock_movement (tenant_id, item_id, occurred_at);

-- ---------------------------------------------------------------------------
-- The fold (D4, D12, D20, D24)
-- ---------------------------------------------------------------------------

CREATE TABLE stock (
    id                   uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id            uuid NOT NULL REFERENCES tenant(id),
    item_id              uuid NOT NULL REFERENCES item(id),

    -- Six dimensions in seven columns. location and package answer the same
    -- question at two resolutions and a package's location is a property of the
    -- package, so carrying both would be two independently writable
    -- representations of one fact. As an exclusive arm the drift is
    -- unrepresentable rather than merely detected. D24.
    holder_location_id   uuid REFERENCES location(id),
    holder_package_id    uuid REFERENCES package(id),
    lot_id               uuid REFERENCES lot(id),
    status_id            uuid NOT NULL REFERENCES inventory_status(id),
    owner_id             uuid NOT NULL REFERENCES party(id),

    quantity             bigint NOT NULL DEFAULT 0,
    weight_g             bigint,
    allocated_quantity   bigint NOT NULL DEFAULT 0,
    available_quantity   bigint GENERATED ALWAYS AS (quantity - allocated_quantity) STORED,

    resolved_location_id uuid REFERENCES location(id),
    site_id              uuid,

    CONSTRAINT stock_holder_ck
        CHECK (num_nonnulls(holder_location_id, holder_package_id) = 1),
    CONSTRAINT stock_cell_key UNIQUE NULLS NOT DISTINCT
        (tenant_id, item_id, holder_location_id, holder_package_id,
         lot_id, status_id, owner_id)
);

COMMENT ON TABLE stock IS
    'PROJECTION of stock_movement. Read quickly, rebuilt from scratch at any '
    'time, and a scheduled job rebuilds it and reports disagreement. Nothing '
    'writes to it directly: the application role holds no INSERT, UPDATE or '
    'DELETE, so "nothing writes to stock" is a rule the database holds. D25.';

COMMENT ON COLUMN stock.resolved_location_id IS
    '@projection: the holder location, or the holder package''s. J5.';
COMMENT ON COLUMN stock.site_id IS
    '@projection from resolved_location_id.';
COMMENT ON COLUMN stock.quantity IS
    '@projection: the signed two-sided fold of stock_movement over the cell key. J1.';
COMMENT ON COLUMN stock.allocated_quantity IS
    '@projection: active cell-bound allocations only. J3.';

-- S25. Availability carries owner_id and status_id in the key so the hot query
-- never joins inventory_status, and it is partial because a zero cell is not
-- available for anything.
CREATE INDEX stock_availability_idx
    ON stock (tenant_id, item_id, site_id, owner_id, status_id)
    INCLUDE (available_quantity)
    WHERE quantity <> 0;

-- D24 corrected the tier-0 request: on holder_location_id this index would miss
-- everything sitting inside a package.
CREATE INDEX stock_resolved_location_idx ON stock (resolved_location_id, item_id);

CREATE VIEW package_content AS
    SELECT id, holder_package_id AS package_id, item_id, lot_id,
           quantity, weight_g AS catch_weight_g
      FROM stock
     WHERE holder_package_id IS NOT NULL;

COMMENT ON VIEW package_content IS
    'D24 retired package_content as a base table. Everything D6 claimed for it '
    'survives, and it gains item_id, status_id and owner_id which it never had.';

-- ---------------------------------------------------------------------------
-- RLS (D18, D19, S8, S9)
-- ---------------------------------------------------------------------------

ALTER TABLE client_event ENABLE ROW LEVEL SECURITY;
ALTER TABLE client_event FORCE ROW LEVEL SECURITY;
CREATE POLICY client_event_tenant_scoped ON client_event
    USING (tenant_id = current_tenant());

ALTER TABLE inventory_status ENABLE ROW LEVEL SECURITY;
ALTER TABLE inventory_status FORCE ROW LEVEL SECURITY;
CREATE POLICY inventory_status_shared_reference ON inventory_status
    USING (tenant_id IS NULL OR tenant_id = current_tenant());

ALTER TABLE party ENABLE ROW LEVEL SECURITY;
ALTER TABLE party FORCE ROW LEVEL SECURITY;
CREATE POLICY party_tenant_scoped ON party USING (tenant_id = current_tenant());

ALTER TABLE lot ENABLE ROW LEVEL SECURITY;
ALTER TABLE lot FORCE ROW LEVEL SECURITY;
CREATE POLICY lot_tenant_scoped ON lot USING (tenant_id = current_tenant());

ALTER TABLE package ENABLE ROW LEVEL SECURITY;
ALTER TABLE package FORCE ROW LEVEL SECURITY;
CREATE POLICY package_tenant_scoped ON package USING (tenant_id = current_tenant());

ALTER TABLE stock_movement ENABLE ROW LEVEL SECURITY;
ALTER TABLE stock_movement FORCE ROW LEVEL SECURITY;
CREATE POLICY stock_movement_tenant_scoped ON stock_movement
    USING (tenant_id = current_tenant());

ALTER TABLE stock ENABLE ROW LEVEL SECURITY;
ALTER TABLE stock FORCE ROW LEVEL SECURITY;
CREATE POLICY stock_tenant_scoped ON stock USING (tenant_id = current_tenant());

-- ---------------------------------------------------------------------------
-- Grants (D25)
-- ---------------------------------------------------------------------------

GRANT SELECT, INSERT, UPDATE, DELETE ON party, lot, package TO spork_app;
GRANT SELECT ON inventory_status TO spork_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON inventory_status TO spork_platform;

-- S6. Fact tables take INSERT and SELECT and nothing else. A movement is what
-- happened; there is no verb for changing what happened.
GRANT SELECT, INSERT ON client_event TO spork_app;
GRANT SELECT, INSERT ON stock_movement TO spork_app;

-- S28 and D25. stock is a projection: the application reads it and the
-- maintainer function writes it. A table-wide GRANT UPDATE here would silently
-- disarm every projection guard in the schema.
GRANT SELECT ON stock TO spork_app;
GRANT SELECT ON package_content TO spork_app;
