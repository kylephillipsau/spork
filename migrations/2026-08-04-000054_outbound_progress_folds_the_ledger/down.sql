-- Reverse of 2026-08-04-000054_outbound_progress_folds_the_ledger.
--
-- Outbound progress goes back to folding `stock_allocation.state`, so a pick
-- recorded in the ledger stops moving any progress number and the cause arm D99
-- added goes unread. Question 165's second half reopens, and the outbound walk's
-- ten-against-zero gap returns.
--
-- Nothing is destroyed: both sources are still there and the columns are rebuilt
-- from whichever one the maintainer reads. That is the property that makes a
-- projection's source reversible at all, and it is why this down is short.

-- Migration 15's body, restored verbatim.
CREATE OR REPLACE FUNCTION projection_fulfilment_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    WITH folded AS (
        SELECT fulfilment_line_id,
               sum(quantity) FILTER (WHERE state IN
                   ('allocated','picking','picked','packed','fulfilled'))  AS covered,
               sum(quantity) FILTER (WHERE state IN
                   ('picked','packed','fulfilled'))                        AS picked,
               sum(quantity) FILTER (WHERE state IN
                   ('packed','fulfilled'))                                 AS packed,
               sum(quantity) FILTER (WHERE state = 'fulfilled')            AS despatched
          FROM stock_allocation
         WHERE tenant_id = p_tenant
           AND fulfilment_line_id IS NOT NULL
         GROUP BY fulfilment_line_id
    ),
    updated AS (
        UPDATE fulfilment_line fl
           SET covered_quantity    = coalesce(f.covered, 0),
               picked_quantity     = coalesce(f.picked, 0),
               packed_quantity     = coalesce(f.packed, 0),
               despatched_quantity = coalesce(f.despatched, 0)
          FROM fulfilment_line l
          LEFT JOIN folded f ON f.fulfilment_line_id = l.id
         WHERE fl.id = l.id AND fl.tenant_id = p_tenant
           AND (fl.covered_quantity, fl.picked_quantity,
                fl.packed_quantity, fl.despatched_quantity)
               IS DISTINCT FROM
               (coalesce(f.covered, 0), coalesce(f.picked, 0),
                coalesce(f.packed, 0), coalesce(f.despatched, 0))
        RETURNING fl.id)
    SELECT count(*) INTO touched FROM updated;

    RETURN touched;
END
$$;

ALTER FUNCTION projection_fulfilment_rebuild(uuid) OWNER TO spork_projection_owner;

COMMENT ON FUNCTION projection_fulfilment_rebuild(uuid) IS
    'Maintainer for fulfilment_line''s four coverage quantities. D53. There is no '
    'fulfilment arm: progress was dropped rather than defined, because a fulfilment '
    'has few lines and a stored label can disagree with them.';

-- Migration 15's column comments, restored.
COMMENT ON COLUMN fulfilment_line.covered_quantity IS
    '@projection of stock_allocation via projection_fulfilment_rebuild (D53, J31). '
    'How much of this commitment is covered by supply that still stands, including '
    'what has despatched. Not stock.allocated_quantity, which excludes despatch '
    'because the stock has left.';
COMMENT ON COLUMN fulfilment_line.picked_quantity IS
    '@projection of stock_allocation via projection_fulfilment_rebuild (D53, J31).';
COMMENT ON COLUMN fulfilment_line.packed_quantity IS
    '@projection of stock_allocation via projection_fulfilment_rebuild (D53, J31).';
COMMENT ON COLUMN fulfilment_line.despatched_quantity IS
    '@projection of stock_allocation via projection_fulfilment_rebuild (D53, J31).';

UPDATE projection_step
   SET note = 'Folds stock_allocation onto the commitment. Reads allocations rather '
              'than cells, so it does not depend on 30.'
 WHERE function_name = 'projection_fulfilment_rebuild';
