-- Migration 83: evidence is a look, and a record points at it.
--
-- D140. A photograph of a crushed carton is worth more than the sentence
-- "damaged on arrival", and there has been no way to attach one to anything.
-- `observation_image` has existed since migration 78 and hangs off an
-- `observation_event`, so the only pictures this system can hold are pictures
-- taken while measuring something.
--
-- # There is no new kind of thing here
--
-- An `observation_event` already records **who looked, when, at which subject,
-- by what means, and on whose authority**. That is what supporting evidence
-- is. So evidence needs no table of its own for the picture, the provenance or
-- the moment — all three exist. What is missing is one link: *this look is
-- offered in support of that record.*
--
-- Which is why this is a link table and not an attachment table. Nothing here
-- stores bytes, a filename, a mime type or an uploader; every one of those
-- questions was answered by D132 and answering them again is how a system ends
-- up with two places a photograph can live and no rule about which.
--
-- # Arms, and why these are the licensed kind
--
-- D23 draws the line: typed nullable FKs with a mutual-exclusion CHECK are
-- right when the arms are *alternative identities of one referent* and wrong
-- when they are *distinct relationships that merely happen to be exclusive
-- today* — causes, demands, sources, where exclusivity is a policy and policies
-- turn out to be wrong.
--
-- One evidence row is about one record. That is not a policy about the world,
-- it is what the row means, and **a look that supports two records is two rows
-- rather than a relaxed constraint** — which is the escape the `discrepancy`
-- source arms never had and the reason they degenerated into a subject union
-- (question 112). A photograph of a crushed carton really can be evidence for
-- both the receipt line and the finding it raised, and it says so twice.
--
-- # Five arms, and no route is required to use one
--
-- Findings, receiving, counts, adjustments and despatch. **Evidence is optional
-- everywhere and required nowhere:** nothing gains a NOT NULL, no existing
-- write path changes, and a record with no evidence is the ordinary case.
-- Adding the sixth is one column here and one line in the writer, which is the
-- whole reason the arms live on this table rather than a nullable
-- `evidence_id` being sprinkled across five record tables.
--
-- # The method a photograph was come by
--
-- `observation_method` describes how a figure was arrived at, and none of its
-- seven values describes a look that produced no figure. Calling a camera an
-- `instrument` would be a small lie with a consequence: `MEASURED_METHODS`
-- treats `instrument` as confirmed, and revalidation would eventually read a
-- photograph as evidence that something had been weighed.
--
-- So the vocabulary grows by one, and **this migration therefore cannot run
-- inside a transaction** — Postgres refuses to use an enum value in the
-- transaction that added it, and `scripts/migrate.sh` detects the statement and
-- drops the wrapper for this file. A failure part-way leaves part of it
-- applied. That is stated here because the migrator says so at the time and the
-- person reading this later will want to know it was expected.

ALTER TYPE observation_method ADD VALUE IF NOT EXISTS 'photographed';

CREATE TABLE evidence (
    id        uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id uuid NOT NULL REFERENCES tenant(id),

    -- The look. S51: the tenant travels in the key, because RLS filters what a
    -- query reads and says nothing about what a row may point at.
    observation_event_id uuid NOT NULL,

    -- The record it supports. Exactly one.
    discrepancy_id        uuid,
    goods_receipt_line_id uuid,
    stock_count_id        uuid,
    stock_movement_id     uuid,
    package_event_id      uuid,

    -- Why this picture is being offered, in the words of whoever offered it.
    -- Nullable: a photograph of a crushed carton against a damage finding needs
    -- no caption, and demanding one buys nothing but empty strings.
    note text,

    recorded_at timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT evidence_event_fk FOREIGN KEY (observation_event_id, tenant_id)
        REFERENCES observation_event (id, tenant_id),
    CONSTRAINT evidence_discrepancy_fk FOREIGN KEY (discrepancy_id, tenant_id)
        REFERENCES discrepancy (id, tenant_id),
    CONSTRAINT evidence_receipt_line_fk FOREIGN KEY (goods_receipt_line_id, tenant_id)
        REFERENCES goods_receipt_line (id, tenant_id),
    CONSTRAINT evidence_stock_count_fk FOREIGN KEY (stock_count_id, tenant_id)
        REFERENCES stock_count (id, tenant_id),
    CONSTRAINT evidence_movement_fk FOREIGN KEY (stock_movement_id, tenant_id)
        REFERENCES stock_movement (id, tenant_id),
    CONSTRAINT evidence_package_event_fk FOREIGN KEY (package_event_id, tenant_id)
        REFERENCES package_event (id, tenant_id),

    CONSTRAINT evidence_one_record_ck
        CHECK (num_nonnulls(discrepancy_id, goods_receipt_line_id, stock_count_id,
                            stock_movement_id, package_event_id) = 1),

    CONSTRAINT evidence_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON TABLE evidence IS
    'A link from a record to the look that supports it. Holds no bytes and no '
    'provenance: observation_image has the picture and observation_event has '
    'who looked, when and by what means, and answering those questions twice is '
    'how a system ends up with two places a photograph can live. One row is '
    'about one record; a look that supports two records is two rows. D140, '
    'migration 83.';

-- One link per look per record. A second attempt to attach the same photograph
-- to the same finding is the same statement, not a second one.
CREATE UNIQUE INDEX evidence_discrepancy_idx
    ON evidence (tenant_id, discrepancy_id, observation_event_id)
    WHERE discrepancy_id IS NOT NULL;
CREATE UNIQUE INDEX evidence_receipt_line_idx
    ON evidence (tenant_id, goods_receipt_line_id, observation_event_id)
    WHERE goods_receipt_line_id IS NOT NULL;
CREATE UNIQUE INDEX evidence_stock_count_idx
    ON evidence (tenant_id, stock_count_id, observation_event_id)
    WHERE stock_count_id IS NOT NULL;
CREATE UNIQUE INDEX evidence_movement_idx
    ON evidence (tenant_id, stock_movement_id, observation_event_id)
    WHERE stock_movement_id IS NOT NULL;
CREATE UNIQUE INDEX evidence_package_event_idx
    ON evidence (tenant_id, package_event_id, observation_event_id)
    WHERE package_event_id IS NOT NULL;

-- Reading a record's evidence is the common query and it goes the other way.
CREATE INDEX evidence_event_idx ON evidence (tenant_id, observation_event_id);

ALTER TABLE evidence ENABLE ROW LEVEL SECURITY;
ALTER TABLE evidence FORCE ROW LEVEL SECURITY;
CREATE POLICY evidence_tenant_scoped ON evidence
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- **No UPDATE and no DELETE**, on Principle 2's terms. Offering a photograph in
-- support of a finding is a thing somebody did, and a mis-attached picture is
-- answered by attaching the right one — the same rule D132 settled for a
-- blurred retake. If mis-attachment turns out to be common rather than
-- hypothetical, the answer is a retraction row naming what it retracts, not a
-- DELETE grant.
GRANT SELECT, INSERT ON evidence TO nylonite_app;
