-- Migration 68 down: an order goes back to being findable only by uuid.

DROP INDEX IF EXISTS order_confirmation_number_idx;
DROP INDEX IF EXISTS order_external_ref_idx;
