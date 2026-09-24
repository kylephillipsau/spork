-- Migration 6: the finding.
--
-- D8: discrepancy is a designed output, not a failure mode. The architecture
-- puts it more strongly: disagreement is the most useful thing the system
-- produces, because a count of minus one means something physical happened that
-- nobody wrote down. Competing systems treat that as an adjustment to be made
-- and forgotten. Here it becomes a finding with an owner, the evidence behind it
-- and a resolution.
--
-- It is also where every job-asserted invariant lands. The register: "A failure
-- raises a discrepancy, never an error, so the model's self-consistency lands in
-- the same queue as every other finding and never stops the floor." Until this
-- migration the J runner had findings and nowhere to put them.

-- ---------------------------------------------------------------------------
-- The kind vocabulary (D33)
-- ---------------------------------------------------------------------------

-- An enum, not a table. D33 drew the line and gave the test:
--
--   A value set is a table when it carries attributes and grows independently
--   of the code that reads it. It is an enum when code branches on it, because
--   then the set is closed by the code that handles it, and adding a value
--   without adding handling is a bug rather than a configuration.
--
-- Every kind here exists because something routes differently on it.
CREATE TYPE discrepancy_kind AS ENUM (
    -- D8, the original six
    'negative_balance', 'count_variance', 'short_pick',
    'damage', 'unexpected_stock', 'receipt_variance',
    -- D21 and the assertion mechanism
    'expiry_mismatch', 'identity_mismatch', 'assertion_unresolvable',
    'asserted_unit_absent', 'asserted_unit_unexpected',
    -- D24, containment and supply
    'containment_conflict', 'stock_without_location', 'refinement_too_deep',
    'supply_withdrawn', 'supply_overdue', 'commitment_unbacked', 'over_receipt',
    -- D22 and D25
    'policy_ambiguous', 'accepted_state_contradicted',
    -- D33, and the scanning kinds from D28
    'lot_missing', 'scan_mismatch',
    'identifier_unknown', 'identifier_ambiguous', 'identifier_unrecognised',
    -- The model's own consistency, which is a finding like any other
    'projection_drift', 'minted_from_assertion', 'tenant_mismatch',
    'projection_writable', 'projection_unforced',
    'definer_unpinned', 'definer_public'
);

CREATE TYPE discrepancy_state AS ENUM
    ('open', 'investigating', 'resolved', 'accepted');

-- ---------------------------------------------------------------------------
-- The finding (D8, amended by D21, D24, D25)
-- ---------------------------------------------------------------------------

CREATE TABLE discrepancy (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     uuid NOT NULL REFERENCES tenant(id),
    kind          discrepancy_kind NOT NULL,

    -- The cell key, so a finding can be investigated against the exact state at
    -- the moment it was observed. D25 added owner_id and the holder arm, which
    -- the original sketch was missing: without them a finding names a cell that
    -- does not exist, because under NULLS NOT DISTINCT a null owner is a
    -- different cell.
    item_id             uuid REFERENCES item(id),
    holder_location_id  uuid REFERENCES location(id),
    holder_package_id   uuid REFERENCES package(id),
    lot_id              uuid REFERENCES lot(id),
    status_id           uuid REFERENCES inventory_status(id),
    owner_id            uuid REFERENCES party(id),

    CONSTRAINT discrepancy_holder_ck
        CHECK (num_nonnulls(holder_location_id, holder_package_id) <= 1),

    -- Quantities are nullable because not every finding is numeric. An
    -- identity_mismatch has no expected quantity and inventing a zero for it
    -- would put a number in a report that means nothing.
    expected_quantity numeric,
    observed_quantity numeric,
    variance numeric GENERATED ALWAYS AS (observed_quantity - expected_quantity) STORED,
    detail            text,

    -- Typed source arms, never a polymorphic (source_type, source_id) pair,
    -- which D10 rejected. The cap is six and it is stated rather than implied:
    -- a seventh needs a recorded decision, because the arm set growing without
    -- one is how a typed union becomes a polymorphic pair by accretion.
    --
    -- Only the arms whose targets exist are present. The rest arrive in the
    -- migration that creates what they point at, because a nullable FK to a
    -- table that does not exist is not a placeholder.
    stock_movement_id uuid REFERENCES stock_movement(id),
    package_event_id  uuid REFERENCES package_event(id),
    -- stock_count_id, observation_id, assertion_check_id, work_task_id and
    -- expected_supply_id are the remaining declared arms. Six is the cap and
    -- package_event_id is inside it.

    CONSTRAINT discrepancy_source_ck
        CHECK (num_nonnulls(stock_movement_id, package_event_id) <= 1),

    -- D8 as amended by the inbound pass. Without a counterparty a supplier
    -- finding has nobody to raise it with, and the whole scorecard is
    -- unbuildable.
    counterparty_party_id uuid REFERENCES party(id),
    respond_by            timestamptz,

    detected_at   timestamptz NOT NULL DEFAULT now(),
    detected_by_id uuid REFERENCES person(id),
    automation_key text,

    -- DECLARED, not derived. It depends on D7's ticket and question 14 is
    -- undecided, so deriving it now would bake in an answer nobody has given.
    state         discrepancy_state NOT NULL DEFAULT 'open',
    resolved_at   timestamptz,
    resolved_by_id uuid REFERENCES person(id),
    resolution_reason text,
    resolving_movement_id uuid REFERENCES stock_movement(id),
    ticket_id     text,

    CONSTRAINT discrepancy_actor_ck
        CHECK (num_nonnulls(detected_by_id, automation_key) = 1),
    CONSTRAINT discrepancy_resolved_ck
        CHECK (state <> 'resolved' OR resolved_at IS NOT NULL)
);

COMMENT ON TABLE discrepancy IS
    'FINDING. The output, not the failure. Never blocks: a discrepancy is raised '
    'asynchronously and the floor keeps moving, which is what lets negative '
    'balances be tolerated and picks against them succeed. D8.';

COMMENT ON COLUMN discrepancy.state IS
    'DECLARED rather than derived, per D25. It depends on D7''s ticket and '
    'question 14 is undecided.';

CREATE INDEX discrepancy_open_idx
    ON discrepancy (tenant_id, kind, detected_at) WHERE state = 'open';
CREATE INDEX discrepancy_counterparty_idx
    ON discrepancy (tenant_id, counterparty_party_id, detected_at)
    WHERE counterparty_party_id IS NOT NULL;

ALTER TABLE discrepancy ENABLE ROW LEVEL SECURITY;
ALTER TABLE discrepancy FORCE ROW LEVEL SECURITY;
CREATE POLICY discrepancy_tenant_scoped ON discrepancy
    USING (tenant_id = current_tenant());

GRANT SELECT, INSERT, UPDATE ON discrepancy TO spork_app;
GRANT SELECT, INSERT ON discrepancy TO spork_scheduler;

-- record_finding is SECURITY DEFINER and owned by the projection owner, so the
-- privilege that matters is the owner's, not the caller's. Granting EXECUTE
-- without this produces a function anyone may call and nobody may complete.
GRANT SELECT, INSERT ON discrepancy TO spork_projection_owner;

-- No DELETE, to anyone. A finding that can be deleted is a finding that can be
-- made to go away, and the whole argument for raising them is that they are
-- evidence. Resolving one is a state change with a reason attached.

-- ---------------------------------------------------------------------------
-- Where the invariant suite files its findings
-- ---------------------------------------------------------------------------

-- The J class produces findings and, per the register, they belong in the same
-- queue as every other finding rather than in a stack trace. This is the
-- function that puts them there, so a scheduled rebuild-and-assert cycle files
-- its disagreements exactly like a picker's short pick.
CREATE FUNCTION record_finding(
        p_tenant uuid,
        p_kind   discrepancy_kind,
        p_detail text,
        p_automation_key text DEFAULT 'invariant_suite')
    RETURNS uuid
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    new_id uuid;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);
    INSERT INTO discrepancy (tenant_id, kind, detail, automation_key)
    VALUES (p_tenant, p_kind, p_detail, p_automation_key)
    RETURNING id INTO new_id;
    RETURN new_id;
END
$$;

ALTER FUNCTION record_finding(uuid, discrepancy_kind, text, text)
    OWNER TO spork_projection_owner;
REVOKE EXECUTE ON FUNCTION record_finding(uuid, discrepancy_kind, text, text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION record_finding(uuid, discrepancy_kind, text, text)
    TO spork_scheduler, spork_platform;

COMMENT ON FUNCTION record_finding(uuid, discrepancy_kind, text, text) IS
    'Files a finding from the rebuild-and-assert cycle. The model disagreeing '
    'with itself is a discrepancy like any other, which is D8 applied to the '
    'model rather than to the warehouse.';
