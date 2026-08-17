-- Reverse of 2026-08-04-000038_policy_candidates.
--
-- The candidate set is derived, so nothing is lost but the ability to ask.

DROP FUNCTION IF EXISTS policy_candidate(uuid, policy_kind, uuid, uuid, uuid,
                                         uuid, uuid, uuid, timestamptz);
