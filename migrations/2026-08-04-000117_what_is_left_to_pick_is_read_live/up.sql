-- Migration 117: what is left to pick is read live, and picks made elsewhere
-- are not picked again.
--
-- The picking walk and its badge counted a line as open while
-- `fulfilment_line.picked_quantity < quantity`. Two things were wrong with that,
-- both found while planning picking in Spork (docs/picking-plan.md):
--
-- 1. **A line NetSuite had picked stayed on the walk.** Its goods are reported,
--    not moved (D172), so `picked_quantity` never counted them, and the walk
--    sent somebody to a shelf for goods already on their way to the bench.
-- 2. **The walk read a cache.** `picked_quantity` waits for the scheduler, so a
--    line picked a moment ago came back on the next read. With a live channel
--    telling every device about a pick as it happens (the plan's groundwork),
--    the next read is immediately after the pick, which is the worst moment to
--    read a cache.
--
-- `line_to_pick` states it once, for a site's open lines: what has been picked
-- here out of storage (D166, net of corrections as D103 nets them, the shape
-- `ledger_views::line_progress_ledger` folds for one line), what the other
-- system reports picked (each external line's newest report, as the bench reads
-- it), and what is left, never below nothing. Invoker's rights, so the tenant's
-- policies apply.

CREATE FUNCTION line_to_pick(p_site uuid)
RETURNS TABLE (fulfilment_line_id uuid, picked bigint, reported bigint, to_pick bigint)
LANGUAGE sql STABLE AS $$
    WITH RECURSIVE open AS (
        SELECT fl.id, fl.quantity
          FROM fulfilment_line fl
          JOIN fulfilment f ON f.id = fl.fulfilment_id
         WHERE f.site_id = p_site
           AND f.state <> 'cancelled'
           AND f.closed_elsewhere IS NULL
    ),
    roots AS (
        SELECT m.id, m.quantity, m.fulfilment_line_id, l.kind AS from_kind
          FROM stock_movement m
          JOIN open o ON o.id = m.fulfilment_line_id
          LEFT JOIN location l ON l.id = m.from_location_id
         WHERE m.reverses_movement_id IS NULL
    ),
    -- A correction takes its shape from the movement it reverses: what matters
    -- is where the root came out of.
    chain AS (
        SELECT id AS movement_id, quantity, 0 AS depth, fulfilment_line_id, from_kind
          FROM roots
        UNION ALL
        SELECT r.id, r.quantity, c.depth + 1, c.fulfilment_line_id, c.from_kind
          FROM chain c
          JOIN stock_movement r ON r.reverses_movement_id = c.movement_id
    ),
    picked AS (
        SELECT fulfilment_line_id,
               sum(CASE WHEN depth % 2 = 0 THEN quantity ELSE -quantity END)::bigint AS q
          FROM chain
         WHERE from_kind IN ('pick_face', 'bulk', 'overflow')
         GROUP BY fulfilment_line_id
    ),
    reported AS (
        SELECT n.fulfilment_line_id, sum(n.quantity)::bigint AS q
          FROM (SELECT DISTINCT ON (ep.fulfilment_line_id, ep.external_line)
                       ep.fulfilment_line_id, ep.quantity
                  FROM external_pick ep
                  JOIN open o ON o.id = ep.fulfilment_line_id
                 ORDER BY ep.fulfilment_line_id, ep.external_line,
                          ep.observed_at DESC, ep.recorded_at DESC, ep.id DESC) n
         GROUP BY n.fulfilment_line_id
    )
    SELECT o.id,
           coalesce(p.q, 0),
           coalesce(r.q, 0),
           greatest(o.quantity - coalesce(p.q, 0) - coalesce(r.q, 0), 0)::bigint
      FROM open o
      LEFT JOIN picked p ON p.fulfilment_line_id = o.id
      LEFT JOIN reported r ON r.fulfilment_line_id = o.id
$$;

COMMENT ON FUNCTION line_to_pick(uuid) IS
    'A site''s open lines: picked here out of storage, reported picked elsewhere, '
    'and what is left to pick, live from the ledger. Migration 117.';

GRANT EXECUTE ON FUNCTION line_to_pick(uuid) TO spork_app;
