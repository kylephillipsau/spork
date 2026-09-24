-- Migration 81: a measurement names the state the thing was in, and some
-- things have no dimensions.
--
-- D138. Two problems arrived together off the warehouse floor and turned out to
-- be one problem.
--
-- An apron is loose in its carton. Folded twice it is 250x180x30; dropped in a
-- tote it is a heap. Two operators measure the same apron and get two different
-- answers, and nothing in the record says why, so honest disagreement reads as a
-- discrepancy about the thing rather than a difference in how it was arranged.
--
-- A lobby pan set is a pan and a 1200mm handle. Their bounding box is mostly
-- air, and which box you get depends entirely on how they were laid on the
-- bench.
--
-- **The assumption underneath both is that an item has dimensions. It does not.
-- A presentation has dimensions.** A rigid carton has exactly one presentation
-- and that is why nobody has noticed: the numbers reproduce because there was
-- never a choice. The moment the subject is a single loose thing, arranging it
-- is part of measuring it, and an unrecorded arrangement is an unrepeatable
-- measurement.
--
-- # On the event, not as a metric
--
-- The obvious build is a code-valued `metric` — the machinery exists, the
-- vocabulary would be data, and no fact table changes shape. It loses on one
-- point, and it is the point this repository keeps relearning.
--
-- D108 resolves **per fact**, independently per metric. A presentation metric
-- would therefore land its own row in `observation_current`, resolved separately
-- from the lengths it qualifies, and a reader joining current presentation to
-- current length would be describing dimensions with an arrangement they were
-- never taken under. The correct read (reach the presentation through the
-- winning length's event) and the wrong read (join the projection twice) are one
-- join each and look identical on the page. That is a trap laid for later code,
-- and D108's own trap was closed structurally rather than with a note.
--
-- `observation_event.observable_id` means **one event is about one subject**, so
-- one presentation per event is not a convention — it is what the table already
-- says. Put it there and the wrong read cannot be written.
--
-- It also lands in the right company. `method`, `ingestion_channel`,
-- `device_id`, `instrument_device_id` and `asserted_by_party_id` all describe
-- the act rather than any one result. *What state the thing was in* is the sixth
-- member of that family.
--
-- This does not collide with D133's one-method rule, and the decision says so
-- explicitly so that nobody applies it sharply later and splits the event.
-- `method` says how the figures were come by; presentation says what was in
-- front of the person. It is a condition of the look, in exactly the sense D132
-- made a photograph an artefact of the look rather than an act of its own.
--
-- # Shipped vocabulary, no tenant column
--
-- `dimension` says of itself "shipped by us; no tenant_id, ever", and this is
-- the same kind of table: seven words for how a physical object was arranged,
-- which is not a thing one warehouse means differently from another. A tenant
-- column here would need an RLS policy, and a policy with no `WITH CHECK`
-- authorises writing what it meant to allow reading (D55) — cost paid for a
-- capability nobody has asked for.
--
-- **The trigger for tenant-defined presentations** is a tenant needing a state
-- this list cannot express. Then it grows the way `metric` did, with the policy
-- written properly.
--
-- # An absence is an answer
--
-- `observation.absent_reason` has held `not_applicable` since migration 7 and
-- has never been reachable: `POST /observations` writes `'quantity'` and
-- requires a unit, so there has been no way to record *this thing has no
-- dimensions*. That gap is why the pan set is a problem at all. Its parts have
-- sizes; the set has none, and until now the only way to say so was to leave the
-- row empty, which is indistinguishable from nobody having looked.
--
-- The rebuild already lets an absence win — it filters retractions and
-- corrections, not absences — but `observation_current` had nowhere to put the
-- reason, so a declared *not applicable* arrived as NULLs and read as silence.
-- One column closes it.
--
-- **A declared absence supersedes an earlier measurement**, because the winner
-- is the most recent eligible row and that is right: somebody who has looked at
-- the thing and said it has no meaningful box knows more than the tape measure
-- reading that preceded them. Correcting *that* is another observation, which is
-- what `corrects_observation_id` is for.

-- ---------------------------------------------------------------------------
-- 1. The vocabulary
-- ---------------------------------------------------------------------------

CREATE TABLE presentation (
    id      uuid PRIMARY KEY DEFAULT uuidv7(),
    code    text NOT NULL,
    label   text NOT NULL,
    ordinal integer NOT NULL,
    CONSTRAINT presentation_code_key UNIQUE (code)
);

COMMENT ON TABLE presentation IS
    'How a physical object was arranged when it was measured. Shipped by us, no '
    'tenant column, on dimension''s precedent: an apron folded and an apron in a '
    'heap are not the same measurement, and which one it was is not something '
    'one warehouse means differently from another. D138, migration 81.';

INSERT INTO presentation (code, label, ordinal) VALUES
    ('as_supplied',  'As supplied',   1),
    ('assembled',    'Assembled',     2),
    ('knocked_down', 'Knocked down',  3),
    ('folded',       'Folded',        4),
    ('rolled',       'Rolled',        5),
    ('flat',         'Laid flat',     6),
    ('compressed',   'Compressed',    7);

-- Migration 7's lesson, applied on the spot: a seed can insert nothing and
-- report success.
DO $$
DECLARE n integer;
BEGIN
    SELECT count(*) INTO n FROM presentation;
    IF n <> 7 THEN
        RAISE EXCEPTION 'presentation seed inserted % rows, expected 7', n;
    END IF;
END
$$;

GRANT SELECT ON presentation TO spork_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON presentation TO spork_platform;

-- ---------------------------------------------------------------------------
-- 2. The act says what it was looking at
-- ---------------------------------------------------------------------------

ALTER TABLE observation_event
    ADD COLUMN presentation_id uuid REFERENCES presentation(id);

COMMENT ON COLUMN observation_event.presentation_id IS
    'The state the subject was in when this act measured it. Nullable, and NULL '
    'is the ordinary case for a carton: a rigid box has one presentation and '
    'recording it would be ceremony. The writer requires it for a length at '
    '`each`, where arranging the thing is part of measuring it. D138.';

CREATE INDEX observation_event_presentation_idx
    ON observation_event (presentation_id) WHERE presentation_id IS NOT NULL;

-- INSERT is granted table-wide on this table, so the new column is already
-- writable and S45 has nothing to catch. Stated rather than assumed, because
-- migration 73 had to grant its columns one at a time and the difference is the
-- shape of the original grant.

-- ---------------------------------------------------------------------------
-- 3. The projection can say "there is none" instead of saying nothing
-- ---------------------------------------------------------------------------

ALTER TABLE observation_current
    ADD COLUMN absent_reason text;

COMMENT ON COLUMN observation_current.absent_reason IS
    '@projection -- why the winning observation carries no value. Without it a '
    'declared `not_applicable` is indistinguishable from nobody having measured, '
    'which is exactly the difference a capture worklist exists to act on. D138.';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('observation_current', 'absent_reason', 'projection_observation_current_rebuild');

-- The live body from migration 58, with `absent_reason` carried through and the
-- marker bumped. Restated in full rather than patched, because that is the only
-- form `CREATE OR REPLACE` has — and reconstructed from 58 rather than from 35,
-- because 40 and 41 put the precedence decision in it and rebuilding from the
-- oldest definition is how migration 56 nearly reverted D65 and D68.
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
    -- last changed: migration 81 (D138)
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

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
               o.value_text, o.absent_reason, o.observed_at, e.recorded_at,
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
            absent_reason,
            observed_at, recorded_at, method, confidence, uncertainty_numeric,
            asserted_by_party_id, observation_precedence_policy_id)
        SELECT tenant_id, observable_id, metric_id, id, result_kind,
               value_numeric, value_instant, value_code_id, value_boolean, value_text,
               absent_reason,
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
               absent_reason  = excluded.absent_reason,
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
            OR oc.absent_reason  IS DISTINCT FROM excluded.absent_reason
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
