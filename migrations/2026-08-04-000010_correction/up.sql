-- Migration 10: correction, and the two timestamps that make it recoverable.
--
-- D8 stated this in one clause: "A correction is a *new* movement carrying
-- `reverses_movement_id` and a reason." Nine migrations later the column did not
-- exist, so the sentence was a promise rather than a design. This builds it, and
-- in building it answers the question D8 left open: which `occurred_at` does a
-- correction carry?
--
-- The answer is the corrected fact's, not the moment of noticing. A movement
-- recorded at 09:14 and corrected at 11:30 produces a correction stamped
-- occurred_at 09:14, recorded_at 11:30. That keeps two different questions
-- separately answerable:
--
--   What did we believe at 10:00?   filter recorded_at <= 10:00  -> the old number
--   What was true at 10:00?         filter occurred_at <= 10:00  -> the new one
--
-- Stamping the correction 11:30 would answer only the first, forever. There
-- would be no query that recovers what actually sat in the bin, because the
-- system would be asserting that stock physically moved at the moment somebody
-- noticed an error.
--
-- Both columns already existed and D24 already folds in
-- (occurred_at, recorded_at, id) order, so a correction is just another
-- out-of-order arrival. J6 and J46 already assert that arrival order does not
-- change the result. No new fold machinery is needed.

-- ---------------------------------------------------------------------------
-- Why an adjustment was made, as distinct from what kind of movement it was
-- ---------------------------------------------------------------------------
--
-- The competitor analysis names this as gap 15: `reason` is a movement type, so
-- "adjustment, damaged" and "adjustment, miscounted" are indistinguishable. They
-- are not the same event in any sense that matters. One is stock that existed
-- and stopped existing. The other is stock that never existed.
--
-- The vocabulary is therefore split by that line and nothing else, because the
-- split is what the whole migration is for. Mixing the two classes in one list
-- would rebuild the ambiguity inside the fix.

CREATE TYPE adjustment_class AS ENUM (
    'record_error',   -- nothing moved; the record was wrong
    'world_event'     -- something moved, and the record is right about it
);

CREATE TABLE adjustment_reason (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   uuid REFERENCES tenant(id),   -- NULL = shared vocabulary (D19 shape 2)
    code        text NOT NULL,
    name        text NOT NULL,
    class       adjustment_class NOT NULL,
    active      boolean NOT NULL DEFAULT true,
    UNIQUE NULLS NOT DISTINCT (tenant_id, code)
);

ALTER TABLE adjustment_reason ENABLE ROW LEVEL SECURITY;
ALTER TABLE adjustment_reason FORCE ROW LEVEL SECURITY;
CREATE POLICY adjustment_reason_shared_reference ON adjustment_reason
    USING (tenant_id IS NULL OR tenant_id = current_tenant());
GRANT SELECT, INSERT, UPDATE, DELETE ON adjustment_reason TO nylonite_app;

INSERT INTO adjustment_reason (tenant_id, code, name, class) VALUES
    (NULL, 'miscount',        'Counted wrong',                        'record_error'),
    (NULL, 'wrong_item',      'Scanned the wrong item',               'record_error'),
    (NULL, 'wrong_location',  'Recorded against the wrong location',  'record_error'),
    (NULL, 'duplicate_entry', 'Recorded twice',                       'record_error'),
    (NULL, 'transcription',   'Typed a different number than counted','record_error'),
    (NULL, 'damaged',         'Damaged',                              'world_event'),
    (NULL, 'found',           'Found',                                'world_event'),
    (NULL, 'expired',         'Expired',                              'world_event'),
    (NULL, 'sample',          'Taken as a sample',                    'world_event'),
    (NULL, 'disposed',        'Disposed of',                          'world_event');

-- Migration 7 seeded zero rows through a join to an unpopulated table and said
-- nothing about it. A seed that silently does nothing is worse than one that
-- fails, so every seed from here asserts its own result.
DO $$
BEGIN
    IF (SELECT count(*) FROM adjustment_reason WHERE tenant_id IS NULL) <> 10 THEN
        RAISE EXCEPTION 'adjustment_reason vocabulary did not seed';
    END IF;
END $$;

-- ---------------------------------------------------------------------------
-- The correction link
-- ---------------------------------------------------------------------------

ALTER TABLE stock_movement
    ADD COLUMN reverses_movement_id uuid REFERENCES stock_movement(id),
    ADD COLUMN adjustment_reason_id uuid REFERENCES adjustment_reason(id);

-- The presence of reverses_movement_id is the entire discriminator, and it is
-- worth stating plainly: set means we were wrong, null means the world changed.
-- Everything downstream reads it that way, so a correction without a stated
-- reason is not admissible.
ALTER TABLE stock_movement
    ADD CONSTRAINT stock_movement_reversal_has_reason_ck
        CHECK (reverses_movement_id IS NULL OR adjustment_reason_id IS NOT NULL),
    ADD CONSTRAINT stock_movement_reversal_not_self_ck
        CHECK (reverses_movement_id IS DISTINCT FROM id);

CREATE INDEX stock_movement_reverses_idx
    ON stock_movement (reverses_movement_id)
    WHERE reverses_movement_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- Where the cross-row rules are not
-- ---------------------------------------------------------------------------
--
-- Four rules govern a correction and none of them fits in a CHECK, because all
-- four compare it to the row it corrects:
--
--   occurred_at    equal to the target's; a wrong one corrupts every as-at
--                  query with no symptom at all
--   mirroring      a reversal runs the other way to its target, or it is a
--                  second movement wearing a correction's label
--   quantity       not larger than the target, or it invents stock
--   reason class   record_error exactly when reverses_movement_id is set
--
-- The obvious home is a BEFORE INSERT trigger, and this migration had one. S7
-- rejected it, and S7 is right. D25 forbids triggers that implement rules,
-- validation, defaults or cascades, and a validation trigger is the named case.
--
-- The reasons hold beyond the letter of the rule. stock_movement is the highest
-- volume table in the schema and a guard costs it a SELECT per insert on the one
-- path that has to stay fast. A trigger is also the most quietly disarmable
-- object in Postgres: ALTER TABLE ... DISABLE TRIGGER leaves the row in
-- pg_trigger, so the enforcement would need its own invariant watching whether
-- it was still switched on. Needing a second mechanism to watch the first is the
-- argument against the first.
--
-- So validation lives in the application write path, which has to read the
-- target anyway to compute the mirrored sides, and is visible and testable where
-- a trigger is neither. See crates/server/src/correction.rs. J50 to J52 are the
-- backstop for rows that arrive by any other route, as findings rather than
-- errors, which is what D8 asks for everywhere else.
