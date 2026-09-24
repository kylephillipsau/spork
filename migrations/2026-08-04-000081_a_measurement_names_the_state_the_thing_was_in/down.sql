-- Migration 81 down: a measurement stops saying what it was looking at.
--
-- What this destroys is not recoverable from anything else. A folded apron's
-- 250x180x30 without the word `folded` is a number nobody can reproduce and
-- nobody can argue with, and a declared *this has no dimensions* reverts to an
-- empty row, which is what "nobody has measured it yet" looks like. The
-- worklist will start asking for both again.

DO $$
DECLARE staged bigint; absent bigint;
BEGIN
    SELECT count(*) INTO staged FROM observation_event WHERE presentation_id IS NOT NULL;
    SELECT count(*) INTO absent FROM observation WHERE absent_reason = 'not_applicable';
    IF staged > 0 OR absent > 0 THEN
        RAISE NOTICE 'reversing D138 discards % recorded presentation(s) and hides % declared absence(s): the observations survive, the reason they hold no number does not', staged, absent;
    END IF;
END
$$;

-- The migration 58 body, restored exactly. `CREATE OR REPLACE` has no other
-- form, and leaving 81's body in place would have the function writing a column
-- that is about to stop existing.
CREATE OR REPLACE FUNCTION public.projection_observation_current_rebuild(p_tenant uuid, p_decisions observation_precedence_decision[] DEFAULT NULL::observation_precedence_decision[])
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    n bigint;
    m bigint;
BEGIN
    -- last changed: migration 58 (D104)
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    CREATE TEMP TABLE decision ON COMMIT DROP AS
    SELECT * FROM unnest(coalesce(p_decisions,
        ARRAY[(NULL, true, true, NULL)::observation_precedence_decision]));

    CREATE TEMP TABLE winner ON COMMIT DROP AS
    WITH applicable AS (
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
$function$;

DELETE FROM projection_rebuild
 WHERE table_name = 'observation_current' AND column_name = 'absent_reason';

ALTER TABLE observation_current DROP COLUMN absent_reason;
ALTER TABLE observation_event DROP COLUMN presentation_id;
DROP TABLE presentation;
