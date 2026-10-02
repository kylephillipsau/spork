-- Migration 111 down: nothing says what it is packed in.

DROP FUNCTION packed_in(uuid, uuid, uuid, uuid, packaging_level);
DROP TABLE subject_packaging;
DROP TABLE packaging_type;
