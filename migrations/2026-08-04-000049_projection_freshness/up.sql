-- Migration 49: a projection says how stale it is allowed to be.
--
-- D95, settling question 163, which the inbound walk raised by reaching the end
-- of a delivery and being unable to fold it.
--
-- Migration 24 decided the **order** the maintainers run in and put it in
-- `projection_step`. Nothing has ever decided their **cadence**. `projection_run_all`
-- is granted to the scheduler, the platform and the projection owner and
-- deliberately not to the app -- D25 keeps the writer away from the maintainer --
-- and S7 examines zero triggers, so nothing folds a write until something
-- external runs. A receiver finishes a delivery and `stock.quantity` does not
-- move.
--
-- **That is not a defect and this migration does not fix it.** It is the design
-- working: the ledger is the truth and `stock` is a cache of it. What was missing
-- is that nobody could tell how old the cache was, and nothing said how old it is
-- allowed to get.
--
-- ---------------------------------------------------------------------------
-- Why synchronous folding is not the answer, measured rather than assumed
-- ---------------------------------------------------------------------------
--
-- Every maintainer is a **full-tenant fold**. `projection_stock_rebuild` reads
-- `stock_movement WHERE tenant_id = p_tenant` in its entirety and recomputes every
-- cell. Its cost is therefore O(history), not O(the write that triggered it), and
-- it grows forever.
--
--   289,080 movements, one year:   1954 ms / 2021 ms / 2073 ms
--   the small fixture tenant:        27 ms
--
-- So folding on the write path would put two seconds and rising on every scan,
-- and the honest reading is that **the cadence has a ceiling that is a property of
-- the fold rather than of the scheduler**: N tenants cost N x 2 s per cycle. Making
-- that O(delta) is a different decision and question 164.
--
-- The README said this fold took "under a second". It takes two. That number was
-- measured before nine migrations of projections were added to the set.

-- ---------------------------------------------------------------------------
-- 1. How fresh each projection must be
-- ---------------------------------------------------------------------------
--
-- NOT NULL and no default, so a step added later cannot avoid the question. The
-- bounds differ by what reads them, which is the only defensible way to set them:
-- the stock family is what the floor reads while standing in front of the goods,
-- and a taxonomy closure changes when somebody reorganises a catalogue.

ALTER TABLE projection_step ADD COLUMN freshness_bound interval;

UPDATE projection_step SET freshness_bound = CASE function_name
    -- The floor reads these. A picker looking at availability is looking at this.
    WHEN 'projection_stock_rebuild'              THEN interval '5 minutes'
    WHEN 'projection_stock_resolve_locations'    THEN interval '5 minutes'
    WHEN 'projection_package_rebuild'            THEN interval '5 minutes'
    WHEN 'projection_package_stamp'              THEN interval '5 minutes'
    -- Read by planning and by the receiving screen rather than by a scanner.
    WHEN 'projection_expected_supply_rebuild'    THEN interval '15 minutes'
    WHEN 'projection_fulfilment_rebuild'         THEN interval '15 minutes'
    WHEN 'projection_inbound_shipment_rebuild'   THEN interval '15 minutes'
    WHEN 'projection_order_rebuild'              THEN interval '15 minutes'
    WHEN 'projection_observation_current_rebuild' THEN interval '15 minutes'
    WHEN 'projection_package_containment_rebuild' THEN interval '15 minutes'
    -- Change when somebody reorganises a catalogue, which is not hourly.
    WHEN 'projection_taxonomy_rebuild'           THEN interval '1 hour'
    WHEN 'projection_item_class_closure_rebuild' THEN interval '1 hour'
    WHEN 'projection_party_class_closure_rebuild' THEN interval '1 hour'
END;

ALTER TABLE projection_step
    ALTER COLUMN freshness_bound SET NOT NULL,
    ADD CONSTRAINT projection_step_freshness_positive_ck
        CHECK (freshness_bound > interval '0');

COMMENT ON COLUMN projection_step.freshness_bound IS
    'How far behind the ledger this projection may fall before the gap is a '
    'finding. NOT NULL with no default, so a step added later has to state one. '
    'J66 is what notices. D95.';

-- ---------------------------------------------------------------------------
-- 2. When each one last ran, per tenant
-- ---------------------------------------------------------------------------
--
-- One row per tenant per step, updated in place. Not a log: the question is "how
-- old is this number", and every answer but the latest is noise.
--
-- **`last_run_at` is written on every run, including a run that changed nothing**,
-- and that is the whole point. A fold that ran and found no work is *fresh*; a
-- fold that has not run is *stale*; and the two are indistinguishable if the
-- stamp only moves on work. `projection_step.last_changed_at` already records the
-- other question -- when the projection last changed -- and D70 uses it for the
-- policy epoch, so the two columns are deliberately not the same fact.

CREATE TABLE projection_freshness (
    tenant_id      uuid NOT NULL REFERENCES tenant(id),
    function_name  text NOT NULL REFERENCES projection_step(function_name),
    last_run_at    timestamptz NOT NULL,
    last_run_ms    integer NOT NULL,
    rows_touched   bigint NOT NULL,
    PRIMARY KEY (tenant_id, function_name)
);

COMMENT ON TABLE projection_freshness IS
    'OPERATIONAL, tenant-scoped. When each maintainer last ran for each tenant, '
    'whether or not it found work. What a reader consults to know how old a '
    'projected number is, and what J66 measures against the declared bound. '
    'Excluded from D68 by name -- see the note on that test. D95.';

ALTER TABLE projection_freshness ENABLE ROW LEVEL SECURITY;
ALTER TABLE projection_freshness FORCE ROW LEVEL SECURITY;
CREATE POLICY projection_freshness_tenant_scoped ON projection_freshness
    USING (tenant_id = current_tenant());

GRANT SELECT ON projection_freshness
    TO spork_app, spork_platform, spork_scheduler;
GRANT SELECT, INSERT, UPDATE ON projection_freshness TO spork_projection_owner;

-- ---------------------------------------------------------------------------
-- 3. The orchestrator records itself
-- ---------------------------------------------------------------------------
--
-- The stamp is written by `projection_run_all` rather than by the maintainers,
-- which keeps D68's property exactly where D68 put it: **a rebuild that changes
-- nothing still writes nothing.** What writes is the orchestrator, about itself.

CREATE OR REPLACE FUNCTION projection_run_all(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    r       record;
    n       bigint;
    total   bigint := 0;
    started timestamptz;
BEGIN
    FOR r IN SELECT function_name FROM projection_step ORDER BY ordinal
    LOOP
        started := clock_timestamp();
        EXECUTE format('SELECT %I($1)', r.function_name) USING p_tenant INTO n;
        n := coalesce(n, 0);
        total := total + n;

        -- D70. Only on work, so D68's property survives: a rebuild that changes
        -- nothing writes nothing, and that now includes this row.
        IF n > 0 THEN
            UPDATE projection_step
               SET last_changed_at = now()
             WHERE function_name = r.function_name;
        END IF;

        -- D95. Always, because a fold that ran and found nothing is fresh.
        INSERT INTO projection_freshness
            (tenant_id, function_name, last_run_at, last_run_ms, rows_touched)
        VALUES (p_tenant, r.function_name, now(),
                (extract(epoch FROM clock_timestamp() - started) * 1000)::integer, n)
        ON CONFLICT (tenant_id, function_name) DO UPDATE
           SET last_run_at  = excluded.last_run_at,
               last_run_ms  = excluded.last_run_ms,
               rows_touched = excluded.rows_touched;
    END LOOP;
    RETURN total;
END
$$;

-- ---------------------------------------------------------------------------
-- 4. What a reader asks
-- ---------------------------------------------------------------------------
--
-- So a screen can say "as at 14:03" rather than presenting a projected number as
-- though it were the ledger. NULL means the fold has never run for this tenant,
-- which is a different answer from "it ran a long time ago" and reads that way.

CREATE FUNCTION projection_age(p_tenant uuid, p_function text)
    RETURNS interval
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    SELECT now() - last_run_at FROM projection_freshness
     WHERE tenant_id = p_tenant AND function_name = p_function
$$;

COMMENT ON FUNCTION projection_age(uuid, text) IS
    'How long ago this maintainer last ran for this tenant. NULL when it never '
    'has, which is not the same answer as a large interval. D95.';

GRANT EXECUTE ON FUNCTION projection_age(uuid, text)
    TO spork_app, spork_platform, spork_scheduler;
