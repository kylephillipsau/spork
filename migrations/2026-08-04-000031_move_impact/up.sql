-- Migration 31: the blast radius counts bindings, because resolutions are unbounded.
--
-- D73, settling question 95, raised by D23 on 2026-08-01:
--
--   "`affected_resolution_count` is computed before a taxonomy move -- against
--    what? Active bindings is cheap; actual future resolutions is unbounded. It
--    needs a defined denominator or the number is theatre."
--
-- D72 built the number two days ago and did not answer this. It shipped
-- `item_class_move_impact` and `party_class_move_impact` with a definition that
-- is *nearly* right, and the gap is the one D22 warned about in the same breath
-- as it asked for the number.

-- ---------------------------------------------------------------------------
-- 1. What D72 counted, and what it missed
-- ---------------------------------------------------------------------------
--
-- D72 counted the symmetric difference of the old and new ancestor chains: the
-- classes a subtree stops inheriting from and the ones it starts inheriting from.
-- Bindings scoped there start or stop matching, so they plainly change.
--
-- **But D22's failure mode is subtler than membership, and it says so:**
--
--   "Membership changes are intuitive -- move a class out of `dairy` and dairy's
--    rules stop applying. But cross-dimension flips are possible: a binding at
--    Product-3/Space-0 beats one at Product-2/Space-2; re-parent so the first is
--    depth 2 and the second now wins, with nothing about either binding changed."
--
-- Resolution orders matches by **depth vector, compared lexicographically**. So a
-- binding can keep matching, on a class that is still an ancestor, and still
-- change the outcome -- because the class sits at a different *depth* than it did.
--
--     A > B > X          move X directly under A
--     A > X
--
-- A is an ancestor before and after. D72's symmetric difference reports **zero**.
-- But A's depth relative to X went from 2 to 1, so every binding scoped to A
-- moved one place up the Product axis and may now outrank a Space binding it
-- previously lost to. **A number that reports zero for that is exactly the
-- theatre question 95 named.**

-- ---------------------------------------------------------------------------
-- 2. The denominator, stated once
-- ---------------------------------------------------------------------------
--
-- **Active bindings whose depth vector for the moved subtree changes.**
--
-- Three words, each load-bearing:
--
--   *bindings*, not resolutions. A resolution is per item, per kind, per instant,
--   per request node -- 95 is right that it is unbounded, and it is the wrong
--   unit anyway. Bindings are finite, countable, and every resolution that
--   changes must involve at least one binding whose vector changed. **Counting
--   the cause rather than the effect is what makes the number finite and true.**
--
--   *active*: it has a value version whose `effective` range covers the moment
--   asked, and no other binding supersedes it. D22 makes scope immutable and a
--   change a new binding superseding the old, so a superseded row is history and
--   counting it inflates the warning.
--
--   *depth vector changes*: gained an ancestor, lost one, **or kept one at a
--   different depth**. The third is what D72 missed.
--
-- The depth change is the same for every member of the subtree, which is what
-- makes this computable at the class rather than per item: a member sitting k
-- below the moved class sees every ancestor at k + its depth from the class, and
-- k cancels out of the comparison.
--
-- Not counted, deliberately: a binding naming an `item_id` or a `party_id`
-- directly. It sits at the most specific node of its dimension and no re-parenting
-- moves it. D72 counted these and was wrong to -- the same error in the other
-- direction, inflating rather than understating.

DROP FUNCTION IF EXISTS item_class_move_impact(uuid, uuid);
DROP FUNCTION IF EXISTS party_class_move_impact(uuid, uuid);

-- The bindings themselves, because a count nobody can expand is a number to
-- click past. The screen shows N and lists them on demand from one function.
CREATE FUNCTION item_class_move_affected(
        p_class_id uuid, p_new_parent_id uuid, p_at timestamptz DEFAULT now())
    RETURNS SETOF uuid
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    WITH old_chain AS (
        SELECT ancestor_id AS class_id, depth AS d
          FROM item_class_closure WHERE descendant_id = p_class_id),
    new_chain AS (
        SELECT p_class_id AS class_id, 0 AS d
         UNION ALL
        SELECT ancestor_id, depth + 1
          FROM item_class_closure WHERE descendant_id = p_new_parent_id),
    -- FULL JOIN so a class present on one side only counts too: that is the
    -- gained-and-lost half D72 had, arriving here as a depth of NULL against a
    -- depth of something.
    changed AS (
        SELECT coalesce(o.class_id, n.class_id) AS class_id
          FROM old_chain o FULL JOIN new_chain n ON n.class_id = o.class_id
         WHERE o.d IS DISTINCT FROM n.d)
    SELECT b.id
      FROM policy_binding b
     WHERE b.item_class_id IN (SELECT class_id FROM changed)
       AND NOT EXISTS (SELECT 1 FROM policy_binding s WHERE s.supersedes_id = b.id)
       AND EXISTS (
           SELECT 1 FROM allocation_policy v
            WHERE v.policy_binding_id = b.id AND v.effective @> p_at
            UNION ALL
           SELECT 1 FROM receiving_policy v
            WHERE v.policy_binding_id = b.id AND v.effective @> p_at
            UNION ALL
           SELECT 1 FROM shelf_life_policy v
            WHERE v.policy_binding_id = b.id AND v.effective @> p_at)
$$;

CREATE FUNCTION party_class_move_affected(
        p_class_id uuid, p_new_parent_id uuid, p_at timestamptz DEFAULT now())
    RETURNS SETOF uuid
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    WITH old_chain AS (
        SELECT ancestor_id AS class_id, depth AS d
          FROM party_class_closure WHERE descendant_id = p_class_id),
    new_chain AS (
        SELECT p_class_id AS class_id, 0 AS d
         UNION ALL
        SELECT ancestor_id, depth + 1
          FROM party_class_closure WHERE descendant_id = p_new_parent_id),
    changed AS (
        SELECT coalesce(o.class_id, n.class_id) AS class_id
          FROM old_chain o FULL JOIN new_chain n ON n.class_id = o.class_id
         WHERE o.d IS DISTINCT FROM n.d)
    SELECT b.id
      FROM policy_binding b
     WHERE b.party_class_id IN (SELECT class_id FROM changed)
       AND NOT EXISTS (SELECT 1 FROM policy_binding s WHERE s.supersedes_id = b.id)
       AND EXISTS (
           SELECT 1 FROM allocation_policy v
            WHERE v.policy_binding_id = b.id AND v.effective @> p_at
            UNION ALL
           SELECT 1 FROM receiving_policy v
            WHERE v.policy_binding_id = b.id AND v.effective @> p_at
            UNION ALL
           SELECT 1 FROM shelf_life_policy v
            WHERE v.policy_binding_id = b.id AND v.effective @> p_at)
$$;

CREATE FUNCTION item_class_move_impact(
        p_class_id uuid, p_new_parent_id uuid, p_at timestamptz DEFAULT now())
    RETURNS bigint
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$ SELECT count(*) FROM item_class_move_affected(
              p_class_id, p_new_parent_id, p_at) $$;

CREATE FUNCTION party_class_move_impact(
        p_class_id uuid, p_new_parent_id uuid, p_at timestamptz DEFAULT now())
    RETURNS bigint
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$ SELECT count(*) FROM party_class_move_affected(
              p_class_id, p_new_parent_id, p_at) $$;

COMMENT ON FUNCTION item_class_move_affected(uuid, uuid, timestamptz) IS
    'The active bindings whose Product depth vector changes if this class moves '
    'to the given parent: gained, lost, or kept at a different depth. NULL parent '
    'means the root. D73.';
COMMENT ON FUNCTION party_class_move_affected(uuid, uuid, timestamptz) IS
    'The same on the Counterparty axis. D73.';
COMMENT ON FUNCTION item_class_move_impact(uuid, uuid, timestamptz) IS
    'How many active bindings the move changes -- question 95''s denominator, '
    'counted rather than estimated. Bindings, not resolutions: a resolution is '
    'per item per kind per instant and is unbounded, and every resolution that '
    'changes involves a binding that changed. D73.';
COMMENT ON FUNCTION party_class_move_impact(uuid, uuid, timestamptz) IS
    'The same on the Counterparty axis. D73.';

GRANT EXECUTE ON FUNCTION
    item_class_move_affected(uuid, uuid, timestamptz),
    party_class_move_affected(uuid, uuid, timestamptz),
    item_class_move_impact(uuid, uuid, timestamptz),
    party_class_move_impact(uuid, uuid, timestamptz)
    TO spork_app, spork_platform, spork_scheduler, spork_projection_owner;

-- ---------------------------------------------------------------------------
-- 3. The number the approver saw, frozen on the fact
-- ---------------------------------------------------------------------------
--
-- D22 asked for this and named it `affected_resolution_count`, "computed before
-- the move, frozen on the fact". The name goes, for the reason above -- it counts
-- bindings. The freezing stays, and it is worth being clear about why this is not
-- the fifth stored counter to drift.
--
-- **Every count that drifted in this repository was a count of the present**, and
-- so was re-derivable at any moment: the invariant total, the question total, the
-- vacuity marks, the `@projection` markers. Storing them was redundancy, and
-- redundancy rots.
--
-- This one is not re-derivable. It is a fact about a past instant: how many
-- bindings *were* active and *would have* changed, given a taxonomy shape that no
-- longer exists and a set of bindings that has moved on since. **Recomputing it
-- tomorrow answers a different question**, which is the same reason D58 stores a
-- lot's country of origin and migration 13 makes a price a term of one order. It
-- is evidence of what somebody was told before they agreed to something, and that
-- is not a cache.
--
-- Nullable, and J60 rather than NOT NULL. A NULL means the row predates this
-- migration -- D72's own re-parent is such a row wherever migration 30 has
-- already shipped -- and backfilling it with a computed number would be inventing
-- evidence of a conversation that did not happen. J60 reports them; it does not
-- fabricate them.

ALTER TABLE policy_change ADD COLUMN affected_binding_count integer;

ALTER TABLE policy_change
    ADD CONSTRAINT policy_change_affected_count_ck
        CHECK (affected_binding_count IS NULL
               OR (affected_binding_count >= 0 AND change_kind = 'reparented'));

COMMENT ON COLUMN policy_change.affected_binding_count IS
    'How many active bindings this move was calculated to change, as shown to '
    'whoever approved it. Frozen because it cannot be recomputed: the taxonomy '
    'shape it was measured against is gone. NULL means the move predates D73. '
    'D22 called this affected_resolution_count; it counts bindings, and the '
    'rename is question 95''s answer. D73.';

GRANT INSERT (affected_binding_count) ON policy_change TO spork_app;

-- ---------------------------------------------------------------------------
-- 4. What this does not solve
-- ---------------------------------------------------------------------------
--
-- **Nothing checks that the stored number is the one the function returned.** It
-- cannot: verifying it needs the pre-move taxonomy, which the move destroyed, and
-- the only mechanism that could catch it at write time is a trigger, which S7
-- forbids. The number rests on the application calling the function it is given
-- -- the same position S23 is in, and honest to state rather than paper over.
--
-- **The count is per-move, not per-member.** A move affecting three bindings
-- across four hundred items reports three. That is the right unit for "should I
-- do this", and the wrong one for "who is affected" -- the second is the SETOF
-- function joined to the members, and no screen has asked for it yet.
--
-- Retiring and renaming a class are still not acts. Retiring is question 149.
-- **Renaming is deliberately left as an ordinary UPDATE**: D22's sketch listed
-- `renamed` as an action, but resolution matches on `id` through the closures and
-- nothing anywhere matches on `code`, so a rename cannot change which policy
-- wins. It is the one taxonomy edit that is genuinely cosmetic.
