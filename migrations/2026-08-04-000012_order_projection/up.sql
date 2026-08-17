-- Migration 12: order's projection columns were demoted in prose and nowhere else.
--
-- Found while answering question 127, which wanted a price column on `order_line`
-- and a currency on `order`. Deciding the grant for a new column on `order`
-- meant reading the existing one, and the existing one is wrong.
--
-- D42 demoted four `order` columns to `@projection` of `intention_amendment`, and
-- `projection_order_rebuild` duly maintains them: it folds the amendments and
-- UPDATEs promised_from, promised_to, required_by and state. What migration 9
-- did not do is any of the three things that make a projection real:
--
--   1. register the columns in `projection_rebuild`
--   2. mark them with an `@projection` column comment
--   3. grant UPDATE by column instead of by table
--
-- Migration 9 wrote the comment explaining exactly this, four lines below the
-- grant that breaks it: "Column-level wherever a table carries a projection, per
-- D25 and J36. A table-wide UPDATE here would let the application write the very
-- columns the maintainer exists to own, and the guard would still read as though
-- it were on." It then applied that to fulfilment, fulfilment_line and
-- consignment, and left `order` on a table-wide grant.
--
-- The failure is the inverse of the one J36 caught on `package`. There the marker
-- existed and the grant was wrong, so J36 reported it. Here the marker is missing,
-- so J36 cannot see the columns at all and passes while the exact thing it exists
-- to prevent is true. A guard is only as good as the marking, and until now
-- nothing checked the marking.
--
-- S5 is the check that would have caught it, and it has been sitting at
-- Check::Pending("the rebuild function registry does not exist yet") since before
-- the registry was built. The registry exists and holds eighteen rows. That is a
-- third failure mode for this suite, after "the check was wrong" and "the check
-- examined nothing": a check whose stated precondition has since been met, still
-- doing nothing, still reading as deliberate.

-- ---------------------------------------------------------------------------
-- 1. Register
-- ---------------------------------------------------------------------------

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('order', 'promised_from', 'projection_order_rebuild'),
    ('order', 'promised_to',   'projection_order_rebuild'),
    ('order', 'required_by',   'projection_order_rebuild'),
    ('order', 'state',         'projection_order_rebuild');

-- ---------------------------------------------------------------------------
-- 2. Mark
-- ---------------------------------------------------------------------------

COMMENT ON COLUMN "order".promised_from IS
    '@projection of intention_amendment via projection_order_rebuild (D42, J46).';
COMMENT ON COLUMN "order".promised_to IS
    '@projection of intention_amendment via projection_order_rebuild (D42, J46).';
COMMENT ON COLUMN "order".required_by IS
    '@projection of intention_amendment via projection_order_rebuild (D42, J46).';
COMMENT ON COLUMN "order".state IS
    '@projection of intention_amendment via projection_order_rebuild (D42, J46).';

-- ---------------------------------------------------------------------------
-- 3. Grant by column
-- ---------------------------------------------------------------------------
--
-- The four projection columns are absent from both lists. An order's initial
-- values still arrive on INSERT, because the fold coalesces onto what is already
-- there rather than replacing it, so the original is the base of the fold and not
-- an amendment. After that they move only through intention_amendment.

REVOKE UPDATE ON "order" FROM nylonite_app;
REVOKE INSERT ON "order" FROM nylonite_app;

GRANT INSERT (id, tenant_id, site_id, customer_party_id, confirmation_number,
              source_channel, external_ref, supersedes_order_id, placed_at,
              promised_from, promised_to, required_by, state),
      UPDATE (site_id, customer_party_id, confirmation_number, external_ref,
              supersedes_order_id)
    ON "order" TO nylonite_app;

-- ---------------------------------------------------------------------------
-- 4. The two states the marker was conflating
-- ---------------------------------------------------------------------------
--
-- Running the three-way diff for the first time found two more, and they are the
-- same mistake in opposite directions.
--
-- `consignment.status`, `.eta` and `.price_minor` are commented @projection of
-- the in-force carrier advice. Carrier advice arrives with inbound EDI and does
-- not exist, so there is no function and no registry row, and the two sides
-- agreed by being equally empty exactly as `order`'s did.
--
-- `fulfilment.progress` and `fulfilment_line.allocated_quantity` are commented
-- and registered against `projection_fulfilment_rebuild`, which migration 9
-- never wrote. The registry has been naming a function that does not exist since
-- the day it was added.
--
-- Both are the same conflation. `@projection` was carrying two claims at once:
-- that a rebuild function owns this column, and that the application must never
-- write it. The second is true and useful now for all five columns; the first is
-- not true of any of them yet. So the marker splits, and the guards that matter
-- keep working because they match on the prefix.
--
--   @projection            owned by a live rebuild, registered, diffed three ways
--   @projection(pending)   protected from the app, maintainer not yet written
--
-- Nothing here invents a rebuild function to make a check pass. Writing
-- `projection_fulfilment_rebuild` is D15 work with a real decision in it, and
-- guessing at it inside a migration named for something else is how the
-- registry came to name it in the first place.

COMMENT ON COLUMN consignment.status IS
    '@projection(pending) of the in-force carrier advice; no maintainer until inbound EDI exists.';
COMMENT ON COLUMN consignment.eta IS
    '@projection(pending) of the in-force carrier advice; no maintainer until inbound EDI exists.';
COMMENT ON COLUMN consignment.price_minor IS
    '@projection(pending) of the in-force carrier advice; no maintainer until inbound EDI exists.';

COMMENT ON COLUMN fulfilment.progress IS
    '@projection(pending) of fulfilment_line; projection_fulfilment_rebuild was registered by migration 9 and never written. Question 134.';
COMMENT ON COLUMN fulfilment_line.allocated_quantity IS
    '@projection(pending) of stock_allocation; same missing maintainer. Question 134.';

DELETE FROM projection_rebuild WHERE function_name = 'projection_fulfilment_rebuild';
