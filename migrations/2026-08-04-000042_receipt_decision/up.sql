-- Migration 42: the receipt names the policy that governed it.
--
-- D87. J63 has been pending since D76: *"every act a policy governed names the
-- version that governed it, so an audit reads the decision rather than
-- reconstructing it."*
--
-- It is the invariant that makes D76's central claim true rather than
-- aspirational. 148 asked whether the closures should become temporal so a past
-- resolution could be replayed, and D76 answered that **the audit is served by
-- recording the decision, not by replaying the world** -- while
-- `goods_receipt_line` went on recording `accepted_at` and `accepted_by_id` and
-- nothing about which `receiving_policy` version governed the acceptance. So
-- *"why was this lot accepted"* stayed unanswerable, which is the question 148
-- was triggered by in the first place.
--
-- The pattern is already here twice over: `stock_movement.item_packing_config_id`
-- names the version that converted it (D57, D58), and
-- `expected_supply.receiving_policy_id` names the one that shaped the promise.
-- This is the missing third.

-- ---------------------------------------------------------------------------
-- 1. The column, named the way S16 requires
-- ---------------------------------------------------------------------------
--
-- D76 wrote the blocker as `decided_by_receiving_policy_id`, and that name would
-- have failed S16: it derives the value table by trimming `_policy_id`, so it
-- would look for a `decided_by_receiving_policy` table and not find one. The
-- convention is already set by `expected_supply.receiving_policy_id`, and a
-- second spelling for the same relationship is how a rule becomes unenforceable.
-- **The blocker's name was mine and it was wrong; the invariant's statement was
-- right.**

ALTER TABLE goods_receipt_line
    ADD COLUMN receiving_policy_id uuid REFERENCES receiving_policy(id);

COMMENT ON COLUMN goods_receipt_line.receiving_policy_id IS
    'The receiving policy version that governed this line''s disposition -- the '
    'tolerances it was measured against and whether a lot was required. Recorded '
    'because it cannot be reconstructed: the binding may be superseded, its '
    'effective range closed, and the taxonomy it resolved through re-parented, '
    'and each of those is legitimate. D76, D87.';

CREATE INDEX goods_receipt_line_receiving_policy_idx
    ON goods_receipt_line (receiving_policy_id)
    WHERE receiving_policy_id IS NOT NULL;

GRANT INSERT (receiving_policy_id), UPDATE (receiving_policy_id)
    ON goods_receipt_line TO nylonite_app;

-- ---------------------------------------------------------------------------
-- 2. What is not enforced here
-- ---------------------------------------------------------------------------
--
-- **Not NOT NULL.** A line dispositioned before this migration legitimately has
-- none, and backfilling one would invent evidence of a decision nobody recorded
-- -- the objection D73 raised against backfilling a blast radius and D76 against
-- backfilling a class origin. J63 reports them instead, so the set is visible and
-- shrinking rather than silently absent.
--
-- **Not frozen.** Once a receipt has been compared against a policy version, that
-- reference should not move; nothing records when it changed, so the rule can be
-- stated and not checked. It is the same gap question 152 carries for a
-- `resolved_*` annotation, and it is the same gap for the same reason.
--
-- **Not written by anything yet.** No receiving path resolves a policy and stamps
-- it, because D62 built the path before there was a resolver to call. J63 is what
-- will notice the first line that is dispositioned without one.
