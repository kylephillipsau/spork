-- Migration 26: three indexes, chosen by measurement, out of a hundred and thirty-two.
--
-- D66, settling question 142. The question asked which foreign keys need covering
-- indexes and said the answer should be *"a considered subset chosen from
-- measurement rather than from taste"*. D65 built the year of history that made
-- measuring possible. This is the measurement and what it decided.
--
-- **Selectivity decides, not foreign-key-ness.** Postgres indexes the target of a
-- foreign key and never the source, and the standard advice is to index every
-- source column. Against 289,083 movements that advice is wrong more often than
-- it is right:
--
--   candidate                     rows matched      unindexed    indexed
--   ---------------------------------------------------------------------
--   stock_movement lot pair       3 of 289,083       21.96 ms    0.27 ms
--   stock_movement location pair  70,080 of 289,083  12.41 ms   24.78 ms
--   goods_receipt_line.item_id    1 of 35,041        22.40 ms    0.044 ms
--
-- The location index makes its query **twice as slow**. Twenty-four per cent of a
-- table is not an index lookup, it is a sequential scan with extra steps, and the
-- planner is right to say so. That is one of the hundred and thirty-two, chosen
-- because it looked as plausible as the others.

-- ---------------------------------------------------------------------------
-- The two on the ledger
-- ---------------------------------------------------------------------------
--
-- Three rows out of two hundred and eighty-nine thousand. **The question behind
-- it is a recall**: this lot is contaminated or mis-dated, where did it go and
-- what is left. D14 built lot tracking for that and D31 sets a retention floor so
-- the answer survives; without an index the answer takes a full scan of the
-- ledger, and it gets slower every day the ledger grows.
--
-- Partial, because most movements carry no lot: D33 makes tracking a property of
-- a product rather than a mode the system runs in, and two thirds of the
-- generated catalogue is untracked. The partial predicate keeps the index
-- proportional to the tracked subset rather than to the table — 864 kB against a
-- 63 MB table.
--
-- Two indexes rather than one over both columns: a movement's lot may change on
-- one side only, and the recall question is "either side", which a bitmap OR of
-- two indexes answers and a composite does not.

CREATE INDEX stock_movement_from_lot_idx
    ON stock_movement (from_lot_id) WHERE from_lot_id IS NOT NULL;

CREATE INDEX stock_movement_to_lot_idx
    ON stock_movement (to_lot_id) WHERE to_lot_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- The one on the receipt line
-- ---------------------------------------------------------------------------
--
-- One row in thirty-five thousand, and five hundred times faster indexed. The
-- question is "what have we received of this item and against which promises",
-- which is the supplier scorecard D8 wants and the shortage investigation the
-- receiving screen needs.

CREATE INDEX goods_receipt_line_item_idx ON goods_receipt_line (item_id);

-- ---------------------------------------------------------------------------
-- What was measured and rejected, which is most of it
-- ---------------------------------------------------------------------------
--
-- **The location pair.** Measured slower. Twenty-four per cent selectivity.
--
-- **The actor columns** — `recorded_by_id`, `authorised_by_id`, `accepted_by_id`
-- and the rest. The fixture has three people, so one of them matches thirty-eight
-- per cent of the ledger and the measurement describes the fixture rather than
-- the domain. **This is not a decision, it is an admission that the instrument
-- cannot read it**, and question 147 carries it rather than letting a guess pass
-- as a result.
--
-- **The parent-delete cost.** Deleting one `location` costs 67.6 ms, of which
-- 60.8 ms is two sequential scans of `stock_movement` for referencing rows. That
-- is the cost of *discovering the delete is illegal*, because a location with
-- movements against it cannot be removed without destroying what the ledger
-- means, and the foreign key correctly refuses. Locations are deleted
-- approximately never, so 60 ms of rare admin work does not buy 2.3 MB of index
-- and a write cost on the hottest table in the schema.
--
-- **The reaper, which question 142 named as the case that made this live rather
-- than theoretical.** It is already covered: `stock_allocation.stock_id` carries
-- `stock_allocation_stock_idx`, and deleting an empty cell costs 0.669 ms with a
-- 0.316 ms foreign key check. **The question was wrong about its own strongest
-- argument**, which is worth recording because it was written from the same
-- reading of the same standard advice that this migration is refusing.
--
-- **The remaining hundred and twenty-odd.** Validation foreign keys on small
-- reference tables, never joined from, on tables the planner reads in a page.
-- An index each would cost write throughput on every insert to buy nothing
-- measurable.
