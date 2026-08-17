-- Migration 79 down: nothing resolves a barcode again.
--
-- Reversing this destroys what identifiers mean, and that is not recoverable
-- from anything else in the database: a closed `effective` range is the only
-- record of what a scan meant last year, and D31 retains it indefinitely for
-- exactly that reason.

DO $$
DECLARE n bigint; closed bigint;
BEGIN
    SELECT count(*), count(*) FILTER (WHERE NOT upper_inf(effective))
      INTO n, closed
      FROM item_barcode;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D34 destroys % barcode binding(s), % of them closed: a closed range is the evidence for what a historical scan meant and has no other home', n, closed;
    END IF;
END
$$;

DROP TABLE item_barcode;

-- Added by this migration purely to offer `(id, tenant_id)` to the composite
-- foreign key above, so it goes with it. `id` remains the primary key.
ALTER TABLE item DROP CONSTRAINT item_tenant_key;
