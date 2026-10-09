-- Migration 133 down: a thing is a box when its packaging type has six sides.

DROP FUNCTION is_box(uuid, uuid, uuid, uuid, packaging_level);
DROP TABLE subject_shape;
