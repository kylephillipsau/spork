-- Migration 13: price on an order line, and nothing else.
--
-- Question 127, raised by D44. Metcash's purchase order acknowledgement confirms
-- GTIN, PO line number, price, quantity, pack size, Ti/Hi, unit of measure and
-- shelf life, and "the amount payable is calculated by reference to POA confirmed
-- price and the quantity received". Every one of those had a home except price.
--
-- The reason it was deferred rather than added inline was that a price column
-- looks like it settles a commercial question, and settling one by implication
-- inside an EDI decision is how boundaries move without anyone deciding to move
-- them. So this states the boundary first and then adds two columns.

-- ---------------------------------------------------------------------------
-- What this is not
-- ---------------------------------------------------------------------------
--
-- Not a cost. `stock_movement.unit_cost_minor` stays deferred. What we pay for
-- goods and what a customer pays us are different numbers reached by different
-- means, and the only thing they share is a data type.
--
-- Not a rate. D40 already put `storage_rate` and `handling_rate` under D22's
-- lattice, resolved most-specific-wins over client, item class and site. A rate
-- is a policy that produces a charge. A price is a term of one order, agreed
-- with one counterparty, and putting it in the lattice would make it resolvable
-- for orders that never agreed it.
--
-- Not a total. D40 keeps invoice rendering out and produces charge lines and
-- their inputs. An extended amount is quantity times price, which is derivable,
-- and D25 forbids maintained tables that cannot be rebuilt. S40 asserts the
-- absence rather than trusting it, the same way S36 does for received quantity.
--
-- Not tax. `unit_price_minor` is tax-exclusive, stated here because a column
-- that is silently tax-inclusive somewhere and tax-exclusive elsewhere is a
-- commercial bug with no symptom until an invoice is wrong. Australian B2B trade
-- prices ex-GST and adds it, and the finance system that renders the invoice is
-- where GST belongs.

-- ---------------------------------------------------------------------------
-- Currency, on the order
-- ---------------------------------------------------------------------------
--
-- One order, one currency. A line priced in a different currency to its order is
-- not a thing anyone sells, and the alternative is a currency column on every
-- line agreeing with itself. The existing non-goal stands: currency is recorded
-- and nothing converts.

ALTER TABLE "order" ADD COLUMN currency text;

ALTER TABLE "order"
    ADD CONSTRAINT order_currency_ck
        CHECK (currency IS NULL OR currency ~ '^[A-Z]{3}$');

-- consignment.currency has existed since migration 9 with no such check. The
-- rule is the same rule, and one validated currency column beside an
-- unvalidated one is the inconsistency this record keeps refusing.
ALTER TABLE consignment
    ADD CONSTRAINT consignment_currency_ck
        CHECK (currency IS NULL OR currency ~ '^[A-Z]{3}$');

-- ---------------------------------------------------------------------------
-- Price, on the line
-- ---------------------------------------------------------------------------
--
-- Minor units as an integer, matching consignment.price_minor and
-- record_scheme_field's money_minor. No float and no rounding surprise.
--
-- The basis quantity is what makes integer minor units survive contact with
-- grocery. A price of 3.45 per 100 is exact as (345, 100); as a per-each price
-- it is 0.0345, which cents cannot express and which would force either a
-- decimal type or a silent rounding. It also matches how the commercial document
-- reads, since a counterparty quotes a price per some quantity rather than per
-- each, and EDIFACT's PRI segment carries exactly this pair.

ALTER TABLE order_line
    ADD COLUMN unit_price_minor     bigint,
    ADD COLUMN price_basis_quantity bigint;

ALTER TABLE order_line
    -- Either the line is priced or it is not. A price with no basis is
    -- ambiguous by exactly the factor that matters.
    ADD CONSTRAINT order_line_price_pair_ck
        CHECK (num_nonnulls(unit_price_minor, price_basis_quantity) <> 1),
    ADD CONSTRAINT order_line_price_basis_ck
        CHECK (price_basis_quantity IS NULL OR price_basis_quantity > 0),
    -- Zero is a real price and negative is not. A free line is priced at zero
    -- and a credit is not an order line.
    ADD CONSTRAINT order_line_price_sign_ck
        CHECK (unit_price_minor IS NULL OR unit_price_minor >= 0);

COMMENT ON COLUMN order_line.unit_price_minor IS
    'Tax-exclusive, in minor units of the order''s currency, per price_basis_quantity units. The price in force, not the price as received.';
COMMENT ON COLUMN order_line.price_basis_quantity IS
    'The quantity unit_price_minor is quoted per. 345 per 100 rather than 0.0345 each.';

-- ---------------------------------------------------------------------------
-- Where the counterparty's price goes, which is not here
-- ---------------------------------------------------------------------------
--
-- Metcash's wording separates two numbers: the price on their purchase order,
-- and the confirmed price on our acknowledgement, which is the one payment
-- references. They can differ, and an acknowledgement that could not differ from
-- the order would not be worth sending.
--
-- Only the second belongs on the line. `order_line.unit_price_minor` is the term
-- in force. The price as received is a statement of record exchanged with
-- another party, stored exactly as exchanged, which neither side may
-- unilaterally revise, and D21 already built the table for that. It lands on
-- `assertion` when the inbound migration exists, and J18's shape applies: a
-- promoted value still equals its assertion's.
--
-- So this adds one column rather than an ordered/confirmed pair. A second column
-- here would be the assertion layer built badly and in the wrong place, and it
-- would have no writer until inbound EDI exists anyway.
--
-- What is missing is the amendment path. `intention_amendment` is
-- `order_id`-only with four order-level covered columns, so a price change to a
-- line has nowhere to go and today moves by UPDATE like any other intention
-- column. Once amendments reach lines, this becomes a @projection under D42 and
-- D48's revision_class distinguishes a renegotiation from a mis-keyed price.
-- That is question 135.
