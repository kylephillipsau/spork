-- Migration 38: the candidate set, and the depth vector that orders it.
--
-- D82. D22 was adopted on 2026-08-01 and its resolver was never built. D70 built
-- the cache-invalidation contract for it, D73 the impact numbers a screen would
-- show, D78 hard-coded a precedence rule into a maintainer because there was
-- nothing to ask, and J63, S23 and J22 have all been waiting.
--
-- **The split between SQL and code is D22's, not a convenience.** Matching is
-- ancestor-or-self over closures, which is where the data is; ordering is
-- lexicographic over a per-kind precedence order, which D22 puts *"in code, where
-- the precedence order is a visible `const DIMENSIONS` on the kind's Rust type"*
-- and D81 justified. So this migration produces candidates with their depth
-- vector, and `crates/policy` decides which one wins.

-- ---------------------------------------------------------------------------
-- 1. Specificity, defined once
-- ---------------------------------------------------------------------------
--
-- A depth vector component is **how specific this binding is on this axis**,
-- higher being more specific, and zero meaning the binding declared nothing there.
--
--   Product:      0 any -> 1..N the class's ancestor count -> 1000 the item
--   Counterparty: 0 any -> 1..N the class's ancestor count -> 1000 the party
--   Space:        0 any -> 1 site -> 2 zone
--   Ownership:    0 any -> 1 owner party
--   Metric:       0 any -> 1 metric        (flat, which is question 96)
--   Tenancy:      0 platform -> 1 tenant   (not declarable; D22's mandatory first)
--
-- **1000 rather than N+1 for the leaf.** An item is more specific than any class
-- that could ever contain it, and computing "deeper than the deepest class"
-- per-request would make the answer depend on the shape of the tree elsewhere.
-- A constant that no class depth can reach says the same thing and cannot drift
-- with the taxonomy.
--
-- A class's depth is its **ancestor count including itself**, so a root class
-- scores 1 and "any" keeps 0. Counting ancestors *excluding* itself was the first
-- draft and it collided: a binding on a root class scored the same as one that
-- declared no product at all, which would have made "all protective equipment"
-- and "anything whatsoever" the same specificity. Read from the closure D63
-- maintains, so a re-parent moves it and D72's fold is what makes that a recorded
-- act.

CREATE FUNCTION policy_candidate(
        p_tenant     uuid,
        p_kind       policy_kind,
        p_item       uuid DEFAULT NULL,
        p_party      uuid DEFAULT NULL,
        p_site       uuid DEFAULT NULL,
        p_zone       uuid DEFAULT NULL,
        p_owner      uuid DEFAULT NULL,
        p_metric     uuid DEFAULT NULL,
        p_at         timestamptz DEFAULT now())
    RETURNS TABLE (
        policy_binding_id uuid,
        tenancy      integer,
        product      integer,
        counterparty integer,
        space        integer,
        ownership    integer,
        metric       integer)
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    WITH item_classes AS (
        -- Every class at-or-above the request item, with that class's own depth.
        SELECT cc.ancestor_id AS class_id,
               (SELECT count(*) FROM item_class_closure d
                 WHERE d.descendant_id = cc.ancestor_id)::integer AS class_depth
          FROM item_classification ic
          JOIN item_class_closure cc ON cc.descendant_id = ic.item_class_id
         WHERE ic.item_id = p_item AND ic.tenant_id = p_tenant
    ),
    party_classes AS (
        SELECT cc.ancestor_id AS class_id,
               (SELECT count(*) FROM party_class_closure d
                 WHERE d.descendant_id = cc.ancestor_id)::integer AS class_depth
          FROM party p
          JOIN party_class_closure cc ON cc.descendant_id = p.party_class_id
         WHERE p.id = p_party AND p.tenant_id = p_tenant
    )
    SELECT b.id,
           -- Tenancy is not declarable: a tenant's binding always outranks a
           -- platform-shipped one, on every kind. D22, S15.
           (b.tenant_id IS NOT NULL)::integer,
           CASE WHEN b.item_id IS NOT NULL THEN 1000
                WHEN b.item_class_id IS NOT NULL
                     THEN (SELECT class_depth FROM item_classes
                            WHERE class_id = b.item_class_id)
                ELSE 0 END,
           CASE WHEN b.party_id IS NOT NULL THEN 1000
                WHEN b.party_class_id IS NOT NULL
                     THEN (SELECT class_depth FROM party_classes
                            WHERE class_id = b.party_class_id)
                ELSE 0 END,
           CASE WHEN b.zone_id IS NOT NULL THEN 2
                WHEN b.site_id IS NOT NULL THEN 1 ELSE 0 END,
           (b.owner_party_id IS NOT NULL)::integer,
           (b.metric_id IS NOT NULL)::integer
      FROM policy_binding b
     WHERE b.kind = p_kind
       AND (b.tenant_id = p_tenant OR b.tenant_id IS NULL)
       -- A superseded binding is history. D22 makes scope immutable and a change
       -- a new binding, so this is how the old one stops matching.
       AND NOT EXISTS (SELECT 1 FROM policy_binding s WHERE s.supersedes_id = b.id)
       -- Every declared axis must be at-or-above the request. The matching
       -- language has cardinality one -- is this node an ancestor-or-self of that
       -- one -- which is why there is no `<`, no LIKE and no boolean connective
       -- anywhere in the data.
       AND (b.item_class_id IS NULL
            OR b.item_class_id IN (SELECT class_id FROM item_classes))
       AND (b.item_id IS NULL OR b.item_id = p_item)
       AND (b.party_class_id IS NULL
            OR b.party_class_id IN (SELECT class_id FROM party_classes))
       AND (b.party_id IS NULL OR b.party_id = p_party)
       AND (b.site_id IS NULL OR b.site_id = p_site)
       AND (b.zone_id IS NULL OR b.zone_id = p_zone)
       AND (b.owner_party_id IS NULL OR b.owner_party_id = p_owner)
       AND (b.metric_id IS NULL OR b.metric_id = p_metric)
       -- And it must have a value in force at the instant asked about. A binding
       -- whose versions have all expired is not a match, which is the half D70
       -- pointed out no epoch can see.
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

COMMENT ON FUNCTION policy_candidate(uuid, policy_kind, uuid, uuid, uuid, uuid,
                                     uuid, uuid, timestamptz) IS
    'Every binding of this kind whose declared axes are at-or-above the request, '
    'with its depth vector. Ordering them is the caller''s -- D22 puts the '
    'precedence order in code, and D81 says why each one is what it is. D22, D82.';

GRANT EXECUTE ON FUNCTION policy_candidate(uuid, policy_kind, uuid, uuid, uuid,
                                           uuid, uuid, uuid, timestamptz)
    TO spork_app, spork_platform, spork_scheduler;

-- ---------------------------------------------------------------------------
-- 2. What this does not do
-- ---------------------------------------------------------------------------
--
-- **It does not pick a winner.** That is `crates/policy`, over the vector this
-- returns, in the order D81 declared. Splitting it here is what keeps the
-- ordering a visible const rather than an ORDER BY nobody reads.
--
-- **It does not return the value row.** Each kind's `%_policy` table has its own
-- columns and its own clamped fields, so fetching the winner's value is per-kind
-- and belongs with the kind. Three of eleven have value tables.
--
-- **The effective-range test names the three built tables.** A fourth would have
-- to be added here, which is exactly the coupling S50 exists to catch -- it reads
-- the same set from the catalogue and would report the omission.
