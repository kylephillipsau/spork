-- Migration 41: one answer per metric, and the row that says which policy gave it.
--
-- D86, settling question 156. Two halves that are one change.
--
-- D85 handed the maintainer **one answer for the whole run**, which is right
-- while every binding is tenant-wide and wrong the moment one names a metric --
-- and naming a metric is exactly what D81 ordered this kind for. D23's sentence
-- is two different answers for two different metrics:
--
--   "trust supplier dimensions for items we have never measured, but never trust
--    their weight over our scale."
--
-- Until now that sentence could be stored and not obeyed.

-- ---------------------------------------------------------------------------
-- 1. A decision is a value, so it can be passed
-- ---------------------------------------------------------------------------
--
-- The maintainer still must not resolve: D22 puts the precedence order in Rust
-- and a second one in PL/pgSQL is the two representations that eventually
-- disagree. So the caller resolves **per metric** and passes what it got.
--
-- A composite type rather than parallel arrays, and certainly rather than
-- `jsonb`, which principle 3 forbids outright. The type is what makes the
-- argument readable at the call site and typed at the boundary.
--
-- `metric_id IS NULL` is the fallback decision -- the one that applies where no
-- metric-scoped binding matched -- which is the same shape as a NULL axis meaning
-- "any" everywhere else in D22's lattice.

CREATE TYPE observation_precedence_decision AS (
    metric_id                        uuid,
    prefer_own                       boolean,
    accept_counterparty              boolean,
    observation_precedence_policy_id uuid
);

COMMENT ON TYPE observation_precedence_decision IS
    'One resolved precedence answer, for one metric or for all of them. Produced '
    'by the caller, which is where the resolver lives, and consumed by a '
    'maintainer that is not allowed to decide for itself. D22, D86.';

-- ---------------------------------------------------------------------------
-- 2. The column S16 and S43 threw out, coming back for the right reason
-- ---------------------------------------------------------------------------
--
-- D78 added `observation_precedence_policy_id` because J10's statement named it,
-- and both invariants refused it: S16 because a `<kind>_policy_id` column must be
-- a foreign key to its value table and there was none, S43 because it was
-- registered to a maintainer whose body never wrote it. **Both objections are
-- gone** -- D85 built the table, and this migration writes the column.
--
-- D23's general rule is what it is for: *"any projection maintained under a
-- policy must record the policy row that produced it. Otherwise the
-- rebuild-and-assert job reports every policy change as drift -- the projection
-- was correct under the old policy and correct under the new one, and a rebuild
-- cannot tell the difference without knowing which applied."*
--
-- J10 is that job, and from here it recomputes each row **under the policy that
-- row records** rather than under one global rule.

ALTER TABLE observation_current
    ADD COLUMN observation_precedence_policy_id uuid
        REFERENCES observation_precedence_policy(id);

COMMENT ON COLUMN observation_current.observation_precedence_policy_id IS
    '@projection -- the precedence version that chose this value. NULL means the '
    'default rule chose it, which is a fact rather than an absence: it is what a '
    'run with no matching binding legitimately produces. D23, D86.';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('observation_current', 'observation_precedence_policy_id',
     'projection_observation_current_rebuild');

-- ---------------------------------------------------------------------------
-- 3. The maintainer, deciding per metric from what it was told
-- ---------------------------------------------------------------------------

DROP FUNCTION IF EXISTS projection_observation_current_rebuild(uuid, boolean, boolean);

CREATE FUNCTION projection_observation_current_rebuild(
        p_tenant uuid,
        p_decisions observation_precedence_decision[] DEFAULT NULL)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    n bigint;
    m bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    -- D78's rule as a row, so "nobody resolved anything" and "the resolver said
    -- exactly this" travel the same path and there is one code path rather than
    -- two. `projection_run_all` still calls with no decisions at all.
    CREATE TEMP TABLE decision ON COMMIT DROP AS
    SELECT * FROM unnest(coalesce(p_decisions,
        ARRAY[(NULL, true, true, NULL)::observation_precedence_decision]));

    CREATE TEMP TABLE winner ON COMMIT DROP AS
    WITH applicable AS (
        -- The metric-scoped decision where there is one, the fallback otherwise.
        SELECT o.id AS observation_id, d.prefer_own, d.accept_counterparty,
               d.observation_precedence_policy_id
          FROM observation o
          JOIN LATERAL (
              SELECT * FROM decision dd
               WHERE dd.metric_id = o.metric_id OR dd.metric_id IS NULL
               ORDER BY (dd.metric_id IS NULL)
               LIMIT 1) d ON true
         WHERE o.tenant_id = p_tenant
    ),
    eligible AS (
        SELECT o.id, o.tenant_id, o.observable_id, o.metric_id, o.result_kind,
               o.value_numeric, o.value_instant, o.value_code_id, o.value_boolean,
               o.value_text, o.observed_at, e.recorded_at,
               e.method, o.confidence, o.uncertainty_numeric,
               e.asserted_by_party_id,
               a.prefer_own, a.observation_precedence_policy_id
          FROM observation o
          JOIN observation_event e ON e.id = o.observation_event_id
          JOIN applicable a ON a.observation_id = o.id
         WHERE o.tenant_id = p_tenant
           AND o.retracts_observation_id IS NULL
           AND NOT EXISTS (SELECT 1 FROM observation r
                            WHERE r.retracts_observation_id = o.id)
           AND NOT EXISTS (SELECT 1 FROM observation c
                            WHERE c.corrects_observation_id = o.id)
           AND (e.asserted_by_party_id IS NULL
                OR (a.accept_counterparty
                    AND EXISTS (SELECT 1 FROM observation_acceptance oa
                                 WHERE oa.observation_id = o.id)))
    )
    SELECT DISTINCT ON (observable_id, metric_id) *
      FROM eligible
     ORDER BY observable_id, metric_id,
              (prefer_own AND asserted_by_party_id IS NOT NULL),
              observed_at DESC, recorded_at DESC, id DESC;

    WITH upserted AS (
        INSERT INTO observation_current AS oc (
            tenant_id, observable_id, metric_id, observation_id, result_kind,
            value_numeric, value_instant, value_code_id, value_boolean, value_text,
            observed_at, recorded_at, method, confidence, uncertainty_numeric,
            asserted_by_party_id, observation_precedence_policy_id)
        SELECT tenant_id, observable_id, metric_id, id, result_kind,
               value_numeric, value_instant, value_code_id, value_boolean, value_text,
               observed_at, recorded_at, method, confidence, uncertainty_numeric,
               asserted_by_party_id, observation_precedence_policy_id
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
               asserted_by_party_id = excluded.asserted_by_party_id,
               observation_precedence_policy_id =
                   excluded.observation_precedence_policy_id
         WHERE oc.observation_id IS DISTINCT FROM excluded.observation_id
            OR oc.value_numeric  IS DISTINCT FROM excluded.value_numeric
            OR oc.value_instant  IS DISTINCT FROM excluded.value_instant
            OR oc.value_code_id  IS DISTINCT FROM excluded.value_code_id
            OR oc.value_boolean  IS DISTINCT FROM excluded.value_boolean
            OR oc.value_text     IS DISTINCT FROM excluded.value_text
            OR oc.observed_at    IS DISTINCT FROM excluded.observed_at
            OR oc.observation_precedence_policy_id
                 IS DISTINCT FROM excluded.observation_precedence_policy_id
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
    DROP TABLE decision;
    RETURN n + m;
END
$$;

ALTER FUNCTION projection_observation_current_rebuild(
        uuid, observation_precedence_decision[])
    OWNER TO nylonite_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_observation_current_rebuild(
        uuid, observation_precedence_decision[]) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_observation_current_rebuild(
        uuid, observation_precedence_decision[])
    TO nylonite_scheduler, nylonite_platform;
GRANT SELECT ON observation_precedence_policy TO nylonite_projection_owner;
