-- Migration 9: the outbound side.
--
-- D15 settled the shape: three groupings, not one. An order is what the customer
-- asked for, a fulfilment is a commitment to ship part of it, and a consignment
-- is what a carrier collects. They are separate because they genuinely vary
-- independently, and the join table between the last two is the capability
-- NetSuite does not have.

-- ---------------------------------------------------------------------------
-- Packaging presets
-- ---------------------------------------------------------------------------

CREATE TABLE package_type (
    id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           uuid REFERENCES tenant(id),   -- NULL = shared preset
    name                text NOT NULL,
    carrier_package_code text,
    -- A box's dimensions are fixed. A pallet's height is not, which is why this
    -- is a flag rather than an assumption.
    dimensions_fixed    boolean NOT NULL DEFAULT true,
    tare_weight_g       bigint,
    reusable            boolean NOT NULL DEFAULT false,
    max_payload_g       bigint,
    max_cube_mm3        bigint,
    effective_from      date NOT NULL DEFAULT CURRENT_DATE,
    CONSTRAINT package_type_name_key UNIQUE NULLS NOT DISTINCT (tenant_id, name)
);

ALTER TABLE package_type ENABLE ROW LEVEL SECURITY;
ALTER TABLE package_type FORCE ROW LEVEL SECURITY;
CREATE POLICY package_type_shared_reference ON package_type
    USING (tenant_id IS NULL OR tenant_id = current_tenant());
GRANT SELECT, INSERT, UPDATE, DELETE ON package_type TO nylonite_app;

-- ---------------------------------------------------------------------------
-- What the customer asked for (D15, D39, D42, D44)
-- ---------------------------------------------------------------------------

CREATE TYPE order_state AS ENUM ('placed', 'on_hold', 'cancelled');

CREATE TABLE "order" (
    id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           uuid NOT NULL REFERENCES tenant(id),
    site_id             uuid,
    customer_party_id   uuid REFERENCES party(id),
    confirmation_number text,

    -- D39. Exactly one system of record per order, declared at creation and
    -- recorded on the row. An order whose record of authority is external is
    -- amended through that system, not here. Bidirectional merge was refused
    -- outright: two systems that both accept edits to one order need a conflict
    -- resolution nobody can explain to a person on a dock.
    source_channel      text NOT NULL,
    external_ref        text,

    -- D44. Metcash will not implement a purchase order change message and
    -- instead cancels and re-raises, so an externally-authoritative order is
    -- amended by succession. Without the link the pair is two unrelated rows and
    -- the second one's history starts from nothing.
    supersedes_order_id uuid REFERENCES "order"(id),

    -- Tier-0 item 11. Lateness is undetectable on the customer side without
    -- these, and they are not backfillable for orders already placed.
    promised_from       timestamptz,
    promised_to         timestamptz,
    required_by         timestamptz,

    placed_at           timestamptz NOT NULL DEFAULT now(),
    -- DECLARED, per D25. Whether an order is on hold is a decision somebody
    -- made, not a fold of anything.
    state               order_state NOT NULL DEFAULT 'placed',

    CONSTRAINT order_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id),
    CONSTRAINT order_promised_window_ck
        CHECK (promised_to IS NULL OR promised_from IS NULL OR promised_to >= promised_from),
    CONSTRAINT order_not_own_successor_ck CHECK (supersedes_order_id <> id)
);

COMMENT ON TABLE "order" IS
    'INTENTION. Ours: created here, complete here, and an operation running '
    'nothing else works. An order arriving from NetSuite is our own intention '
    'reaching us through a channel, not an assertion. D39.';

CREATE TABLE order_line (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id        uuid NOT NULL,
    order_id         uuid NOT NULL REFERENCES "order"(id),
    item_id          uuid NOT NULL REFERENCES item(id),
    quantity_ordered bigint NOT NULL,
    line_number      integer,

    -- Price is deliberately absent. Question 127 carries it, because it touches
    -- D40's boundary and the deferred stock_movement.unit_cost_minor, and
    -- settling a commercial question by implication inside a schema migration is
    -- how boundaries move without anyone deciding to move them. The grocery
    -- channel needs it before the first EDI order.

    CONSTRAINT order_line_quantity_ck CHECK (quantity_ordered > 0)
);

CREATE INDEX order_line_order_idx ON order_line (order_id);

-- D42. An amendment to an intention is a fact. row_audit was refused twice, and
-- the requirement behind it is this: the order is the significant mutable entity
-- left in a model that mostly has none, which is why the audit question kept
-- coming back about the ship-to address specifically.
--
-- No field column and no before column. The previous value is the previous
-- amendment, and you fold.
CREATE TABLE intention_amendment (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       uuid NOT NULL REFERENCES tenant(id),
    client_event_id uuid NOT NULL,
    order_id        uuid NOT NULL REFERENCES "order"(id),

    occurred_at     timestamptz NOT NULL,
    recorded_at     timestamptz NOT NULL DEFAULT now(),
    recorded_by_id  uuid REFERENCES person(id),
    automation_key  text,
    reason          text,

    -- The covered columns, each nullable: an amendment sets what it changed and
    -- leaves the rest alone. Folding in (occurred_at, recorded_at, id) order
    -- gives the current value, which is J46 and is the same shape as J6.
    new_promised_from timestamptz,
    new_promised_to   timestamptz,
    new_required_by   timestamptz,
    new_state         order_state,

    CONSTRAINT intention_amendment_actor_ck
        CHECK (num_nonnulls(recorded_by_id, automation_key) = 1),
    CONSTRAINT intention_amendment_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT intention_amendment_changes_something_ck
        CHECK (num_nonnulls(new_promised_from, new_promised_to,
                            new_required_by, new_state) > 0)
);

COMMENT ON TABLE intention_amendment IS
    'FACT. Cheaper than what it replaced: row_audit would have doubled the write '
    'volume of the largest tables to record nothing, and that argument reverses '
    'on a table with hundreds of rows a day. D42.';

-- ---------------------------------------------------------------------------
-- The commitment to ship (D15, D25)
-- ---------------------------------------------------------------------------

CREATE TYPE fulfilment_state AS ENUM ('planned', 'released', 'cancelled');

CREATE TABLE fulfilment (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id    uuid NOT NULL,
    order_id     uuid NOT NULL REFERENCES "order"(id),
    site_id      uuid,

    -- DECLARED. Whether a fulfilment is released is a decision.
    state        fulfilment_state NOT NULL DEFAULT 'planned',
    -- @projection. How far along it is falls out of the movements.
    progress     text,

    -- D11 separates the operator, the workers and the accountable, so picked and
    -- packed are two people and two times rather than one actor column.
    picked_by_id uuid REFERENCES person(id),
    packed_by_id uuid REFERENCES person(id),
    picked_at    timestamptz,
    packed_at    timestamptz,

    CONSTRAINT fulfilment_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id)
);

COMMENT ON COLUMN fulfilment.progress IS
    '@projection: folded from the movements against this fulfilment''s lines. '
    'order.fulfilment_status was dropped because it would be a third hop. D25.';

CREATE TABLE fulfilment_line (
    id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id          uuid NOT NULL,
    fulfilment_id      uuid NOT NULL REFERENCES fulfilment(id),
    order_line_id      uuid NOT NULL REFERENCES order_line(id),
    quantity           bigint NOT NULL,

    -- @projection, J31: active allocations against this line, either arm.
    allocated_quantity bigint NOT NULL DEFAULT 0,
    uncovered_quantity bigint GENERATED ALWAYS AS (quantity - allocated_quantity) STORED,

    CONSTRAINT fulfilment_line_quantity_ck CHECK (quantity > 0)
);

COMMENT ON COLUMN fulfilment_line.allocated_quantity IS
    '@projection of stock_allocation. J31.';

CREATE INDEX fulfilment_line_uncovered_idx
    ON fulfilment_line (fulfilment_id) WHERE uncovered_quantity > 0;

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('fulfilment', 'progress', 'projection_fulfilment_rebuild'),
    ('fulfilment_line', 'allocated_quantity', 'projection_fulfilment_rebuild');

-- D12's demand arm, which migration 3 left off because fulfilment_line did not
-- exist. An allocation is an intention: advisory, and the ledger does not wait
-- on it.
ALTER TABLE stock_allocation
    ADD COLUMN fulfilment_line_id uuid REFERENCES fulfilment_line(id);
CREATE INDEX stock_allocation_fulfilment_line_idx
    ON stock_allocation (fulfilment_line_id) WHERE fulfilment_line_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- Freight (D1, D15)
-- ---------------------------------------------------------------------------

CREATE TABLE carrier (
    id        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid REFERENCES tenant(id),
    name      text NOT NULL,
    code      text NOT NULL,
    CONSTRAINT carrier_code_key UNIQUE NULLS NOT DISTINCT (tenant_id, code)
);

-- The carrier and the way we reach them are separate, which is the whole point
-- of D1: a carrier booked through an intermediary today and booked directly
-- tomorrow is the same carrier, and the cost history survives the change.
CREATE TABLE freight_provider (
    id        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid REFERENCES tenant(id),
    name      text NOT NULL,
    kind      text NOT NULL,
    CONSTRAINT freight_provider_kind_ck CHECK (kind IN ('machship', 'direct_api', 'manual'))
);

CREATE TABLE carrier_service (
    id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    carrier_id uuid NOT NULL REFERENCES carrier(id),
    name       text NOT NULL,
    code       text NOT NULL,
    CONSTRAINT carrier_service_code_key UNIQUE (carrier_id, code)
);

CREATE TABLE consignment (
    id                       uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id                uuid NOT NULL REFERENCES tenant(id),

    -- D15 dropped consignment.fulfilment_id. A consignment reaches its
    -- fulfilments through consignment_package to package to fulfilment, which is
    -- what physically happens: one collection can carry parcels from several
    -- commitments. A direct FK was a second, weaker representation that silently
    -- forbade the consolidation it appeared to model, and it is the mistake D44
    -- later recognised again in a different place.
    carrier_id               uuid REFERENCES carrier(id),
    freight_provider_id      uuid REFERENCES freight_provider(id),
    carrier_service_id       uuid REFERENCES carrier_service(id),
    provider_consignment_id  text,
    carrier_consignment_number text,

    despatch_at              timestamptz,
    -- @projection of the in-force carrier advice, per D21 and D1's amendment.
    eta                      timestamptz,
    status                   text,
    price_minor              bigint,
    currency                 text
);

COMMENT ON COLUMN consignment.status IS
    '@projection: the in-force carrier_status_advice assertion. D1 as amended.';
COMMENT ON COLUMN consignment.eta IS '@projection of the in-force carrier advice.';
COMMENT ON COLUMN consignment.price_minor IS '@projection of the in-force carrier advice.';

CREATE TABLE consignment_package (
    consignment_id uuid NOT NULL REFERENCES consignment(id),
    package_id     uuid NOT NULL REFERENCES package(id),
    PRIMARY KEY (consignment_id, package_id)
);

COMMENT ON TABLE consignment_package IS
    'The join NetSuite has no equivalent of. It is what lets a consignment span '
    'fulfilments and a fulfilment span consignments. D15.';

-- ---------------------------------------------------------------------------
-- package joins the outbound side
-- ---------------------------------------------------------------------------

ALTER TABLE package
    -- Nullable, and it means only what it was introduced for: a tote or a
    -- putaway licence plate is not shipping anywhere. Every package on a
    -- consignment has a fulfilment; not every package is on a consignment.
    ADD COLUMN fulfilment_id   uuid REFERENCES fulfilment(id),
    ADD COLUMN package_type_id uuid REFERENCES package_type(id),
    ADD COLUMN sequence        integer,
    -- Frozen at seal, never live projections. A shipped package's dimensions are
    -- a historical fact about that consignment: a retroactive correction to the
    -- preset must not rewrite the number a freight invoice was computed against.
    ADD COLUMN length_mm       integer,
    ADD COLUMN width_mm        integer,
    ADD COLUMN height_mm       integer,
    ADD COLUMN gross_weight_g  bigint,
    ADD COLUMN dimensions_source text,
    ADD COLUMN sealed_at       timestamptz;

-- The despatch columns are the application's: measured or confirmed at pack
-- time and frozen at seal. Granted explicitly because migration 4 narrowed
-- package to column-level UPDATE.
-- INSERT is column-level too, and for the same reason as UPDATE: a row inserted
-- with progress or sscc already set has written a projection just as surely as
-- an update would. barcode and sscc are absent because D29 mints a package from
-- an event and J6 folds the identifier out of that event.
GRANT INSERT (id, tenant_id, fulfilment_id, package_type_id, sequence, length_mm,
              width_mm, height_mm, gross_weight_g, dimensions_source, sealed_at),
      UPDATE (fulfilment_id, package_type_id, sequence, length_mm, width_mm,
              height_mm, gross_weight_g, dimensions_source, sealed_at)
    ON package TO nylonite_app;

COMMENT ON COLUMN package.dimensions_source IS
    'computed | confirmed | corrected. Whether the carton was measured, agreed '
    'or fixed afterwards, which is the input to the freight variance question.';

-- ---------------------------------------------------------------------------
-- RLS and grants
-- ---------------------------------------------------------------------------

ALTER TABLE "order" ENABLE ROW LEVEL SECURITY;
ALTER TABLE "order" FORCE ROW LEVEL SECURITY;
CREATE POLICY order_tenant_scoped ON "order" USING (tenant_id = current_tenant());

ALTER TABLE order_line ENABLE ROW LEVEL SECURITY;
ALTER TABLE order_line FORCE ROW LEVEL SECURITY;
CREATE POLICY order_line_tenant_scoped ON order_line USING (tenant_id = current_tenant());

ALTER TABLE intention_amendment ENABLE ROW LEVEL SECURITY;
ALTER TABLE intention_amendment FORCE ROW LEVEL SECURITY;
CREATE POLICY intention_amendment_tenant_scoped ON intention_amendment
    USING (tenant_id = current_tenant());

ALTER TABLE fulfilment ENABLE ROW LEVEL SECURITY;
ALTER TABLE fulfilment FORCE ROW LEVEL SECURITY;
CREATE POLICY fulfilment_tenant_scoped ON fulfilment USING (tenant_id = current_tenant());

ALTER TABLE fulfilment_line ENABLE ROW LEVEL SECURITY;
ALTER TABLE fulfilment_line FORCE ROW LEVEL SECURITY;
CREATE POLICY fulfilment_line_tenant_scoped ON fulfilment_line
    USING (tenant_id = current_tenant());

ALTER TABLE carrier ENABLE ROW LEVEL SECURITY;
ALTER TABLE carrier FORCE ROW LEVEL SECURITY;
CREATE POLICY carrier_shared_reference ON carrier
    USING (tenant_id IS NULL OR tenant_id = current_tenant());

ALTER TABLE freight_provider ENABLE ROW LEVEL SECURITY;
ALTER TABLE freight_provider FORCE ROW LEVEL SECURITY;
CREATE POLICY freight_provider_shared_reference ON freight_provider
    USING (tenant_id IS NULL OR tenant_id = current_tenant());

ALTER TABLE consignment ENABLE ROW LEVEL SECURITY;
ALTER TABLE consignment FORCE ROW LEVEL SECURITY;
CREATE POLICY consignment_tenant_scoped ON consignment USING (tenant_id = current_tenant());

GRANT SELECT, INSERT, UPDATE ON "order", order_line TO nylonite_app;

-- Column-level wherever a table carries a projection, per D25 and J36. A
-- table-wide UPDATE here would let the application write the very columns the
-- maintainer exists to own, and the guard would still read as though it were on.
GRANT SELECT ON fulfilment TO nylonite_app;
GRANT INSERT (id, tenant_id, order_id, site_id, state, picked_by_id, packed_by_id,
              picked_at, packed_at),
      UPDATE (state, site_id, picked_by_id, packed_by_id, picked_at, packed_at)
    ON fulfilment TO nylonite_app;

GRANT SELECT ON fulfilment_line TO nylonite_app;
GRANT INSERT (id, tenant_id, fulfilment_id, order_line_id, quantity),
      UPDATE (quantity)
    ON fulfilment_line TO nylonite_app;

GRANT SELECT ON consignment TO nylonite_app;
GRANT INSERT (id, tenant_id, carrier_id, freight_provider_id, carrier_service_id,
              provider_consignment_id, carrier_consignment_number, despatch_at, currency),
      UPDATE (carrier_id, freight_provider_id, carrier_service_id,
              provider_consignment_id, carrier_consignment_number,
              despatch_at, currency) ON consignment TO nylonite_app;

GRANT SELECT, INSERT, UPDATE, DELETE ON carrier, freight_provider, carrier_service,
    consignment_package TO nylonite_app;

-- intention_amendment is a fact: SELECT and INSERT, and no verb for changing
-- what happened.
GRANT SELECT, INSERT ON intention_amendment TO nylonite_app;

-- ---------------------------------------------------------------------------
-- The amendment fold (D42, J46)
-- ---------------------------------------------------------------------------

CREATE FUNCTION projection_order_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    -- Last writer per covered column, in register order. Not one winning row:
    -- an amendment that changed only the promised window must not clear a state
    -- set by an earlier one, which is why the fold is per column rather than per
    -- row. Same shape as J6 and the same ordering.
    WITH folded AS (
        SELECT order_id,
               (array_remove(array_agg(new_promised_from ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS promised_from,
               (array_remove(array_agg(new_promised_to ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS promised_to,
               (array_remove(array_agg(new_required_by ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS required_by,
               (array_remove(array_agg(new_state ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS state
          FROM intention_amendment
         WHERE tenant_id = p_tenant
         GROUP BY order_id
    ),
    updated AS (
        UPDATE "order" o
           SET promised_from = COALESCE(f.promised_from, o.promised_from),
               promised_to   = COALESCE(f.promised_to,   o.promised_to),
               required_by   = COALESCE(f.required_by,   o.required_by),
               state         = COALESCE(f.state,         o.state)
          FROM folded f
         WHERE o.id = f.order_id AND o.tenant_id = p_tenant
        RETURNING o.id)
    SELECT count(*) INTO touched FROM updated;
    RETURN touched;
END
$$;

ALTER FUNCTION projection_order_rebuild(uuid) OWNER TO nylonite_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_order_rebuild(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_order_rebuild(uuid)
    TO nylonite_scheduler, nylonite_platform;
GRANT SELECT, UPDATE ON "order" TO nylonite_projection_owner;
GRANT SELECT ON intention_amendment TO nylonite_projection_owner;
