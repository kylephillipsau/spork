-- Migration 19: the demand document, adopted rather than assumed.
--
-- D59. Nineteen places in the decision record name `purchase_order` or
-- `purchase_order_line`. D25 gives its status columns, D24 gives an index over
-- it and makes it the first arm of `expected_supply`, D39 declares it ours, D43
-- makes a receipt one per demand document, and question 107 was settled by
-- calling `expected_supply.owner_id` a projection of its source line.
--
-- None of them defines it. **That is the fourth time**: `item_barcode` sat here
-- before D34, `device` before D27, `goods_receipt_line` before D45, and each was
-- found the same way — by something downstream needing a column nobody had
-- written down.
--
-- Found this time by trying to finish `migration-1.md`'s tier-0 list. Its last
-- outstanding item is three columns on `goods_receipt_line`; that table names
-- `expected_supply`; `expected_supply` requires exactly one of four provenance
-- arms, because "none" is meaningless for a projection of a promise; and not one
-- of the four exists. The list's last item was three tables deep the whole time.

-- ---------------------------------------------------------------------------
-- A purchase order is an order we place
-- ---------------------------------------------------------------------------
--
-- The shape mirrors `order` deliberately rather than by accident. D39 puts both
-- in the same category in the same sentence: *"`order` and `purchase_order` are
-- ours. They are created here, they are complete here, and an operation running
-- nothing else works."* One is what a customer asked us for; the other is what we
-- asked a supplier for. Everything that follows from being our own intention
-- reaching us through a channel follows for both.
--
-- So `source_channel_id` and `external_ref` carry D54's meaning unchanged, and
-- D50's price shape applies unchanged: a price is a term of one order, the
-- currency belongs to the document and the money to the line.

CREATE TYPE purchase_order_state AS ENUM ('draft', 'issued', 'cancelled');

COMMENT ON TYPE purchase_order_state IS
    'D25: the DECLARED half. draft, issued and cancelled are decisions somebody '
    'made. partially_received and closed are arithmetic over the ledger and are '
    'not in this enum, which is the split D25 drew and NetSuite did not.';

CREATE TABLE purchase_order (
    id                  uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id           uuid NOT NULL REFERENCES tenant(id),
    site_id             uuid,
    supplier_party_id   uuid REFERENCES party(id),

    -- Our reference, and theirs. D39's one system of record per document.
    order_number        text,
    source_channel_id   uuid NOT NULL REFERENCES source_channel(id),
    external_ref        text,

    -- D50, unchanged: one document, one currency, and a per-line column would
    -- only ever agree with itself.
    currency            text,

    -- D25. One timestamp per state reached, no history table, because an
    -- intention is mutable by definition and giving it an immutable event log
    -- contradicts its own category.
    state               purchase_order_state NOT NULL DEFAULT 'draft',
    issued_at           timestamptz,
    cancelled_at        timestamptz,
    created_at          timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT purchase_order_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id),
    CONSTRAINT purchase_order_currency_ck
        CHECK (currency IS NULL OR currency ~ '^[A-Z]{3}$'),
    -- The timestamps and the state agree, which is what makes them an event log
    -- transposed rather than two facts that can drift apart.
    CONSTRAINT purchase_order_issued_ck
        CHECK (state <> 'issued' OR issued_at IS NOT NULL),
    CONSTRAINT purchase_order_cancelled_ck
        CHECK (state <> 'cancelled' OR cancelled_at IS NOT NULL),
    -- The composite target its line points at, so a line cannot join another
    -- tenant's order. Declared here rather than added afterwards, because the
    -- line's foreign key is written inline and needs it to already exist.
    CONSTRAINT purchase_order_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON TABLE purchase_order IS
    'INTENTION. Ours: created here, complete here. What we asked a supplier for, '
    'which is the inbound mirror of `order`. D39, D59.';

-- `receipt_status` is deliberately absent, and D25 adopted it.
--
-- D25 specifies `receipt_status -- @projection: none|partial|complete|over`,
-- arithmetic over the ledger rather than a decision. Its source is
-- `expected_supply`, which arrives in the next migration. Adding the column now
-- would mean a projection with no maintainer, which is exactly what migration 12
-- spent a commit undoing on `order` and what question 134 spent another
-- answering on `fulfilment`. It arrives with the thing that computes it.

CREATE TABLE purchase_order_line (
    id                  uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id           uuid NOT NULL,
    purchase_order_id   uuid NOT NULL REFERENCES purchase_order(id),
    item_id             uuid NOT NULL REFERENCES item(id),
    quantity_ordered    bigint NOT NULL,
    line_number         integer,

    -- D50's pair, on the buy side. A supplier quotes per some quantity exactly as
    -- a customer is quoted per some quantity.
    unit_price_minor     bigint,
    price_basis_quantity bigint,

    -- The window. D24 gives `expected_supply` an `expected_from`/`expected_to`
    -- pair because a dock appointment has two ends, and the ordered arm projects
    -- them from here.
    expected_from       timestamptz,
    expected_to         timestamptz,

    -- What the goods will be ON ARRIVAL, which settled question 107. These are
    -- the source of `expected_supply.owner_id` and `.status_id`, and they
    -- describe the arrival state rather than a mid-flight claim, so they stay
    -- correct when the order is amended.
    --
    -- Both nullable. A NULL owner is our own stock, which is the ordinary case
    -- and what D20 made the third-party arm an exception to. A NULL status means
    -- the receiving policy decides, which is what D37 put `default_status_id` on
    -- `receiving_policy` for.
    owner_party_id      uuid REFERENCES party(id),
    status_id           uuid REFERENCES inventory_status(id),

    CONSTRAINT purchase_order_line_order_fk FOREIGN KEY (purchase_order_id, tenant_id)
        REFERENCES purchase_order(id, tenant_id),
    CONSTRAINT purchase_order_line_quantity_ck CHECK (quantity_ordered > 0),
    CONSTRAINT purchase_order_line_price_pair_ck
        CHECK (num_nonnulls(unit_price_minor, price_basis_quantity) <> 1),
    CONSTRAINT purchase_order_line_price_basis_ck
        CHECK (price_basis_quantity IS NULL OR price_basis_quantity > 0),
    CONSTRAINT purchase_order_line_price_sign_ck
        CHECK (unit_price_minor IS NULL OR unit_price_minor >= 0),
    CONSTRAINT purchase_order_line_window_ck
        CHECK (expected_to IS NULL OR expected_from IS NULL OR expected_to >= expected_from),
    -- D24's idempotency guard needs this to point at. `expected_supply` carries
    -- UNIQUE (tenant_id, purchase_order_line_id) per arm, and the composite key
    -- is what stops it naming a line of another tenant.
    CONSTRAINT purchase_order_line_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON COLUMN purchase_order_line.owner_party_id IS
    'Whose stock this will be on arrival. Projected onto expected_supply.owner_id, '
    'which is why it describes the arrival state rather than custody in transit. '
    'Question 107, D20, D24.';

-- D24 names this one directly in its index list.
CREATE INDEX purchase_order_line_order_idx ON purchase_order_line (purchase_order_id);
CREATE INDEX purchase_order_line_item_idx ON purchase_order_line (tenant_id, item_id);
-- The open-order read: what is still coming, by site.
CREATE INDEX purchase_order_open_idx ON purchase_order (tenant_id, site_id)
    WHERE state = 'issued';
CREATE INDEX purchase_order_source_channel_idx ON purchase_order (source_channel_id);
CREATE INDEX purchase_order_supplier_idx ON purchase_order (supplier_party_id);

-- ---------------------------------------------------------------------------
-- Amendments do not reach here yet, and D42 said they would
-- ---------------------------------------------------------------------------
--
-- D42's sketch gave `intention_amendment` three subject arms:
-- `order_id | purchase_order_id | transfer_order_id`, exactly one. Migration 9
-- built the order arm alone, and D51 then settled that one amendment names one
-- subject and made that rule concrete for orders and their lines.
--
-- Widening it to purchase orders is the same shape a third time and it is not
-- free: `intention_amendment.order_id` is NOT NULL, so a second subject means
-- making it nullable, extending the subject CHECK S41 derives, and deciding
-- whether a purchase order line takes amendments the way an order line now does.
-- Doing that inside a migration named for something else is what D50 and D44 both
-- refused, so the columns here are ordinary mutable intention columns and
-- question 145 carries the rest.

-- ---------------------------------------------------------------------------
-- RLS and grants
-- ---------------------------------------------------------------------------

ALTER TABLE purchase_order ENABLE ROW LEVEL SECURITY;
ALTER TABLE purchase_order FORCE ROW LEVEL SECURITY;
CREATE POLICY purchase_order_tenant_scoped ON purchase_order
    USING (tenant_id = current_tenant());

ALTER TABLE purchase_order_line ENABLE ROW LEVEL SECURITY;
ALTER TABLE purchase_order_line FORCE ROW LEVEL SECURITY;
CREATE POLICY purchase_order_line_tenant_scoped ON purchase_order_line
    USING (tenant_id = current_tenant());

-- Column-level from the start rather than table-wide and corrected later, which
-- is migration 12's lesson applied on the way in. Every non-projection column is
-- in a list, which S45 checks; `source_channel_id` is INSERT-only for D54's
-- reason, that the system of record is declared at creation.
GRANT SELECT ON purchase_order TO spork_app;
GRANT INSERT (id, tenant_id, site_id, supplier_party_id, order_number,
              source_channel_id, external_ref, currency, state, issued_at,
              cancelled_at, created_at),
      UPDATE (site_id, supplier_party_id, order_number, external_ref, currency,
              state, issued_at, cancelled_at)
    ON purchase_order TO spork_app;

GRANT SELECT ON purchase_order_line TO spork_app;
GRANT INSERT (id, tenant_id, purchase_order_id, item_id, quantity_ordered,
              line_number, unit_price_minor, price_basis_quantity,
              expected_from, expected_to, owner_party_id, status_id),
      UPDATE (quantity_ordered, line_number, unit_price_minor,
              price_basis_quantity, expected_from, expected_to,
              owner_party_id, status_id)
    ON purchase_order_line TO spork_app;
