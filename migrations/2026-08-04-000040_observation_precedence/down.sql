-- Reverse of 2026-08-04-000040_observation_precedence.
--
-- `ALTER TYPE ... ADD VALUE` has no inverse in Postgres, so `policy_kind` keeps
-- `observation_precedence` after this runs -- the same residue migration 30
-- documented, and survivable for the same reason: the up migration adds it with
-- IF NOT EXISTS.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM observation_precedence_policy;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D85 destroys % precedence policy version(s): the '
                     'maintainer goes back to deciding for itself', n;
    END IF;
END $$;

-- The acts that recorded these versions go first: policy_change holds a foreign
-- key to the binding, so deleting the binding under it is refused. Found by
-- phase 4, which is the phase that runs a reversal with the fixture in place.
DELETE FROM policy_change
 WHERE policy_binding_id IN (SELECT id FROM policy_binding
                              WHERE kind = 'observation_precedence');

DROP TABLE IF EXISTS observation_precedence_policy;
DELETE FROM policy_binding WHERE kind = 'observation_precedence';

-- And the resolver stops knowing about a table that no longer exists. Its body is
-- a string, so Postgres would not have refused the drop -- the function would
-- simply have failed the next time anybody resolved anything.
DROP POLICY IF EXISTS allocation_policy_via_binding ON allocation_policy;
DROP POLICY IF EXISTS receiving_policy_via_binding ON receiving_policy;
DROP POLICY IF EXISTS shelf_life_policy_via_binding ON shelf_life_policy;
ALTER TABLE allocation_policy DISABLE ROW LEVEL SECURITY;
ALTER TABLE receiving_policy DISABLE ROW LEVEL SECURITY;
ALTER TABLE shelf_life_policy DISABLE ROW LEVEL SECURITY;

DROP FUNCTION IF EXISTS projection_observation_current_rebuild(uuid, boolean, boolean);

CREATE OR REPLACE FUNCTION projection_observation_current_rebuild(p_tenant uuid)
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
                OR EXISTS (SELECT 1 FROM observation_acceptance a
                            WHERE a.observation_id = o.id))
    )
    SELECT DISTINCT ON (observable_id, metric_id) *
      FROM eligible
     ORDER BY observable_id, metric_id,
              (asserted_by_party_id IS NOT NULL),
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

ALTER FUNCTION projection_observation_current_rebuild(uuid)
    OWNER TO nylonite_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_observation_current_rebuild(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_observation_current_rebuild(uuid)
    TO nylonite_scheduler, nylonite_platform;

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
$$;
