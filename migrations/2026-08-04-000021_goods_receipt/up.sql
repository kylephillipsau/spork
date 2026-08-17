-- Migration 21: the receipt, and the last item on the tier-0 list.
--
-- D61, building D45. `migration-1.md` was written on 2026-08-04 as an audit of
-- the inbound analysis's tier-0 list — *"cheap now, ruinous later, do before any
-- inbound code"* — and this closes the last item on it.
--
-- The item is three columns. Getting to them took three tables, because the line
-- names `expected_supply`, which needs an origin, which needed a purchase order
-- nobody had defined. The list described its own last entry as a column set for
-- two weeks while it was three tables deep.

-- ---------------------------------------------------------------------------
-- 1. The receipt
-- ---------------------------------------------------------------------------
--
-- D44 widened the header source set to four and D43 made a delivery's receipts
-- one per demand document. Only the ordered arm exists, on the same rule
-- `expected_supply` and `discrepancy` follow.
--
-- **The CHECK is `<= 1`, never `= 1`, and S3 exists because of this table.** An
-- unsolicited delivery has no demand-side cause and is still a receipt: goods on
-- the dock are goods on the dock. Writing `= 1` here is the mistake D16 made by
-- repeating D10, and S3 was written to catch it by name.

CREATE TABLE goods_receipt (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL REFERENCES tenant(id),
    site_id          uuid,

    purchase_order_id uuid,

    received_at      timestamptz NOT NULL DEFAULT now(),
    recorded_at      timestamptz NOT NULL DEFAULT now(),
    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid REFERENCES person(id),
    automation_key   text,
    note             text,

    CONSTRAINT goods_receipt_site_fk FOREIGN KEY (site_id, tenant_id)
        REFERENCES site(id, tenant_id),
    CONSTRAINT goods_receipt_po_fk FOREIGN KEY (purchase_order_id, tenant_id)
        REFERENCES purchase_order(id, tenant_id),
    -- S3 reads the name as well as the shape. The transfer, advice and return
    -- arms join this CHECK as their documents arrive.
    CONSTRAINT goods_receipt_demand_ck
        CHECK (num_nonnulls(purchase_order_id) <= 1),
    CONSTRAINT goods_receipt_actor_ck
        CHECK (num_nonnulls(recorded_by_id, automation_key) = 1),
    CONSTRAINT goods_receipt_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT goods_receipt_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON TABLE goods_receipt IS
    'GROUPING. One delivery against at most one demand document. The <= 1 is D10 '
    'corrected and S3 is the check that caught D16 repeating the mistake. D43, D45.';

-- `status` is absent although D25 adopted it, for the same reason
-- `purchase_order.receipt_status` is: `@projection: lines + movements` needs
-- somebody to decide what partial means for a receipt whose lines were accepted,
-- rejected and matched in different proportions, which is a D15 question and not
-- a fold. Question 146 carries both.

-- ---------------------------------------------------------------------------
-- 2. The line, and the columns the list has been waiting for
-- ---------------------------------------------------------------------------

CREATE TABLE goods_receipt_line (
    id               uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id        uuid NOT NULL,
    goods_receipt_id uuid NOT NULL,

    -- Unresolvable content is a finding, not a row. D45.
    item_id          uuid NOT NULL REFERENCES item(id),

    -- **One supply arm.** The inbound sketch gave the line two — a purchase order
    -- line and an asserted unit content — which are two of the four D24 later
    -- unified into `expected_supply`. Carrying the pair would rebuild that union
    -- one level down and break on the two arms the sketch never had. NULL is a
    -- blind or unexpected line, which is the `<= 1` argument a third time.
    expected_supply_id uuid,

    -- SNAPSHOT at capture, never read live. D23 cites this column as the
    -- precedent for freezing a resolution on first use: what the paperwork said
    -- when the goods were counted, so that amending the order afterwards cannot
    -- rewrite what the receiver was working from.
    expected_quantity bigint,

    -- migration-1.md tier-0 item 4, in D58's corrected shape. The middle column
    -- is a packaging level rather than a unit, because a carton's factor varies
    -- by item and by date, and `item_packing_config_id` is what makes D23's
    -- versioning of that factor load-bearing.
    entered_quantity        bigint,
    entered_packaging_level packaging_level,
    item_packing_config_id  uuid,

    -- Nullable; absence is a finding, never a refusal. D45.
    lot_id           uuid,

    accepted_at      timestamptz,
    accepted_by_id   uuid REFERENCES person(id),
    rejected_at      timestamptz,
    rejected_by_id   uuid REFERENCES person(id),
    rejected_reason_id uuid REFERENCES adjustment_reason(id),
    -- A blind line reconciled to supply afterwards, which is the case that makes
    -- `expected_supply_id` nullable rather than merely convenient.
    matched_at       timestamptz,
    matched_by_id    uuid REFERENCES person(id),

    recorded_at      timestamptz NOT NULL DEFAULT now(),
    client_event_id  uuid NOT NULL,
    recorded_by_id   uuid REFERENCES person(id),
    automation_key   text,

    CONSTRAINT goods_receipt_line_receipt_fk
        FOREIGN KEY (goods_receipt_id, tenant_id) REFERENCES goods_receipt(id, tenant_id),
    CONSTRAINT goods_receipt_line_supply_fk
        FOREIGN KEY (expected_supply_id, tenant_id)
        REFERENCES expected_supply(id, tenant_id),
    CONSTRAINT goods_receipt_line_lot_fk
        FOREIGN KEY (lot_id, tenant_id) REFERENCES lot(id, tenant_id),
    CONSTRAINT goods_receipt_line_config_fk
        FOREIGN KEY (item_packing_config_id, tenant_id)
        REFERENCES item_packing_config(id, tenant_id),
    CONSTRAINT goods_receipt_line_client_event_fk
        FOREIGN KEY (tenant_id, client_event_id)
        REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT goods_receipt_line_actor_ck
        CHECK (num_nonnulls(recorded_by_id, automation_key) = 1),
    -- The same four rules migration 18 put on the movement, because it is the
    -- same evidence about the same conversion.
    CONSTRAINT goods_receipt_line_entered_pair_ck
        CHECK (num_nonnulls(entered_quantity, entered_packaging_level) <> 1),
    CONSTRAINT goods_receipt_line_entered_positive_ck
        CHECK (entered_quantity IS NULL OR entered_quantity > 0),
    CONSTRAINT goods_receipt_line_entered_needs_config_ck
        CHECK (entered_packaging_level IS NULL
               OR entered_packaging_level = 'each'
               OR item_packing_config_id IS NOT NULL),
    CONSTRAINT goods_receipt_line_config_needs_level_ck
        CHECK (item_packing_config_id IS NULL OR entered_packaging_level IS NOT NULL),
    -- Accepted and rejected are not both true of one line.
    CONSTRAINT goods_receipt_line_disposition_ck
        CHECK (accepted_at IS NULL OR rejected_at IS NULL),
    CONSTRAINT goods_receipt_line_tenant_key UNIQUE (id, tenant_id)
);

COMMENT ON TABLE goods_receipt_line IS
    'GROUPING. What was counted, what the paperwork said at the time, and what '
    'converted one into the other. It carries no received quantity: that is a '
    'fold over stock_movement, which S36 asserts. D45.';

CREATE INDEX goods_receipt_line_receipt_idx ON goods_receipt_line (goods_receipt_id);
CREATE INDEX goods_receipt_line_supply_idx ON goods_receipt_line (expected_supply_id)
    WHERE expected_supply_id IS NOT NULL;
CREATE INDEX goods_receipt_po_idx ON goods_receipt (purchase_order_id)
    WHERE purchase_order_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 3. The cause arm on the ledger
-- ---------------------------------------------------------------------------
--
-- D45: *"Received is `SUM(stock_movement.quantity)` grouped by
-- `goods_receipt_line_id`, which is a batch load rather than an N+1 because D10
-- made the cause a typed FK."* Without this column that sentence describes a fold
-- with no key to group on, and J26 could not run — which is the same shape as the
-- defect defining `goods_receipt_line` originally found in J26 itself.

ALTER TABLE stock_movement
    ADD COLUMN goods_receipt_line_id uuid;

ALTER TABLE stock_movement
    ADD CONSTRAINT stock_movement_receipt_line_fk
        FOREIGN KEY (goods_receipt_line_id, tenant_id)
        REFERENCES goods_receipt_line(id, tenant_id),
    -- S3 again, and for the same reason: an internal move has no demand-side
    -- cause and is still a movement.
    ADD CONSTRAINT stock_movement_cause_ck
        CHECK (num_nonnulls(goods_receipt_line_id) <= 1);

CREATE INDEX stock_movement_receipt_line_idx
    ON stock_movement (goods_receipt_line_id) WHERE goods_receipt_line_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 4. RLS and grants
-- ---------------------------------------------------------------------------

ALTER TABLE goods_receipt ENABLE ROW LEVEL SECURITY;
ALTER TABLE goods_receipt FORCE ROW LEVEL SECURITY;
CREATE POLICY goods_receipt_tenant_scoped ON goods_receipt
    USING (tenant_id = current_tenant());

ALTER TABLE goods_receipt_line ENABLE ROW LEVEL SECURITY;
ALTER TABLE goods_receipt_line FORCE ROW LEVEL SECURITY;
CREATE POLICY goods_receipt_line_tenant_scoped ON goods_receipt_line
    USING (tenant_id = current_tenant());

GRANT SELECT ON goods_receipt TO nylonite_app;
GRANT INSERT (id, tenant_id, site_id, purchase_order_id, received_at, recorded_at,
              client_event_id, recorded_by_id, automation_key, note),
      UPDATE (note)
    ON goods_receipt TO nylonite_app;

GRANT SELECT ON goods_receipt_line TO nylonite_app;
GRANT INSERT (id, tenant_id, goods_receipt_id, item_id, expected_supply_id,
              expected_quantity, entered_quantity, entered_packaging_level,
              item_packing_config_id, lot_id, accepted_at, accepted_by_id,
              rejected_at, rejected_by_id, rejected_reason_id, matched_at,
              matched_by_id, recorded_at, client_event_id, recorded_by_id,
              automation_key),
      -- A receipt line is a record of what was counted. Disposition and the late
      -- match are the only things that legitimately move afterwards; the counted
      -- quantity is not one of them.
      UPDATE (accepted_at, accepted_by_id, rejected_at, rejected_by_id,
              rejected_reason_id, matched_at, matched_by_id, expected_supply_id)
    ON goods_receipt_line TO nylonite_app;

GRANT INSERT (goods_receipt_line_id) ON stock_movement TO nylonite_app;
GRANT SELECT ON goods_receipt_line TO nylonite_projection_owner;

-- ---------------------------------------------------------------------------
-- 5. quantity_received finally has a source
-- ---------------------------------------------------------------------------
--
-- J26 as corrected. The original folded `goods_receipt_line.quantity`, which does
-- not exist and never did — the invariant named the right relationship over the
-- wrong table and could not have run. It folds the ledger, grouped by the typed
-- cause arm above.

CREATE OR REPLACE FUNCTION projection_expected_supply_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
BEGIN
    PERFORM set_config('nylonite.tenant_id', p_tenant::text, true);

    WITH ordered AS (
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

    -- D61, J26. The ledger grouped by the receipt line's supply row. Only
    -- movements that put goods somewhere count: a putaway move afterwards names
    -- the same receipt line and must not be counted as a second arrival, which is
    -- the double-count a stored accumulator would have made permanent.
    UPDATE expected_supply e
       SET quantity_received = coalesce(r.q, 0)
      FROM expected_supply c
      LEFT JOIN (SELECT grl.expected_supply_id, sum(m.quantity)::bigint AS q
                   FROM stock_movement m
                   JOIN goods_receipt_line grl ON grl.id = m.goods_receipt_line_id
                  WHERE grl.expected_supply_id IS NOT NULL
                    AND m.from_location_id IS NULL AND m.from_package_id IS NULL
                  GROUP BY grl.expected_supply_id) r ON r.expected_supply_id = c.id
     WHERE e.id = c.id AND e.tenant_id = p_tenant
       AND e.quantity_received IS DISTINCT FROM coalesce(r.q, 0);

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

ALTER FUNCTION projection_expected_supply_rebuild(uuid) OWNER TO nylonite_projection_owner;

COMMENT ON COLUMN expected_supply.quantity_received IS
    '@projection of stock_movement grouped by the receipt line''s supply row, via projection_expected_supply_rebuild (D45, D61, J26).';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('expected_supply', 'quantity_received', 'projection_expected_supply_rebuild');
