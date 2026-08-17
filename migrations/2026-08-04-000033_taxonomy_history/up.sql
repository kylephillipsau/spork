-- Migration 33: the taxonomy could not be replayed, and the audit does not want a replay.
--
-- D76, settling question 148:
--
--   "Whether the closures become temporal. D72 makes a re-parent a dated fact, so
--    `policy_change` now knows the taxonomy's shape at any past instant -- but
--    `item_class_closure` and `party_class_closure` hold only the present one, and
--    every resolution walks the closure. A resolution replayed for an audit
--    therefore answers with today's taxonomy [...] The cost is not small -- a
--    temporal closure is a range per edge, and D22's resolver walks it on every
--    lookup."
--
-- Four of that question's premises were checked before anything was built, and
-- the first one is false.

-- ---------------------------------------------------------------------------
-- 1. `policy_change` did not know the shape at any past instant
-- ---------------------------------------------------------------------------
--
-- A `reparented` row states where a class went. **Nothing states where it was.**
-- D74 declined `from_parent_id` on the reasoning that the previous parent is the
-- previous act, "or the class's original parent if there is none" -- and the
-- original parent is precisely what D72 turned into a projection and overwrote.
--
-- Measured on the fixture before this migration: GLOVE_SUPPLIER was created at
-- the root and moved under SUPPLIER at 00:12, and **its parent at 00:11 was
-- unrecoverable**. One act, naming a destination, over a base that no longer
-- existed.
--
-- So a temporal closure would have been a precise index over a history with a
-- hole at the beginning of every class's life. **The first thing 148 needs is not
-- ranges on the closure. It is a base state.**
--
-- D22 asked for one and it was never built. Its sketch listed four actions --
-- `created | reparented | renamed | retired` -- and D72 built `reparented`, D74
-- built `retired` and declined `renamed`. This is the fourth, and with it the
-- sketch is complete.
--
-- A column would have been cheaper and is the wrong shape: creating a class is a
-- governance act with an author, a moment and a reason, and `item_class` has no
-- timestamp of any kind to hang the other two off.

ALTER TABLE policy_change
    DROP CONSTRAINT policy_change_parent_only_on_reparent_ck;

ALTER TABLE policy_change
    ADD CONSTRAINT policy_change_parent_only_on_placement_ck
        CHECK ((new_item_parent_id IS NULL AND new_party_parent_id IS NULL)
               OR change_kind IN ('created', 'reparented'));

COMMENT ON CONSTRAINT policy_change_parent_only_on_placement_ck ON policy_change IS
    'A destination belongs to an act that places the class somewhere: its creation '
    'or its move. NULL is a real destination meaning the root, which is why the '
    'fold takes it verbatim. D72, D76.';

-- ---------------------------------------------------------------------------
-- 2. The fold takes the creation as its base
-- ---------------------------------------------------------------------------
--
-- One line changes: the parentage fold reads `created` as well as `reparented`.
-- The register order already resolves them -- a creation is the earliest act a
-- class has, so `DISTINCT ON ... ORDER BY occurred_at DESC` picks it only when
-- nothing has moved the class since.

CREATE OR REPLACE FUNCTION projection_taxonomy_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
    n bigint;
    total bigint := 0;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH folded AS (
        SELECT DISTINCT ON (item_class_id)
               item_class_id, new_item_parent_id AS new_parent_id
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('created', 'reparented')
           AND item_class_id IS NOT NULL
         ORDER BY item_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE item_class c
           SET parent_id = f.new_parent_id
          FROM folded f
         WHERE c.id = f.item_class_id AND c.tenant_id = p_tenant
           AND c.parent_id IS DISTINCT FROM f.new_parent_id
        RETURNING c.id)
    SELECT count(*) INTO touched FROM updated;
    total := total + touched;

    WITH folded AS (
        SELECT DISTINCT ON (party_class_id)
               party_class_id, new_party_parent_id AS new_parent_id
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('created', 'reparented')
           AND party_class_id IS NOT NULL
         ORDER BY party_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE party_class c
           SET parent_id = f.new_parent_id
          FROM folded f
         WHERE c.id = f.party_class_id AND c.tenant_id = p_tenant
           AND c.parent_id IS DISTINCT FROM f.new_parent_id
        RETURNING c.id)
    SELECT count(*) INTO n FROM updated;
    total := total + n;

    WITH folded AS (
        SELECT DISTINCT ON (item_class_id)
               item_class_id,
               CASE WHEN change_kind = 'retired' THEN occurred_at END AS retired_at
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('retired', 'reinstated')
           AND item_class_id IS NOT NULL
         ORDER BY item_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE item_class c
           SET retired_at = f.retired_at
          FROM folded f
         WHERE c.id = f.item_class_id AND c.tenant_id = p_tenant
           AND c.retired_at IS DISTINCT FROM f.retired_at
        RETURNING c.id)
    SELECT count(*) INTO n FROM updated;
    total := total + n;

    WITH folded AS (
        SELECT DISTINCT ON (party_class_id)
               party_class_id,
               CASE WHEN change_kind = 'retired' THEN occurred_at END AS retired_at
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('retired', 'reinstated')
           AND party_class_id IS NOT NULL
         ORDER BY party_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    updated AS (
        UPDATE party_class c
           SET retired_at = f.retired_at
          FROM folded f
         WHERE c.id = f.party_class_id AND c.tenant_id = p_tenant
           AND c.retired_at IS DISTINCT FROM f.retired_at
        RETURNING c.id)
    SELECT count(*) INTO n FROM updated;
    total := total + n;

    RETURN total;
END
$$;

ALTER FUNCTION projection_taxonomy_rebuild(uuid) OWNER TO nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- 3. The shape at a past instant, derived rather than stored
-- ---------------------------------------------------------------------------
--
-- With a base state the closure at any instant is a fold and a walk, so **no
-- range, no maintainer, and nothing on the floor's path.** The cost is paid by
-- the audit that asks, which is the only caller there is.
--
-- **Both time axes, and both are mandatory**, because question 84 is about
-- exactly this trap: *"'what did the pallet weigh on Monday' and 'what did we
-- believe on Monday' differ by one predicate, and getting it wrong in a dispute
-- is worse than not having the capability."* A single-timestamp function would
-- have to pick one silently. This one cannot be called without saying which
-- question is being asked:
--
--   p_valid_at   the shape as it was          (occurred_at)
--   p_known_at   according to what was known  (recorded_at), defaulting to now
--
-- Classes with no act at or before the instant do not appear, which is correct
-- twice over: a class created later did not exist, and a class whose origin was
-- never recorded cannot be placed. J62 reports the second so it is a shrinking
-- set rather than a silent one.

CREATE FUNCTION item_class_closure_as_at(
        p_tenant uuid, p_valid_at timestamptz, p_known_at timestamptz DEFAULT now())
    RETURNS TABLE (ancestor_id uuid, descendant_id uuid, depth integer)
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    WITH RECURSIVE placed AS (
        SELECT DISTINCT ON (item_class_id)
               item_class_id AS id, new_item_parent_id AS parent_id
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('created', 'reparented')
           AND item_class_id IS NOT NULL
           AND occurred_at <= p_valid_at
           AND recorded_at <= p_known_at
         ORDER BY item_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    walk AS (
        SELECT p.id AS ancestor_id, p.id AS descendant_id, 0 AS depth FROM placed p
         UNION ALL
        SELECT w.ancestor_id, c.id, w.depth + 1
          FROM walk w JOIN placed c ON c.parent_id = w.descendant_id
    ) CYCLE descendant_id SET looped USING trail
    SELECT ancestor_id, descendant_id, depth FROM walk WHERE NOT looped
$$;

CREATE FUNCTION party_class_closure_as_at(
        p_tenant uuid, p_valid_at timestamptz, p_known_at timestamptz DEFAULT now())
    RETURNS TABLE (ancestor_id uuid, descendant_id uuid, depth integer)
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    WITH RECURSIVE placed AS (
        SELECT DISTINCT ON (party_class_id)
               party_class_id AS id, new_party_parent_id AS parent_id
          FROM policy_change
         WHERE tenant_id = p_tenant
           AND change_kind IN ('created', 'reparented')
           AND party_class_id IS NOT NULL
           AND occurred_at <= p_valid_at
           AND recorded_at <= p_known_at
         ORDER BY party_class_id, occurred_at DESC, recorded_at DESC, id DESC
    ),
    walk AS (
        SELECT p.id AS ancestor_id, p.id AS descendant_id, 0 AS depth FROM placed p
         UNION ALL
        SELECT w.ancestor_id, c.id, w.depth + 1
          FROM walk w JOIN placed c ON c.parent_id = w.descendant_id
    ) CYCLE descendant_id SET looped USING trail
    SELECT ancestor_id, descendant_id, depth FROM walk WHERE NOT looped
$$;

COMMENT ON FUNCTION item_class_closure_as_at(uuid, timestamptz, timestamptz) IS
    'The Product taxonomy as it stood at p_valid_at, according to what was '
    'recorded by p_known_at. Both axes are named because question 84 is about '
    'confusing them. Derived from the acts, so there is no temporal closure table '
    'to maintain and nothing on the resolver''s path. D76.';
COMMENT ON FUNCTION party_class_closure_as_at(uuid, timestamptz, timestamptz) IS
    'The same for the Counterparty taxonomy. D76.';

GRANT EXECUTE ON FUNCTION
    item_class_closure_as_at(uuid, timestamptz, timestamptz),
    party_class_closure_as_at(uuid, timestamptz, timestamptz)
    TO nylonite_app, nylonite_platform, nylonite_scheduler, nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- 4. What is deliberately not built, and the trigger for revisiting it
-- ---------------------------------------------------------------------------
--
-- **No temporal closure table.** 148 asks for a range per edge. Three of its
-- other premises argue against building one now:
--
--   * **There is no resolver.** D70 says so in as many words. 148 prices the
--     change as "the resolver walks it on every lookup", which is a retrofit cost
--     against code that does not exist -- and it is the reason to settle the shape
--     now rather than the reason to build storage now.
--   * **The scale is unmeasured.** `history.sql` builds a year of movements and
--     no taxonomy at all. D65, D69 and D71 each refused to answer a cost question
--     without an instrument; this one has none, so any number here would be
--     invented.
--   * **It would not answer the question 148 is triggered by.** "The first audit
--     that asks why a lot was accepted" -- `goods_receipt_line` records
--     `accepted_at` and `accepted_by_id` and nothing about which policy version
--     governed the decision. A replay would *reconstruct* an answer; it would not
--     *report* one, and a reconstruction is a claim about the past rather than
--     evidence of it.
--
-- **The audit is served by recording the decision, not by rebuilding the world.**
-- That is already how this schema answers the same question elsewhere:
-- `stock_movement.item_packing_config_id` names the version that converted it
-- (D57, D58), `goods_receipt_line.item_packing_config_id` the same, and D73 froze
-- a count precisely because it could not be recomputed later.
--
-- So the commitment is J63, and it is `Pending` rather than absent: **every act a
-- policy governed names the version that governed it.** It blocks on a column
-- that does not exist yet, S42 asserts that the column really is absent, and the
-- day a resolver adds it the invariant wakes up on its own. The stamp arrives
-- with the resolver, not before it -- which is the position D70 took when it
-- declined to invent a cache for a resolver that does not exist.
--
-- The materialised temporal closure is revisited when there is a resolver, and a
-- measurement showing the reconstruction above is too slow for something that
-- actually calls it.
