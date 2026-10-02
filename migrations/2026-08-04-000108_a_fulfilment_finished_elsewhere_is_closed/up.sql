-- Migration 108: a fulfilment finished in the other system is closed. D187.
--
-- An item fulfilment picked on the handheld reaches Spork as work to pack
-- (D172). Packed and shipped there instead, or deleted, it went on being work
-- here: the bridge sends only what is Picked, and something no longer Picked
-- simply stopped arriving. So Spork's queues filled with orders long gone.
--
-- The bridge now asks which fulfilments Spork still has open, reads each one's
-- status in the other system, and says when one has moved on. Spork records
-- it here, and an open fulfilment is one with nothing recorded.
--
-- # Closed, not cancelled
--
-- Cancelled is "this is not going ahead". These went ahead, somewhere else,
-- so they say how: packed, shipped, or gone (deleted there). Nothing in the
-- ledger moves: Spork did not pack them. An item fulfilment sent back to
-- Picked opens again.

ALTER TABLE fulfilment
    ADD COLUMN closed_elsewhere text,
    ADD COLUMN closed_elsewhere_at timestamptz,
    ADD CONSTRAINT fulfilment_closed_elsewhere_ck
        CHECK (closed_elsewhere IS NULL OR closed_elsewhere IN ('packed', 'shipped', 'gone')),
    ADD CONSTRAINT fulfilment_closed_elsewhere_pair_ck
        CHECK ((closed_elsewhere IS NULL) = (closed_elsewhere_at IS NULL));

COMMENT ON COLUMN fulfilment.closed_elsewhere IS
    'Finished in the system it came from, as that system last said: packed, '
    'shipped, or gone (deleted there). Not work here while set. Migration 108, D187.';

GRANT UPDATE (closed_elsewhere, closed_elsewhere_at) ON fulfilment TO spork_app;
