-- Migration 74: the order a picker walks.
--
-- 5,980 bins are on file and nothing says which one you reach first. A pick list
-- ordered by bin code sends somebody from `A.01.01` to `A.02.01` because the
-- strings sort that way, which is only the right route if the racking happens to
-- run in alphabetical order — and in Brisbane it does not: bin `B.49.01` sits at
-- position 0 while `A.01.01` sits at 1.
--
-- **Declared, not derived.** Somebody walks the floor and decides. It cannot be
-- computed from the code, and it cannot be computed from the coordinates either:
-- `location` has `x_mm`, `y_mm` and `z_mm`, and the shortest path between two
-- points in a warehouse is not a straight line because there are racks in the
-- way. So this is a number a person maintains, which is why it is an ordinary
-- column with an ordinary grant rather than a projection.
--
-- # Positive, so that "unsequenced" cannot pose as "first"
--
-- The export encodes *no position* as `0`, 150 times — including `3PL` and
-- `ASSEMBLY-BIN`, which are not places on a walking route at all, and 143
-- Brisbane bins. Loaded literally, every one of them sorts ahead of the first
-- real bin and the pick list opens with a detour to the assembly bench.
--
-- The CHECK makes that unrepresentable. A position is `>= 1`; not knowing one is
-- NULL, which sorts last under `ORDER BY ... NULLS LAST` and reads as what it is.
--
-- # Not unique, deliberately
--
-- Two bins can hold the same position, and in this data two pairs do:
-- Brisbane's `I.48.07` and `K.36.05` both at 2592, `K.32.01` and `K.32.02` both
-- at 2615. A unique index would refuse the import of true data, and it would
-- make reordering miserable — swapping two bins under a unique constraint needs
-- a third value nobody wants to store.
--
-- More to the point, two bins claiming one position is a **disagreement about
-- the floor**, which this system reports rather than refuses. J71.

ALTER TABLE location ADD COLUMN pick_sequence integer;

ALTER TABLE location ADD CONSTRAINT location_pick_sequence_ck
    CHECK (pick_sequence IS NULL OR pick_sequence >= 1);

COMMENT ON COLUMN location.pick_sequence IS
    'The order a picker reaches this bin, within its site. DECLARED: a person '
    'walks the floor and decides, and it cannot be derived from the code or from '
    'x/y/z because there are racks in the way. NULL means nobody has said, and '
    'sorts last. Never 0 -- the export uses 0 for "unsequenced", and loaded '
    'literally that sorts ahead of every real bin. Not unique: two bins claiming '
    'one position is a finding (J71), not a violation. Migration 74.';

-- The index the pick list reads: everything in one site, in walking order. Bins
-- with no position are excluded rather than sorted, because a partial index over
-- the ones that have one is the query that matters.
CREATE INDEX location_pick_sequence_idx
    ON location (tenant_id, site_id, pick_sequence)
    WHERE pick_sequence IS NOT NULL AND active;

-- S45: a column the application cannot write is a column nothing can fill.
GRANT INSERT (pick_sequence), UPDATE (pick_sequence) ON location TO nylonite_app;
