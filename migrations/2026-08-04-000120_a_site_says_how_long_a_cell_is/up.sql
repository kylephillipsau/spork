-- A site says how long one of its layout's cells is (D210, docs/picking-plan.md
-- Proposal B).
--
-- D173 drew places relative to each other and never measured them. Routes need
-- distances, and the user is building the floor from tape measurements, so a
-- site gains a scale: one number, how many millimetres a cell is. Nothing
-- stored changes meaning. Positions stay in cells, and the screens show
-- metres once the scale is set. Unset, the layout is drawn as before, not to
-- scale.
--
-- One number per site is enough because a rack's size is its own measurement,
-- not its bay count. A rack of eighteen bays is as long as it is measured to
-- be, and its bays share that length.

ALTER TABLE site ADD COLUMN cell_mm integer;
ALTER TABLE site ADD CONSTRAINT site_cell_mm_ck CHECK (cell_mm IS NULL OR cell_mm > 0);

COMMENT ON COLUMN site.cell_mm IS
  'D210: how many millimetres one layout cell is; null while the layout is not to scale.';
