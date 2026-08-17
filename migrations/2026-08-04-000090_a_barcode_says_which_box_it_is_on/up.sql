-- Migration 90: a barcode says which box it is on, and who said so.
--
-- D164. Migration 79 built `item_barcode` and closed with a note saying what it
-- did not do: *"`unit_id` does not imply a packaging level, though D34 assumed
-- it would. The built vocabulary does not: `unit` holds `ea` in the count
-- dimension and nothing above it — no carton, no inner, no pallet. So a carton
-- GTIN cannot say it is a carton, and the resolver does not pretend otherwise."*
--
-- **That is the gap, and a carton full of individually barcoded boxes is the
-- shape that falls into it.** The box carries a GTIN and the carton carries its
-- own; both resolve to the same item and mean different quantities of it, and
-- with only `unit_id` to distinguish them the table can hold both and tell them
-- apart by nothing. A scan of the carton then reads as a scan of a box.
--
-- **The level rather than the unit, and it is not the same question.**
-- `unit_id` with `quantity` answers *how many base units does one scan mean*,
-- which is arithmetic; the level answers *what is the label stuck to*, which is
-- a fact about the packaging. Conflating them is what produced the gap: adding
-- `carton` to the `unit` vocabulary — the alternative migration 79 names, and
-- leaves to D23 — would put a packaging level in the table of measures, where
-- `kg` and `mm` live, and every unit conversion would then have to know that
-- some of its rows are not measures at all.
--
-- Nullable, and NULL is not a default. An existing row genuinely does not say
-- what level it is for, and writing `each` across the table would be inventing
-- the answer this column exists to record. The resolver keeps its behaviour for
-- those: it returns the item and the operator names the level.
ALTER TABLE item_barcode ADD COLUMN packaging_level packaging_level;

COMMENT ON COLUMN item_barcode.packaging_level IS
    'What the label is stuck to: the box, the carton it ships in, the pallet. '
    'Distinct from unit_id + quantity, which answer how many base units one '
    'scan means. NULL is a binding that predates D164 and does not say. '
    'Migration 90.';

-- **Who bound it, which is migration 79''s own objection answered.**
--
-- That file refuses a feed writing here directly: *"a supplier would silently
-- rewrite what a scan means — the poisoning D19 exists to prevent, one level
-- below measurements and with a worse blast radius, because a wrong
-- measurement produces a bad autofill and a wrong barcode binding produces
-- stock movements against the wrong item."* It ends by saying an assertion
-- lands and *something with a name promotes it*.
--
-- A person standing at the shelf with the box in one hand and the scanner in
-- the other **is** the something with a name. D11 makes the actor the
-- non-repudiable floor of every fact in this system, and a binding with no
-- actor is exactly the anonymous write that comment refuses. So the column is
-- what separates the two cases structurally rather than by convention: an
-- operator''s binding names them, and a feed has nobody to name.
--
-- Nullable for the same reason as the level: the seed''s bindings and anything
-- an importer wrote have no person behind them, and inventing one would be the
-- lie this column exists to prevent.
ALTER TABLE item_barcode ADD COLUMN bound_by_person_id uuid REFERENCES person(id);

COMMENT ON COLUMN item_barcode.bound_by_person_id IS
    'Who said this barcode means this item, when a person did. NULL is a '
    'binding that arrived from a feed, an importer or the seed and has nobody '
    'behind it -- which is a real and different answer from an unnamed person. '
    'D11, D164. Migration 90.';

-- The maintenance direction, and the one a screen asks: everything this item
-- answers to, newest binding first. `item_barcode_item_idx` already covers the
-- lookup; this is about the level being in the row the index leads to, so no
-- second index is added. Said out loud because the obvious next edit is to add
-- one, and it would be an index nothing reads.
