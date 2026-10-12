-- Migration 136: a line is checked at the bench. D244.
--
-- The bench recorded what went into a carton, and nothing on it asked whether
-- what reached the bench was what the order asked for. Now the packer checks
-- each line: what should be there is shown, and they confirm it or say what
-- they see. A check is a fact; a line's newest check wins.
--
-- A carton's typed weight is the second check. Whether it agrees with what it
-- should weigh is a tolerance, and a tolerance is the site's to say, not the
-- screen's: three per cent to start.

CREATE TABLE pack_check (
    id                 uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id          uuid NOT NULL REFERENCES tenant(id),

    fulfilment_line_id uuid NOT NULL,

    -- What the bench showed should be there, and what the packer saw, in the
    -- units the line counts. The same figure twice is a confirmation.
    expected_quantity  bigint NOT NULL,
    seen_quantity      bigint NOT NULL,

    occurred_at        timestamptz NOT NULL,
    client_event_id    uuid NOT NULL,
    recorded_by_id     uuid NOT NULL REFERENCES person(id),
    recorded_at        timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT pack_check_counts_ck
        CHECK (expected_quantity >= 0 AND seen_quantity >= 0),
    CONSTRAINT pack_check_line_fk
        FOREIGN KEY (fulfilment_line_id, tenant_id) REFERENCES fulfilment_line(id, tenant_id),
    CONSTRAINT pack_check_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT pack_check_event_key UNIQUE (tenant_id, client_event_id)
);

COMMENT ON TABLE pack_check IS
    'A line checked at the pack bench: what should have been there and what the '
    'packer saw. The newest check of a line wins. D244, migration 136.';

CREATE INDEX pack_check_line_idx
    ON pack_check (fulfilment_line_id, occurred_at DESC, recorded_at DESC, id DESC);

ALTER TABLE pack_check ENABLE ROW LEVEL SECURITY;
ALTER TABLE pack_check FORCE ROW LEVEL SECURITY;
CREATE POLICY pack_check_tenant_scoped ON pack_check
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON pack_check TO spork_app;

ALTER TABLE site
    ADD COLUMN weight_tolerance_per_mille integer NOT NULL DEFAULT 30,
    ADD CONSTRAINT site_weight_tolerance_ck CHECK (weight_tolerance_per_mille BETWEEN 1 AND 500);
COMMENT ON COLUMN site.weight_tolerance_per_mille IS
    'How far, in parts per thousand, a carton''s typed weight may be from what it '
    'should weigh and still agree. 30 to start. D244, migration 136.';
GRANT UPDATE (weight_tolerance_per_mille) ON site TO spork_app;
