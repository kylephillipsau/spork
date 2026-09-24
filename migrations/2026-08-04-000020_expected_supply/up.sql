-- Migration 20: one promise identity, whatever document produced it.
--
-- D60, building D24's supply side. `expected_supply` is the table that makes a
-- promise of goods arriving have **one identity regardless of which document
-- produced it**, so the allocator learns two supply kinds forever and a new
-- source is a new arm on a projection rather than a branch in the hot path.
--
-- Thirteen pending checks name this table, more than any other absent object in
-- the suite. It is also the second of the three that close `migration-1.md`.

-- ---------------------------------------------------------------------------
-- 1. Vocabulary
-- ---------------------------------------------------------------------------

-- D24. Where the dates came from, because a dock appointment and an inference
-- from a lead time are not the same claim and a query that treats them alike will
-- promise stock against a guess.
CREATE TYPE date_confidence AS ENUM ('advised', 'ordered', 'inferred', 'none');

-- D24's closed_reason set, verbatim. A row leaves the promisable pool for one of
-- six stated reasons, and "it disappeared" is not among them.
CREATE TYPE supply_closed_reason AS ENUM
    ('received_in_full', 'short_closed', 'superseded', 'cancelled', 'expired', 'withdrawn');

-- ---------------------------------------------------------------------------
-- 2. The projection
-- ---------------------------------------------------------------------------
--
-- Keyed and read the way `stock` is, deliberately: on-hand availability is one
-- indexed read on `stock`, and available-to-promise over future supply is one
-- indexed read here. D24 calls that a widening dressed as a narrowing, because
-- ATP over future supply was not slow before — it was unbuildable.
--
-- **One arm exists.** D24 gives four — purchase order line, transfer order line,
-- asserted unit content, return authorisation line — with
-- `CHECK (num_nonnulls(<the four>) = 1)`, because "none" is meaningless for a
-- projection of a promise. Three of those targets do not exist, and `discrepancy`
-- already set the precedent for that case: *"Only the arms whose targets exist are
-- present. The rest arrive in the migration that creates what they point at,
-- because a nullable FK to a table that does not exist is not a placeholder."*
--
-- So the arm is NOT NULL today and becomes the four-way CHECK when it has
-- company. S24 asserts one partial unique index per arm, which is the same claim
-- from the other side and does not care how many arms there are.

CREATE TABLE expected_supply (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id   uuid NOT NULL REFERENCES tenant(id),
    site_id     uuid,

    -- Unresolvable content produces a finding, not a row. D24.
    item_id     uuid NOT NULL REFERENCES item(id),

    -- @projection from the source line: what the goods will be ON ARRIVAL.
    -- Question 107 settled that this is the arrival state rather than a claim
    -- about custody in transit, which is why amending the order keeps it correct.
    owner_id    uuid REFERENCES party(id),
    status_id   uuid REFERENCES inventory_status(id),

    -- The ordered arm. D24's amendment 3 notes the transfer arm has zero exposure
    -- to D21's rule 3 and is therefore the one to build first; it needs a
    -- transfer order, which is its own document and its own decision, so the
    -- ordered arm goes first instead and carries the same absence of exposure.
    purchase_order_line_id uuid NOT NULL,

    expected_from   timestamptz,
    expected_to     timestamptz,
    date_confidence date_confidence NOT NULL DEFAULT 'none',

    -- The five maintained quantities S26 caps. A sixth requires a recorded
    -- decision, which is the point of capping them.
    quantity_expected     bigint NOT NULL DEFAULT 0,
    quantity_refined      bigint NOT NULL DEFAULT 0,
    quantity_received     bigint NOT NULL DEFAULT 0,
    quantity_closed_short bigint NOT NULL DEFAULT 0,
    quantity_allocated    bigint NOT NULL DEFAULT 0,

    -- Both written out rather than layered, because Postgres will not let a
    -- generated column read another generated column. D24 states promisable as
    -- outstanding minus allocated; this is that, expanded.
    quantity_outstanding bigint GENERATED ALWAYS AS
        (quantity_expected - quantity_refined - quantity_received - quantity_closed_short) STORED,
    quantity_promisable bigint GENERATED ALWAYS AS
        (quantity_expected - quantity_refined - quantity_received - quantity_closed_short
         - quantity_allocated) STORED,

    closed_at     timestamptz,
    closed_reason supply_closed_reason,

    -- S16: a consuming column is named <kind>_policy_id and is an FK to the value
    -- table. The resolution is frozen on the row at first use, which is the
    -- precedent D23 cites goods_receipt_line for.
    receiving_policy_id  uuid REFERENCES receiving_policy(id),
    allocation_policy_id uuid REFERENCES allocation_policy(id),

    CONSTRAINT expected_supply_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id),
    -- Composite through tenant, so a promise cannot draw on another tenant's
    -- order line. J14's lesson, applied on the way in.
    CONSTRAINT expected_supply_po_line_fk
        FOREIGN KEY (purchase_order_line_id, tenant_id)
        REFERENCES purchase_order_line(id, tenant_id),
    CONSTRAINT expected_supply_closed_ck
        CHECK ((closed_at IS NULL) = (closed_reason IS NULL)),
    CONSTRAINT expected_supply_window_ck
        CHECK (expected_to IS NULL OR expected_from IS NULL OR expected_to >= expected_from),
    CONSTRAINT expected_supply_quantities_ck
        CHECK (quantity_expected >= 0 AND quantity_refined >= 0
           AND quantity_received >= 0 AND quantity_closed_short >= 0
           AND quantity_allocated >= 0),
    CONSTRAINT expected_supply_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON TABLE expected_supply IS
    'PROJECTION. One identity for a promise of goods arriving, whatever document '
    'produced it, so the allocator learns two supply kinds forever. D24, D60.';

-- Deliberately absent, each because its target is:
--
--   transfer_order_line_id, asserted_unit_content_id, return_authorisation_line_id
--   refines_expected_supply_id  -- D24 pairs it with the asserted arm by CHECK,
--                                  so without that arm it could never be set
--   advised_lot_code, advised_expiry_date  -- RAW supplier strings off an advice
--   derived_from_assertion_id, inbound_shipment_id  -- J41 stays pending on these
--
-- Every one of them arrives with the thing it points at. A nullable column that
-- nothing can ever populate is the `@projection(pending)` mistake in a different
-- costume: it reads as capability and delivers none.

-- S24. One partial unique index per provenance arm, which is the idempotency
-- guard that makes reprocessing a message safe. Explicitly NOT a unique index
-- over (item_id, owner_id, status_id): two purchase orders may promise the same
-- goods to the same place and they are two promises.
CREATE UNIQUE INDEX expected_supply_po_line_key
    ON expected_supply (tenant_id, purchase_order_line_id)
    WHERE purchase_order_line_id IS NOT NULL;

-- S25. The availability read carries owner and status in the key, so no
-- availability query joins `inventory_status` to find out whether a row counts.
CREATE INDEX expected_supply_availability_idx
    ON expected_supply (tenant_id, item_id, site_id, owner_id, status_id, expected_from)
    WHERE closed_at IS NULL;

ALTER TABLE expected_supply ENABLE ROW LEVEL SECURITY;
ALTER TABLE expected_supply FORCE ROW LEVEL SECURITY;
CREATE POLICY expected_supply_tenant_scoped ON expected_supply
    USING (tenant_id = current_tenant());

-- A projection: the application reads it and the maintainer writes it. S45 is
-- satisfied because the table appears in no application write grant at all, which
-- is the honest way to say the application never writes it.
GRANT SELECT ON expected_supply TO spork_app;
GRANT SELECT, INSERT, UPDATE ON expected_supply TO spork_projection_owner;

-- ---------------------------------------------------------------------------
-- 3. The second supply arm on an allocation
-- ---------------------------------------------------------------------------
--
-- Migration 3 said this in as many words: *"The second supply arm is
-- expected_supply_id, which arrives with the supply migration."* Here it is.
--
-- An allocation against future supply records on `expected_supply.quantity_allocated`
-- only and never reaches `stock`, which is what makes cross-dock expressible
-- without inventing stock rows for goods that are not there.

ALTER TABLE stock_allocation
    ADD COLUMN expected_supply_id uuid REFERENCES expected_supply(id) ON DELETE RESTRICT;

ALTER TABLE stock_allocation
    -- D23's discriminated-union rule. Exactly one supply, because an allocation
    -- against nothing is not an allocation.
    ADD CONSTRAINT stock_allocation_supply_ck
        CHECK (num_nonnulls(stock_id, expected_supply_id) = 1);

CREATE INDEX stock_allocation_expected_supply_idx
    ON stock_allocation (expected_supply_id) WHERE expected_supply_id IS NOT NULL;

GRANT INSERT (expected_supply_id), UPDATE (expected_supply_id)
    ON stock_allocation TO spork_app;

-- ---------------------------------------------------------------------------
-- 4. The maintainer
-- ---------------------------------------------------------------------------
--
-- **Upsert keyed on the arm, never truncate-and-regenerate.** J30 states it and
-- the reason is `stock_allocation.expected_supply_id ON DELETE RESTRICT`: live
-- allocations hold these ids, so regenerating the table would either fail or
-- orphan a commitment. Identity survives a rebuild, which is what lets an
-- allocation outlive one.
--
-- Three of the five quantities have no source yet and the function says so
-- rather than leaving them to a default nobody re-reads. `quantity_refined`
-- needs the asserted arm to have children; `quantity_received` needs
-- `goods_receipt_line`, which is the next migration; `quantity_closed_short`
-- needs the source line's agreed release, which is a supplier conversation
-- nothing models yet.

CREATE FUNCTION projection_expected_supply_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    WITH ordered AS (
        -- Only an issued order promises anything. A draft is a document somebody
        -- is still writing and a cancelled one promises nothing, which is the
        -- DECLARED half of D25's split doing exactly the work it was split for.
        SELECT l.id AS line_id, l.tenant_id, po.site_id, l.item_id,
               l.owner_party_id, l.status_id,
               l.expected_from, l.expected_to, l.quantity_ordered
          FROM purchase_order_line l
          JOIN purchase_order po ON po.id = l.purchase_order_id
         WHERE l.tenant_id = p_tenant AND po.state = 'issued'
    ),
    upserted AS (
        INSERT INTO expected_supply AS e (
            tenant_id, site_id, item_id, owner_id, status_id,
            purchase_order_line_id, expected_from, expected_to, date_confidence,
            quantity_expected)
        SELECT o.tenant_id, o.site_id, o.item_id, o.owner_party_id, o.status_id,
               o.line_id, o.expected_from, o.expected_to,
               -- The dates came off the order rather than an advice, and saying
               -- so is what stops a promise being made against a guess.
               'ordered'::date_confidence,
               o.quantity_ordered
          FROM ordered o
        ON CONFLICT (tenant_id, purchase_order_line_id)
            WHERE purchase_order_line_id IS NOT NULL
        DO UPDATE SET site_id = EXCLUDED.site_id,
                      item_id = EXCLUDED.item_id,
                      owner_id = EXCLUDED.owner_id,
                      status_id = EXCLUDED.status_id,
                      expected_from = EXCLUDED.expected_from,
                      expected_to = EXCLUDED.expected_to,
                      date_confidence = EXCLUDED.date_confidence,
                      quantity_expected = EXCLUDED.quantity_expected
        RETURNING e.id)
    SELECT count(*) INTO touched FROM upserted;

    -- J4. Active allocations naming this row, on the same state set J3 uses for
    -- the cell: a despatched or released allocation has stopped claiming supply.
    UPDATE expected_supply e
       SET quantity_allocated = coalesce(a.q, 0)
      FROM expected_supply c
      LEFT JOIN (SELECT expected_supply_id, sum(quantity)::bigint AS q
                   FROM stock_allocation
                  WHERE state IN ('allocated','picking','picked','packed')
                    AND expected_supply_id IS NOT NULL
                  GROUP BY expected_supply_id) a ON a.expected_supply_id = c.id
     WHERE e.id = c.id AND e.tenant_id = p_tenant
       AND e.quantity_allocated IS DISTINCT FROM coalesce(a.q, 0);

    -- A promise whose order was cancelled leaves the promisable pool, with the
    -- reason recorded rather than the row deleted. D24's closed_reason set exists
    -- so that "why is this not promisable" always has an answer.
    UPDATE expected_supply e
       SET closed_at = now(), closed_reason = 'cancelled'
      FROM purchase_order_line l
      JOIN purchase_order po ON po.id = l.purchase_order_id
     WHERE e.purchase_order_line_id = l.id
       AND e.tenant_id = p_tenant
       AND po.state = 'cancelled'
       AND e.closed_at IS NULL;

    RETURN touched;
END
$$;

ALTER FUNCTION projection_expected_supply_rebuild(uuid) OWNER TO spork_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_expected_supply_rebuild(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_expected_supply_rebuild(uuid)
    TO spork_scheduler, spork_platform;

GRANT SELECT ON purchase_order, purchase_order_line TO spork_projection_owner;

COMMENT ON COLUMN expected_supply.quantity_expected IS
    '@projection of the source line via projection_expected_supply_rebuild (D24, D60).';
COMMENT ON COLUMN expected_supply.quantity_allocated IS
    '@projection of stock_allocation via projection_expected_supply_rebuild (D24, J4).';
COMMENT ON COLUMN expected_supply.quantity_refined IS
    '@projection(pending) of refining children; the asserted arm does not exist yet. J8.';
COMMENT ON COLUMN expected_supply.quantity_received IS
    '@projection(pending) of receipts; goods_receipt_line arrives next. J26.';
COMMENT ON COLUMN expected_supply.quantity_closed_short IS
    '@projection(pending) of the source line''s agreed release, which nothing models yet.';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('expected_supply', 'quantity_expected',  'projection_expected_supply_rebuild'),
    ('expected_supply', 'quantity_allocated', 'projection_expected_supply_rebuild');
