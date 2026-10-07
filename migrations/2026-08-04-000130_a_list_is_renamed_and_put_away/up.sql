-- Migration 130: a list is renamed, changed and put away (D235).
--
-- D179 kept a list as it was given and never edited it: "a corrected list is
-- another list". In use, a sheet's name was typed wrong, a second page of
-- codes arrived for the same sheet, a code was pasted that was never on it,
-- and a finished or mistaken list stayed in everybody's picker for good.
--
-- So a list is worked as the layout is (D209): **the list as it is now**, its
-- name and its items, which the act changes in place, and **each change kept**
-- beside it, as it was and as it became, under the act that made it. Nothing
-- measured is touched: a list only narrows the item list.
--
-- **Put away, not deleted.** A list put away is gone from every picker; its
-- row and its items stay, and so does who put it away.

ALTER TABLE item_list
    ADD COLUMN removed_at timestamptz;

COMMENT ON COLUMN item_list.removed_at IS
    'When it was put away: gone from every picker, its row and items kept. D235.';

CREATE TABLE item_list_change (
    id              uuid NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    tenant_id       uuid NOT NULL REFERENCES tenant(id),
    item_list_id    uuid NOT NULL,
    client_event_id uuid NOT NULL,
    recorded_by_id  uuid NOT NULL REFERENCES person(id),
    recorded_at     timestamptz NOT NULL DEFAULT now(),
    -- `renamed`: its name before and after. `added`: the items put on it, at
    -- their places. `taken_off`: the items taken off, at the places they had.
    -- `removed`: put away.
    change          text NOT NULL,
    before          jsonb,
    after           jsonb,
    CONSTRAINT item_list_change_kind_ck CHECK (change IN ('renamed', 'added', 'taken_off', 'removed')),
    CONSTRAINT item_list_change_list_fk
        FOREIGN KEY (item_list_id, tenant_id) REFERENCES item_list(id, tenant_id),
    CONSTRAINT item_list_change_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    -- One act, one change: a retried press finds the change it made.
    CONSTRAINT item_list_change_event_key UNIQUE (tenant_id, client_event_id)
);

CREATE INDEX item_list_change_list_idx ON item_list_change (item_list_id, id);

ALTER TABLE item_list_change ENABLE ROW LEVEL SECURITY;
ALTER TABLE item_list_change FORCE ROW LEVEL SECURITY;
CREATE POLICY item_list_change_tenant_scoped ON item_list_change
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- The list as it is now changes in place; its history is appended to only.
GRANT UPDATE (name, removed_at) ON item_list TO spork_app;
GRANT DELETE ON item_list_entry TO spork_app;
GRANT SELECT, INSERT ON item_list_change TO spork_app;

COMMENT ON TABLE item_list_change IS
    'D235: each list renamed, added to, taken from or put away, as it was and as it became, under the act that did it.';
