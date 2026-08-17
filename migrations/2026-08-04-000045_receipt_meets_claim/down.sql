-- Reverse of 2026-08-04-000045_receipt_meets_claim.
--
-- The two halves of a receipt come apart again, and D21's freeze loses the half
-- it names second.

DROP FUNCTION IF EXISTS goods_receipt_variance(uuid);

CREATE OR REPLACE FUNCTION asserted_unit_content_resolve(
        p_content uuid, p_item uuid, p_po_line uuid, p_actor uuid, p_method text)
    RETURNS void
    LANGUAGE plpgsql
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    used bigint;
BEGIN
    PERFORM 1 FROM asserted_unit_content WHERE id = p_content FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'no asserted unit content %', p_content;
    END IF;
    SELECT count(*) INTO used FROM assertion_check
     WHERE asserted_unit_content_id = p_content;
    IF used > 0 THEN
        RAISE EXCEPTION 'asserted unit content % is frozen: % check(s) have compared against it',
            p_content, used;
    END IF;
    UPDATE asserted_unit_content
       SET resolved_item_id = coalesce(p_item, resolved_item_id),
           resolved_purchase_order_line_id =
               coalesce(p_po_line, resolved_purchase_order_line_id),
           resolved_at = now(), resolved_by_id = p_actor, resolution_method = p_method
     WHERE id = p_content;
END
$$;

DROP INDEX IF EXISTS goods_receipt_line_asserted_content_idx;
ALTER TABLE goods_receipt_line
    DROP CONSTRAINT IF EXISTS goods_receipt_line_asserted_content_fk;
ALTER TABLE goods_receipt_line DROP COLUMN IF EXISTS asserted_unit_content_id;
