-- Migration 64 down: restore migration 63's foreign key onto stock(id).
--
-- This reverses into a state S28 fails on, which is correct for a down migration
-- and worth saying out loud: reversing a correction restores the defect it
-- corrected. The NOTICE reports what reversing costs, in the idiom the other
-- down migrations use.

DO $$
DECLARE
    orphans bigint;
BEGIN
    -- A count whose cell has since been reaped cannot satisfy the key being
    -- restored. Those rows are cleared rather than deleted: the count itself is a
    -- fact and D25 does not let a down migration destroy one, but the snapshot
    -- that can no longer resolve has to go for the constraint to be creatable.
    SELECT count(*) INTO orphans
      FROM stock_count c
     WHERE c.stock_id IS NOT NULL
       AND NOT EXISTS (SELECT 1 FROM stock s
                        WHERE s.id = c.stock_id AND s.tenant_id = c.tenant_id);

    IF orphans > 0 THEN
        UPDATE stock_count c
           SET stock_id = NULL
         WHERE c.stock_id IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM stock s
                            WHERE s.id = c.stock_id AND s.tenant_id = c.tenant_id);
        RAISE NOTICE 'reversing migration 64 clears the cell snapshot on % count(s) whose '
                     'stock row has since been reaped: which cell they were taken against '
                     'stops being answerable', orphans;
    END IF;
END $$;

COMMENT ON COLUMN stock_count.stock_id IS
    'Optional link to a live cell row when the count is of an existing cell. '
    'Null when counting empty space that has no stock row yet.';

ALTER TABLE stock_count
    ADD CONSTRAINT stock_count_stock_fk
        FOREIGN KEY (stock_id, tenant_id) REFERENCES stock(id, tenant_id);
