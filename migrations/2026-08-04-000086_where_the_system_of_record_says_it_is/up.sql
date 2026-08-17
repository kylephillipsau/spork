-- Migration 86: where the system of record says a thing is.
--
-- The warehouse is being measured with a clipboard because nothing here knows
-- which shelf to walk to. NetSuite does, and exports it: item, bin, location,
-- quantity, as an Inventory Balance search. This is where that lands.
--
-- # It is not `stock`, and the distinction is the whole table
--
-- `stock` is a projection of `stock_movement` — migration 2 says it plainly:
-- *"stock_movement is the ledger and stock is the fold of it"*. Every number in
-- it is derived from an act this system recorded, with an author and a moment.
-- Nothing here has that. This is a **report from somewhere else**, believed
-- because of where it came from rather than because of what it is made of.
--
-- Writing these 2,905 rows as movements was the obvious alternative and it is
-- wrong: it would be this system asserting it holds Alpha's stock, which
-- starts the integrity machinery comparing a six-day-old snapshot against a
-- ledger that has recorded nothing, and every finding it raised would be false.
-- A separate table cannot be mistaken for the ledger by anything that reads it.
--
-- # Nor is it a D21 assertion
--
-- That model is for *"a statement of record exchanged with another party,
-- stored exactly as exchanged, which neither side may unilaterally revise"* —
-- and its own header says the cut is control: both sides hold a copy and will
-- quote it back. Nobody is going to quote an inventory export back at us. It is
-- a snapshot of a system we are migrating off, refreshed whenever somebody runs
-- the search again.
--
-- # `as_at` is not decoration
--
-- The printed sheet that prompted this was six days old, and in six days one
-- item had moved bins, one had doubled, and one had left Melbourne entirely.
-- A number from a report must say how old it is or it will be read as current
-- forever — the same argument D95 makes about projection freshness.

CREATE TABLE reported_stock (
    id            uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id     uuid NOT NULL REFERENCES tenant(id),

    site_id       uuid NOT NULL,
    item_id       uuid NOT NULL,
    -- Nullable: a report can name a quantity at a site without naming a shelf,
    -- and refusing that would lose the quantity as well as the position.
    location_id   uuid,

    on_hand       numeric NOT NULL,
    available     numeric,
    -- The exporting system's own word — `Good`, `Damaged`. Text rather than an
    -- enum because it is somebody else's vocabulary and D33's test is whether
    -- *our* code branches on it. Nothing here does.
    status        text,

    -- When the report was taken, not when it was loaded.
    as_at         timestamptz NOT NULL,
    -- Which export this row came from, so a bad load can be identified and
    -- replaced rather than picked apart row by row.
    source        text NOT NULL,

    loaded_at     timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT reported_stock_on_hand_ck CHECK (on_hand >= 0),
    CONSTRAINT reported_stock_available_ck CHECK (available IS NULL OR available >= 0),
    CONSTRAINT reported_stock_site_fk
        FOREIGN KEY (site_id, tenant_id) REFERENCES site(id, tenant_id),
    CONSTRAINT reported_stock_item_fk
        FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT reported_stock_location_fk
        FOREIGN KEY (location_id, tenant_id) REFERENCES location(id, tenant_id)
);

-- One row per item per shelf per report. A second load of the same export
-- replaces rather than doubles: an inventory report is a snapshot, and two
-- copies of a snapshot is not twice the stock.
CREATE UNIQUE INDEX reported_stock_position_idx
    ON reported_stock (tenant_id, site_id, item_id, location_id, source)
    WHERE location_id IS NOT NULL;

CREATE UNIQUE INDEX reported_stock_siteless_idx
    ON reported_stock (tenant_id, site_id, item_id, source)
    WHERE location_id IS NULL;

-- The read the capture worklist makes: everything at one site, in walk order.
CREATE INDEX reported_stock_walk_idx ON reported_stock (tenant_id, site_id, item_id);

COMMENT ON TABLE reported_stock IS
    'Where the system of record says a thing is, as at a moment. Not `stock`, '
    'which is folded from this system''s own ledger, and not a D21 assertion, '
    'which is a document both parties hold. A snapshot from elsewhere, believed '
    'because of its source, and carrying the date that says how much to believe '
    'it. D158.';

-- ---------------------------------------------------------------------------
-- Row-level security, which the first draft of this migration forgot
-- ---------------------------------------------------------------------------
--
-- **And nothing caught it**, which is worth writing down. J-checks assert
-- `relforcerowsecurity` on every table carrying an `@projection` column, and
-- this table carries none — so a new tenant-scoped table arrived with row-level
-- security off and the invariant suite stayed green. The check is extended in
-- the same commit as this fix.
--
-- Every row here belongs to exactly one tenant. There is no shared-reference
-- case: another company's shelves are not reference data for this one.

ALTER TABLE reported_stock ENABLE ROW LEVEL SECURITY;
ALTER TABLE reported_stock FORCE ROW LEVEL SECURITY;

CREATE POLICY reported_stock_own ON reported_stock FOR ALL
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- DELETE is in the set because a report is replaced wholesale rather than
-- amended: loading a fresh export clears the rows the previous one wrote for
-- that source. That is the one place where deleting is right in a system that
-- otherwise keeps everything — because this is not a record of what happened,
-- it is a copy of what somebody else currently believes.
GRANT SELECT, INSERT, UPDATE, DELETE ON reported_stock TO nylonite_app;
GRANT SELECT ON reported_stock TO nylonite_platform;
GRANT SELECT ON reported_stock TO nylonite_scheduler;
