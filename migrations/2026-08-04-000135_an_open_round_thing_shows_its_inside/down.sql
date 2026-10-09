-- Migration 135 down: a wrapping has a side, a lid and a base; an open one's
-- inside is let go.

DROP FUNCTION wrap_of(uuid, uuid, uuid, uuid, packaging_level);
DELETE FROM round_wrap_picture WHERE part IN ('inside', 'floor');
ALTER TABLE round_wrap_picture DROP CONSTRAINT round_wrap_picture_part_ck;
ALTER TABLE round_wrap_picture ADD CONSTRAINT round_wrap_picture_part_ck CHECK (part IN ('side', 'lid', 'base'));

CREATE FUNCTION wrap_of(p_item uuid, p_style uuid, p_lot uuid, p_part uuid, p_level packaging_level)
RETURNS TABLE (side text, lid text, base text, made_from uuid[])
LANGUAGE sql STABLE AS $$
    SELECT (SELECT p.digest FROM round_wrap_picture p WHERE p.round_wrap_id = w.id AND p.part = 'side'),
           (SELECT p.digest FROM round_wrap_picture p WHERE p.round_wrap_id = w.id AND p.part = 'lid'),
           (SELECT p.digest FROM round_wrap_picture p WHERE p.round_wrap_id = w.id AND p.part = 'base'),
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
    'A subject''s newest wrapping in its photographs. D240.';
