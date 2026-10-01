-- Migration 102: a list of items to work through. D179.
--
-- Work arrives as a list somebody else drew up: a sheet of thirty items whose
-- weights and sizes are missing, printed in bin order, with boxes to fill in.
-- The item list can narrow itself to what needs measuring here, but that is
-- every item this system has not measured, thousands of them, and not the
-- thirty on the sheet. The sheet is the question, so the sheet is kept.
--
-- # Kept, not computed
--
-- A list is the items it was made with, in the order they were given, which
-- is the order on the paper. It is not a saved search: the sheet does not
-- change when an item is measured, and neither does the list. What has been
-- done is read from the items, so the list shows its progress without being
-- written to.
--
-- # An act
--
-- Making a list is somebody's act, with its `client_event` (D5) and its
-- person, at the site it is worked at. Facts only: a list is never edited,
-- and a corrected list is another list.

CREATE TABLE item_list (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),
    -- Where it is worked; the list is shown there. Absent when the person
    -- making it was signed on at no site.
    site_id          uuid,
    name             text NOT NULL,

    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid NOT NULL REFERENCES person(id),
    recorded_at      timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT item_list_tenant_key UNIQUE (id, tenant_id),
    CONSTRAINT item_list_name_ck CHECK (length(btrim(name)) BETWEEN 1 AND 120),
    CONSTRAINT item_list_site_fk
        FOREIGN KEY (site_id, tenant_id) REFERENCES site(id, tenant_id),
    CONSTRAINT item_list_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),
    -- One act, one list: a retried press finds the list it already made.
    CONSTRAINT item_list_event_key UNIQUE (tenant_id, client_event_id)
);

COMMENT ON TABLE item_list IS
    'A list of items to work through, kept as it was given: a sheet of items '
    'to weigh and measure, say. Never edited. D179.';

CREATE TABLE item_list_entry (
    item_list_id uuid NOT NULL,
    tenant_id    uuid NOT NULL,
    item_id      uuid NOT NULL,
    -- Its place on the list, from 1: the order on the paper.
    position     integer NOT NULL,

    PRIMARY KEY (item_list_id, item_id),
    CONSTRAINT item_list_entry_position_key UNIQUE (item_list_id, position),
    CONSTRAINT item_list_entry_position_ck CHECK (position > 0),
    CONSTRAINT item_list_entry_list_fk
        FOREIGN KEY (item_list_id, tenant_id) REFERENCES item_list(id, tenant_id),
    CONSTRAINT item_list_entry_item_fk
        FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id)
);

COMMENT ON TABLE item_list_entry IS
    'An item on a list, at its place on it. D179.';

CREATE INDEX item_list_site_idx ON item_list (tenant_id, site_id, recorded_at DESC);

ALTER TABLE item_list ENABLE ROW LEVEL SECURITY;
ALTER TABLE item_list FORCE ROW LEVEL SECURITY;
CREATE POLICY item_list_tenant_scoped ON item_list
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

ALTER TABLE item_list_entry ENABLE ROW LEVEL SECURITY;
ALTER TABLE item_list_entry FORCE ROW LEVEL SECURITY;
CREATE POLICY item_list_entry_tenant_scoped ON item_list_entry
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- Facts: INSERT only for the app (S6). A corrected list is another list.
GRANT SELECT, INSERT ON item_list TO spork_app;
GRANT SELECT, INSERT ON item_list_entry TO spork_app;
