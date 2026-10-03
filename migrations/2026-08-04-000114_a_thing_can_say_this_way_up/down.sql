-- Migration 114 down: anything goes in any way up.

DROP FUNCTION keeps_upright(uuid, uuid, uuid, uuid, packaging_level);
DROP TABLE subject_upright;
