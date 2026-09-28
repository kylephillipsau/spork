-- Migration 94: a pick made elsewhere is reported, not moved (D172).
--
-- At Melbourne the goods are picked on the WMS handheld and NetSuite records
-- the item fulfilment as Picked. The packing happens here. So this system is
-- told a pick happened that it did not see: which line, how many, by whom,
-- and when the other system says so.
--
-- # It is not a movement
--
-- Migration 86 made the argument for reported stock and it holds word for word
-- here: writing NetSuite's pick as stock_movement "would be this system
-- asserting it holds Alpha's stock". No bin here held these goods and no
-- ledger here saw them leave one. A movement from a phantom location would put
-- a quantity on the ledger that nobody here observed, and every check that
-- reads the ledger would start comparing against a fiction.
--
-- # And it is not picked_quantity
--
-- picked_quantity is a fold of the ledger and J68 says so exactly: add a
-- reported number to it and J68 stops being true. It would also fire J56's
-- `picked > covered` on every NetSuite pick, because nothing allocates stock
-- to a line this system holds no stock for. So the report folds into its own
-- column, external_picked_quantity, beside picked_quantity and never into it.
--
-- # A level, not a delta
--
-- Each row says how many the other system says are picked on that line as at
-- observed_at: a level, which D8 makes the shape of an observation. The
-- current level of a line is its newest report, by observed_at then
-- recorded_at then id (the J6/J46 idiom), so reports arriving out of order
-- fold to the same answer. An un-pick is a later report of fewer, including
-- zero; nothing is ever rewritten. The goods reach this system's ledger when
-- they are handed over (migration 95), not here.

-- ---------------------------------------------------------------------------
-- 1. The fact
-- ---------------------------------------------------------------------------

CREATE TABLE external_pick (
    id                 uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id          uuid NOT NULL REFERENCES tenant(id),
    client_event_id    uuid NOT NULL,

    fulfilment_line_id uuid NOT NULL,
    source_channel_id  uuid NOT NULL REFERENCES source_channel(id),

    -- How many the other system says are picked on this line. A level.
    quantity           bigint NOT NULL,

    -- The document, as a person quotes it and as the channel keys it: the
    -- item fulfilment's number and internal id, and the line within it.
    document           text NOT NULL,
    external_id        text NOT NULL,
    external_line      text NOT NULL,

    -- Who picked, in the other system's words: the WMS user's name. Text,
    -- because it is somebody else's roster and nothing here branches on it,
    -- and a person row invented for it would be a person nobody signed on as.
    picked_by          text,

    -- When the other system says this was so (the item fulfilment's last
    -- modification), which is what orders the reports. Not when it reached us.
    observed_at        timestamptz NOT NULL,
    recorded_at        timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT external_pick_quantity_ck CHECK (quantity >= 0),
    CONSTRAINT external_pick_client_event_fk FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT external_pick_line_fk FOREIGN KEY (fulfilment_line_id, tenant_id)
        REFERENCES fulfilment_line(id, tenant_id)
);

-- The same report twice is one report: a resend of an item fulfilment that has
-- not changed since is not a second observation of it.
CREATE UNIQUE INDEX external_pick_report_key
    ON external_pick (tenant_id, fulfilment_line_id, external_line, observed_at);

CREATE INDEX external_pick_line_idx ON external_pick (fulfilment_line_id);

COMMENT ON TABLE external_pick IS
    'FACT. A pick another system says it made: how many on one line, as at '
    'observed_at. A level, not a movement: the goods reach the ledger when they '
    'are handed over, and a report never does. The current level is the newest '
    'row per (line, external_line). D172.';
COMMENT ON COLUMN external_pick.quantity IS
    'How many the other system says are picked on the line as at observed_at. '
    'A later report replaces it, including with zero for an un-pick. D172.';

ALTER TABLE external_pick ENABLE ROW LEVEL SECURITY;
ALTER TABLE external_pick FORCE ROW LEVEL SECURITY;
CREATE POLICY external_pick_own ON external_pick FOR ALL
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- A fact: inserted, never updated or deleted (S6).
GRANT SELECT, INSERT ON external_pick TO spork_app;
GRANT SELECT ON external_pick TO spork_projection_owner;
GRANT SELECT ON external_pick TO spork_platform;
GRANT SELECT ON external_pick TO spork_scheduler;

-- ---------------------------------------------------------------------------
-- 2. The fold
-- ---------------------------------------------------------------------------

ALTER TABLE fulfilment_line
    ADD COLUMN external_picked_quantity bigint NOT NULL DEFAULT 0;

COMMENT ON COLUMN fulfilment_line.external_picked_quantity IS
    '@projection of external_pick via projection_fulfilment_rebuild (D172). '
    'What another system says is picked on this line: the sum of each external '
    'line''s newest report. Never added to picked_quantity, which is this '
    'system''s own ledger (J68).';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('fulfilment_line', 'external_picked_quantity', 'projection_fulfilment_rebuild');

CREATE OR REPLACE FUNCTION public.projection_fulfilment_rebuild(p_tenant uuid)
 RETURNS bigint
 LANGUAGE plpgsql
 SECURITY DEFINER
 SET search_path TO 'pg_catalog', 'public'
AS $function$
DECLARE
    touched bigint;
BEGIN
    -- last changed: migration 94 (D172)
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    WITH ledger AS (
        SELECT m.fulfilment_line_id,
               -- D99's shape rules over D103's effective quantity. None reads
               -- `reason`, which is text with no CHECK.
               --
               -- D166: out of *storage*, not merely out of a location. A leg
               -- from `staging` to a carton moves goods that were picked
               -- already.
               sum(v.effective_quantity) FILTER (
                   WHERE fl.kind IN ('pick_face', 'bulk', 'overflow'))    AS picked,
               sum(v.effective_quantity) FILTER (
                   WHERE p.status IN ('sealed', 'despatched'))           AS packed,
               sum(v.effective_quantity) FILTER (
                   WHERE m.to_location_id IS NULL
                     AND m.to_package_id IS NULL)                        AS despatched
          FROM stock_movement m
          JOIN stock_movement_effective v
            ON v.movement_id = m.id AND v.tenant_id = m.tenant_id
          LEFT JOIN package p ON p.id = m.to_package_id
          LEFT JOIN location fl ON fl.id = m.from_location_id
         WHERE m.tenant_id = p_tenant
           AND m.fulfilment_line_id IS NOT NULL
         GROUP BY m.fulfilment_line_id
    ),
    intention AS (
        SELECT fulfilment_line_id, sum(quantity)::bigint AS covered
          FROM stock_allocation
         WHERE tenant_id = p_tenant
           AND state IN ('allocated','picking','picked','packed','fulfilled')
           AND fulfilment_line_id IS NOT NULL
         GROUP BY fulfilment_line_id
    ),
    -- D172: each external line's newest report, summed per line. A level, so
    -- the newest wins rather than adding up.
    reported AS (
        SELECT fulfilment_line_id, sum(quantity)::bigint AS external_picked
          FROM (SELECT DISTINCT ON (fulfilment_line_id, external_line)
                       fulfilment_line_id, quantity
                  FROM external_pick
                 WHERE tenant_id = p_tenant
                 ORDER BY fulfilment_line_id, external_line,
                          observed_at DESC, recorded_at DESC, id DESC) newest
         GROUP BY fulfilment_line_id
    ),
    updated AS (
        UPDATE fulfilment_line fl
           SET covered_quantity         = coalesce(i.covered, 0),
               picked_quantity          = coalesce(g.picked, 0),
               packed_quantity          = coalesce(g.packed, 0),
               despatched_quantity      = coalesce(g.despatched, 0),
               external_picked_quantity = coalesce(r.external_picked, 0)
          FROM fulfilment_line l
          LEFT JOIN intention i ON i.fulfilment_line_id = l.id
          LEFT JOIN ledger    g ON g.fulfilment_line_id = l.id
          LEFT JOIN reported  r ON r.fulfilment_line_id = l.id
         WHERE fl.id = l.id AND fl.tenant_id = p_tenant
           AND (fl.covered_quantity, fl.picked_quantity,
                fl.packed_quantity, fl.despatched_quantity,
                fl.external_picked_quantity)
               IS DISTINCT FROM
               (coalesce(i.covered, 0), coalesce(g.picked, 0),
                coalesce(g.packed, 0), coalesce(g.despatched, 0),
                coalesce(r.external_picked, 0))
        RETURNING fl.id)
    SELECT count(*) INTO touched FROM updated;

    RETURN touched;
END
$function$;
