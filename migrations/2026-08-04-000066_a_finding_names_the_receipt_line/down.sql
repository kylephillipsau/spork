-- Migration 66 down: the finding stops naming its receipt line, and the
-- disposition function goes back to writing prose only.

ALTER TABLE discrepancy DROP CONSTRAINT IF EXISTS discrepancy_source_ck;

DO $$
DECLARE
    n bigint;
BEGIN
    SELECT count(*) INTO n FROM discrepancy WHERE goods_receipt_line_id IS NOT NULL;
    IF n > 0 THEN
        RAISE NOTICE 'reversing migration 66 destroys the receipt-line link on % finding(s): '
                     'which line raised them is recoverable only by matching prose again', n;
    END IF;
END $$;

DROP INDEX IF EXISTS discrepancy_goods_receipt_line_idx;
ALTER TABLE discrepancy DROP CONSTRAINT IF EXISTS discrepancy_goods_receipt_line_fk;
ALTER TABLE discrepancy DROP COLUMN IF EXISTS goods_receipt_line_id;

ALTER TABLE discrepancy
    ADD CONSTRAINT discrepancy_source_ck
        CHECK (num_nonnulls(
            stock_movement_id,
            package_event_id,
            stock_count_id
        ) <= 1);

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

ALTER FUNCTION goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid)
    OWNER TO nylonite_mediation_owner;
REVOKE EXECUTE ON FUNCTION
    goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION
    goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid)
    TO nylonite_app;

COMMENT ON FUNCTION goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid) IS
    'Applies a decision D88 made under a policy D82 resolved and D83 clamped: the '
    'disposition, the version that governed it, and the discrepancy it raised, in '
    'one transaction. Refuses a second disposition -- a change of mind is a '
    'correction. D24, D87, D88, D89.';
