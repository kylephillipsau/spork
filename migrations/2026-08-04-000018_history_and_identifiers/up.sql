-- Migration 18: the columns you cannot add later, and the keys you cannot change later.
--
-- Two decisions in one migration because they share one argument. Everything here
-- is cheap today and expensive or impossible once rows exist, which is the test
-- `migration-1.md` applied to its tier-0 list and never applied to itself.

-- ===========================================================================
-- D57. Identifiers become time-ordered
-- ===========================================================================
--
-- Question 141. Forty-three columns default to `gen_random_uuid()`, which is
-- version 4 and therefore random, so every insert lands at an arbitrary point in
-- the primary key's B-tree instead of at its right edge. The published cost at
-- scale is roughly a quarter more index, substantial write amplification, and
-- bulk loads an order of magnitude slower.
--
-- UUIDv7 puts a millisecond timestamp in the high bits, so inserts append while
-- the value stays generatable anywhere. That second property is not incidental
-- here: D5 has a handheld observing reality and minting identifiers for what it
-- saw, offline, with no round trip. A sequence cannot do that, which is why this
-- is v7 rather than bigint.
--
-- Postgres 18 has `uuidv7()` natively, which is why the deployment moved.
--
-- Derived from the catalogue rather than listed. A list of forty-three columns
-- would be correct today and short by one the next time somebody adds a table.

DO $$
DECLARE
    r record;
    n integer := 0;
BEGIN
    FOR r IN
        SELECT c.relname AS tbl, a.attname AS col
          FROM pg_class c
          JOIN pg_namespace ns ON ns.oid = c.relnamespace
          JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
          JOIN pg_attrdef d ON d.adrelid = c.oid AND d.adnum = a.attnum
         WHERE ns.nspname = 'public'
           AND c.relkind = 'r'
           AND pg_get_expr(d.adbin, d.adrelid) = 'gen_random_uuid()'
    LOOP
        EXECUTE format('ALTER TABLE %I ALTER COLUMN %I SET DEFAULT uuidv7()', r.tbl, r.col);
        n := n + 1;
    END LOOP;
    RAISE NOTICE 'moved % identifier default(s) from gen_random_uuid() to uuidv7()', n;
END
$$;

-- What this does not settle. The server generates identifiers for rows it
-- creates; the floor generates them for everything it observes, and that half
-- lives in a handheld that does not exist yet. A server-side default alone would
-- leave the fact tables — the ones this is for — taking random keys from the
-- writer that produces most of their rows. Question 141 carries the remainder and
-- names the `uuid` crate's `v7` feature as the other side of it.
--
-- S47 asserts the absence of `gen_random_uuid()` rather than the presence of
-- `uuidv7()`, because the failure mode is a new table created from the old
-- pattern, not an existing one reverting.

-- ===========================================================================
-- D58. The columns whose absence destroys history
-- ===========================================================================
--
-- Three items off `migration-1.md`'s outstanding list. What they have in common is
-- not that they are useful, it is that **the information they would carry is
-- unrecoverable once the moment passes**. A column added later starts empty and
-- every row written before it is permanently silent on the subject.

-- ---------------------------------------------------------------------------
-- Where a lot came from, and when it was made
-- ---------------------------------------------------------------------------
--
-- Zero mentions in the decision record before now. Not backfillable in the
-- strongest sense: nobody can reconstruct where a lot came from after the pallet
-- has gone, and no supplier will answer the question a year later.
--
-- It belongs on `lot` rather than on `item` because it varies lot to lot for
-- food — the same item arrives from Malaysia in March and Vietnam in June — which
-- is the reason the inbound pass put it here. Coles requires country of origin on
-- every master carton, so this is also a labelling input rather than a nicety.

ALTER TABLE lot
    ADD COLUMN country_of_origin text,
    ADD COLUMN production_date   date;

-- ISO 3166-1 alpha-2, checked in the same shape as D50's currency check. Two
-- letters is a format, not a vocabulary: a country table would be a reference
-- list to maintain for a value that is validated by shape and consumed as a
-- label.
ALTER TABLE lot
    ADD CONSTRAINT lot_country_of_origin_ck
        CHECK (country_of_origin IS NULL OR country_of_origin ~ '^[A-Z]{2}$'),
    -- A lot made after it expires is a keying error, and it is the one date
    -- relationship that is always wrong rather than merely unusual.
    ADD CONSTRAINT lot_production_before_expiry_ck
        CHECK (production_date IS NULL OR expiry_date IS NULL
               OR production_date <= expiry_date);

COMMENT ON COLUMN lot.country_of_origin IS
    'ISO 3166-1 alpha-2. On the lot rather than the item because it varies lot to '
    'lot for food, and it is a master carton labelling input. D58.';
COMMENT ON COLUMN lot.production_date IS
    'When the goods were made, which is not when we received them. D58.';

-- ---------------------------------------------------------------------------
-- What the goods cost, which is not what they sell for
-- ---------------------------------------------------------------------------
--
-- `migration-1.md` item 10, deferred since it was written and unblocked by D50,
-- which drew the boundary: a cost is not a price. What we pay for goods and what
-- a customer pays us are different numbers reached by different means.
--
-- The shape mirrors D50's exactly, and for the same reason: a supplier quotes per
-- some quantity as readily as a customer is quoted per some quantity, so minor
-- units alone are ambiguous by the factor that matters.
--
-- **The currency sits in the row, and that is the interesting difference.** On an
-- order the money is on the line and the currency is on the order, so no CHECK
-- can reach both and J54 has to be a job. A movement has no parent carrying a
-- currency, so the pairing is expressible where it belongs — in a constraint that
-- cannot be violated rather than a check that reports afterwards.

ALTER TABLE stock_movement
    ADD COLUMN unit_cost_minor    bigint,
    ADD COLUMN cost_basis_quantity bigint,
    ADD COLUMN cost_currency      text;

ALTER TABLE stock_movement
    -- All three or none. A cost with no basis is ambiguous; a cost with no
    -- currency is a number with no units.
    ADD CONSTRAINT stock_movement_cost_triple_ck
        CHECK (num_nonnulls(unit_cost_minor, cost_basis_quantity, cost_currency) IN (0, 3)),
    ADD CONSTRAINT stock_movement_cost_basis_ck
        CHECK (cost_basis_quantity IS NULL OR cost_basis_quantity > 0),
    -- Zero is a real cost: free goods, samples, a supplier's replacement for a
    -- short delivery. Negative is not.
    ADD CONSTRAINT stock_movement_cost_sign_ck
        CHECK (unit_cost_minor IS NULL OR unit_cost_minor >= 0),
    ADD CONSTRAINT stock_movement_cost_currency_ck
        CHECK (cost_currency IS NULL OR cost_currency ~ '^[A-Z]{3}$');

COMMENT ON COLUMN stock_movement.unit_cost_minor IS
    'What we paid for the goods, tax-exclusive, in minor units of cost_currency, '
    'per cost_basis_quantity units. Not a price (D50), not a landed cost, not a '
    'valuation. Nullable: most movements buy nothing. D58.';

-- What a cost is not, stated here because a cost column silently changing meaning
-- is a commercial bug with no symptom until a margin is wrong.
--
--   Not a landed cost. Freight and duty apportioned across lines is a computation
--   over facts that live elsewhere, and D40 keeps charge lines and their inputs
--   rather than the rendering. Storing an apportioned number here would make it
--   disagree with its own inputs the moment a freight invoice is corrected, which
--   is what S36 and S40 refuse in their own areas.
--
--   Not a valuation. What the stock is worth now is a finance question over a
--   costing method nobody here has chosen, and D40's non-goals put stock
--   valuation outside this system.
--
--   Not tax-inclusive, for D50's reason exactly.
--
-- S48 asserts the absence rather than trusting the comment.

-- ---------------------------------------------------------------------------
-- What was actually keyed, and what turned it into a number
-- ---------------------------------------------------------------------------
--
-- `migration-1.md` item 4, and the half that bites is the packing config.
--
-- Somebody scans three cases. The ledger stores 36, because `stock_movement.quantity`
-- is in the item's base unit and everything downstream depends on that. Correct a
-- case pack from twelve to twenty-four next year and every historical 36 silently
-- becomes a different number of cases. D23 already versions `item_packing_config`
-- by `effective_from` for exactly this reason; without the foreign key that
-- versioning protects nothing, because nothing records which version applied.
--
-- The list called the middle column `entered_unit`, and it is not a unit. A case
-- is not a dimensional unit: its factor varies by item and by date, which is the
-- whole reason `item_packing_config` exists and is versioned. `packaging_level`
-- is the vocabulary D43 already established for exactly this, so it is reused
-- rather than reinvented.

ALTER TABLE stock_movement
    ADD COLUMN entered_quantity        bigint,
    ADD COLUMN entered_packaging_level packaging_level,
    ADD COLUMN item_packing_config_id  uuid;

ALTER TABLE stock_movement
    -- Composite through tenant, per S37's idiom and J14's lesson: without it a
    -- movement can name another tenant's packing config and translate its
    -- quantity by somebody else's case size.
    ADD CONSTRAINT stock_movement_packing_config_fk
        FOREIGN KEY (item_packing_config_id, tenant_id)
        REFERENCES item_packing_config(id, tenant_id),
    -- The quantity and what it was counted in travel together.
    ADD CONSTRAINT stock_movement_entered_pair_ck
        CHECK (num_nonnulls(entered_quantity, entered_packaging_level) <> 1),
    ADD CONSTRAINT stock_movement_entered_positive_ck
        CHECK (entered_quantity IS NULL OR entered_quantity > 0),
    -- Any level above `each` needs the config that converted it. At `each` there
    -- is nothing to convert and demanding a config would make the common case
    -- carry a reference to nothing.
    ADD CONSTRAINT stock_movement_entered_needs_config_ck
        CHECK (entered_packaging_level IS NULL
               OR entered_packaging_level = 'each'
               OR item_packing_config_id IS NOT NULL),
    -- And a config with no level is a reference nobody can use.
    ADD CONSTRAINT stock_movement_config_needs_level_ck
        CHECK (item_packing_config_id IS NULL OR entered_packaging_level IS NOT NULL);

COMMENT ON COLUMN stock_movement.entered_quantity IS
    'What the operator actually keyed or scanned, before conversion to the base '
    'unit. quantity is the truth; this is the evidence for it. D58.';
COMMENT ON COLUMN stock_movement.item_packing_config_id IS
    'The packing config version that converted entered_quantity into quantity. '
    'Without it, correcting a case pack rewrites what every historical receipt '
    'meant. D23 versions the config by effective_from; this is what makes that '
    'versioning load-bearing. D58, J57.';

CREATE INDEX stock_movement_packing_config_idx
    ON stock_movement (item_packing_config_id)
    WHERE item_packing_config_id IS NOT NULL;
