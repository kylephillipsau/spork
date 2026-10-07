-- Migration 127: one rule for the cut a photograph shows. D183, amended.
--
-- A side said to look like another (migration 105) is a photograph row of its
-- own that points at the photograph taken, and its cut is that photograph's.
-- Each read wrote that lookup for itself. The item page followed the pointer;
-- the pack bench and the list picture looked for a cut of the row itself, so
-- a carton printed alike front and back was drawn in the packing plan with a
-- plain back, a plain left and a plain bottom, while its page showed all six.
--
-- So the lookup is one function every read joins. Plain SQL, STABLE and not
-- STRICT, so the planner inlines it where it is joined: a read costs what the
-- lookup it replaces did, one probe of the cut's index per photograph.

CREATE FUNCTION cut_of(p_image uuid)
RETURNS TABLE (digest text, corners double precision[])
LANGUAGE sql STABLE AS $$
    SELECT c.digest, c.corners
      FROM observation_image oi
      JOIN observation_image_cut c ON c.observation_image_id = coalesce(oi.same_as_id, oi.id)
     WHERE oi.id = p_image
     ORDER BY c.recorded_at DESC, c.id DESC
     LIMIT 1
$$;

COMMENT ON FUNCTION cut_of(uuid) IS
    'The cut a photograph shows: its own newest, or, for a side said to look like '
    'another, the newest of the photograph it looks like. D176, D183.';
