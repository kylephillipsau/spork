-- Migration 28: two actor indexes, once the instrument could read them.
--
-- D69, settling question 147. D66 measured every index candidate except the actor
-- columns and could not measure those: `history.sql` had three people, so one of
-- them matched thirty-eight per cent of the ledger and the number described the
-- fixture rather than the domain. 147 said the instrument had to change first,
-- and it has: forty people over the year with turnover, five of whom leave.
--
--   distinct actors on the ledger   40
--   rows per person                 min 1,170   median 7,610   max 26,280
--   median selectivity              2.63%, against 38% before
--
-- | query | rows | unindexed | indexed |
-- |---|---|---|---|
-- | everything one person recorded | 7,610 of 289,083 | 18.28 ms | 1.94 ms |
-- | one person, one day            | 20               | 52.47 ms | **0.031 ms** |
-- | their work sessions            | —                |  1.90 ms | 0.074 ms |
--
-- **The day-scoped one is the shape that matters and the one that was worst.**
-- "What did this person do on that shift" is the question D11 split the operator,
-- the workers and the accountable to make answerable, and the architecture's own
-- argument for naming a person rather than a crew: *"nobody can follow up a
-- question with 'Casual Melbourne'."* Unindexed it costs fifty-two milliseconds
-- and scans the whole ledger to return twenty rows.

-- ---------------------------------------------------------------------------
-- One composite, not two indexes
-- ---------------------------------------------------------------------------
--
-- Equality on the actor, range on the moment, in that order. Measured with the
-- composite alone it serves both shapes — 1.94 ms for the whole-year question and
-- 0.031 ms for the day — so the plain `(recorded_by_id)` index it would otherwise
-- duplicate is not taken. That is 9.2 MB rather than 11.3 MB, and one index to
-- maintain on the hottest table rather than two.

CREATE INDEX stock_movement_actor_idx
    ON stock_movement (recorded_by_id, occurred_at);

-- Cheap and on a small table: the work-session lookup, which is also what
-- question 133 will want when it comes to tell a self-caught slip from a finding.
CREATE INDEX client_event_actor_idx ON client_event (recorded_by_id);

-- ---------------------------------------------------------------------------
-- The other actor columns, and why each is not here
-- ---------------------------------------------------------------------------
--
-- **`stock_movement.authorised_by_id`: zero non-null rows in a year.** That is
-- not a gap in the fixture, it is D11's shape — the accountable is named on the
-- exceptional movement, not the ordinary one, so the column is sparse by design.
-- There is nothing to measure and a partial index over an empty set would be a
-- guess dressed as a decision.
--
-- **`goods_receipt_line.accepted_by_id`: fully populated and not selective.**
-- Three receivers cover eight deliveries a day, so one of them matches a third of
-- the table. **That is a property of receiving rather than of the fixture**:
-- goods inward is done by few people because there are few deliveries, and 147's
-- criticism does not apply to a number that is small for a real reason.
--
-- **`discrepancy.detected_by_id`.** The table holds one row after a year. The
-- findings queue is small by design — that is D8 working, not a population
-- waiting to grow.
--
-- The rest are the validation foreign keys D66 already dismissed.
