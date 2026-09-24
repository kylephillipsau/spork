-- Migration 46: the receipt line counts in base units, like everything else.
--
-- D92, settling question 158, which D91 raised by getting a subtraction wrong.
--
-- The question was filed as "where the conversion from an entered packaging level
-- to base units lives". That framing is a symptom. **The conversion already lives
-- exactly where Principle 5 puts it -- in the writer, with the canonical value
-- stored and the entered form preserved beside it.** What is missing is the column
-- to store it in.
--
--   observation             entered_value + entered_unit_id   -> value_numeric
--   asserted_unit_content   entered_quantity + entered_unit_id -> quantity
--   stock_movement          entered_quantity + level + config  -> quantity
--   goods_receipt_line      entered_quantity + level + config  -> (nothing)
--
-- One member of the family stores the vocabulary and not the meaning, and every
-- symptom follows from that: `disposition()` comparing cartons with base units,
-- D91's first draft doing the same in SQL, and D91's variance having to read the
-- ledger for a number the line should have held -- which returns nothing for a
-- rejected line, so the report goes blank on exactly the lines it exists for.
--
-- **The absence was noticed once already and worked around.** D45: *"The original
-- folded `goods_receipt_line.quantity`, which does not exist and never did -- the
-- invariant named the right relationship over the wrong table and could not have
-- run."* J26 was rewritten to fold the ledger instead. The instinct was right and
-- the column is what should have changed.

-- ---------------------------------------------------------------------------
-- 1. The canonical column, with no vocabulary beside it
-- ---------------------------------------------------------------------------
--
-- Principle 5's structural trick, borrowed verbatim from `observation`: *"there is
-- no unit column, which makes non-canonical storage structurally unrepresentable
-- rather than merely discouraged."* `quantity` is base units because there is
-- nowhere to say it is anything else.
--
-- Not a duplicate of `stock_movement.quantity`. The line says **what we counted**;
-- the ledger says **what entered the world**. They agree on an accepted line and
-- differ on every rejected one, and the current schema can only express the
-- second -- which is why a line refused for over-delivery cannot state the
-- over-delivery it was refused for.

ALTER TABLE goods_receipt_line
    ADD COLUMN quantity bigint,
    -- Both or neither, mirroring `goods_receipt_line_entered_pair_ck`. A counted
    -- line has a count in both vocabularies; an uncounted line has neither.
    ADD CONSTRAINT goods_receipt_line_quantity_pair_ck
        CHECK (num_nonnulls(quantity, entered_quantity) <> 1),
    ADD CONSTRAINT goods_receipt_line_quantity_positive_ck
        CHECK (quantity IS NULL OR quantity > 0);

COMMENT ON COLUMN goods_receipt_line.quantity IS
    'What was counted, in the item''s base unit. There is no unit or packaging '
    'level column beside it, so a quantity stored in anything else cannot be '
    'represented -- the same structural guarantee observation.value_numeric has. '
    'What entered the world is stock_movement.quantity and differs on a rejected '
    'line. Principle 5, D45, D92.';

-- The counted quantity is not one of the things that legitimately moves after the
-- fact, which is the argument D45 already made for the entered columns. INSERT
-- only, and the app never gets UPDATE.
GRANT INSERT (quantity) ON goods_receipt_line TO spork_app;

-- Existing rows, where the ledger can answer for them. A line that landed stock
-- has its base quantity in the movement it caused; a rejected line has nothing to
-- recover from and stays NULL rather than being given an invented number.
DO $$
DECLARE
    filled  bigint;
    unknown bigint;
BEGIN
    UPDATE goods_receipt_line l
       SET quantity = m.landed
      FROM (SELECT goods_receipt_line_id AS id, sum(quantity)::bigint AS landed
              FROM stock_movement
             WHERE goods_receipt_line_id IS NOT NULL
               AND from_location_id IS NULL AND from_package_id IS NULL
             GROUP BY goods_receipt_line_id) m
     WHERE l.id = m.id AND l.entered_quantity IS NOT NULL;
    GET DIAGNOSTICS filled = ROW_COUNT;

    SELECT count(*) INTO unknown FROM goods_receipt_line
     WHERE entered_quantity IS NOT NULL AND quantity IS NULL;

    IF filled > 0 OR unknown > 0 THEN
        RAISE NOTICE 'D92 recovered the base quantity of % counted line(s) from the ledger; '
                     '% counted nothing into it and stay unknown', filled, unknown;
    END IF;
END $$;

-- ---------------------------------------------------------------------------
-- 2. The factor, as data rather than as arithmetic somebody remembers
-- ---------------------------------------------------------------------------
--
-- **This is not the write path.** Principle 5 has the writer convert and store the
-- canonical value, and there is no SQL conversion function for units either --
-- `unit.factor_num/factor_den` is data the writer reads. This function exists so
-- the conversion can be *checked* independently, which is J65 below, and so a
-- report can explain a number rather than restate it.
--
-- NULL where the cascade cannot answer: a config with no `cartons_per_layer`
-- cannot convert a layer, and a made-up 1 there would be a wrong answer rather
-- than a missing one.

CREATE FUNCTION packing_factor(p_config uuid, p_level packaging_level)
    RETURNS bigint
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    SELECT CASE p_level
             WHEN 'each'   THEN 1
             WHEN 'inner'  THEN c.units_per_inner
             WHEN 'carton' THEN c.units_per_inner * c.inners_per_carton
             WHEN 'layer'  THEN c.units_per_inner * c.inners_per_carton
                                * c.cartons_per_layer
             WHEN 'pallet' THEN c.units_per_inner * c.inners_per_carton
                                * c.cartons_per_layer * c.layers_per_pallet
           END::bigint
      FROM (SELECT p_config AS id) k
      LEFT JOIN item_packing_config c ON c.id = k.id
$$;

COMMENT ON FUNCTION packing_factor(uuid, packaging_level) IS
    'How many base units one unit of this packaging level holds, under this '
    'config version. NULL where the cascade is incomplete, because a missing '
    'factor is not a factor of one. Not the write path -- the writer converts and '
    'stores the canonical value, per Principle 5. This is what lets J65 re-derive '
    'the conversion independently. D58, D92.';

GRANT EXECUTE ON FUNCTION packing_factor(uuid, packaging_level)
    TO spork_app, spork_platform, spork_scheduler, spork_projection_owner;

-- ---------------------------------------------------------------------------
-- 3. The variance reads the line, and the ledger separately
-- ---------------------------------------------------------------------------
--
-- D91 had to take the counted base from the ledger because the line did not hold
-- one, which meant a rejected line reported NULL -- and a rejected line is the
-- case a variance report is for.
--
-- Both numbers are now here and they answer different questions. `counted` is what
-- the receiver counted, present on every counted line whatever its disposition.
-- `landed` is what entered the ledger, absent on a refusal. Where they differ, the
-- difference is the disposition, which is the point.

-- Dropped rather than replaced: the return type gains a column, and
-- CREATE OR REPLACE cannot change one.
DROP FUNCTION goods_receipt_variance(uuid);

CREATE FUNCTION goods_receipt_variance(p_receipt uuid)
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
           u.code,
           l.entered_quantity,
           l.entered_packaging_level,
           cc.lot_code,
           lo.code,
           cc.expiry_date,
           lo.expiry_date
      FROM goods_receipt_line l
      JOIN asserted_unit_content cc ON cc.id = l.asserted_unit_content_id
      LEFT JOIN unit u ON u.id = cc.entered_unit_id
      LEFT JOIN lot lo ON lo.id = l.lot_id
      LEFT JOIN LATERAL (
          SELECT sum(m.quantity) AS base
            FROM stock_movement m
           WHERE m.goods_receipt_line_id = l.id
             AND m.from_location_id IS NULL
             AND m.from_package_id IS NULL) landed ON true
     WHERE l.goods_receipt_id = p_receipt
$$;

COMMENT ON FUNCTION goods_receipt_variance(uuid) IS
    'What was declared against what was counted, line by line -- quantity, lot and '
    'expiry. The base quantities are commensurable and subtracted; the entered '
    'pair is reported in its own vocabulary and is not, because a packaging level '
    'is not a unit. counted is what the receiver counted and landed is what '
    'reached the ledger; they differ by the disposition. D21, D91, D92.';

GRANT EXECUTE ON FUNCTION goods_receipt_variance(uuid)
    TO spork_app, spork_platform, spork_scheduler;
