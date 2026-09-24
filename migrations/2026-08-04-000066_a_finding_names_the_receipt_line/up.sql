-- Migration 66: a finding names the receipt line it came from.
--
-- `goods_receipt_line_dispose` raises the discrepancy D88 decided on, and the
-- only thing tying that finding back to the line was the sentence it wrote into
-- `detail`. The receipt replay path then read the sentence back:
--
--     SELECT id FROM discrepancy WHERE detail LIKE '%<line id>%'
--       ORDER BY detected_at DESC LIMIT 1
--
-- **That is the failure Principle 3 names, arriving on a table that had escaped
-- it.** S31 forbids exactly this shape for `activity_event.detail` -- an
-- identification datum recovered by matching free text rather than read from a
-- typed column -- and the reasoning was never about `activity_event`. It is also
-- not merely inelegant: the match is a substring over prose, so any later finding
-- whose text happens to mention the same line wins on `detected_at DESC`, and a
-- replay returns a discrepancy the act it is replaying never raised.
--
-- The other three source arms are already typed. `stock_count_id` arrived one
-- migration ago for this reason and `require_count_facts` reads it directly,
-- which is why the count path never needed a `LIKE`. This is the fourth arm.

ALTER TABLE discrepancy
    ADD COLUMN goods_receipt_line_id uuid;

-- Composite through `tenant_id`, S51: *"RLS filters reads and says nothing about
-- what a row may point at."* Without it a finding in one tenant can name another
-- tenant's receipt line, and every report about either reads consistent.
ALTER TABLE discrepancy
    ADD CONSTRAINT discrepancy_goods_receipt_line_fk
        FOREIGN KEY (goods_receipt_line_id, tenant_id)
        REFERENCES goods_receipt_line(id, tenant_id);

-- S3's `<= 1`, and never `= 1`: a finding raised by a job has no source fact at
-- all, which is most of them.
ALTER TABLE discrepancy
    DROP CONSTRAINT discrepancy_source_ck;

ALTER TABLE discrepancy
    ADD CONSTRAINT discrepancy_source_ck
        CHECK (num_nonnulls(
            stock_movement_id,
            package_event_id,
            stock_count_id,
            goods_receipt_line_id
        ) <= 1);

CREATE INDEX discrepancy_goods_receipt_line_idx
    ON discrepancy (goods_receipt_line_id)
    WHERE goods_receipt_line_id IS NOT NULL;

COMMENT ON COLUMN discrepancy.goods_receipt_line_id IS
    'Source arm: the receipt line whose disposition raised this finding. Typed '
    'because the alternative was a LIKE over detail, which is S31''s rule and '
    'returns the wrong row as soon as two findings mention one line. D8, D88, '
    'Principle 3.';

-- S45: a column granted to nobody is the mistake every other guard here is
-- pointed away from. The same four roles that hold the rest of this table.
GRANT INSERT (goods_receipt_line_id), SELECT (goods_receipt_line_id),
      UPDATE (goods_receipt_line_id)
    ON discrepancy TO spork_app;
GRANT INSERT (goods_receipt_line_id), SELECT (goods_receipt_line_id)
    ON discrepancy TO spork_mediation_owner;
GRANT INSERT (goods_receipt_line_id), SELECT (goods_receipt_line_id)
    ON discrepancy TO spork_projection_owner;
GRANT INSERT (goods_receipt_line_id), SELECT (goods_receipt_line_id)
    ON discrepancy TO spork_scheduler;

-- ---------------------------------------------------------------------------
-- The disposition writes the arm
-- ---------------------------------------------------------------------------
--
-- Same signature, so this is a replace. **`CREATE OR REPLACE` keeps the owner and
-- the ACL and does not keep the attributes**, so `SECURITY DEFINER` and
-- `search_path` are restated here rather than assumed: dropping either turns
-- D89's mediated write back into a function the app cannot run, or a definer with
-- an unpinned path, which are the two halves of what S54 and J37 watch.
--
-- `p_client_event_id` stays unused, deliberately and now visibly. A discrepancy
-- is a Finding rather than a Fact, so S19 does not ask it for a `client_event`
-- FK, and it already carries `detected_by_id` and `detected_at` for the person
-- and the moment. Adding a column to carry the act as well would be a third
-- record of the same thing.

CREATE OR REPLACE FUNCTION goods_receipt_line_dispose(
        p_line             uuid,
        p_receiving_policy_id uuid,
        p_accept           boolean,
        p_raise            text,
        p_actor            uuid,
        p_client_event_id  uuid)
    RETURNS uuid
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    l          record;
    d_id       uuid;
BEGIN
    SELECT * INTO l FROM goods_receipt_line WHERE id = p_line FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'no receipt line %', p_line;
    END IF;

    IF l.accepted_at IS NOT NULL OR l.rejected_at IS NOT NULL THEN
        RAISE EXCEPTION 'receipt line % is already disposed of', p_line
            USING HINT = 'a change of mind is a correction, not a second disposition';
    END IF;

    IF p_accept THEN
        UPDATE goods_receipt_line
           SET accepted_at = now(), accepted_by_id = p_actor,
               receiving_policy_id = p_receiving_policy_id
         WHERE id = p_line;
    ELSE
        UPDATE goods_receipt_line
           SET rejected_at = now(), rejected_by_id = p_actor,
               receiving_policy_id = p_receiving_policy_id
         WHERE id = p_line;
    END IF;

    IF p_raise IS NOT NULL THEN
        INSERT INTO discrepancy (tenant_id, kind, item_id, lot_id,
            expected_quantity, observed_quantity, detected_at, detected_by_id,
            state, goods_receipt_line_id, detail)
        VALUES (l.tenant_id, p_raise::discrepancy_kind, l.item_id, l.lot_id,
                l.expected_quantity, l.entered_quantity, now(), p_actor,
                'open', p_line,
                -- Prose for a person reading the queue. Nothing queries it:
                -- the line is the column above and the policy is on the line.
                format('receipt line %s disposed under receiving policy %s',
                       p_line, p_receiving_policy_id))
        RETURNING id INTO d_id;
    END IF;

    RETURN d_id;
END
$$;

ALTER FUNCTION goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid)
    OWNER TO spork_mediation_owner;
REVOKE EXECUTE ON FUNCTION
    goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION
    goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid)
    TO spork_app;

COMMENT ON FUNCTION goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid) IS
    'Applies a decision D88 made under a policy D82 resolved and D83 clamped: the '
    'disposition, the version that governed it, and the discrepancy it raised, in '
    'one transaction. The finding names its receipt line in a typed column '
    '(migration 66) rather than in prose. Refuses a second disposition -- a change '
    'of mind is a correction. D24, D87, D88, D89.';
