-- The layout remembers who moved what, and when (D209).
--
-- D173 drew places and never said who drew them: "who drew what is the
-- layout's history, which is not built yet". The plan editor makes the
-- layout something people change by hand, so each change is kept: one row per
-- place moved, resized, turned, renamed, added or removed, naming the act it
-- was part of, and so the person and the moment.
--
-- **The place as it was and as it became**, as JSON, rather than a column per
-- field: the box is what changes, and a change that touches one field reads
-- the same as one that touches five. A removed place's row is the only record
-- left of it, so `place_id` is not a reference: the history outlives the place.

CREATE TABLE place_change (
    id              uuid NOT NULL DEFAULT uuidv7() PRIMARY KEY,
    tenant_id       uuid NOT NULL REFERENCES tenant(id),
    site_id         uuid NOT NULL,
    place_id        uuid NOT NULL,
    client_event_id uuid NOT NULL,
    change          text NOT NULL,
    before          jsonb,
    after           jsonb,
    CONSTRAINT place_change_kind_ck CHECK (change IN ('added', 'changed', 'removed')),
    CONSTRAINT place_change_sides_ck CHECK (
        (change = 'added' AND before IS NULL AND after IS NOT NULL)
        OR (change = 'changed' AND before IS NOT NULL AND after IS NOT NULL)
        OR (change = 'removed' AND before IS NOT NULL AND after IS NULL)),
    CONSTRAINT place_change_site_fk FOREIGN KEY (site_id, tenant_id) REFERENCES site(id, tenant_id),
    CONSTRAINT place_change_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id)
);

CREATE INDEX place_change_site_idx ON place_change (site_id, id);
CREATE INDEX place_change_place_idx ON place_change (place_id);

ALTER TABLE place_change ENABLE ROW LEVEL SECURITY;
ALTER TABLE place_change FORCE ROW LEVEL SECURITY;
CREATE POLICY place_change_tenant_scoped ON place_change
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- Appended to, never edited: a history that can be rewritten is not one.
GRANT SELECT, INSERT ON place_change TO spork_app;

COMMENT ON TABLE place_change IS
  'D209: each place added, changed or removed by hand, as it was and as it became, under the act that did it.';
