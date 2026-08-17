-- Migration 34: statements of record neither side may revise.
--
-- D21 was adopted on 2026-08-01 and never built. Nine invariants have been
-- waiting on it since -- S17, S18, S35, J17, J18, J19, J47, J48, J49 -- which is
-- the largest blocked set in the register, and no question defers it. This is
-- that decision, built, with D44's two amendments folded in where they touch the
-- same tables.
--
-- D21's definition, unedited: "an assertion is a statement of record exchanged
-- with another party, stored exactly as exchanged, which neither side may
-- unilaterally revise."
--
-- **The cut is control, not authorship.** Our own outbound despatch advice is as
-- unrevisable as a supplier's inbound one, because they hold a copy and will
-- quote it back. That is why `direction` exists and why every table here is
-- symmetric.

-- ---------------------------------------------------------------------------
-- 1. Vocabulary
-- ---------------------------------------------------------------------------
--
-- D33's test: a fixed set that code branches on is an enum. Every set here is
-- closed and named in D21 or D44, except `granularity`, which neither states --
-- it stays text rather than having its values invented in a migration.

CREATE TYPE message_direction AS ENUM ('inbound', 'outbound');

CREATE TYPE message_channel AS ENUM
    ('edi', 'portal', 'csv', 'email', 'api', 'webhook', 'print');

CREATE TYPE message_parse_status AS ENUM
    ('pending', 'parsed', 'partial', 'failed', 'unsupported');

-- D21's six, plus D44's two: "`order_response` is the outbound ORDRSP and nothing
-- held the ORDERS it answers, which is a gap the enumeration pointed at."
CREATE TYPE assertion_kind AS ENUM
    ('despatch_advice', 'carrier_status', 'equipment_docket', 'delivery_receipt',
     'order_response', 'price_advice', 'purchase_order', 'document_response');

-- Not `status`, and not by accident: S18 forbids the word across this set,
-- because D25 makes our position a fact of ours rather than a column on their
-- claim.
CREATE TYPE assertion_stance_kind AS ENUM
    ('pending', 'in_force', 'rejected', 'superseded', 'withdrawn_by_author', 'expired');

CREATE TYPE assertion_check_outcome AS ENUM
    ('agreed', 'disagreed', 'unverifiable', 'unchecked_at_close');

-- ---------------------------------------------------------------------------
-- 2. party_message -- the artefact, verbatim
-- ---------------------------------------------------------------------------
--
-- Principle 3, restated by D21: the model contains no `jsonb` column. The payload
-- is `bytea` because what arrived is bytes, and re-encoding it is already an
-- interpretation.
--
-- **This is not itself an assertion**, which is why it may carry `parse_status`
-- without S18 objecting: parsing is something we do to a message, not a position
-- on a claim.

CREATE TABLE party_message (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),
    party_id         uuid,

    direction        message_direction NOT NULL,
    channel          message_channel NOT NULL,
    transport_ref    text,
    content_type     text,

    payload          bytea NOT NULL,
    byte_count       integer NOT NULL,
    content_hash     bytea NOT NULL,

    occurred_at      timestamptz NOT NULL,
    recorded_at      timestamptz NOT NULL DEFAULT now(),
    client_event_id  uuid NOT NULL,

    parse_status     message_parse_status NOT NULL DEFAULT 'pending',
    parser_version   text,

    -- D44. Their parsing of our outbound message arrives as its own inbound
    -- message, so the link is message to message and needs no new table.
    acknowledges_party_message_id uuid REFERENCES party_message(id),
    acknowledgement_outcome       text,

    CONSTRAINT party_message_party_fk FOREIGN KEY (party_id, tenant_id)
        REFERENCES party(id, tenant_id),
    CONSTRAINT party_message_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT party_message_byte_count_ck CHECK (byte_count >= 0),
    CONSTRAINT party_message_ack_ck
        CHECK (acknowledgement_outcome IS NULL
               OR acknowledges_party_message_id IS NOT NULL),
    CONSTRAINT party_message_tenant_key UNIQUE (id, tenant_id),
    CONSTRAINT party_message_dedupe_key
        UNIQUE (tenant_id, party_id, content_hash, transport_ref)
);

COMMENT ON TABLE party_message IS
    'The artefact as it was exchanged, verbatim. Replaces D1''s provider_exchange. '
    'Not an assertion: it is the envelope a claim arrived in, and a message may '
    'carry several claims or none. D21, D44.';
COMMENT ON COLUMN party_message.payload IS
    'bytea, never jsonb. Principle 3: re-encoding what arrived is already an '
    'interpretation, and the point of this row is that it is not one. D21.';

-- ---------------------------------------------------------------------------
-- 3. assertion -- the envelope
-- ---------------------------------------------------------------------------
--
-- No `status` column. Our position is `assertion_stance`, because a claim's state
-- in our system is a fact about us, not a property of their statement -- and
-- writing it onto their row would be revising their row.
--
-- No unique on `(author_reference, author_version)`. D21 is explicit and D5 is
-- why: **a duplicate resend must be storable and raise a finding, not be refused
-- at the write.** Refusing it at the write loses the evidence that it happened.

CREATE TABLE assertion (
    id                       uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id                uuid NOT NULL REFERENCES tenant(id),
    kind                     assertion_kind NOT NULL,
    direction                message_direction NOT NULL,

    -- Rule 2. An access-control boundary, not metadata.
    author_party_id          uuid NOT NULL,
    transmitted_by_party_id  uuid,
    owner_party_id           uuid,
    site_id                  uuid,

    author_reference         text,
    author_version           text,
    message_function         text,

    asserted_at              timestamptz,
    received_at              timestamptz NOT NULL DEFAULT now(),

    party_message_id         uuid,
    captured_by_id           uuid REFERENCES person(id),
    client_event_id          uuid NOT NULL,

    -- THEIR claim that this replaces that.
    supersedes_assertion_id  uuid REFERENCES assertion(id),
    -- OUR transcription fix, which is only legitimate where no artefact exists.
    correction_of_assertion_id uuid REFERENCES assertion(id),

    CONSTRAINT assertion_author_fk FOREIGN KEY (author_party_id, tenant_id)
        REFERENCES party(id, tenant_id),
    CONSTRAINT assertion_transmitter_fk
        FOREIGN KEY (transmitted_by_party_id, tenant_id)
        REFERENCES party(id, tenant_id),
    CONSTRAINT assertion_owner_fk FOREIGN KEY (owner_party_id, tenant_id)
        REFERENCES party(id, tenant_id),
    CONSTRAINT assertion_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id),
    CONSTRAINT assertion_message_fk FOREIGN KEY (party_message_id, tenant_id)
        REFERENCES party_message(id, tenant_id),
    CONSTRAINT assertion_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),

    -- It came from somewhere: an artefact, or a person keying paper.
    CONSTRAINT assertion_provenance_ck
        CHECK (party_message_id IS NOT NULL OR captured_by_id IS NOT NULL),
    -- Correcting a transcription is only meaningful where there is no artefact
    -- to read again. With one, the artefact is the record and a correction is a
    -- new assertion.
    CONSTRAINT assertion_correction_ck
        CHECK (correction_of_assertion_id IS NULL OR party_message_id IS NULL),

    CONSTRAINT assertion_tenant_key UNIQUE (id, tenant_id),
    -- Composite FK target for the typed bodies.
    CONSTRAINT assertion_kind_key UNIQUE (id, kind),
    -- Target for supersession, which may not cross author or kind.
    CONSTRAINT assertion_supersession_key UNIQUE (id, tenant_id, author_party_id, kind)
);

COMMENT ON TABLE assertion IS
    'A statement of record exchanged with another party, which neither side may '
    'unilaterally revise. Immutable by grant rather than by trigger: S7 forbids '
    'the trigger, and the application holds INSERT and SELECT only. D21.';
COMMENT ON COLUMN assertion.author_party_id IS
    'Rule 2: an assertion always names its author. An access-control boundary '
    'rather than metadata -- who may see a claim follows from whose claim it is. '
    'D21.';

CREATE INDEX assertion_reference_idx
    ON assertion (tenant_id, author_party_id, kind, author_reference);
CREATE INDEX assertion_supersedes_idx
    ON assertion (supersedes_assertion_id) WHERE supersedes_assertion_id IS NOT NULL;
CREATE INDEX assertion_message_idx
    ON assertion (party_message_id) WHERE party_message_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 4. assertion_stance -- our position, which is ours
-- ---------------------------------------------------------------------------

CREATE TABLE assertion_stance (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),
    assertion_id     uuid NOT NULL,

    stance           assertion_stance_kind NOT NULL,
    reason_code      text,
    note             text,
    successor_assertion_id uuid,

    occurred_at      timestamptz NOT NULL,
    recorded_at      timestamptz NOT NULL DEFAULT now(),
    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid REFERENCES person(id),
    automation_key   text,
    authorised_by_id uuid REFERENCES person(id),

    CONSTRAINT assertion_stance_assertion_fk
        FOREIGN KEY (assertion_id, tenant_id) REFERENCES assertion(id, tenant_id),
    CONSTRAINT assertion_stance_successor_fk
        FOREIGN KEY (successor_assertion_id, tenant_id)
        REFERENCES assertion(id, tenant_id),
    CONSTRAINT assertion_stance_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT assertion_stance_superseded_ck
        CHECK (stance <> 'superseded' OR successor_assertion_id IS NOT NULL),
    -- D11: machine actors are permitted on assertion-ingestion facts, and only
    -- here. `stock_movement` keeps its NOT NULL person.
    CONSTRAINT assertion_stance_actor_ck
        CHECK (num_nonnulls(recorded_by_id, automation_key) = 1),
    CONSTRAINT assertion_stance_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON TABLE assertion_stance IS
    'Our position on a claim: a fact, because it is ours. The column is `stance` '
    'rather than `status` and S18 keeps it that way -- writing our position onto '
    'their row would be revising their row. D21, D25.';

CREATE INDEX assertion_stance_current_idx
    ON assertion_stance (assertion_id, occurred_at DESC, recorded_at DESC, id DESC);

-- ---------------------------------------------------------------------------
-- 5. assertion_check -- rule 4, made into a table
-- ---------------------------------------------------------------------------
--
-- "Exists to be compared. A claim never checked is itself a finding."

-- `discrepancy` was created with PRIMARY KEY (id) alone, so it had no target for
-- the composite tenant FK every other table here uses. Added rather than dropping
-- to a tenant-blind reference: a check in one tenant pointing at a discrepancy in
-- another is exactly the hole D55 found open since migration 1.
ALTER TABLE discrepancy
    ADD CONSTRAINT discrepancy_tenant_key UNIQUE (id, tenant_id);

CREATE TABLE assertion_check (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),
    assertion_id     uuid NOT NULL,

    asserted_unit_id uuid,
    asserted_unit_content_id uuid,
    metric_id        uuid REFERENCES metric(id),

    outcome          assertion_check_outcome NOT NULL,

    asserted_numeric numeric,
    observed_numeric numeric,
    asserted_text    text,
    observed_text    text,
    variance_numeric numeric GENERATED ALWAYS AS
        (observed_numeric - asserted_numeric) STORED,

    discrepancy_id   uuid,

    checked_at       timestamptz NOT NULL,
    recorded_at      timestamptz NOT NULL DEFAULT now(),
    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid REFERENCES person(id),
    automation_key   text,

    CONSTRAINT assertion_check_assertion_fk
        FOREIGN KEY (assertion_id, tenant_id) REFERENCES assertion(id, tenant_id),
    CONSTRAINT assertion_check_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT assertion_check_discrepancy_fk
        FOREIGN KEY (discrepancy_id, tenant_id)
        REFERENCES discrepancy(id, tenant_id),
    -- D8: a disagreement is a discrepancy, or it is not a disagreement.
    CONSTRAINT assertion_check_disagreed_ck
        CHECK (outcome <> 'disagreed' OR discrepancy_id IS NOT NULL),
    CONSTRAINT assertion_check_actor_ck
        CHECK (num_nonnulls(recorded_by_id, automation_key) = 1),
    CONSTRAINT assertion_check_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON TABLE assertion_check IS
    'A claim was compared with reality. `metric_id` is D23''s vocabulary rather '
    'than a second one, so a supplier-declared weight and our scale reading are '
    'one query. D21 rule 4.';

CREATE INDEX assertion_check_assertion_idx ON assertion_check (assertion_id);

-- ---------------------------------------------------------------------------
-- 6. Typed bodies, one per kind
-- ---------------------------------------------------------------------------
--
-- The body's `kind` is a stored generated constant and the FK is composite, so
-- "this body belongs to an assertion of the matching kind" is **declarative**
-- rather than a trigger. Verified before this migration was written: attaching a
-- despatch advice body to an assertion of another kind fails on the foreign key.

CREATE TABLE despatch_advice (
    assertion_id     uuid PRIMARY KEY,
    kind             assertion_kind GENERATED ALWAYS AS
                         ('despatch_advice'::assertion_kind) STORED,
    tenant_id        uuid NOT NULL REFERENCES tenant(id),

    -- The SUBJECT this claim is about, which is ours and not theirs.
    inbound_shipment_id uuid,

    ship_from_gln    text,
    ship_to_gln      text,
    gsin             text,
    ginc             text,

    carrier_party_id uuid,
    conveyance_ref   text,
    container_ref    text,
    seal_number      text,

    despatched_at    timestamptz,
    estimated_arrival_at timestamptz,

    split_shipment   boolean,
    completes_order  boolean,
    granularity      text,

    resolved_purchase_order_id uuid,
    resolved_at      timestamptz,
    resolved_by_id   uuid REFERENCES person(id),
    resolution_method text,

    CONSTRAINT despatch_advice_assertion_fk FOREIGN KEY (assertion_id, kind)
        REFERENCES assertion(id, kind),
    CONSTRAINT despatch_advice_tenant_fk FOREIGN KEY (assertion_id, tenant_id)
        REFERENCES assertion(id, tenant_id),
    CONSTRAINT despatch_advice_carrier_fk FOREIGN KEY (carrier_party_id, tenant_id)
        REFERENCES party(id, tenant_id),
    CONSTRAINT despatch_advice_po_fk
        FOREIGN KEY (resolved_purchase_order_id, tenant_id)
        REFERENCES purchase_order(id, tenant_id)
);

COMMENT ON COLUMN despatch_advice.granularity IS
    'Text rather than an enum because neither D21 nor D43 states its value set, '
    'and inventing one in a migration is how an undesigned vocabulary ships. '
    'Question 143''s neighbourhood. D21.';

-- D44. "A counterparty's statement of record about our claim: authored by them,
-- held by them, quoted back in a chargeback. D21's definition unedited."
CREATE TABLE document_response (
    assertion_id     uuid PRIMARY KEY,
    kind             assertion_kind GENERATED ALWAYS AS
                         ('document_response'::assertion_kind) STORED,
    tenant_id        uuid NOT NULL REFERENCES tenant(id),

    -- The claim being disposed of. Always travelling the other way, which J49
    -- is what checks.
    subject_assertion_id uuid NOT NULL,
    response_code    text,
    response_reason  text,

    CONSTRAINT document_response_assertion_fk FOREIGN KEY (assertion_id, kind)
        REFERENCES assertion(id, kind),
    CONSTRAINT document_response_tenant_fk FOREIGN KEY (assertion_id, tenant_id)
        REFERENCES assertion(id, tenant_id),
    CONSTRAINT document_response_subject_fk
        FOREIGN KEY (subject_assertion_id, tenant_id)
        REFERENCES assertion(id, tenant_id),
    CONSTRAINT document_response_not_self_ck
        CHECK (subject_assertion_id <> assertion_id)
);

-- ---------------------------------------------------------------------------
-- 7. The declared hierarchy
-- ---------------------------------------------------------------------------
--
-- No weights and no Ti/Hi. Those are observations whose observable is the
-- asserted unit and whose author is the counterparty (D23), so a supplier's
-- declared carton weight and our scale reading are compared in one vocabulary
-- rather than two parallel ones.
--
-- Nesting is unbounded here because this is the cold path; it collapses to D24's
-- cap at receipt.

CREATE TABLE asserted_unit (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),
    assertion_id     uuid NOT NULL,
    parent_asserted_unit_id uuid,

    level_code       text NOT NULL,
    sscc             text,
    sequence         integer,

    raw_package_type_code text,
    resolved_package_type_id uuid REFERENCES package_type(id),

    CONSTRAINT asserted_unit_assertion_fk
        FOREIGN KEY (assertion_id, tenant_id) REFERENCES assertion(id, tenant_id),
    CONSTRAINT asserted_unit_parent_fk
        FOREIGN KEY (parent_asserted_unit_id, tenant_id)
        REFERENCES asserted_unit(id, tenant_id),
    CONSTRAINT asserted_unit_not_own_parent_ck
        CHECK (parent_asserted_unit_id IS DISTINCT FROM id),
    CONSTRAINT asserted_unit_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON COLUMN asserted_unit.level_code IS
    'The author''s own level vocabulary, kept in the author''s words (rule 5). '
    'D43 stores the order level as a node too, which is why S35 exists: a '
    'non-physical level must carry no SSCC and contribute no package at receipt. '
    'D21, D43.';

CREATE INDEX asserted_unit_assertion_idx ON asserted_unit (assertion_id);
CREATE INDEX asserted_unit_parent_idx ON asserted_unit (parent_asserted_unit_id)
    WHERE parent_asserted_unit_id IS NOT NULL;

CREATE TABLE asserted_unit_content (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),
    asserted_unit_id uuid NOT NULL,

    -- Rule 5: recorded in the author's vocabulary. The raw_* columns are
    -- immutable; the resolved_* columns are our annotation.
    raw_gtin         text,
    raw_item_code    text,
    resolved_item_id uuid,

    raw_po_reference text,
    raw_po_line_number text,
    resolved_purchase_order_line_id uuid,

    -- Structural: the receipt compares these line by line, which is why they are
    -- here rather than being observations.
    quantity         numeric,
    entered_quantity numeric,
    entered_unit_id  uuid REFERENCES unit(id),

    lot_code         text,
    expiry_date      date,
    best_before_date date,

    resolved_at      timestamptz,
    resolved_by_id   uuid REFERENCES person(id),
    resolution_method text,

    CONSTRAINT asserted_unit_content_unit_fk
        FOREIGN KEY (asserted_unit_id, tenant_id)
        REFERENCES asserted_unit(id, tenant_id),
    -- `item` is referenced by id alone throughout this schema -- twelve tables do
    -- it -- because tenant agreement on an item reference is J20's job rather
    -- than a composite key's. Following the convention rather than inventing a
    -- thirteenth shape for one column.
    CONSTRAINT asserted_unit_content_item_fk
        FOREIGN KEY (resolved_item_id) REFERENCES item(id),
    CONSTRAINT asserted_unit_content_po_line_fk
        FOREIGN KEY (resolved_purchase_order_line_id, tenant_id)
        REFERENCES purchase_order_line(id, tenant_id),
    CONSTRAINT asserted_unit_content_tenant_key UNIQUE (id, tenant_id)
);

CREATE INDEX asserted_unit_content_unit_idx
    ON asserted_unit_content (asserted_unit_id);

-- The two references assertion_check makes into the hierarchy, added now that
-- both tables exist.
ALTER TABLE assertion_check
    ADD CONSTRAINT assertion_check_unit_fk
        FOREIGN KEY (asserted_unit_id, tenant_id)
        REFERENCES asserted_unit(id, tenant_id),
    ADD CONSTRAINT assertion_check_content_fk
        FOREIGN KEY (asserted_unit_content_id, tenant_id)
        REFERENCES asserted_unit_content(id, tenant_id);

-- ---------------------------------------------------------------------------
-- 8. inbound_shipment -- a subject, not an assertion
-- ---------------------------------------------------------------------------
--
-- D21: "Filing it as an assertion means a resend mints a second row and orphans
-- every FK pointing at the first -- the `consignment.fulfilment_id` defect D15
-- already deleted once."
--
-- **Nothing here is NOT NULL that requires an assertion**, so blind receipt --
-- rung zero of the degradation ladder -- is a schema property rather than a
-- workflow branch.

CREATE TABLE inbound_shipment (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),
    site_id          uuid,
    supplier_party_id uuid,
    owner_party_id   uuid,

    vendor_shipment_ref text,

    in_force_assertion_id uuid,
    granularity      text,
    estimated_arrival_at timestamptz,

    asserted_unit_count integer,
    asserted_base_quantity numeric,
    first_asserted_at timestamptz,
    superseded_count integer NOT NULL DEFAULT 0,

    CONSTRAINT inbound_shipment_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id),
    CONSTRAINT inbound_shipment_supplier_fk
        FOREIGN KEY (supplier_party_id, tenant_id) REFERENCES party(id, tenant_id),
    CONSTRAINT inbound_shipment_owner_fk FOREIGN KEY (owner_party_id, tenant_id)
        REFERENCES party(id, tenant_id),
    CONSTRAINT inbound_shipment_in_force_fk
        FOREIGN KEY (in_force_assertion_id, tenant_id)
        REFERENCES assertion(id, tenant_id),
    CONSTRAINT inbound_shipment_tenant_key UNIQUE (id, tenant_id),
    CONSTRAINT inbound_shipment_vendor_ref_key
        UNIQUE (tenant_id, supplier_party_id, vendor_shipment_ref)
);

COMMENT ON COLUMN inbound_shipment.in_force_assertion_id IS
    '@projection -- folded by projection_inbound_shipment_rebuild from the latest '
    'assertion_stance per claim. The subject survives a resend; the claim about '
    'it does not. D21.';
COMMENT ON COLUMN inbound_shipment.asserted_unit_count IS
    '@projection -- the in-force claim''s node count, for D24''s gate check. D21.';
COMMENT ON COLUMN inbound_shipment.asserted_base_quantity IS
    '@projection -- the in-force claim''s declared total. D21.';
COMMENT ON COLUMN inbound_shipment.first_asserted_at IS
    '@projection -- when this shipment was first claimed, across every version. D21.';
COMMENT ON COLUMN inbound_shipment.superseded_count IS
    '@projection -- how many times the claim has been replaced. A resend that '
    'mints a second subject is the defect D15 deleted once; this counts them '
    'instead. D21.';

ALTER TABLE despatch_advice
    ADD CONSTRAINT despatch_advice_shipment_fk
        FOREIGN KEY (inbound_shipment_id, tenant_id)
        REFERENCES inbound_shipment(id, tenant_id);

-- ---------------------------------------------------------------------------
-- 9. The maintainer for those five projections
-- ---------------------------------------------------------------------------
--
-- D63 and D64: a projection column with no maintainer is a claim nobody keeps.
-- D68's guard is here too -- a run that changes nothing writes nothing.

CREATE FUNCTION projection_inbound_shipment_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    n bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH latest_stance AS (
        SELECT DISTINCT ON (s.assertion_id)
               s.assertion_id, s.stance
          FROM assertion_stance s
         WHERE s.tenant_id = p_tenant
         ORDER BY s.assertion_id, s.occurred_at DESC, s.recorded_at DESC, s.id DESC
    ),
    claims AS (
        SELECT d.inbound_shipment_id AS shipment_id, a.id AS assertion_id,
               a.asserted_at, ls.stance
          FROM despatch_advice d
          JOIN assertion a ON a.id = d.assertion_id
          LEFT JOIN latest_stance ls ON ls.assertion_id = a.id
         WHERE a.tenant_id = p_tenant AND d.inbound_shipment_id IS NOT NULL
    ),
    in_force AS (
        SELECT DISTINCT ON (shipment_id) shipment_id, assertion_id
          FROM claims WHERE stance = 'in_force'
         ORDER BY shipment_id, asserted_at DESC NULLS LAST, assertion_id DESC
    ),
    rolled AS (
        SELECT c.shipment_id,
               min(c.asserted_at) AS first_asserted_at,
               count(*) FILTER (WHERE c.stance = 'superseded')::integer AS superseded_count
          FROM claims c GROUP BY c.shipment_id
    ),
    measured AS (
        SELECT f.shipment_id, f.assertion_id,
               (SELECT count(*)::integer FROM asserted_unit u
                 WHERE u.assertion_id = f.assertion_id) AS unit_count,
               (SELECT sum(cc.quantity) FROM asserted_unit_content cc
                  JOIN asserted_unit u ON u.id = cc.asserted_unit_id
                 WHERE u.assertion_id = f.assertion_id) AS base_quantity
          FROM in_force f
    ),
    updated AS (
        UPDATE inbound_shipment s
           SET in_force_assertion_id  = m.assertion_id,
               asserted_unit_count    = m.unit_count,
               asserted_base_quantity = m.base_quantity,
               first_asserted_at      = r.first_asserted_at,
               superseded_count       = coalesce(r.superseded_count, 0)
          FROM rolled r
          LEFT JOIN measured m ON m.shipment_id = r.shipment_id
         WHERE s.id = r.shipment_id AND s.tenant_id = p_tenant
           AND (s.in_force_assertion_id  IS DISTINCT FROM m.assertion_id
             OR s.asserted_unit_count    IS DISTINCT FROM m.unit_count
             OR s.asserted_base_quantity IS DISTINCT FROM m.base_quantity
             OR s.first_asserted_at      IS DISTINCT FROM r.first_asserted_at
             OR s.superseded_count       IS DISTINCT FROM coalesce(r.superseded_count, 0))
        RETURNING s.id)
    SELECT count(*) INTO n FROM updated;

    RETURN n;
END
$$;

ALTER FUNCTION projection_inbound_shipment_rebuild(uuid)
    OWNER TO nylonite_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_inbound_shipment_rebuild(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_inbound_shipment_rebuild(uuid)
    TO nylonite_scheduler, nylonite_platform;
GRANT SELECT, UPDATE ON inbound_shipment TO nylonite_projection_owner;
GRANT SELECT ON assertion, assertion_stance, despatch_advice, asserted_unit,
    asserted_unit_content TO nylonite_projection_owner;

-- 95, between fulfilment and expected_supply. D24's supply side has assertions
-- refining a promise, so the in-force claim must be settled before
-- projection_expected_supply_rebuild at 100 reads anything derived from it.
INSERT INTO projection_step (function_name, ordinal, note) VALUES
    ('projection_inbound_shipment_rebuild', 95,
     'D21. The in-force claim about a shipment, and the numbers D24 gates on.');

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('inbound_shipment', 'in_force_assertion_id',  'projection_inbound_shipment_rebuild'),
    ('inbound_shipment', 'asserted_unit_count',    'projection_inbound_shipment_rebuild'),
    ('inbound_shipment', 'asserted_base_quantity', 'projection_inbound_shipment_rebuild'),
    ('inbound_shipment', 'first_asserted_at',      'projection_inbound_shipment_rebuild'),
    ('inbound_shipment', 'superseded_count',       'projection_inbound_shipment_rebuild');

-- ---------------------------------------------------------------------------
-- 10. D8 gains the link D21 promised it
-- ---------------------------------------------------------------------------

ALTER TABLE discrepancy ADD COLUMN assertion_check_id uuid;
ALTER TABLE discrepancy
    ADD CONSTRAINT discrepancy_assertion_check_fk
        FOREIGN KEY (assertion_check_id, tenant_id)
        REFERENCES assertion_check(id, tenant_id);

COMMENT ON COLUMN discrepancy.assertion_check_id IS
    'The comparison that raised this, when a claim disagreed with reality. D21''s '
    'amendment to D8.';

-- ---------------------------------------------------------------------------
-- 11. RLS
-- ---------------------------------------------------------------------------

ALTER TABLE party_message ENABLE ROW LEVEL SECURITY;
ALTER TABLE party_message FORCE ROW LEVEL SECURITY;
CREATE POLICY party_message_tenant_scoped ON party_message
    USING (tenant_id = current_tenant());

ALTER TABLE assertion ENABLE ROW LEVEL SECURITY;
ALTER TABLE assertion FORCE ROW LEVEL SECURITY;
CREATE POLICY assertion_tenant_scoped ON assertion
    USING (tenant_id = current_tenant());

ALTER TABLE assertion_stance ENABLE ROW LEVEL SECURITY;
ALTER TABLE assertion_stance FORCE ROW LEVEL SECURITY;
CREATE POLICY assertion_stance_tenant_scoped ON assertion_stance
    USING (tenant_id = current_tenant());

ALTER TABLE assertion_check ENABLE ROW LEVEL SECURITY;
ALTER TABLE assertion_check FORCE ROW LEVEL SECURITY;
CREATE POLICY assertion_check_tenant_scoped ON assertion_check
    USING (tenant_id = current_tenant());

ALTER TABLE despatch_advice ENABLE ROW LEVEL SECURITY;
ALTER TABLE despatch_advice FORCE ROW LEVEL SECURITY;
CREATE POLICY despatch_advice_tenant_scoped ON despatch_advice
    USING (tenant_id = current_tenant());

ALTER TABLE document_response ENABLE ROW LEVEL SECURITY;
ALTER TABLE document_response FORCE ROW LEVEL SECURITY;
CREATE POLICY document_response_tenant_scoped ON document_response
    USING (tenant_id = current_tenant());

ALTER TABLE asserted_unit ENABLE ROW LEVEL SECURITY;
ALTER TABLE asserted_unit FORCE ROW LEVEL SECURITY;
CREATE POLICY asserted_unit_tenant_scoped ON asserted_unit
    USING (tenant_id = current_tenant());

ALTER TABLE asserted_unit_content ENABLE ROW LEVEL SECURITY;
ALTER TABLE asserted_unit_content FORCE ROW LEVEL SECURITY;
CREATE POLICY asserted_unit_content_tenant_scoped ON asserted_unit_content
    USING (tenant_id = current_tenant());

ALTER TABLE inbound_shipment ENABLE ROW LEVEL SECURITY;
ALTER TABLE inbound_shipment FORCE ROW LEVEL SECURITY;
CREATE POLICY inbound_shipment_tenant_scoped ON inbound_shipment
    USING (tenant_id = current_tenant());

-- ---------------------------------------------------------------------------
-- 12. Grants -- where rule 1 actually lives
-- ---------------------------------------------------------------------------
--
-- **"No UPDATE, no DELETE, ever. A revision is a new assertion."** S7 forbids the
-- trigger that would enforce that, so it is enforced the way D72 enforced a
-- projection: the application is never given the privilege.
--
-- The exception is exactly the one D21 states. `resolved_*` columns are our
-- annotation, not the counterparty's words: "a GTIN unresolvable today becomes
-- resolvable when the item is created tomorrow, and refusing that would discard a
-- claim because our catalogue was behind."

GRANT SELECT, INSERT ON party_message TO nylonite_app;
GRANT UPDATE (parse_status, parser_version, acknowledges_party_message_id,
              acknowledgement_outcome) ON party_message TO nylonite_app;

GRANT SELECT, INSERT ON assertion TO nylonite_app;
GRANT SELECT, INSERT ON assertion_stance TO nylonite_app;
GRANT SELECT, INSERT ON assertion_check TO nylonite_app;
GRANT SELECT, INSERT ON document_response TO nylonite_app;
GRANT SELECT, INSERT ON asserted_unit TO nylonite_app;

GRANT SELECT, INSERT ON despatch_advice TO nylonite_app;
GRANT UPDATE (inbound_shipment_id, resolved_purchase_order_id, resolved_at,
              resolved_by_id, resolution_method) ON despatch_advice TO nylonite_app;

GRANT SELECT, INSERT ON asserted_unit_content TO nylonite_app;
GRANT UPDATE (resolved_item_id, resolved_purchase_order_line_id, resolved_at,
              resolved_by_id, resolution_method)
    ON asserted_unit_content TO nylonite_app;

-- A subject, not a claim: the application creates and edits it, except the five
-- columns the maintainer owns.
GRANT SELECT, INSERT ON inbound_shipment TO nylonite_app;
GRANT UPDATE (site_id, supplier_party_id, owner_party_id, vendor_shipment_ref,
              granularity, estimated_arrival_at) ON inbound_shipment TO nylonite_app;

GRANT UPDATE (assertion_check_id) ON discrepancy TO nylonite_app;

-- ---------------------------------------------------------------------------
-- 13. What this does not build
-- ---------------------------------------------------------------------------
--
-- **The bodies for six of the eight kinds.** `carrier_status`,
-- `equipment_docket`, `delivery_receipt`, `order_response`, `price_advice` and
-- `purchase_order` have no table. The two built are the two with invariants
-- waiting on them and consumers in the record -- D43's hierarchy and D44's
-- disposition. The composite-FK pattern is the same for each, and inventing five
-- more bodies with no reader is how a schema grows columns nobody fills.
--
-- **The ingestion adapters.** Nothing parses an EDI message into these tables;
-- J41 has been pending on that since before this migration and still is.
--
-- **The freeze on re-resolution.** D21: "once an `assertion_check` or a
-- `goods_receipt_line` references it, it may not be rewritten." That is a
-- cross-row rule about a value's *history*, and nothing here records when a
-- `resolved_*` column changed -- so it cannot be checked, only stated. Question
-- 152 carries it.
