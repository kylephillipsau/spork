-- Migration 123: what NetSuite says an item is sold in, and its supplier's
-- part number (D217).
--
-- The capture sheet a list replaces (D179) prints both beside each item: the
-- unit to measure, "CTN", "Each", "Roll", "Pair" or nothing set, and the
-- supplier's part number to find it by. Both are NetSuite's to say and change,
-- as the shelves are (D212), so they are held as migration 86 holds the
-- balance: a report from somewhere else, with its age and its feed, replaced
-- wholesale by the next load rather than edited here. The item master import
-- reads a code and a description and nothing to decide; this is a second feed
-- because it changes on NetSuite's schedule, not the master's.

CREATE TABLE reported_item (
    id            uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id     uuid NOT NULL REFERENCES tenant(id),
    item_id       uuid NOT NULL,
    -- NetSuite's sale unit, as it names it. Null is "not set" there.
    selling_unit  text,
    -- The supplier's code for it (NetSuite's vendor name/code).
    supplier_part text,
    -- When NetSuite said so, and which feed said it.
    as_at         timestamptz NOT NULL,
    source        text NOT NULL,
    loaded_at     timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT reported_item_item_fk FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT reported_item_once UNIQUE (tenant_id, item_id, source)
);

COMMENT ON TABLE reported_item IS
  'What NetSuite says an item is sold in and its supplier part number: a report, replaced by each load of its feed. D217, migration 123.';

ALTER TABLE reported_item ENABLE ROW LEVEL SECURITY;
ALTER TABLE reported_item FORCE ROW LEVEL SECURITY;

CREATE POLICY reported_item_own ON reported_item FOR ALL
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- DELETE for migration 86's reason: a report is replaced, not amended.
GRANT SELECT, INSERT, UPDATE, DELETE ON reported_item TO spork_app;
GRANT SELECT ON reported_item TO spork_platform;
GRANT SELECT ON reported_item TO spork_scheduler;
