-- Reverse of 2026-08-04-000011_revision_class.

DROP INDEX IF EXISTS intention_amendment_correction_idx;

ALTER TABLE intention_amendment
    DROP CONSTRAINT IF EXISTS intention_amendment_correction_has_reason_ck;

ALTER TABLE intention_amendment
    DROP COLUMN IF EXISTS revision_class;

ALTER TYPE revision_class RENAME TO adjustment_class;
