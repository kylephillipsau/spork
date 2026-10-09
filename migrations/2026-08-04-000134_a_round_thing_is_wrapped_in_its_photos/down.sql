-- Migration 134 down: a round thing is shown by its photographs, unwrapped.

DROP FUNCTION wrap_of(uuid, uuid, uuid, uuid, packaging_level);
DROP TABLE round_wrap_picture;
DROP TABLE round_wrap;
