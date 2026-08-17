-- Reverse of 2026-08-04-000041_precedence_per_row.
--
-- The maintainer goes back to one answer for the whole run, and the column that
-- said which policy produced a row goes with it -- so a rebuild stops being able
-- to tell a policy change from drift, which is the state D23's general rule
-- exists to end.

DELETE FROM projection_rebuild
 WHERE table_name = 'observation_current'
   AND column_name = 'observation_precedence_policy_id';

DROP FUNCTION IF EXISTS projection_observation_current_rebuild(
    uuid, observation_precedence_decision[]);

ALTER TABLE observation_current DROP COLUMN IF EXISTS observation_precedence_policy_id;

DROP TYPE IF EXISTS observation_precedence_decision;

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
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

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
           AND (e.asserted_by_party_id IS NULL
                OR (p_accept_counterparty
                    AND EXISTS (SELECT 1 FROM observation_acceptance a
                                 WHERE a.observation_id = o.id)))
    )
    SELECT DISTINCT ON (observable_id, metric_id) *
      FROM eligible
     ORDER BY observable_id, metric_id,
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
    OWNER TO nylonite_projection_owner;
REVOKE EXECUTE ON FUNCTION
    projection_observation_current_rebuild(uuid, boolean, boolean) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION
    projection_observation_current_rebuild(uuid, boolean, boolean)
    TO nylonite_scheduler, nylonite_platform;
