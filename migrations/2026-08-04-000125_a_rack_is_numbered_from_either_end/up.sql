-- Migration 125: a rack is numbered from either end of its front (D220).
--
-- A place's labels were read from the front's left: E-01 at the left end as
-- the front is faced, on along to E-18 at the right, and round onto the back.
-- Racks are numbered from whichever end the warehouse chose. Where they start
-- at the right, E-01 is at the front's right end, the labels run leftwards,
-- round the left end, and back along the other side to E-36, which is behind
-- E-01 at the right end.
--
-- Nothing about a cell changes. Its bay is still the column counted from the
-- front's left as the front is faced, so what is behind what, and where a bin
-- stands, are worked out as before. Only which label a column carries turns
-- round.

ALTER TABLE place ADD COLUMN from_right boolean NOT NULL DEFAULT false;

COMMENT ON COLUMN place.from_right IS
    'Its bays numbered from the right end of its front, leftwards, rather than '
    'from the left: the first label is at the right as the front is faced, and '
    'on a rack with two sides the last is behind it. D220.';
