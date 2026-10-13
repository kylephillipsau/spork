-- Migration 137: who packed it, as NetSuite says (D254).
--
-- Orders are packed in NetSuite, and the fast fulfilment script sets each
-- item fulfilment's Packed By as it marks it Packed: the name the packer
-- chose to pack as. Spork learns that a fulfilment it knows was packed, and
-- not by whom; and a fulfilment packed before Spork ever saw it Picked (about
-- one in five) never reaches it at all.
--
-- So the Bridge reports the fulfilments NetSuite has packed, each with its
-- Packed By, as it reports the balance and the open orders. Unlike those it is
-- **not replaced by each load**: a load is the last five weeks, sent again
-- every few minutes, and a row is kept once seen. Packed By is NetSuite's word
-- and follows it when it changes.
--
-- When it was packed is Spork's, because NetSuite keeps no time for it:
--   - when Spork saw NetSuite close a fulfilment it knew (closed_elsewhere_at);
--   - otherwise when a load first reported it, which is within a sync of the
--     packing while the Bridge runs, and nobody packs without NetSuite open;
--   - and for the feed's first load, which reports weeks gone, the
--     fulfilment's own date at the start of the site's day.

CREATE TABLE reported_packing (
    id                     uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id              uuid NOT NULL REFERENCES tenant(id),
    -- The warehouse it was packed at.
    site_id                uuid NOT NULL,
    -- The item fulfilment, as a person quotes it (IF272977), and NetSuite's id.
    fulfilment_number      text NOT NULL,
    fulfilment_external_id text NOT NULL,
    -- NetSuite's Packed By, as it reads; NULL when it names nobody.
    packed_by              text,
    packed_at              timestamptz NOT NULL,
    -- When NetSuite last said so, and which feed said it.
    as_at                  timestamptz NOT NULL,
    source                 text NOT NULL,
    loaded_at              timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT reported_packing_site_fk FOREIGN KEY (site_id, tenant_id) REFERENCES site(id, tenant_id),
    CONSTRAINT reported_packing_once UNIQUE (tenant_id, source, fulfilment_external_id)
);

COMMENT ON TABLE reported_packing IS
  'The item fulfilments NetSuite has packed, with its Packed By: a report kept once seen, not replaced. D254, migration 137.';

CREATE INDEX reported_packing_site_at_idx ON reported_packing (tenant_id, site_id, packed_at);

ALTER TABLE reported_packing ENABLE ROW LEVEL SECURITY;
ALTER TABLE reported_packing FORCE ROW LEVEL SECURITY;

CREATE POLICY reported_packing_own ON reported_packing FOR ALL
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- UPDATE, for Packed By changed in NetSuite. No DELETE: a row is kept.
GRANT SELECT, INSERT, UPDATE ON reported_packing TO spork_app;
GRANT SELECT ON reported_packing TO spork_platform;
GRANT SELECT ON reported_packing TO spork_scheduler;
