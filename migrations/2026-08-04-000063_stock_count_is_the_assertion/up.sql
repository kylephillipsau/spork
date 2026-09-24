-- Migration 63: stock_count is the assertion; variance is a finding.
--
-- D8: a count never writes the ledger. Comparing it to the fold produces a
-- count_variance finding; resolution (POST /adjustments) may post a movement
-- later, with resolving_movement_id on the finding.
--
-- D9's challenge fields live on the count row (act and result are one number).
-- Migration 6 left stock_count_id as a declared arm of discrepancy with no
-- target; this migration creates the target and adds the arm.

-- ---------------------------------------------------------------------------
-- The count (FACT)
-- ---------------------------------------------------------------------------

CREATE TABLE stock_count (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id       uuid NOT NULL REFERENCES tenant(id),

    -- Optional link to a live cell row when the count is of an existing cell.
    -- Null when counting empty space that has no stock row yet.
    stock_id        uuid,

    item_id         uuid NOT NULL REFERENCES item(id),
    holder_location_id uuid,
    holder_package_id  uuid,
    lot_id          uuid,
    status_id       uuid NOT NULL REFERENCES inventory_status(id),
    owner_id        uuid NOT NULL,

    CONSTRAINT stock_count_holder_ck
        CHECK (num_nonnulls(holder_location_id, holder_package_id) = 1),
    CONSTRAINT stock_count_stock_fk
        FOREIGN KEY (stock_id, tenant_id) REFERENCES stock(id, tenant_id),
    CONSTRAINT stock_count_location_fk
        FOREIGN KEY (holder_location_id, tenant_id) REFERENCES location(id, tenant_id),
    CONSTRAINT stock_count_package_fk
        FOREIGN KEY (holder_package_id, tenant_id) REFERENCES package(id, tenant_id),
    CONSTRAINT stock_count_lot_fk
        FOREIGN KEY (lot_id, tenant_id) REFERENCES lot(id, tenant_id),
    CONSTRAINT stock_count_owner_fk
        FOREIGN KEY (owner_id, tenant_id) REFERENCES party(id, tenant_id),

    counted_quantity bigint NOT NULL,
    -- What the fold said at the moment of capture (may be 0 if no stock row).
    system_quantity  bigint NOT NULL,
    CONSTRAINT stock_count_quantities_ck
        CHECK (counted_quantity >= 0 AND system_quantity >= 0),

    counted_at       timestamptz NOT NULL,
    recorded_at      timestamptz NOT NULL DEFAULT now(),
    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid REFERENCES person(id),
    automation_key   text,
    CONSTRAINT stock_count_actor_ck
        CHECK (num_nonnulls(recorded_by_id, automation_key) = 1),
    CONSTRAINT stock_count_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),

    blind            boolean NOT NULL DEFAULT false,

    -- D9: challenge lives with the value
    challenged       boolean NOT NULL DEFAULT false,
    challenge_context text,
    confirmed        boolean,
    CONSTRAINT stock_count_challenge_ck
        CHECK (NOT challenged OR challenge_context IS NOT NULL),
    CONSTRAINT stock_count_confirmed_ck
        CHECK (confirmed IS NULL OR challenged),

    CONSTRAINT stock_count_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON TABLE stock_count IS
    'FACT. A cycle count / stocktake assertion: this much was counted here. '
    'Never writes the ledger (D8). Variance raises discrepancy.kind = count_variance. '
    'Challenge fields are on this row (D9). Append-only for the application.';

CREATE INDEX stock_count_cell_idx
    ON stock_count (tenant_id, item_id, holder_location_id, holder_package_id, counted_at DESC);
CREATE INDEX stock_count_stock_idx
    ON stock_count (stock_id) WHERE stock_id IS NOT NULL;

ALTER TABLE stock_count ENABLE ROW LEVEL SECURITY;
ALTER TABLE stock_count FORCE ROW LEVEL SECURITY;
CREATE POLICY stock_count_tenant_scoped ON stock_count
    USING (tenant_id = current_tenant());

-- Fact: INSERT only for the app (S6).
GRANT SELECT, INSERT ON stock_count TO spork_app;
GRANT SELECT ON stock_count TO spork_scheduler;
GRANT SELECT ON stock_count TO spork_platform;
GRANT SELECT ON stock_count TO spork_projection_owner;

-- ---------------------------------------------------------------------------
-- Discrepancy source arm
-- ---------------------------------------------------------------------------

ALTER TABLE discrepancy
    ADD COLUMN stock_count_id uuid;

ALTER TABLE discrepancy
    ADD CONSTRAINT discrepancy_stock_count_fk
        FOREIGN KEY (stock_count_id, tenant_id)
        REFERENCES stock_count(id, tenant_id);

ALTER TABLE discrepancy
    DROP CONSTRAINT discrepancy_source_ck;

ALTER TABLE discrepancy
    ADD CONSTRAINT discrepancy_source_ck
        CHECK (num_nonnulls(
            stock_movement_id,
            package_event_id,
            stock_count_id
        ) <= 1);

COMMENT ON COLUMN discrepancy.stock_count_id IS
    'Source arm: the stock_count whose variance raised this finding. D8, migration 63.';
