-- Reverse of 2026-08-04-000039_policy_values.
--
-- Derived, so nothing is lost but the ability to read a winner's value without
-- knowing which table it lives in.

DROP FUNCTION IF EXISTS policy_value(policy_kind, uuid[], timestamptz);
