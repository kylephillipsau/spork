-- Migration 14: the amendment reaches the line.
--
-- Question 135, raised by D50 and by the schema failing to hold what D50 had
-- just put in it. `intention_amendment` is `order_id`-only with four order-level
-- covered columns, so a change to a line's price or quantity has nowhere to go
-- and moves by UPDATE like an ordinary intention column. Migration 13 added
-- `unit_price_minor` and could not make it a `@projection`, because there was no
-- amendment that could name it.
--
-- D42's own sketch listed `quantity_changed`, `line_added` and `line_removed` in
-- a `kind` enum the built table does not have. The schema has been narrower than
-- the decision that specified it since migration 9, and price is simply the first
-- thing to run into it.
--
-- What makes this worth a migration rather than a column: a line's quantity and
-- price are the two numbers a counterparty disputes. "We ordered 40, not 60" and
-- "we agreed 3.20, not 3.45" are the arguments that actually happen, and under
-- the current schema the answer to both is an UPDATE with no author, no moment
-- and no reason.

-- ---------------------------------------------------------------------------
-- 1. A line can be removed, which today it cannot
-- ---------------------------------------------------------------------------
--
-- `order_line_quantity_ck` forbids zero, and the application has never had DELETE
-- on `order_line`, so a cancelled line is currently inexpressible: there is no
-- value it can take and no verb that removes it. That is not conservatism, it is
-- a gap, and it is why D42's `line_removed` had nowhere to land.
--
-- Removal is a state rather than a deletion because `fulfilment_line` references
-- the line by foreign key and J31 folds allocations across it. A DELETE would
-- either fail or take the commitment with it, and a commitment that was made is a
-- fact about what the floor was told to do even after the customer changes their
-- mind.

CREATE TYPE order_line_state AS ENUM ('active', 'removed');

ALTER TABLE order_line
    ADD COLUMN line_state order_line_state NOT NULL DEFAULT 'active';

-- The default stays, unlike `revision_class`'s in migration 11, and the
-- difference is worth stating because the two look alike.
--
-- S39 asserts the *absence* of a default on `revision_class` because there the
-- common value is the dangerous one: a correction written without thinking
-- becomes a `world_event` and sorts by the wrong moment. Here there is exactly
-- one legal value at insert. A line is created active; it becomes removed only by
-- an amendment saying who removed it and when. The default is the base of the
-- fold, not a guess standing in for a decision.

COMMENT ON COLUMN order_line.line_state IS
    '@projection of intention_amendment via projection_order_rebuild (D42, D51, J46). '
    'A removed line keeps its row: fulfilment_line references it and a commitment '
    'that was made stays a fact after the customer changes their mind.';

-- ---------------------------------------------------------------------------
-- 2. The subject an amendment names
-- ---------------------------------------------------------------------------
--
-- The composite key exists so the foreign key below can be composite. Without it
-- an amendment could name `order` A and `order_line` B belonging to order C, and
-- the fold would write B while every report about A read consistent. Same idiom
-- as `location.zone_id`'s composite FK through `site_id`, which S37 asserts.

ALTER TABLE order_line
    ADD CONSTRAINT order_line_order_key UNIQUE (id, order_id);

ALTER TABLE intention_amendment
    ADD COLUMN order_line_id uuid,

    -- The covered line columns, each nullable on the same rule as the order-level
    -- four: an amendment sets what it changed and leaves the rest alone.
    ADD COLUMN new_quantity_ordered     bigint,
    ADD COLUMN new_unit_price_minor     bigint,
    ADD COLUMN new_price_basis_quantity bigint,
    ADD COLUMN new_line_state           order_line_state;

ALTER TABLE intention_amendment
    ADD CONSTRAINT intention_amendment_line_fk
        FOREIGN KEY (order_line_id, order_id) REFERENCES order_line(id, order_id);

-- One amendment names one subject: an order or one of its lines, never both.
--
-- The alternative was to let a single amendment carry a promised-window change
-- and a line quantity together, on the grounds that one person did both in one
-- keystroke. It is refused because the fold groups by subject, and a row that is
-- in both groups has to be folded twice under two different keys to mean what it
-- says. Two facts from one keystroke share a `client_event_id`, which is what
-- that column is for, and the fold stays one pass per subject.
ALTER TABLE intention_amendment
    ADD CONSTRAINT intention_amendment_subject_ck
        CHECK (CASE WHEN order_line_id IS NULL
                    THEN num_nonnulls(new_quantity_ordered, new_unit_price_minor,
                                      new_price_basis_quantity, new_line_state) = 0
                    ELSE num_nonnulls(new_promised_from, new_promised_to,
                                      new_required_by, new_state) = 0
               END);

-- ---------------------------------------------------------------------------
-- 3. What keeps the per-column fold from breaking the pair
-- ---------------------------------------------------------------------------
--
-- D50 made a price two columns: `(345, 100)` is 3.45 per 100, and 345 alone is
-- ambiguous by exactly the factor that matters. `order_line_price_pair_ck`
-- enforces that on the line, both set or neither.
--
-- D42's fold is per column, so nothing stops an amendment setting the price and
-- not the basis: fold that onto an unpriced line and the result is `(320, NULL)`,
-- which the line's own CHECK rejects, and the rebuild fails on data that was
-- legal when it was written. The projection would be unrebuildable, which is the
-- one thing D25 does not tolerate.
--
-- The fix is a row-level rule on the amendment rather than a special case in the
-- fold: an amendment that touches the price states both halves of it. Then the
-- per-column fold can never separate them, because no row ever carried one
-- without the other. The constraint is what makes the general mechanism safe,
-- which is the same move `intention_amendment_actor_ck` already makes.
ALTER TABLE intention_amendment
    ADD CONSTRAINT intention_amendment_price_pair_ck
        CHECK (num_nonnulls(new_unit_price_minor, new_price_basis_quantity) <> 1),

    -- The line's own domain rules, restated on the amendment. Folding a value the
    -- target column would reject is the same unrebuildable projection by another
    -- route, and catching it at write time names the amendment that did it.
    ADD CONSTRAINT intention_amendment_quantity_ck
        CHECK (new_quantity_ordered IS NULL OR new_quantity_ordered > 0),
    ADD CONSTRAINT intention_amendment_price_basis_ck
        CHECK (new_price_basis_quantity IS NULL OR new_price_basis_quantity > 0),
    ADD CONSTRAINT intention_amendment_price_sign_ck
        CHECK (new_unit_price_minor IS NULL OR new_unit_price_minor >= 0);

-- Widened from the four order-level columns. An amendment that changes nothing is
-- a row with an author, a moment and no content, and it would fold to a no-op
-- while reading as a change in every history query.
ALTER TABLE intention_amendment
    DROP CONSTRAINT intention_amendment_changes_something_ck;

ALTER TABLE intention_amendment
    ADD CONSTRAINT intention_amendment_changes_something_ck
        CHECK (num_nonnulls(new_promised_from, new_promised_to, new_required_by,
                            new_state, new_quantity_ordered, new_unit_price_minor,
                            new_price_basis_quantity, new_line_state) > 0);

CREATE INDEX intention_amendment_line_idx
    ON intention_amendment (tenant_id, order_line_id)
    WHERE order_line_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 4. Mark, register, grant: the three things migration 9 did not do
-- ---------------------------------------------------------------------------
--
-- D49's list, applied on the way in this time rather than three migrations later.
-- Migration 12 had to go back and do it for `order`, and S5's three-way diff now
-- fails loudly if any leg is skipped.

COMMENT ON COLUMN order_line.quantity_ordered IS
    '@projection of intention_amendment via projection_order_rebuild (D42, D51, J46). '
    'The quantity in force. The original arrives on INSERT and is the base of the fold.';
COMMENT ON COLUMN order_line.unit_price_minor IS
    '@projection of intention_amendment via projection_order_rebuild (D42, D51, J46). '
    'Tax-exclusive, in minor units of the order''s currency, per price_basis_quantity '
    'units. The price in force, not the price as received.';
COMMENT ON COLUMN order_line.price_basis_quantity IS
    '@projection of intention_amendment via projection_order_rebuild (D42, D51, J46). '
    'The quantity unit_price_minor is quoted per. 345 per 100 rather than 0.0345 each.';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('order_line', 'quantity_ordered',     'projection_order_rebuild'),
    ('order_line', 'unit_price_minor',     'projection_order_rebuild'),
    ('order_line', 'price_basis_quantity', 'projection_order_rebuild'),
    ('order_line', 'line_state',           'projection_order_rebuild');

-- `order_line` has carried a table-wide INSERT and UPDATE since migration 9, on
-- the same grant statement as `order`, which is why migration 12 found one and
-- not the other.
REVOKE INSERT, UPDATE ON order_line FROM spork_app;

-- INSERT covers the original values, because the fold coalesces onto the row
-- rather than replacing it, so an order line's first quantity and price are the
-- base of the fold and not an amendment.
--
-- `line_state` is absent from INSERT, which is where it differs from `order`'s
-- four. A line has one legal state at creation and the default supplies it; a
-- grant would let the application insert a line already removed, which is not a
-- fact about anything.
--
-- UPDATE is `line_number` alone. Everything else on an order line is either the
-- subject of an amendment or the identity of the line itself: changing `item_id`
-- is not an edit, it is a different line, and expressing it as one keeps the
-- history honest. So after this, every number on an order line that a
-- counterparty would argue about moves only through a fact with an author.
GRANT INSERT (id, tenant_id, order_id, item_id, quantity_ordered, line_number,
              unit_price_minor, price_basis_quantity),
      UPDATE (line_number)
    ON order_line TO spork_app;

-- ---------------------------------------------------------------------------
-- 5. The fold, extended to the second subject
-- ---------------------------------------------------------------------------
--
-- One function rather than a `projection_order_line_rebuild` beside it. It is one
-- fold of one source table under one ordering, and splitting it would give two
-- maintainers that must agree about `(occurred_at, recorded_at, id)` forever. The
-- registry already allows one function to own columns on several tables.
--
-- The return value now counts rows touched on both tables. Migration 9 counted
-- orders, and a caller reading it as "orders amended" would be wrong either way
-- once lines exist.

CREATE OR REPLACE FUNCTION projection_order_rebuild(p_tenant uuid)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    touched bigint;
    touched_lines bigint;
BEGIN
    PERFORM set_config('spork.tenant_id', p_tenant::text, true);

    -- Last writer per covered column, in register order. Not one winning row:
    -- an amendment that changed only the promised window must not clear a state
    -- set by an earlier one, which is why the fold is per column rather than per
    -- row. Same shape as J6 and the same ordering.
    WITH folded AS (
        SELECT order_id,
               (array_remove(array_agg(new_promised_from ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS promised_from,
               (array_remove(array_agg(new_promised_to ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS promised_to,
               (array_remove(array_agg(new_required_by ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS required_by,
               (array_remove(array_agg(new_state ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS state
          FROM intention_amendment
         WHERE tenant_id = p_tenant
           AND order_line_id IS NULL
         GROUP BY order_id
    ),
    updated AS (
        UPDATE "order" o
           SET promised_from = COALESCE(f.promised_from, o.promised_from),
               promised_to   = COALESCE(f.promised_to,   o.promised_to),
               required_by   = COALESCE(f.required_by,   o.required_by),
               state         = COALESCE(f.state,         o.state)
          FROM folded f
         WHERE o.id = f.order_id AND o.tenant_id = p_tenant
        RETURNING o.id)
    SELECT count(*) INTO touched FROM updated;

    -- D51. The same fold under the same ordering, grouped by line.
    --
    -- The price pair travels together because `intention_amendment_price_pair_ck`
    -- guarantees no row ever set one half alone, so folding the halves separately
    -- can never produce a combination no amendment wrote.
    WITH folded_line AS (
        SELECT order_line_id,
               (array_remove(array_agg(new_quantity_ordered ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS quantity_ordered,
               (array_remove(array_agg(new_unit_price_minor ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS unit_price_minor,
               (array_remove(array_agg(new_price_basis_quantity ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS price_basis_quantity,
               (array_remove(array_agg(new_line_state ORDER BY occurred_at DESC,
                    recorded_at DESC, id DESC), NULL))[1] AS line_state
          FROM intention_amendment
         WHERE tenant_id = p_tenant
           AND order_line_id IS NOT NULL
         GROUP BY order_line_id
    ),
    updated_line AS (
        UPDATE order_line l
           SET quantity_ordered     = COALESCE(f.quantity_ordered,     l.quantity_ordered),
               unit_price_minor     = COALESCE(f.unit_price_minor,     l.unit_price_minor),
               price_basis_quantity = COALESCE(f.price_basis_quantity, l.price_basis_quantity),
               line_state           = COALESCE(f.line_state,           l.line_state)
          FROM folded_line f
         WHERE l.id = f.order_line_id AND l.tenant_id = p_tenant
        RETURNING l.id)
    SELECT count(*) INTO touched_lines FROM updated_line;

    RETURN touched + touched_lines;
END
$$;

ALTER FUNCTION projection_order_rebuild(uuid) OWNER TO spork_projection_owner;
REVOKE EXECUTE ON FUNCTION projection_order_rebuild(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION projection_order_rebuild(uuid)
    TO spork_scheduler, spork_platform;

-- The maintainer could read `order_line` and not write it, which would have made
-- the rebuild fail at the first amended line rather than at deploy.
GRANT SELECT, UPDATE ON order_line TO spork_projection_owner;
