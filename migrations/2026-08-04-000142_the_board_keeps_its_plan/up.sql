-- Migration 142: the board keeps its plan. D245, docs/packing-plan.md
-- (settled 4).
--
-- The packing plan was worked out afresh each time it was drawn and never
-- kept, so a planned box had no link to the carton made from it. Now the
-- board's plan is kept on the server, per fulfilment: a draft, replaced as it
-- changes, not a fact. It is what was planned: a parcel of a box type, which
-- lines' units go in it, and the pallet it goes on. A planned parcel holds the
-- id its carton will be made with, so the carton made from it is the same
-- parcel, and making it uses the plan's row up.

CREATE TABLE pack_plan_parcel (
    -- The id its carton will have: `POST /packages` takes it.
    id              uuid PRIMARY KEY,
    tenant_id       uuid NOT NULL REFERENCES tenant(id),
    fulfilment_id   uuid NOT NULL,
    package_type_id uuid NOT NULL REFERENCES package_type(id),

    -- The pallet it goes on: a planned one, or one already made.
    on_parcel_id    uuid,
    on_package_id   uuid,
    -- Where on it, as `package_event` says a spot (D246): from the front left
    -- corner of the deck, and turned a quarter round or not.
    at_x_mm         integer,
    at_y_mm         integer,
    at_z_mm         integer,
    turned          boolean NOT NULL DEFAULT false,

    -- Placed by the packer, not by Suggest: Suggest plans round these.
    placed_by_hand  boolean NOT NULL,
    -- The order the board shows them in.
    position        integer NOT NULL,
    updated_at      timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT pack_plan_parcel_tenant_key UNIQUE (id, tenant_id),
    CONSTRAINT pack_plan_parcel_fulfilment_fk
        FOREIGN KEY (fulfilment_id, tenant_id) REFERENCES fulfilment(id, tenant_id),
    CONSTRAINT pack_plan_parcel_on_parcel_fk
        FOREIGN KEY (on_parcel_id, tenant_id) REFERENCES pack_plan_parcel(id, tenant_id)
        ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT pack_plan_parcel_on_package_fk
        FOREIGN KEY (on_package_id, tenant_id) REFERENCES package(id, tenant_id),
    CONSTRAINT pack_plan_parcel_on_one_ck
        CHECK (num_nonnulls(on_parcel_id, on_package_id) <= 1),
    CONSTRAINT pack_plan_parcel_not_on_itself_ck
        CHECK (on_parcel_id IS DISTINCT FROM id),
    CONSTRAINT pack_plan_parcel_spot_ck
        CHECK (num_nonnulls(at_x_mm, at_y_mm, at_z_mm) IN (0, 3)
               AND (at_x_mm IS NULL OR num_nonnulls(on_parcel_id, on_package_id) = 1))
);

COMMENT ON TABLE pack_plan_parcel IS
    'D245. A parcel the board plans for a fulfilment, not yet made: its box type, '
    'the pallet it goes on, and whether the packer placed it or Suggest did. A '
    'draft, replaced as the plan changes; making the carton uses the row up.';

CREATE INDEX pack_plan_parcel_fulfilment_idx ON pack_plan_parcel (fulfilment_id, position);

CREATE TABLE pack_plan_line (
    tenant_id          uuid NOT NULL REFERENCES tenant(id),
    parcel_id          uuid NOT NULL,
    fulfilment_line_id uuid NOT NULL,
    -- In the units the line counts.
    quantity           bigint NOT NULL,

    PRIMARY KEY (parcel_id, fulfilment_line_id),
    CONSTRAINT pack_plan_line_parcel_fk
        FOREIGN KEY (parcel_id, tenant_id) REFERENCES pack_plan_parcel(id, tenant_id)
        ON DELETE CASCADE,
    CONSTRAINT pack_plan_line_line_fk
        FOREIGN KEY (fulfilment_line_id, tenant_id) REFERENCES fulfilment_line(id, tenant_id),
    CONSTRAINT pack_plan_line_quantity_ck CHECK (quantity > 0)
);

COMMENT ON TABLE pack_plan_line IS
    'D245. So many of a line''s units planned into a planned parcel.';

ALTER TABLE pack_plan_parcel ENABLE ROW LEVEL SECURITY;
ALTER TABLE pack_plan_parcel FORCE ROW LEVEL SECURITY;
CREATE POLICY pack_plan_parcel_tenant_scoped ON pack_plan_parcel
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

ALTER TABLE pack_plan_line ENABLE ROW LEVEL SECURITY;
ALTER TABLE pack_plan_line FORCE ROW LEVEL SECURITY;
CREATE POLICY pack_plan_line_tenant_scoped ON pack_plan_line
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- A draft, not a fact: replaced whole as it changes (S6 is for facts).
GRANT SELECT, INSERT, UPDATE, DELETE ON pack_plan_parcel TO spork_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON pack_plan_line TO spork_app;
