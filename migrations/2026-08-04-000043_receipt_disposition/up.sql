-- Migration 43: the transaction that disposes of a receipt line.
--
-- D89. Three pieces were built and none was connected: D82 and D83 resolve and
-- clamp, D88 decides, D87 has the column. This is the act that joins them, and
-- until now `goods_receipt_line.accepted_at` had been written by nothing but the
-- fixture since migration 21.
--
-- **In SQL rather than in the server crate, and the reason is the pattern already
-- here.** D85 and D86 settled it for a projection: the caller resolves, because
-- D22 puts the precedence order in code, and the effect is a database function it
-- hands the answer to. A disposition is the same shape -- a decision made in Rust,
-- applied to rows in one transaction -- and D24 requires that transaction to be
-- one: *"at receipt, in the same transaction as the movements."*

CREATE FUNCTION goods_receipt_line_dispose(
        p_line             uuid,
        p_receiving_policy_id uuid,
        p_accept           boolean,
        p_raise            text,
        p_actor            uuid,
        p_client_event_id  uuid)
    RETURNS uuid
    LANGUAGE plpgsql
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

    -- **A disposition happens once.** D21's rule for a resolved annotation and
    -- D87's note about freezing are the same rule arriving here first: once a
    -- line has been accepted against a policy, changing it silently rewrites what
    -- the acceptance meant. A correction is a new act, which is D51's shape and
    -- not this function's job.
    IF l.accepted_at IS NOT NULL OR l.rejected_at IS NOT NULL THEN
        RAISE EXCEPTION 'receipt line % is already disposed of', p_line
            USING HINT = 'a change of mind is a correction, not a second disposition';
    END IF;

    -- J63: the version that governed it, whatever the outcome. A refusal is a
    -- governed act too.
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

    -- The finding D88 decided to raise, as a row rather than a log line. D8 makes
    -- a discrepancy the record of a disagreement, and an over-receipt nobody can
    -- point at is an over-receipt nobody will chase.
    IF p_raise IS NOT NULL THEN
        INSERT INTO discrepancy (tenant_id, kind, item_id, lot_id,
            expected_quantity, observed_quantity, detected_at, detected_by_id,
            state, detail)
        VALUES (l.tenant_id, p_raise::discrepancy_kind, l.item_id, l.lot_id,
                l.expected_quantity, l.entered_quantity, now(), p_actor,
                'open',
                format('receipt line %s disposed under receiving policy %s',
                       p_line, p_receiving_policy_id))
        RETURNING id INTO d_id;
    END IF;

    RETURN d_id;
END
$$;

COMMENT ON FUNCTION goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid) IS
    'Applies a decision D88 made under a policy D82 resolved and D83 clamped: the '
    'disposition, the version that governed it, and the discrepancy it raised, in '
    'one transaction. Refuses a second disposition -- a change of mind is a '
    'correction. D24, D87, D88, D89.';

GRANT EXECUTE ON FUNCTION
    goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid)
    TO spork_app;
