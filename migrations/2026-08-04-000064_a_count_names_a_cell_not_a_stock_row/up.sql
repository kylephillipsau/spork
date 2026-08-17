-- Migration 64: a count names a cell, not a stock row.
--
-- Correcting migration 63, which added the second foreign key to `stock(id)`
-- that S28 forbids:
--
--     Exactly one foreign key in the schema targets stock(id), and it is
--     stock_allocation.stock_id, declared ON DELETE RESTRICT.
--
-- **The rule is not arbitrary and the reason is that `stock` is reapable.** J32
-- makes the reap a phase inside the rebuild, and J33 requires the rebuild to
-- preserve row identity only for cells that are still live -- a cell that goes to
-- zero is deleted. Every foreign key pointing at `stock.id` therefore has to
-- answer what happens when the cell it names empties, and there are only two
-- answers. `stock_allocation` takes the first: RESTRICT, so a reap that would
-- orphan a live commitment fails loudly and somebody unwinds it. That is correct
-- for an allocation, which is a claim on stock that exists *now*.
--
-- A count is the other kind of thing entirely. It is an assertion about what was
-- on a shelf at a moment, and the moment is in the past. Migration 63's
-- constraint defaults to NO ACTION, which blocks the delete exactly as RESTRICT
-- does but without saying so, so the first cycle count of a cell that later
-- empties would have wedged the projection rebuild -- silently, because nothing
-- in the suite watches a reap that did not happen.
--
-- **The column set is why the key was never needed.** S2:
--
--     Every table naming a stock cell carries the complete column set. The "or
--     FK to stock.id" disjunction is removed: under a reapable stock the two are
--     not equivalent.
--
-- `stock_count` already carries all six -- item, holder location, holder package,
-- lot, status, owner -- and those survive the reap, which is the whole point of
-- requiring them. The cell is identified by what it is, not by the identifier of
-- a projection row that may since have been collected.

ALTER TABLE stock_count
    DROP CONSTRAINT stock_count_stock_fk;

-- **The column stays, and it is now a snapshot rather than a reference.** D62's
-- adjustment path reads it to check that an adjustment resolving a count is
-- against the same cell the count was taken against, and that check is worth
-- keeping. What it may not do is constrain the projection: after a reap the value
-- names a row that is gone, which is a true statement about the past and exactly
-- what `expected_quantity` on `goods_receipt_line` already is.
COMMENT ON COLUMN stock_count.stock_id IS
    'The stock cell row this count was taken against, as it stood at capture. '
    'Deliberately not a foreign key: stock is reapable (J32, J33) and S28 permits '
    'exactly one key onto it, stock_allocation''s. After a reap this names a row '
    'that no longer exists, which is a fact about the past rather than a dangling '
    'reference. The cell itself is identified by the six key columns beside it, '
    'per S2. NULL when counting space that holds no stock row. D8, S2, S28.';

-- The index is unaffected and still earns its place: the adjustment path looks a
-- count up by the cell it named.
