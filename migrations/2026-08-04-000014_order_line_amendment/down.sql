-- Reverse of 2026-08-04-000014_order_line_amendment.

-- The function body is restored to migration 9's rather than dropped, because
-- migration 9 created it and a down that dropped it would leave the schema
-- reversible only in one direction. The duplication is deliberate and is the cost
-- of CREATE OR REPLACE: down.sql has to carry the text it is reverting to.

CREATE OR REPLACE FUNCTION projection_order_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH folded AS (
        SELECT order_id,
               (array_remove(array_agg(new_promised_from ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS promised_from,
               (array_remove(array_agg(new_promised_to ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS promised_to,
               (array_remove(array_agg(new_required_by ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS required_by,
               (array_remove(array_agg(new_state ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS state
          FROM intention_amendment
         WHERE tenant_id = p_tenant
         GROUP BY order_id
    ),
    updated AS (
        UPDATE "order" o
           SET promised_from = COALESCE(f.promised_from, o.promised_from),
               promised_to   = COALESCE(f.promised_to,   o.promised_to),
               required_by   = COALESCE(f.required_by,   o.required_by),
               state         = COALESCE(f.state,         o.state)
          FROM folded f
         WHERE o.id = f.order_id AND o.tenant_id = p_tenant
        RETURNING o.id)
    SELECT count(*) INTO touched FROM updated;
    RETURN touched;
END
$$;

ALTER FUNCTION projection_order_rebuild(uuid) OWNER TO nylonite_projection_owner;

REVOKE SELECT, UPDATE ON order_line FROM nylonite_projection_owner;

REVOKE INSERT, UPDATE ON order_line FROM nylonite_app;
GRANT SELECT, INSERT, UPDATE ON order_line TO nylonite_app;

DELETE FROM projection_rebuild
 WHERE table_name = 'order_line' AND function_name = 'projection_order_rebuild';

-- Migration 13's text, which this migration prefixed rather than replaced.
COMMENT ON COLUMN order_line.quantity_ordered IS NULL;
COMMENT ON COLUMN order_line.unit_price_minor IS
    'Tax-exclusive, in minor units of the order''s currency, per price_basis_quantity units. The price in force, not the price as received.';
COMMENT ON COLUMN order_line.price_basis_quantity IS
    'The quantity unit_price_minor is quoted per. 345 per 100 rather than 0.0345 each.';

DROP INDEX IF EXISTS intention_amendment_line_idx;

-- An amendment naming a line cannot exist in the schema this reverses to.
--
-- It is not a matter of losing a column. The pre-D51 table has four covered
-- columns, all order-level, and `intention_amendment_changes_something_ck`
-- requires at least one of them to be set. A line-level amendment sets none, so
-- reversing leaves a row asserting that nothing changed, which the older schema
-- forbids by construction. Reordering does not help: the row is unrepresentable
-- whether the constraint is added before the columns are dropped or after.
--
-- So the down deletes them, and says how many out loud. **This is in tension with
-- a principle worth naming rather than stepping around**: facts are appended and
-- never deleted, which is why `stock_movement` has no DELETE grant and why
-- nothing may remove a `discrepancy`. The resolution is that a down migration is
-- not an application path. It is an operator withdrawing a capability, and the
-- facts recorded through that capability go with it — the same way the columns
-- below do. What must not happen is that it goes quietly.
DO $$
DECLARE
    doomed bigint;
BEGIN
    SELECT count(*) INTO doomed FROM intention_amendment WHERE order_line_id IS NOT NULL;
    IF doomed > 0 THEN
        RAISE NOTICE 'reversing D51 destroys % line-level amendment(s): they have no expression in the schema this reverts to', doomed;
        DELETE FROM intention_amendment WHERE order_line_id IS NOT NULL;
    END IF;
END
$$;

ALTER TABLE intention_amendment
    DROP CONSTRAINT IF EXISTS intention_amendment_changes_something_ck;

ALTER TABLE intention_amendment
    ADD CONSTRAINT intention_amendment_changes_something_ck
        CHECK (num_nonnulls(new_promised_from, new_promised_to,
                            new_required_by, new_state) > 0);

ALTER TABLE intention_amendment
    DROP CONSTRAINT IF EXISTS intention_amendment_price_sign_ck,
    DROP CONSTRAINT IF EXISTS intention_amendment_price_basis_ck,
    DROP CONSTRAINT IF EXISTS intention_amendment_quantity_ck,
    DROP CONSTRAINT IF EXISTS intention_amendment_price_pair_ck,
    DROP CONSTRAINT IF EXISTS intention_amendment_subject_ck,
    DROP CONSTRAINT IF EXISTS intention_amendment_line_fk;

ALTER TABLE intention_amendment
    DROP COLUMN IF EXISTS new_line_state,
    DROP COLUMN IF EXISTS new_price_basis_quantity,
    DROP COLUMN IF EXISTS new_unit_price_minor,
    DROP COLUMN IF EXISTS new_quantity_ordered,
    DROP COLUMN IF EXISTS order_line_id;

ALTER TABLE order_line DROP CONSTRAINT IF EXISTS order_line_order_key;

COMMENT ON COLUMN order_line.line_state IS NULL;
ALTER TABLE order_line DROP COLUMN IF EXISTS line_state;

DROP TYPE IF EXISTS order_line_state;
