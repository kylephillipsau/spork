-- Migration 11: the same question on the order side, and it was answering wrongly.
--
-- D47 split a movement by whether the world changed or the record was wrong.
-- Question 130 asked whether `intention_amendment` needs the same split. It does,
-- and the argument is stronger here, because the current design does not merely
-- lose history. It computes the wrong current value.
--
-- The order is placed at 01:00 promising Thursday, which is a typo: the customer
-- said Friday. At 02:00 they genuinely move it to Saturday. At 03:00 the typo is
-- found. Today the only expressible fix is an amendment stamped 03:00, and since
-- J46 folds in (occurred_at, recorded_at, id) order that amendment sorts last and
-- wins. The order now promises Friday. The customer is expecting Saturday, and
-- nothing anywhere reports a problem.
--
-- Carrying the corrected moment's occurred_at instead puts the fix at 01:00,
-- where the fold reads it before the customer's 02:00 change and the answer comes
-- out Saturday. So the ordering key D24 chose was already right; what was missing
-- was any way to say that an amendment is about an earlier moment than the one it
-- was written in.
--
-- This is why the split cannot be left to convention. Both amendments are legal
-- rows with legal timestamps, and the difference between them decides what the
-- warehouse ships.

-- ---------------------------------------------------------------------------
-- One question, one type
-- ---------------------------------------------------------------------------
--
-- Migration 10 named this `adjustment_class`, which is movement vocabulary for a
-- distinction that is not about movements. The question "was the world different,
-- or was the record wrong" is the same question wherever a fact can be revised,
-- and it has now come up twice in two migrations. Two enums with identical values
-- and different names is the near-duplicate the record keeps refusing.
--
-- Renamed rather than edited in place: migration 10 is committed and its up/down
-- pair is verified, so changing it would make the history describe something that
-- no longer exists.

ALTER TYPE adjustment_class RENAME TO revision_class;

-- ---------------------------------------------------------------------------
-- The class on an amendment
-- ---------------------------------------------------------------------------
--
-- Unlike a movement, an amendment has no single row to point at. The value it
-- revises is the previous amendment, or the order's own original, so there is
-- nothing for a `reverses_` foreign key to name. The discriminator is therefore
-- an explicit column, and it is NOT NULL because an amendment whose class is
-- unknown is exactly the ambiguity this removes.

ALTER TABLE intention_amendment
    ADD COLUMN revision_class revision_class NOT NULL DEFAULT 'world_event';

-- The default exists to backfill and then goes, so every future writer states
-- which kind of amendment it is making rather than inheriting the common case.
ALTER TABLE intention_amendment
    ALTER COLUMN revision_class DROP DEFAULT;

-- D47 required a reason on a correction and this is the same claim: that an
-- earlier record was wrong. `reason` is nullable for ordinary amendments, where
-- the state change usually speaks for itself.
ALTER TABLE intention_amendment
    ADD CONSTRAINT intention_amendment_correction_has_reason_ck
        CHECK (revision_class <> 'record_error' OR reason IS NOT NULL);

CREATE INDEX intention_amendment_correction_idx
    ON intention_amendment (tenant_id, order_id)
    WHERE revision_class = 'record_error';

-- ---------------------------------------------------------------------------
-- What this does to J44
-- ---------------------------------------------------------------------------
--
-- J44 says no externally-authoritative order is amended locally, which follows
-- from D39: one system of record per order, and an order whose authority is
-- external is amended through that system. D44 added succession for the same
-- reason, because Metcash cancels and re-raises rather than sending a change.
--
-- That rule was written before this split existed and is now too wide by exactly
-- one case. If we mis-parse a quantity out of an inbound ORDERS message, the
-- counterparty's document is not wrong and their intention has not changed. Our
-- transcription of it is wrong. Forbidding the fix leaves knowingly wrong data in
-- place and offers no alternative, since succession is the counterparty's
-- mechanism and we cannot cancel their purchase order to fix our own parse bug.
--
-- So J44 narrows to `world_event`: a local amendment claiming the intention
-- changed is still forbidden on an externally-authoritative order, because that
-- is the bidirectional merge D39 refused outright. A `record_error` amendment is
-- permitted, because it claims only that we copied their document down wrong.
--
-- The register carries the narrowed statement. Nothing is enforced here, because
-- which `source_channel` values are externally authoritative is not declared
-- anywhere yet, which is question 132.
