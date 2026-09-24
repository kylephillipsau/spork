-- Migration 47: a claim can say cartons, and we keep the word they used.
--
-- D93, settling question 159.
--
-- The question was "whether a claim can say cartons". The research answer is that
-- the standards keep exactly the split this schema keeps -- **UN/ECE
-- Recommendation 20 is units of measurement and Recommendation 21 is types of
-- cargo, packages and packaging materials**, two lists, and Rec 21 says its codes
-- "may be used in combination with a data element specifying unit of measurement".
-- `unit` and `packaging_level` are that division. D58 reached it independently.
--
-- Both message families put a code from those lists beside the quantity. X12
-- 856's SN1 pairs SN102 with SN103 -- CA for case, EA for each. EDIFACT DESADV's
-- QTY is `qualifier:quantity:measureUnitCode` -- PCE, CT, PK, defaulting to PCE.
-- So "40 CT" is ordinary traffic, and today it lands in `entered_unit_id`, an FK
-- to a table whose entire `count` dimension is one row, `ea` at factor 1/1.
--
-- **The defect underneath is smaller and worse than the question as filed.** D58's
-- correction -- *"The list called the middle column `entered_unit`, and it is not
-- a unit"* -- was applied to `stock_movement` in migration 18 and
-- `goods_receipt_line` in migration 21. `asserted_unit_content` did not exist yet.
-- Migration 34 then built it from D45's pre-D58 sketch and nobody swept back.
--
-- Which left a **Rule 5 violation in the one table category that exists to prevent
-- it**. D21: *"Recorded in the author's vocabulary. Resolution into ours is a
-- separate, fallible, recorded step."* Every other counterparty value here has a
-- raw twin -- `raw_gtin`, `raw_item_code`, `raw_po_reference`,
-- `raw_po_line_number` -- and `asserted_unit.level_code` is kept raw as received.
-- The unit code was the only one converted on write with the original discarded.

-- ---------------------------------------------------------------------------
-- 1. What they said
-- ---------------------------------------------------------------------------
--
-- Verbatim, unnormalised, never interpreted. Precedent is the sibling column
-- `asserted_unit.raw_package_type_code`.
--
-- No code-list column beside it, and that is a judgement rather than an omission:
-- X12 and Rec 21 overlap in spelling without agreeing in meaning, so the list a
-- code came from is genuinely load-bearing. It is knowable from
-- `party_message` -- the artefact records its own standard -- and duplicating it
-- here would be a second place for it to be wrong. Question 160.

ALTER TABLE asserted_unit_content
    ADD COLUMN raw_unit_code text;

COMMENT ON COLUMN asserted_unit_content.raw_unit_code IS
    'The unit or package-type code exactly as the author wrote it -- CT, CA, PCE, '
    'KGM. Immutable, uninterpreted, and the only record of what they actually '
    'said: before D93 the code was resolved on write and discarded, which is the '
    'one thing D21 rule 5 forbids. D21, D93.';

-- ---------------------------------------------------------------------------
-- 2. Our reading of it, in whichever vocabulary it names
-- ---------------------------------------------------------------------------
--
-- `entered_unit_id` is renamed. It was never an entered value: it is our mapping
-- of their code into our dimensional vocabulary, and calling it `entered_` is what
-- made a claim of 400 base units against "40 ea" look like a legible row for as
-- long as the assertion tables have existed. The pair is now `raw_unit_code` ->
-- one of two resolutions, which is the shape every other column on this table
-- already had.

ALTER TABLE asserted_unit_content
    RENAME COLUMN entered_unit_id TO resolved_unit_id;

COMMENT ON COLUMN asserted_unit_content.resolved_unit_id IS
    'Our reading of raw_unit_code when it names a dimensional unit -- a catch '
    'weight in KGM. Renamed from entered_unit_id by D93: it is a resolution and '
    'never was an entered value. D21, D93.';

-- The other arm. A carton is not a dimension member -- D58 settled that, and Rec
-- 21 existing as a separate list is the standards body agreeing.
ALTER TABLE asserted_unit_content
    ADD COLUMN resolved_packaging_level packaging_level,
    ADD COLUMN item_packing_config_id   uuid,

    -- S3's `<= 1`, a third time. Exactly one reading, or none yet.
    ADD CONSTRAINT asserted_unit_content_one_vocabulary_ck
        CHECK (num_nonnulls(resolved_unit_id, resolved_packaging_level) <= 1),

    -- The same pair of implications `goods_receipt_line` carries, for the same
    -- reason: a level above `each` means nothing without the case pack that
    -- sizes it, and a config with no level to apply it to is a dangling
    -- reference.
    ADD CONSTRAINT asserted_unit_content_level_needs_config_ck
        CHECK (resolved_packaging_level IS NULL
               OR resolved_packaging_level = 'each'
               OR item_packing_config_id IS NOT NULL),
    ADD CONSTRAINT asserted_unit_content_config_needs_level_ck
        CHECK (item_packing_config_id IS NULL
               OR resolved_packaging_level IS NOT NULL),

    -- Composite through tenant_id, J14's lesson and the same shape the other two
    -- tables use: without it a claim converts by another tenant's carton size.
    ADD CONSTRAINT asserted_unit_content_config_fk
        FOREIGN KEY (item_packing_config_id, tenant_id)
        REFERENCES item_packing_config(id, tenant_id);

COMMENT ON COLUMN asserted_unit_content.resolved_packaging_level IS
    'Our reading of raw_unit_code when it names a package type rather than a unit '
    '-- CT, CA, PF. The two halves of a receipt speak one vocabulary from here: '
    'this is the column goods_receipt_line has carried since migration 21. D93.';

COMMENT ON COLUMN asserted_unit_content.item_packing_config_id IS
    'The case-pack version that sizes the declared level, so a corrected case '
    'pack cannot rewrite what a claim meant. D58''s argument, third table. D93.';

CREATE INDEX asserted_unit_content_config_idx
    ON asserted_unit_content (item_packing_config_id)
    WHERE item_packing_config_id IS NOT NULL;

-- Ingestion writes all three. The app still has no UPDATE on this table, because
-- a claim is immutable (D77) and a re-resolution goes through
-- `asserted_unit_content_resolve` under D90's freeze.
GRANT INSERT (raw_unit_code, resolved_packaging_level, item_packing_config_id),
      SELECT (raw_unit_code, resolved_packaging_level, item_packing_config_id)
    ON asserted_unit_content TO spork_app;
GRANT SELECT (raw_unit_code, resolved_packaging_level, item_packing_config_id)
    ON asserted_unit_content TO spork_projection_owner;

-- ---------------------------------------------------------------------------
-- 3. The variance quotes their word rather than ours
-- ---------------------------------------------------------------------------
--
-- Same signature, so this is a replace. `declared_entered_unit` now reports the
-- code the author used, falling back to our resolution for rows written before
-- there was anywhere to keep it.

CREATE OR REPLACE FUNCTION goods_receipt_variance(p_receipt uuid)
    RETURNS TABLE (
        goods_receipt_line_id     uuid,
        declared_base_quantity    numeric,
        counted_base_quantity     bigint,
        base_quantity_variance    numeric,
        landed_base_quantity      numeric,
        declared_entered_quantity numeric,
        declared_entered_unit     text,
        counted_entered_quantity  bigint,
        counted_packaging_level   packaging_level,
        declared_lot              text,
        counted_lot               text,
        declared_expiry           date,
        counted_expiry            date)
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    SELECT l.id,
           cc.quantity,
           l.quantity,
           l.quantity - cc.quantity,
           landed.base,
           cc.entered_quantity,
           coalesce(cc.raw_unit_code, u.code, cc.resolved_packaging_level::text),
           l.entered_quantity,
           l.entered_packaging_level,
           cc.lot_code,
           lo.code,
           cc.expiry_date,
           lo.expiry_date
      FROM goods_receipt_line l
      JOIN asserted_unit_content cc ON cc.id = l.asserted_unit_content_id
      LEFT JOIN unit u ON u.id = cc.resolved_unit_id
      LEFT JOIN lot lo ON lo.id = l.lot_id
      LEFT JOIN LATERAL (
          SELECT sum(m.quantity) AS base
            FROM stock_movement m
           WHERE m.goods_receipt_line_id = l.id
             AND m.from_location_id IS NULL
             AND m.from_package_id IS NULL) landed ON true
     WHERE l.goods_receipt_id = p_receipt
$$;

-- ---------------------------------------------------------------------------
-- 4. The freeze reads the renamed column
-- ---------------------------------------------------------------------------
--
-- Unchanged in behaviour. The resolution columns D90 protects do not yet include
-- the two added above: the app has no UPDATE on this table at all, so they are
-- protected by the same absence. Widening the function is question 161's
-- business rather than a change smuggled into a rename.

COMMENT ON FUNCTION asserted_unit_content_resolve(uuid, uuid, uuid, uuid, text) IS
    'The mediated re-resolution D21 requires and D90 built. It does not yet cover '
    'the packaging-level arm D93 added, which is INSERT-only until the function '
    'can run as the role it exists to constrain -- question 161. D90, D93.';
