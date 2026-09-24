-- Migration 40: which observation wins, configured rather than compiled.
--
-- D85, settling question 154. D78 hard-coded a precedence rule into
-- `projection_observation_current_rebuild` -- *ours beats theirs, then the most
-- recent* -- and said why: the kind was not in `policy_kind`, the value table did
-- not exist, and D70 had established there was no resolver. All three have since
-- stopped being true.
--
-- **A correction to D78 on the way past.** It said *"trusting theirs where we have
-- nothing is a policy decision and is not made here."* That reads as though the
-- default refuses it, and the default does no such thing: the fold orders ours
-- before theirs and takes what is left, so where we have measured nothing, an
-- accepted counterparty value already becomes current -- the fixture's pallet
-- height is exactly that. What the default refuses is theirs beating **ours**.
-- The distinction is the whole of D23's sentence and D78 blurred it.

ALTER TYPE policy_kind ADD VALUE IF NOT EXISTS 'observation_precedence';

-- ---------------------------------------------------------------------------
-- 1. The value table, on S14's template
-- ---------------------------------------------------------------------------
--
-- Two fields, and both are read straight out of D23:
--
--   "trust supplier dimensions for items we have never measured, but never trust
--    their weight over our scale."
--
-- The first clause is `accept_counterparty`; the second is `prefer_own`. D81 put
-- Metric at the head of this kind's precedence order for exactly this reason --
-- the sentence is two different answers for two different metrics, and it is
-- inexpressible if Counterparty outranks Metric.
--
-- Nothing else is invented. An age limit, a confidence threshold and a
-- device-class rule are all plausible and none is in D23, and a field with no
-- sentence behind it is the guess D80 refused for `plugin_id` and D78 refused for
-- a column S16 then removed.

CREATE TABLE observation_precedence_policy (
    id                 uuid PRIMARY KEY DEFAULT uuidv7(),
    policy_binding_id  uuid NOT NULL,
    kind               policy_kind NOT NULL DEFAULT 'observation_precedence',
    effective          tstzrange NOT NULL,

    -- Never trust their number over ours, whichever is more recent.
    prefer_own         boolean NOT NULL DEFAULT true,
    -- May an accepted counterparty value be current at all, where we have none?
    accept_counterparty boolean NOT NULL DEFAULT true,

    CONSTRAINT observation_precedence_policy_kind_ck
        CHECK (kind = 'observation_precedence'::policy_kind),
    CONSTRAINT observation_precedence_policy_binding_fk
        FOREIGN KEY (policy_binding_id, kind) REFERENCES policy_binding(id, kind),
    CONSTRAINT observation_precedence_policy_no_overlap
        EXCLUDE USING gist (policy_binding_id WITH =, effective WITH &&)
);

COMMENT ON TABLE observation_precedence_policy IS
    'Which observation becomes the current value. D23''s sentence, made '
    'configurable: prefer_own is "never trust their weight over our scale", '
    'accept_counterparty is "trust supplier dimensions for items we have never '
    'measured". D22, D23, D85.';

GRANT SELECT ON observation_precedence_policy TO spork_app, spork_scheduler,
    spork_projection_owner;
GRANT SELECT, INSERT, UPDATE, DELETE ON observation_precedence_policy
    TO spork_platform;

-- ---------------------------------------------------------------------------
-- 1a. A hole this migration walked into, and closes for the whole class
-- ---------------------------------------------------------------------------
--
-- The first draft gave this table a row-level policy and **S9 rejected the
-- shape**. Checking why turned up the actual problem: `allocation_policy`,
-- `receiving_policy` and `shelf_life_policy` have **row-level security disabled
-- entirely**, and `spork_app` holds SELECT, INSERT and UPDATE on all three.
-- Any tenant's connection could read -- and rewrite -- another tenant's policy
-- values.
--
-- That is the D55 class again, and D79 missed it: value tables carry no
-- `tenant_id` column, so the audit that classified every table as strictly-owned
-- or shared-reference **skipped them as neither**. Their tenancy is real and
-- indirect, reached through `policy_binding`, and indirect is exactly what an
-- audit keyed on a column name cannot see.
--
-- So the shape S9 rejected is the right shape, and S9 gains it as a fourth
-- permitted expression rather than the table losing its policy: a value row is
-- readable when its binding is, which is the only sentence that can be true for
-- a table with no tenant of its own.

DO $$
DECLARE t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['allocation_policy', 'receiving_policy',
                             'shelf_life_policy', 'observation_precedence_policy']
    LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY %I ON %I USING (EXISTS (SELECT 1 FROM policy_binding b '
            'WHERE b.id = policy_binding_id AND (b.tenant_id IS NULL OR '
            'b.tenant_id = current_tenant())))', t || '_via_binding', t);
    END LOOP;
END $$;

-- ---------------------------------------------------------------------------
-- 2. The maintainer stops guessing and starts being told
-- ---------------------------------------------------------------------------
--
-- **This is the boundary where "the resolver is in code" meets "projections are
-- maintained in SQL", and it has to be resolved rather than fudged.**
--
-- The maintainer cannot resolve the policy itself: D22 puts the precedence order
-- in Rust *"where it is a visible const"*, D81 justified it there, and a second
-- ordering written in PL/pgSQL would be the two representations that eventually
-- disagree -- the argument D36 makes about ceilings, applied to resolution.
--
-- So the maintainer takes the answer as arguments. The caller resolves --
-- `policy_candidate`, `resolve`, `policy_value`, `apply_clamps` -- and passes what
-- it got. **The defaults are D78's rule**, so `projection_run_all` behaves exactly
-- as it did and the change is opt-in per caller rather than a silent flip.
--
-- What this does not do is resolve *per observable*. One answer covers the run,
-- which is right while the only bindings anyone writes are per-tenant, and wrong
-- the day a binding names a metric -- which is precisely what D81 ordered this
-- kind for. Question 156.

-- Dropped rather than replaced: a new parameter list makes an *overload*, and
-- `projection_run_all` calls maintainers by name with one argument, so both would
-- match and the call would be ambiguous. Found by the fixture on the first run.
DROP FUNCTION IF EXISTS projection_observation_current_rebuild(uuid);

CREATE FUNCTION projection_observation_current_rebuild(
        p_tenant uuid,
        p_prefer_own boolean DEFAULT true,
        p_accept_counterparty boolean DEFAULT true)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    n bigint;
    m bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    CREATE TEMP TABLE winner ON COMMIT DROP AS
    WITH eligible AS (
        SELECT o.id, o.tenant_id, o.observable_id, o.metric_id, o.result_kind,
               o.value_numeric, o.value_instant, o.value_code_id, o.value_boolean,
               o.value_text, o.observed_at, e.recorded_at,
               e.method, o.confidence, o.uncertainty_numeric,
               e.asserted_by_party_id
          FROM observation o
          JOIN observation_event e ON e.id = o.observation_event_id
         WHERE o.tenant_id = p_tenant
           AND o.retracts_observation_id IS NULL
           AND NOT EXISTS (SELECT 1 FROM observation r
                            WHERE r.retracts_observation_id = o.id)
           AND NOT EXISTS (SELECT 1 FROM observation c
                            WHERE c.corrects_observation_id = o.id)
           -- Theirs needs an acceptance, and now also needs the policy to admit
           -- counterparty values at all.
           AND (e.asserted_by_party_id IS NULL
                OR (p_accept_counterparty
                    AND EXISTS (SELECT 1 FROM observation_acceptance a
                                 WHERE a.observation_id = o.id)))
    )
    SELECT DISTINCT ON (observable_id, metric_id) *
      FROM eligible
     ORDER BY observable_id, metric_id,
              -- D85. Ours before theirs only when the policy says so; otherwise
              -- recency alone decides and a supplier's fresher number wins.
              (p_prefer_own AND asserted_by_party_id IS NOT NULL),
              observed_at DESC, recorded_at DESC, id DESC;

    WITH upserted AS (
        INSERT INTO observation_current AS oc (
            tenant_id, observable_id, metric_id, observation_id, result_kind,
            value_numeric, value_instant, value_code_id, value_boolean, value_text,
            observed_at, recorded_at, method, confidence, uncertainty_numeric,
            asserted_by_party_id)
        SELECT tenant_id, observable_id, metric_id, id, result_kind,
               value_numeric, value_instant, value_code_id, value_boolean, value_text,
               observed_at, recorded_at, method, confidence, uncertainty_numeric,
               asserted_by_party_id
          FROM winner
        ON CONFLICT (observable_id, metric_id) DO UPDATE
           SET observation_id = excluded.observation_id,
               result_kind    = excluded.result_kind,
               value_numeric  = excluded.value_numeric,
               value_instant  = excluded.value_instant,
               value_code_id  = excluded.value_code_id,
               value_boolean  = excluded.value_boolean,
               value_text     = excluded.value_text,
               observed_at    = excluded.observed_at,
               recorded_at    = excluded.recorded_at,
               method         = excluded.method,
               confidence     = excluded.confidence,
               uncertainty_numeric = excluded.uncertainty_numeric,
               asserted_by_party_id = excluded.asserted_by_party_id
         WHERE oc.observation_id IS DISTINCT FROM excluded.observation_id
            OR oc.value_numeric  IS DISTINCT FROM excluded.value_numeric
            OR oc.value_instant  IS DISTINCT FROM excluded.value_instant
            OR oc.value_code_id  IS DISTINCT FROM excluded.value_code_id
            OR oc.value_boolean  IS DISTINCT FROM excluded.value_boolean
            OR oc.value_text     IS DISTINCT FROM excluded.value_text
            OR oc.observed_at    IS DISTINCT FROM excluded.observed_at
        RETURNING 1)
    SELECT count(*) INTO n FROM upserted;

    WITH reaped AS (
        DELETE FROM observation_current oc
         WHERE oc.tenant_id = p_tenant
           AND NOT EXISTS (SELECT 1 FROM winner w
                            WHERE w.observable_id = oc.observable_id
                              AND w.metric_id = oc.metric_id)
        RETURNING 1)
    SELECT count(*) INTO m FROM reaped;

    DROP TABLE winner;
    RETURN n + m;
END
$$;

ALTER FUNCTION projection_observation_current_rebuild(uuid, boolean, boolean)
    OWNER TO spork_projection_owner;
REVOKE EXECUTE ON FUNCTION
    projection_observation_current_rebuild(uuid, boolean, boolean) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION
    projection_observation_current_rebuild(uuid, boolean, boolean)
    TO spork_scheduler, spork_platform;

-- ---------------------------------------------------------------------------
-- 3. What J10 now has to know
-- ---------------------------------------------------------------------------
--
-- J10 recomputes the projection and compares, so it holds the default rule a
-- second time on purpose. It is unchanged and still correct **because
-- `projection_run_all` still calls the default** -- and the day an orchestrator
-- passes a resolved policy, J10 has to be told which one, or it will report the
-- policy working as drift. That is D23's own general rule arriving where it was
-- always going to: *"any projection maintained under a policy must record the
-- policy row that produced it."*
--
-- The column that would record it is the one S16 and S43 threw out of D78 for
-- naming a table that did not exist. **That table exists now**, so the objection
-- is gone and the column can come back -- with the resolver wired through, not
-- before. Question 156 carries both halves together, because they are one change.

-- ---------------------------------------------------------------------------
-- 4. The coupling migration 38 predicted, walked into within the hour
-- ---------------------------------------------------------------------------
--
-- Migration 38 said it in its own closing comment: *"the effective-range test
-- names the three built tables. A fourth would have to be added here, which is
-- exactly the coupling S50 exists to catch."* It was half right. The coupling is
-- real -- a fourth kind resolved to **nothing**, because `policy_candidate` did
-- not know its value table existed -- and S50 does **not** catch it. S50 is about
-- what the policy *epoch* can see, and this is about what the *resolver* can see.
-- Two different questions that read alike.
--
-- So both functions learn the fourth table, and **S52 is the check that will not
-- let a fifth be forgotten**: every `%_policy` table must appear in both, read
-- from the catalogue rather than from a list.

CREATE OR REPLACE FUNCTION policy_candidate(
        p_tenant uuid, p_kind policy_kind, p_item uuid DEFAULT NULL,
        p_party uuid DEFAULT NULL, p_site uuid DEFAULT NULL, p_zone uuid DEFAULT NULL,
        p_owner uuid DEFAULT NULL, p_metric uuid DEFAULT NULL,
        p_at timestamptz DEFAULT now())
    RETURNS TABLE (policy_binding_id uuid, tenancy integer, product integer,
                   counterparty integer, space integer, ownership integer,
                   metric integer)
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    WITH item_classes AS (
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
       AND NOT EXISTS (SELECT 1 FROM policy_binding s WHERE s.supersedes_id = b.id)
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
       AND EXISTS (
           SELECT 1 FROM allocation_policy v
            WHERE v.policy_binding_id = b.id AND v.effective @> p_at
            UNION ALL
           SELECT 1 FROM receiving_policy v
            WHERE v.policy_binding_id = b.id AND v.effective @> p_at
            UNION ALL
           SELECT 1 FROM shelf_life_policy v
            WHERE v.policy_binding_id = b.id AND v.effective @> p_at
            UNION ALL
           SELECT 1 FROM observation_precedence_policy v
            WHERE v.policy_binding_id = b.id AND v.effective @> p_at)
$$;

CREATE OR REPLACE FUNCTION policy_value(
        p_kind policy_kind, p_bindings uuid[], p_at timestamptz DEFAULT now())
    RETURNS TABLE (policy_binding_id uuid, field text, value numeric)
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    SELECT v.policy_binding_id, f.field, f.value
      FROM allocation_policy v
      CROSS JOIN LATERAL (VALUES
          ('weight_rotation',      v.weight_rotation::numeric),
          ('weight_travel',        v.weight_travel::numeric),
          ('weight_consolidation', v.weight_consolidation::numeric),
          ('allow_partial',        v.allow_partial::integer::numeric)) AS f(field, value)
     WHERE p_kind = 'allocation'
       AND v.policy_binding_id = ANY(p_bindings) AND v.effective @> p_at
     UNION ALL
    SELECT v.policy_binding_id, f.field, f.value
      FROM receiving_policy v
      CROSS JOIN LATERAL (VALUES
          ('respond_by_hours',    v.respond_by_hours::numeric),
          ('require_lot',         v.require_lot::integer::numeric),
          ('tolerance_over_pct',  v.tolerance_over_pct),
          ('tolerance_under_pct', v.tolerance_under_pct)) AS f(field, value)
     WHERE p_kind = 'receiving'
       AND v.policy_binding_id = ANY(p_bindings) AND v.effective @> p_at
     UNION ALL
    SELECT v.policy_binding_id, f.field, f.value
      FROM shelf_life_policy v
      CROSS JOIN LATERAL (VALUES
          ('min_shelf_life_days', v.min_shelf_life_days::numeric),
          ('min_shelf_life_pct',  v.min_shelf_life_pct)) AS f(field, value)
     WHERE p_kind = 'shelf_life'
       AND v.policy_binding_id = ANY(p_bindings) AND v.effective @> p_at
     UNION ALL
    SELECT v.policy_binding_id, f.field, f.value
      FROM observation_precedence_policy v
      CROSS JOIN LATERAL (VALUES
          ('prefer_own',          v.prefer_own::integer::numeric),
          ('accept_counterparty', v.accept_counterparty::integer::numeric)) AS f(field, value)
     WHERE p_kind = 'observation_precedence'
       AND v.policy_binding_id = ANY(p_bindings) AND v.effective @> p_at
$$;
