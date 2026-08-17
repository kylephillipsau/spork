-- Migration 68: an order is found by what a person quotes.
--
-- Stage 1 of the recorded process is one sentence: *"Enter the confirmation
-- number into the search bar."* Every read this system has takes a uuid, which
-- nobody on a dock has. `order.confirmation_number` and `order.external_ref`
-- have both existed since migration 9 and neither has an index, because until
-- now nothing looked an order up by either.
--
-- Two partial indexes rather than one composite. They are alternatives, not a
-- pair: a locally-raised order has a confirmation number and no external
-- reference, and an EDI order is the other way round -- D39 puts exactly one
-- system of record on each order and `source_channel` records which. Indexing
-- them together would build one structure over two populations that never
-- overlap.
--
-- Partial because the columns are nullable and a null is not something anybody
-- searches for; `WHERE ... IS NOT NULL` keeps the index to the rows a search can
-- actually find.
--
-- **Not unique, deliberately.** A confirmation number is the counterparty's
-- word, and D44 records that an externally-authoritative order is amended by
-- cancel-and-reraise -- so the same number legitimately reaches us twice, once
-- on the cancelled order and once on its successor, joined by
-- `supersedes_order_id`. A unique index would refuse the second, which is
-- refusing to record something that happened.

CREATE INDEX order_confirmation_number_idx
    ON "order" (tenant_id, confirmation_number)
    WHERE confirmation_number IS NOT NULL;

CREATE INDEX order_external_ref_idx
    ON "order" (tenant_id, external_ref)
    WHERE external_ref IS NOT NULL;

COMMENT ON INDEX order_confirmation_number_idx IS
    'Stage 1 of the pack process: the number a customer quotes on the phone. Not '
    'unique -- D44 makes cancel-and-reraise the amendment channel for an '
    'externally-authoritative order, so one number can name an order and its '
    'successor. Migration 68.';
