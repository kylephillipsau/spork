-- Migration 54: outbound progress folds the facts that produced it.
--
-- D100, settling the second half of question 165.
--
-- D99 gave the ledger an outbound cause and settled the discriminator; the fold
-- itself was left here deliberately, because moving it needs three things the arm
-- did not: a source for `packed_quantity`, a decision about what a correction does
-- to progress, and J31 split so one source stops answering two kinds of question.
--
-- The shape rules are D99's and do not change:
--
--   picked      the movement left a storage location
--   despatched  the movement has no `to` side at all -- D45's arrival test mirrored
--
-- What follows is the part D99 left open.

-- ---------------------------------------------------------------------------
-- 1. `packed` reads the carton's status, not its `sealed_at`
-- ---------------------------------------------------------------------------
--
-- Sealing a carton moves no stock, so `packed_quantity` cannot be a grouping over
-- the ledger alone. D99 named the derivation -- the movements into a carton,
-- bounded by its seal -- and left the column to read.
--
-- **`package.sealed_at` is the wrong one, and picking it would have undone the
-- decision.** It is DECLARED: migration 9 grants the application both INSERT and
-- UPDATE on it, so it is a timestamp the app can rewrite. Folding progress from it
-- would reintroduce the exact defect that made this whole question worth asking --
-- a physical claim resting on a mutable value -- one column over from the state
-- machine we are moving off.
--
-- `package.status` is a **projection of `package_event`**, the winning row of
-- ('sealed','opened','despatched','voided') by occurred_at. It is a fold of facts,
-- maintained at ordinal 40, and this fold runs at 90.
--
-- **And the status makes the time bound unnecessary.** The bound existed to stop a
-- repack counting twice: stock into carton A, A sealed, A opened, stock moved to
-- carton B, B sealed. Both movements are movements into a sealed carton. But
-- opening A makes its winning status `opened`, so A stops contributing on its own,
-- and only B is counted. The carton's current status already carries the history
-- the bound was reaching for, and reading it costs one join rather than a
-- correlated lookup of each package's latest seal.
--
--   into a tote, then tote into a sealed carton   packed once, at the carton
--   into a carton later opened and not resealed   packed zero -- it is not packed
--   into a carton sealed and then despatched      packed, and stays packed
--
-- The last is D53's nesting: despatched units are still packed, and `despatched`
-- is a subset of `packed` rather than its successor.

-- ---------------------------------------------------------------------------
-- 2. A movement recorded in error did not happen
-- ---------------------------------------------------------------------------
--
-- D47 splits a movement by whether the world changed or the record was wrong, and
-- migration 10 states the rule as *"reason class: record_error exactly when
-- reverses_movement_id is set"*. So a reversal is not an adjustment to a real event.
-- It is the statement that the event was never real.
--
-- **Both rows leave the fold, and missing the second one is the subtle half.** The
-- erroneous movement goes because it did not happen. The reversal goes because of
-- its shape: reversing a despatch means running it backwards, and a despatch is
-- `carton -> nothing`, so its reversal is `nothing -> carton`. That has a
-- `to_package_id` and no `from` side, which is a movement *into* a sealed carton --
-- so a fold that dropped only the original would count the correction as ten more
-- units packed. The pair has to vanish together or a correction inflates the number
-- it was recorded to fix.
--
-- **The inbound fold has the same gap and this migration does not touch it.** D45's
-- `quantity_received` counts movements with no `from` side and excludes neither a
-- reversed arrival nor its reversal, so a receipt recorded in error stays received.
-- Nothing has hit it because the one reversal in the fixture names no receipt line.
-- Widening a decision by touching its fold from another decision's migration is how
-- the two drift apart; question 168 carries it.

-- ---------------------------------------------------------------------------
-- 3. The maintainer
-- ---------------------------------------------------------------------------
--
-- Two sources now, and the split is by what each number is evidence *of*. Coverage
-- asks how much of a commitment is spoken for, which is a question about intentions
-- and D12 makes an allocation exactly that. The other three assert what physically
-- happened, and the scanner is more authoritative than the database.

CREATE OR REPLACE FUNCTION projection_fulfilment_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH ledger AS (
        SELECT m.fulfilment_line_id,
               -- D99's shape rules. Neither reads `reason`, which is text with no
               -- CHECK: re-handling after a pick is holder-to-holder, so a
               -- consolidation from tote to carton names the same line and is
               -- correctly not a second pick.
               sum(m.quantity) FILTER (
                   WHERE m.from_location_id IS NOT NULL)                 AS picked,
               sum(m.quantity) FILTER (
                   WHERE p.status IN ('sealed', 'despatched'))           AS packed,
               sum(m.quantity) FILTER (
                   WHERE m.to_location_id IS NULL
                     AND m.to_package_id IS NULL)                        AS despatched
          FROM stock_movement m
          LEFT JOIN package p ON p.id = m.to_package_id
         WHERE m.tenant_id = p_tenant
           AND m.fulfilment_line_id IS NOT NULL
           -- Section 2: the pair leaves together.
           AND m.reverses_movement_id IS NULL
           AND NOT EXISTS (SELECT 1 FROM stock_movement r
                            WHERE r.reverses_movement_id = m.id)
         GROUP BY m.fulfilment_line_id
    ),
    intention AS (
        -- Unchanged, and deliberately so. `fulfilled` is included here and excluded
        -- from stock.allocated_quantity, which is the whole reason those two columns
        -- stopped sharing a name.
        SELECT fulfilment_line_id, sum(quantity)::bigint AS covered
          FROM stock_allocation
         WHERE tenant_id = p_tenant
           AND state IN ('allocated','picking','picked','packed','fulfilled')
           AND fulfilment_line_id IS NOT NULL
         GROUP BY fulfilment_line_id
    ),
    updated AS (
        -- LEFT JOIN from the line rather than FROM either fold, for the reason D53
        -- gave and which now applies twice: a line whose last allocation was
        -- released, or whose only movement was reversed, has no group on that side
        -- and must go to zero. Joining the other way would leave the previous
        -- numbers standing, which is the failure mode a rebuild exists to be
        -- immune to.
        UPDATE fulfilment_line fl
           SET covered_quantity    = coalesce(i.covered, 0),
               picked_quantity     = coalesce(g.picked, 0),
               packed_quantity     = coalesce(g.packed, 0),
               despatched_quantity = coalesce(g.despatched, 0)
          FROM fulfilment_line l
          LEFT JOIN intention i ON i.fulfilment_line_id = l.id
          LEFT JOIN ledger    g ON g.fulfilment_line_id = l.id
         WHERE fl.id = l.id AND fl.tenant_id = p_tenant
           AND (fl.covered_quantity, fl.picked_quantity,
                fl.packed_quantity, fl.despatched_quantity)
               IS DISTINCT FROM
               (coalesce(i.covered, 0), coalesce(g.picked, 0),
                coalesce(g.packed, 0), coalesce(g.despatched, 0))
        RETURNING fl.id)
    SELECT count(*) INTO touched FROM updated;

    RETURN touched;
END
$$;

ALTER FUNCTION projection_fulfilment_rebuild(uuid) OWNER TO nylonite_projection_owner;

COMMENT ON FUNCTION projection_fulfilment_rebuild(uuid) IS
    'Maintainer for fulfilment_line''s four coverage quantities, from two sources '
    'split by what each is evidence of (D53, D99, D100). covered_quantity folds '
    'stock_allocation because coverage is an intention; picked, packed and '
    'despatched fold stock_movement because they assert what happened. There is no '
    'fulfilment arm: progress was dropped rather than defined, because a fulfilment '
    'has few lines and a stored label can disagree with them.';

-- ---------------------------------------------------------------------------
-- 4. The column comments, which are the register's own source
-- ---------------------------------------------------------------------------
--
-- S5 reads these. Three of the four now name a different source than they did, and
-- a comment left behind would say the fold reads a table it no longer reads.

COMMENT ON COLUMN fulfilment_line.covered_quantity IS
    '@projection of stock_allocation via projection_fulfilment_rebuild (D53, J31). '
    'How much of this commitment is covered by supply that still stands, including '
    'what has despatched. An intention, and the one of the four that is. Not '
    'stock.allocated_quantity, which excludes despatch because the stock has left.';
COMMENT ON COLUMN fulfilment_line.picked_quantity IS
    '@projection of stock_movement via projection_fulfilment_rebuild (D99, D100, J68). '
    'Movements naming this line that left a storage location.';
COMMENT ON COLUMN fulfilment_line.packed_quantity IS
    '@projection of stock_movement and package.status via projection_fulfilment_rebuild '
    '(D99, D100, J68). Movements naming this line into a carton whose winning status '
    'is sealed or despatched. Not package.sealed_at, which the application may UPDATE.';
COMMENT ON COLUMN fulfilment_line.despatched_quantity IS
    '@projection of stock_movement via projection_fulfilment_rebuild (D99, D100, J68). '
    'Movements naming this line with no to side at all, which is D45''s arrival test '
    'mirrored.';

-- ---------------------------------------------------------------------------
-- 5. The step note, which described a dependency that has changed
-- ---------------------------------------------------------------------------
--
-- D98 established that the ordinal is a dependency order and that the notes are
-- where the dependency is stated. This step said *"Reads allocations rather than
-- cells, so it does not depend on 30"*, which was true and is now half wrong: it
-- reads the ledger directly, and it reads package.status from 40.
--
-- The ordinal does not move. 90 is already after both, so the order was correct by
-- luck rather than by statement, and stating it is what stops the next person
-- reordering the sequence and finding out.

UPDATE projection_step
   SET note = 'Folds stock_allocation onto the commitment for covered_quantity, and '
              'stock_movement for the three progress quantities (D100). Depends on 40 '
              'for package.status, which decides which cartons are packed. Reads the '
              'ledger directly rather than through the cells at 30.'
 WHERE function_name = 'projection_fulfilment_rebuild';
