-- Migration 135: an open round thing shows its inside. D241.
--
-- D240 took a round thing's top photograph for its lid and laid it flat
-- across the rim. A bucket with no lid is open: its top photograph looks
-- into it, at its inside wall and its floor, and laid flat it was a picture
-- of a hole painted on a lid.
--
-- Wrapped as open, the camera behind its top photograph is found as its
-- sides' are, and the photograph is read onto its inside wall and its floor
-- where the camera saw them through the opening. Two more pictures of a
-- wrapping: `inside`, the wall unwrapped as its side is, and `floor`, a disc.
-- An open wrapping has no `lid`.

ALTER TABLE round_wrap_picture DROP CONSTRAINT round_wrap_picture_part_ck;
ALTER TABLE round_wrap_picture ADD CONSTRAINT round_wrap_picture_part_ck
    CHECK (part IN ('side', 'lid', 'base', 'inside', 'floor'));

DROP FUNCTION wrap_of(uuid, uuid, uuid, uuid, packaging_level);

-- A subject's newest wrapping, its own only, and its pictures by part.
CREATE FUNCTION wrap_of(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (side text, lid text, base text, inside text, floor text, made_from uuid[])
LANGUAGE sql STABLE AS $$
    SELECT (SELECT p.digest FROM round_wrap_picture p WHERE p.round_wrap_id = w.id AND p.part = 'side'),
           (SELECT p.digest FROM round_wrap_picture p WHERE p.round_wrap_id = w.id AND p.part = 'lid'),
           (SELECT p.digest FROM round_wrap_picture p WHERE p.round_wrap_id = w.id AND p.part = 'base'),
           (SELECT p.digest FROM round_wrap_picture p WHERE p.round_wrap_id = w.id AND p.part = 'inside'),
           (SELECT p.digest FROM round_wrap_picture p WHERE p.round_wrap_id = w.id AND p.part = 'floor'),
           w.made_from
      FROM round_wrap w
     WHERE w.item_id IS NOT DISTINCT FROM p_item
       AND w.item_style_id IS NOT DISTINCT FROM p_style
       AND w.lot_id IS NOT DISTINCT FROM p_lot
       AND w.item_part_id IS NOT DISTINCT FROM p_part
       AND w.packaging_level IS NOT DISTINCT FROM p_level
     ORDER BY w.recorded_at DESC, w.id DESC
     LIMIT 1
$$;

COMMENT ON FUNCTION wrap_of(uuid, uuid, uuid, uuid, packaging_level) IS
    'A subject''s newest wrapping in its photographs, open or lidded. D240, D241.';
