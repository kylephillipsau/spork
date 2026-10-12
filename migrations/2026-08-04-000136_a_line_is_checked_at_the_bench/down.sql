-- Migration 136 down: lines go unchecked, and a weight is a gap with no verdict.

ALTER TABLE site DROP COLUMN weight_tolerance_per_mille;
DROP TABLE pack_check;
