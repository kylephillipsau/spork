-- Migration 128: what NetSuite has still to pick (D231).
--
-- NetSuite prints a picking ticket for each sales order, and the order reaches
-- Spork only once somebody has picked it: the Bridge sends item fulfilments,
-- and NetSuite makes one when the goods are picked. So a batch of tickets sent
-- out to pick was a batch Spork knew nothing of.
--
-- What NetSuite has still to pick is held as the balance is (migration 86) and
-- what an item is sold in (migration 123): a report from somewhere else, with
-- its age and its feed, replaced wholesale by the next load. **Not orders and
-- not work.** An order line here commits nothing and puts nothing on Spork's
-- walk, so nothing is picked in Spork that NetSuite never hears of (D212):
-- the picks are still recorded in NetSuite. Spork says where to go, in what
-- order and with whom, and prints the tickets.
--
-- One row per order line, the order's own particulars on each: a report is
-- read whole and replaced whole, and an order is never asked for apart from
-- its lines.

CREATE TABLE reported_order_line (
    id                uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id         uuid NOT NULL REFERENCES tenant(id),
    -- The warehouse the line is to be picked at.
    site_id           uuid NOT NULL,
    -- The order, as a person quotes it (S268281), and NetSuite's id for it.
    order_number      text NOT NULL,
    order_external_id text,
    ordered_on        date,
    customer          text,
    -- Where it goes: the address as printed, one line of it a line.
    ship_to           text,
    -- What the order says to the people picking it (NetSuite's Picking
    -- Instructions, the order's memo), and what the business notes of the
    -- customer for its own people (Internal Customer Notes): no snakes,
    -- deliveries before two. Printed as written.
    picking_instructions text,
    customer_notes    text,
    ship_via          text,
    po_ref            text,
    -- NetSuite's word for the order: Pending Fulfillment, Partially Fulfilled.
    status            text,
    -- The line: NetSuite's key for it and its place on the order.
    line_key          text NOT NULL,
    line_no           integer,
    -- The item, when the catalogue has its code; the code either way.
    item_id           uuid,
    item_code         text NOT NULL,
    description       text,
    -- The code the supplier prints on it, as the line carries it (Art No.).
    art_no            text,
    -- NetSuite's type for the item (a kit's own line is not picked, D223),
    -- and the kit line a part belongs to.
    item_type         text,
    kit_line          text,
    -- What was ordered, what NetSuite has set aside for it here, and what is
    -- left to pick here, in NetSuite's units.
    ordered           numeric NOT NULL,
    committed         numeric,
    to_pick           numeric NOT NULL,
    -- When NetSuite said so, and which feed said it.
    as_at             timestamptz NOT NULL,
    source            text NOT NULL,
    loaded_at         timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT reported_order_line_site_fk FOREIGN KEY (site_id, tenant_id) REFERENCES site(id, tenant_id),
    CONSTRAINT reported_order_line_item_fk FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT reported_order_line_quantities_ck
        CHECK (ordered >= 0 AND (committed IS NULL OR committed >= 0) AND to_pick >= 0),
    CONSTRAINT reported_order_line_once UNIQUE (tenant_id, site_id, source, order_number, line_key)
);

COMMENT ON TABLE reported_order_line IS
  'What NetSuite has still to pick: its open sales orders'' lines at a site, a report replaced by each load of its feed. Commits nothing and is never work. D231, migration 128.';

CREATE INDEX reported_order_line_number_idx ON reported_order_line (tenant_id, order_number);

ALTER TABLE reported_order_line ENABLE ROW LEVEL SECURITY;
ALTER TABLE reported_order_line FORCE ROW LEVEL SECURITY;

CREATE POLICY reported_order_line_own ON reported_order_line FOR ALL
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- DELETE for migration 86's reason: a report is replaced, not amended.
GRANT SELECT, INSERT, DELETE ON reported_order_line TO spork_app;
GRANT SELECT ON reported_order_line TO spork_platform;
GRANT SELECT ON reported_order_line TO spork_scheduler;
