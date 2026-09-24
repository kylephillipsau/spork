-- Migration 35: the current value of an observed thing, and who is allowed to set it.
--
-- D78. D23 was adopted on 2026-08-01 -- the same day as D21 -- and its projection
-- was never built. Four invariants have been waiting on it: J10, J11, J12 and
-- J18. No question defers it.
--
-- D77 built D21 and leaned on the boundary between the two decisions: an
-- assertion body holds identifiers, structure and the values the receipt compares
-- line by line, and **every other number with a unit is an observation whose
-- observable is the asserted unit**. That boundary was asserted and unexercised,
-- because the observation side had no current value and no way to name an
-- asserted unit as a subject. Both halves are here.

-- ---------------------------------------------------------------------------
-- 1. The arms migration 34 should have added
-- ---------------------------------------------------------------------------
--
-- `observable` says how its own union grows, in a comment written in migration 7:
--
--   "package_type, consignment, device, vehicle_arrival, asserted_unit and
--    asserted_unit_content are the remaining declared arms. **Each arrives as one
--    column in the migration that creates its target.**"
--
-- Migration 34 created both targets and added neither column, so D77 shipped a
-- boundary it had no way to express. This is that correction, and it is the
-- reason D23's registry design is worth the indirection: widening the subject set
-- is one column on a reference table, and nothing holding 10^7 rows changes.

ALTER TABLE observable
    ADD COLUMN asserted_unit_id uuid,
    ADD COLUMN asserted_unit_content_id uuid;

-- Composite, unlike the older arms. Those predate the convention; these targets
-- carry `(id, tenant_id)` and there is no reason to reach a claim in another
-- tenant.
ALTER TABLE observable
    ADD CONSTRAINT observable_asserted_unit_fk
        FOREIGN KEY (asserted_unit_id, tenant_id)
        REFERENCES asserted_unit(id, tenant_id),
    ADD CONSTRAINT observable_asserted_unit_content_fk
        FOREIGN KEY (asserted_unit_content_id, tenant_id)
        REFERENCES asserted_unit_content(id, tenant_id);

-- A generated expression cannot be altered in place, and the column is derived,
-- so dropping it loses nothing.
ALTER TABLE observable DROP COLUMN kind;
ALTER TABLE observable ADD COLUMN kind text GENERATED ALWAYS AS (
    CASE WHEN item_id IS NOT NULL THEN 'item'
         WHEN package_id IS NOT NULL THEN 'package'
         WHEN lot_id IS NOT NULL THEN 'lot'
         WHEN location_id IS NOT NULL THEN 'location'
         WHEN asserted_unit_id IS NOT NULL THEN 'asserted_unit'
         WHEN asserted_unit_content_id IS NOT NULL THEN 'asserted_unit_content' END
) STORED;

ALTER TABLE observable DROP CONSTRAINT observable_one_arm_ck;
ALTER TABLE observable
    ADD CONSTRAINT observable_one_arm_ck
        CHECK (num_nonnulls(item_id, package_id, lot_id, location_id,
                            asserted_unit_id, asserted_unit_content_id) = 1);

CREATE UNIQUE INDEX observable_asserted_unit_idx
    ON observable (tenant_id, asserted_unit_id) WHERE asserted_unit_id IS NOT NULL;
CREATE UNIQUE INDEX observable_asserted_unit_content_idx
    ON observable (tenant_id, asserted_unit_content_id)
    WHERE asserted_unit_content_id IS NOT NULL;

COMMENT ON COLUMN observable.asserted_unit_id IS
    'A declared logistic unit as a subject. This is where a supplier''s carton '
    'weight lives -- an observation whose author is the counterparty -- rather '
    'than as a column on asserted_unit, so their number and our scale reading are '
    'one vocabulary. D21, D23.';

-- ---------------------------------------------------------------------------
-- 2. Acceptance: their number does not become our number by arriving
-- ---------------------------------------------------------------------------
--
-- J11: *"a counterparty-asserted observation enters `observation_current` only
-- with an acceptance."* Nothing expressed that, so it is built here.
--
-- **This is D21's cut applied to measurement.** A supplier's declared weight is
-- theirs and unrevisable; whether we adopt it as the current value is ours, and
-- it is a separate recorded act. Without this, a message would silently set the
-- number a freight invoice is computed against.

-- `observation` carries PRIMARY KEY (id) alone, so there was no target for a
-- composite tenant foreign key -- the second table in two migrations to be found
-- that way, after `discrepancy`. Added here for the same reason, and the wider
-- pattern is question 155: thirty-two tenant-scoped tables have no such key, so
-- this is a mixed convention rather than a local oversight, and J20 is currently
-- the only thing standing between it and a cross-tenant reference.
ALTER TABLE observation
    ADD CONSTRAINT observation_tenant_key UNIQUE (id, tenant_id);

CREATE TABLE observation_acceptance (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),
    observation_id   uuid NOT NULL,

    accepted_at      timestamptz NOT NULL,
    recorded_at      timestamptz NOT NULL DEFAULT now(),
    client_event_id  uuid NOT NULL,
    accepted_by_id   uuid REFERENCES person(id),
    automation_key   text,
    reason           text,

    CONSTRAINT observation_acceptance_observation_fk
        FOREIGN KEY (observation_id, tenant_id)
        REFERENCES observation(id, tenant_id),
    CONSTRAINT observation_acceptance_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),
    -- D11: a machine may accept, and which one is on the row.
    CONSTRAINT observation_acceptance_actor_ck
        CHECK (num_nonnulls(accepted_by_id, automation_key) = 1),
    CONSTRAINT observation_acceptance_once_key UNIQUE (tenant_id, observation_id)
);

COMMENT ON TABLE observation_acceptance IS
    'Our adoption of a counterparty-asserted observation as the current value. '
    'Theirs to state, ours to accept -- the same cut D21 draws for claims, and '
    'the reason J11 can be checked rather than assumed. D23, D25.';

-- ---------------------------------------------------------------------------
-- 3. observation_current
-- ---------------------------------------------------------------------------
--
-- Keyed `(observable_id, metric_id)`: one current value per thing per metric.
--
-- **Three of D23's listed columns are deliberately absent.**
--
--   `cube_numeric` -- D23: "a derived value is stored only when the derivation was
--   a captured act with its own provenance; otherwise it is computed. `cube` is
--   computed." It is also three metrics multiplied together, which no row keyed
--   on one metric can hold.
--
--   `in_breach` -- it is measured against the resolved `specification_policy`, and
--   neither that value table nor the resolver that would read it exists. A column
--   that can only ever be NULL is a claim the schema cannot keep.
--
--   `observation_precedence_policy_id` -- **and this one was written, and two
--   invariants threw it out.** J10's statement names it, so it went in nullable
--   against the day the policy exists. S16 refused it because a `<kind>_policy_id`
--   column must be a foreign key to its value table and there is no
--   `observation_precedence_policy`; S43 refused it because it was registered to a
--   maintainer whose body never writes it, so *"the column keeps whatever it was
--   last set to while every existence check passes"*.
--
--   Both are right, and they are the same objection D70 reached by argument when
--   it declined to invent a cache for a resolver that does not exist. The column
--   arrives with the policy. Until then the precedence is the default rule below,
--   J10 checks the projection against exactly that rule, and the register says so.

CREATE TABLE observation_current (
    tenant_id        uuid NOT NULL REFERENCES tenant(id),
    observable_id    uuid NOT NULL,
    metric_id        uuid NOT NULL REFERENCES metric(id),

    observation_id   uuid NOT NULL,

    result_kind      text,
    value_numeric    numeric,
    value_instant    timestamptz,
    value_code_id    uuid,
    value_boolean    boolean,
    value_text       text,

    observed_at      timestamptz NOT NULL,
    recorded_at      timestamptz NOT NULL,

    method           text,
    confidence       text,
    uncertainty_numeric numeric,
    asserted_by_party_id uuid,

    PRIMARY KEY (observable_id, metric_id),
    CONSTRAINT observation_current_observable_fk
        FOREIGN KEY (observable_id, tenant_id)
        REFERENCES observable(id, tenant_id),
    CONSTRAINT observation_current_observation_fk
        FOREIGN KEY (observation_id, tenant_id)
        REFERENCES observation(id, tenant_id),
    CONSTRAINT observation_current_asserted_by_fk
        FOREIGN KEY (asserted_by_party_id, tenant_id)
        REFERENCES party(id, tenant_id)
);

COMMENT ON TABLE observation_current IS
    '@projection -- the value that currently holds for a thing and a metric, '
    'maintained by projection_observation_current_rebuild. Not a cache of the '
    'latest row: a counterparty''s number needs an acceptance, and a retracted '
    'one is not a value at all. D23.';
CREATE INDEX observation_current_metric_idx
    ON observation_current (tenant_id, metric_id);

-- ---------------------------------------------------------------------------
-- 4. The maintainer, and the default precedence it applies
-- ---------------------------------------------------------------------------
--
-- **Precedence is a policy D22 has not built.** Its kind is not in `policy_kind`,
-- its value table does not exist, and D70 established that there is no resolver.
-- So the maintainer applies a default, and the default is written here where it
-- can be read rather than inferred from an ORDER BY:
--
--   1. A retracted observation is not a value. Nor is the retraction itself.
--   2. A corrected observation loses to its correction.
--   3. A counterparty's observation needs an acceptance to be eligible at all.
--   4. **Ours beats theirs**, which is the conservative half of D23's own example:
--      *"trust supplier dimensions for items we have never measured, but never
--      trust their weight over our scale."* Trusting theirs where we have nothing
--      is a policy decision and is not made here.
--   5. Then latest `observed_at`, then `recorded_at`, then `id`.
--
-- Rule 4 is the one a policy will most likely overturn, and until it can be
-- configured the safe direction is the one that never lets a message overwrite a
-- measurement.

CREATE FUNCTION projection_observation_current_rebuild(p_tenant uuid)
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
        -- `recorded_at` and `method` belong to the act, not the result: an
        -- observation_event is when we wrote it down and how it was taken, and a
        -- single event carries several results.
        SELECT o.id, o.tenant_id, o.observable_id, o.metric_id, o.result_kind,
               o.value_numeric, o.value_instant, o.value_code_id, o.value_boolean,
               o.value_text, o.observed_at, e.recorded_at,
               e.method, o.confidence, o.uncertainty_numeric,
               e.asserted_by_party_id
          FROM observation o
          JOIN observation_event e ON e.id = o.observation_event_id
         WHERE o.tenant_id = p_tenant
           -- (1) a retraction is not a value, and neither is what it retracts
           AND o.retracts_observation_id IS NULL
           AND NOT EXISTS (SELECT 1 FROM observation r
                            WHERE r.retracts_observation_id = o.id)
           -- (2) a corrected row loses to its correction
           AND NOT EXISTS (SELECT 1 FROM observation c
                            WHERE c.corrects_observation_id = o.id)
           -- (3) theirs needs an acceptance
           AND (e.asserted_by_party_id IS NULL
                OR EXISTS (SELECT 1 FROM observation_acceptance a
                            WHERE a.observation_id = o.id))
    )
    SELECT DISTINCT ON (observable_id, metric_id) *
      FROM eligible
     ORDER BY observable_id, metric_id,
              -- (4) ours before theirs
              (asserted_by_party_id IS NOT NULL),
              -- (5) then the most recent
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
         -- D68: a rebuild that changes nothing writes nothing.
         WHERE oc.observation_id IS DISTINCT FROM excluded.observation_id
            OR oc.value_numeric  IS DISTINCT FROM excluded.value_numeric
            OR oc.value_instant  IS DISTINCT FROM excluded.value_instant
            OR oc.value_code_id  IS DISTINCT FROM excluded.value_code_id
            OR oc.value_boolean  IS DISTINCT FROM excluded.value_boolean
            OR oc.value_text     IS DISTINCT FROM excluded.value_text
            OR oc.observed_at    IS DISTINCT FROM excluded.observed_at
        RETURNING 1)
    SELECT count(*) INTO n FROM upserted;

    -- The reaper. A subject whose only observation was retracted, or whose
    -- counterparty value lost its acceptance, has no current value -- and a stale
    -- row here is worse than no row, because everything downstream believes it.
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
    OWNER TO spork_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_observation_current_rebuild(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_observation_current_rebuild(uuid)
    TO spork_scheduler, spork_platform;

GRANT SELECT, INSERT, UPDATE, DELETE ON observation_current
    TO spork_projection_owner;
GRANT SELECT ON observation, observation_event, observation_acceptance, observable
    TO spork_projection_owner;

INSERT INTO projection_step (function_name, ordinal, note) VALUES
    ('projection_observation_current_rebuild', 45,
     'D23. The current value per observed thing and metric, under the default '
     'precedence until the policy exists.');

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('observation_current', 'observation_id',    'projection_observation_current_rebuild'),
    ('observation_current', 'value_numeric',     'projection_observation_current_rebuild'),
    ('observation_current', 'observed_at',       'projection_observation_current_rebuild');

-- ---------------------------------------------------------------------------
-- 5. RLS and grants
-- ---------------------------------------------------------------------------

ALTER TABLE observation_acceptance ENABLE ROW LEVEL SECURITY;
ALTER TABLE observation_acceptance FORCE ROW LEVEL SECURITY;
CREATE POLICY observation_acceptance_tenant_scoped ON observation_acceptance
    USING (tenant_id = current_tenant());

ALTER TABLE observation_current ENABLE ROW LEVEL SECURITY;
ALTER TABLE observation_current FORCE ROW LEVEL SECURITY;
CREATE POLICY observation_current_tenant_scoped ON observation_current
    USING (tenant_id = current_tenant());

-- An acceptance is a fact and is appended, never revised: withdrawing acceptance
-- is a retraction of the observation, not an edit of our adoption of it.
GRANT SELECT, INSERT ON observation_acceptance TO spork_app;

-- A projection. J36's rule: no login role may UPDATE a projection column or
-- DELETE from the table carrying one.
GRANT SELECT ON observation_current TO spork_app;

-- ---------------------------------------------------------------------------
-- 6. What this does not build
-- ---------------------------------------------------------------------------
--
-- **The precedence policy.** `observation_precedence` and `observation_acceptance`
-- are two of D22's eleven policy kinds and `policy_kind` holds three. Adding the
-- enum values without a value table, a binding or a resolver would be vocabulary
-- with nothing behind it, which is what D77 declined for the five unbuilt
-- assertion bodies. Question 154.
--
-- **`package` dimensions are still frozen at seal and are not fed from here.**
-- D23 is explicit that making them projections would break the rule that a
-- shipped package's dimensions are a historical fact -- a retroactive correction
-- would rewrite the number a freight invoice was computed against. J12 asserts the
-- agreement for unsealed packages only, and that is the whole of the relationship.
