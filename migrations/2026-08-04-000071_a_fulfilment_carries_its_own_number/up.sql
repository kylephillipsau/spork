-- Migration 71: a fulfilment carries its own number.
--
-- Every screen so far reaches a fulfilment through the order that raised it, so
-- the only string on the pack bench is the order's confirmation number. That is
-- not the number the work is called by. **A packer is handed an item
-- fulfilment** -- `IF400187` -- and D15 already says why: a commitment to ship
-- part of an order is its own thing, with its own site, its own lines and its
-- own life. A thing with its own life has its own name, and this one has been
-- borrowing the order's.
--
-- Borrowing it does not even identify the row. Migration 9 raised two
-- fulfilments against one order line precisely to make D15's point, and the
-- fixture now carries three under `S260041`. Quoting that number names all
-- three, which is the difference between a reference and a filter.
--
-- Nullable, because a fulfilment raised here rather than received from a channel
-- may not have one yet, and the state where the row exists and the number has
-- not been assigned is real rather than an error to be defaulted away.
--
-- **Not unique, for D44's reason and migration 68's.** An item fulfilment
-- cancelled upstream and reraised arrives carrying the same number, and refusing
-- the second is refusing to record something that happened.

ALTER TABLE fulfilment ADD COLUMN reference text;

COMMENT ON COLUMN fulfilment.reference IS
    'The number the work is called by -- an item fulfilment number such as '
    'IF400187. Distinct from "order".confirmation_number, which names the order '
    'and therefore names every fulfilment raised against it. Nullable: a '
    'fulfilment can exist before it is numbered. Migration 71.';

CREATE INDEX fulfilment_reference_idx
    ON fulfilment (tenant_id, reference)
    WHERE reference IS NOT NULL;

-- **A column with no grant is a column nothing can ever fill.** `fulfilment` is
-- granted column by column rather than table-wide, because migration 9 would
-- otherwise have handed the application the projections the maintainer owns. The
-- consequence is that every column added afterwards is invisible to the writer
-- until it is named here, and S45 exists because that failure is silent: the
-- insert succeeds, the column stays null, and nothing says why.
--
-- Updatable as well as insertable. A number assigned upstream after the
-- fulfilment reached us has to be able to land on the row that is already here.
GRANT INSERT (reference), UPDATE (reference) ON fulfilment TO spork_app;

COMMENT ON INDEX fulfilment_reference_idx IS
    'What the pack queue is searched by. Not unique -- D44 makes '
    'cancel-and-reraise the amendment channel, so one number can name a '
    'fulfilment and its successor. Migration 71.';
