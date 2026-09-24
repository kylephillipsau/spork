-- Migration 53: a pick says which line it served.
--
-- D99, settling the first half of question 165, raised by the outbound walk.
--
-- The walk's finding, asserted rather than described: ten units picked into the
-- carton, `SUM(stock_movement.quantity)` says ten, `fulfilment_line.picked_quantity`
-- says zero. Progress folds `stock_allocation.state`, so a pick recorded in the
-- ledger moves no progress number and nothing compares the two.
--
-- **This is not a new decision.** The Correction to D10 already states the corrected
-- cause set, and `fulfilment_line_id` is the first line of it:
--
--     stock_movement
--       fulfilment_line_id      -- cause
--       goods_receipt_line_id   -- cause
--       discrepancy_id          -- cause
--       CHECK (num_nonnulls(...) <= 1)
--
-- Migration 21 built the inbound arm because D45 needed it and left the rest. D53
-- then recorded the absence as the *reason* progress folds the allocation --
-- "`stock_movement` carries no reference to a fulfilment or an order line, so there
-- is no path from a movement to the commitment it served" -- which is a constraint
-- being reported, not a source being preferred. And mechanism-design retires
-- `package_content` on the sentence "the demand cause lives on the movement that put
-- the stock there, which is where D10 says causes live", which is not yet true of
-- the outbound path. Three passages assume this column. This one adds it.
--
-- The `discrepancy_id` arm is not added and is not missing: migration 6 resolved that
-- cause in the other direction, with `discrepancy.stock_movement_id` and
-- `resolving_movement_id` pointing at the ledger rather than the ledger pointing at
-- the finding. The cause set on this table is two arms, not three.

-- ---------------------------------------------------------------------------
-- 1. The arm
-- ---------------------------------------------------------------------------
--
-- Nullable, because most movements have no demand-side cause at all -- a
-- replenishment, an ad-hoc relocation -- which is the Correction to D10's whole
-- subject and the reason the CHECK below is `<= 1`.
--
-- ADD COLUMN with no default is a catalogue-only change: no table rewrite, which is
-- the thing worth knowing before altering the busiest table in the schema.

ALTER TABLE stock_movement
    ADD COLUMN fulfilment_line_id uuid;

-- Migration 36's convention: every tenant-scoped FK is composite, so a row cannot
-- name a parent belonging to another tenant. `fulfilment_line_tenant_key` is the
-- unique constraint that makes this expressible.
ALTER TABLE stock_movement
    ADD CONSTRAINT stock_movement_fulfilment_line_fk
        FOREIGN KEY (fulfilment_line_id, tenant_id)
        REFERENCES fulfilment_line (id, tenant_id);

-- ---------------------------------------------------------------------------
-- 2. The CHECK stops being vacuous
-- ---------------------------------------------------------------------------
--
-- Migration 21 wrote `CHECK (num_nonnulls(goods_receipt_line_id) <= 1)`. With one
-- argument `num_nonnulls` returns 0 or 1, so the constraint has been **always true**
-- since the day it was written. It has never excluded anything.
--
-- S3 examines it -- `%_cause_ck`, tested for the substring `<= 1` -- and passes,
-- because S3 checks the *form* of the rule and not whether the rule has anything to
-- say. That is D45's own sentence about S3's first outing arriving one table later:
-- *"a vacuous check cannot be wrong out loud."*
--
-- Adding the second arm is what makes it a rule. Nothing about S3 changes; the
-- constraint it has been reading simply starts excluding a row for the first time.
--
-- `<= 1` and not `= 1`, for the third time in this schema and the same reason: an
-- internal move has no demand-side cause and is still a movement. S3 exists to catch
-- the `= 1` version by name, which is how it caught D16-repeats-D10.
--
-- **At most one, not both**, because a cross-dock is two rows and not one. Goods
-- arriving for an order that ships the same day are an arrival movement (no from
-- side, naming the receipt line) and then a pick movement (naming the fulfilment
-- line). D45's arrival fold already counts only the first, by shape, so the two
-- causes never need to sit on one row.

ALTER TABLE stock_movement
    DROP CONSTRAINT stock_movement_cause_ck;

ALTER TABLE stock_movement
    ADD CONSTRAINT stock_movement_cause_ck
        CHECK (num_nonnulls(goods_receipt_line_id, fulfilment_line_id) <= 1);

-- ---------------------------------------------------------------------------
-- 3. The index that makes the fold a batch load
-- ---------------------------------------------------------------------------
--
-- D10's argument 3, and the whole point of a typed FK over a polymorphic pair:
-- `belonging_to` + `grouped_by` batch loading cannot be expressed over
-- `reference_type`/`reference_id`, so the convenient choice would have removed the
-- mechanism principle 6 relies on. Partial, mirroring
-- `stock_movement_receipt_line_idx`, because most movements name no cause -- which is
-- D10's argument 4 and the reason typed columns index well.

CREATE INDEX stock_movement_fulfilment_line_idx
    ON stock_movement (fulfilment_line_id) WHERE fulfilment_line_id IS NOT NULL;

COMMENT ON COLUMN stock_movement.fulfilment_line_id IS
    'FACT. The commitment this movement served, when one did. D10 as corrected, D99. '
    'The key outbound progress groups by; the fold itself is not yet moved off '
    'stock_allocation.state, which is the second half of question 165.';

-- ---------------------------------------------------------------------------
-- 4. The grant
-- ---------------------------------------------------------------------------
--
-- Column-level INSERT, matching migration 21's grant of `goods_receipt_line_id`.
-- Still no UPDATE and no DELETE anywhere on this table: S6 is a grant rather than a
-- convention, and a cause that could be edited afterwards would make the fold
-- rewritable, which is the property the whole outbound argument turns on.

GRANT INSERT (fulfilment_line_id) ON stock_movement TO spork_app;

-- ---------------------------------------------------------------------------
-- 5. What this migration deliberately does not do
-- ---------------------------------------------------------------------------
--
-- `projection_fulfilment_rebuild` is untouched. The four coverage quantities still
-- fold `stock_allocation.state`, J31 still asserts them against it, and the outbound
-- walk still demonstrates the gap.
--
-- The arm is useful on its own -- it settles the first half of 165, it makes the
-- cause CHECK mean something, and it is what the fold will group by -- and moving the
-- fold in the same migration would put a change that needs no discriminator behind a
-- question that does. D99 states the discriminator; the migration that applies it
-- also has to split J31, add the item-agreement finding, and decide what
-- `packed_quantity` reads, none of which this column has to wait for.
