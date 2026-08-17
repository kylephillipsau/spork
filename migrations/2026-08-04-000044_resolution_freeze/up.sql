-- Migration 44: a resolution freezes on first use.
--
-- D90, settling question 152. D21 states the rule and D77 built the columns
-- without it:
--
--   "A re-resolution **freezes on first use**: once an `assertion_check` or a
--    `goods_receipt_line` references it, it may not be rewritten, and a
--    correction writes a new assertion."
--
-- Both halves matter and they pull against each other, which is why the rule is
-- worth the machinery. **Re-resolution must stay possible**, because D21 is
-- explicit: *"a GTIN unresolvable today becomes resolvable when the item is
-- created tomorrow, and refusing that would discard a claim because our catalogue
-- was behind."* And it must stop the moment something has compared against it,
-- because after that a rewrite changes what the comparison meant.

-- ---------------------------------------------------------------------------
-- 1. The mechanism is the one D89 proved, generalised
-- ---------------------------------------------------------------------------
--
-- S7 forbids the trigger and D25 forbids the validation kind by name, so this is
-- not enforceable as a constraint on the table. What D89 established a day ago is
-- that it does not have to be: **take the privilege away and mediate the write
-- through a function that refuses.** D72 did the same for `parent_id`, D77 for a
-- claim, D89 for a disposition. This is the fourth time and the first time it is
-- worth calling a pattern.
--
-- The application keeps INSERT on the row -- resolution at ingestion is a normal
-- write -- and loses UPDATE on exactly the five columns that are our annotation.

REVOKE UPDATE (resolved_item_id, resolved_purchase_order_line_id, resolved_at,
               resolved_by_id, resolution_method)
    ON asserted_unit_content FROM nylonite_app;

CREATE FUNCTION asserted_unit_content_resolve(
        p_content   uuid,
        p_item      uuid,
        p_po_line   uuid,
        p_actor     uuid,
        p_method    text)
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

    -- "Once an assertion_check references it." A comparison has been made against
    -- this line, so the item it resolved to is part of what that comparison
    -- concluded.
    SELECT count(*) INTO used FROM assertion_check
     WHERE asserted_unit_content_id = p_content;

    IF used > 0 THEN
        RAISE EXCEPTION 'asserted unit content % is frozen: % check(s) have compared against it',
            p_content, used
            USING HINT = 'a correction writes a new assertion, which is D21''s rule and D51''s shape';
    END IF;

    UPDATE asserted_unit_content
       SET resolved_item_id = coalesce(p_item, resolved_item_id),
           resolved_purchase_order_line_id =
               coalesce(p_po_line, resolved_purchase_order_line_id),
           resolved_at = now(),
           resolved_by_id = p_actor,
           resolution_method = p_method
     WHERE id = p_content;
END
$$;

COMMENT ON FUNCTION asserted_unit_content_resolve(uuid, uuid, uuid, uuid, text) IS
    'Writes our annotation onto a counterparty''s line, and refuses once anything '
    'has compared against it. Re-resolution stays possible until then, because a '
    'GTIN unresolvable today becomes resolvable when the item is created tomorrow. '
    'D21, D90.';

GRANT EXECUTE ON FUNCTION
    asserted_unit_content_resolve(uuid, uuid, uuid, uuid, text) TO nylonite_app;

-- ---------------------------------------------------------------------------
-- 2. The half that cannot be expressed yet, named rather than implied
-- ---------------------------------------------------------------------------
--
-- D21 names two things that freeze a resolution: an `assertion_check` and a
-- `goods_receipt_line`. Only the first is checkable, because **nothing links a
-- receipt line to the asserted content it was counted against**. D62 built the
-- receiving path before the assertion tables existed and D77 built the assertion
-- tables without going back, so the two halves of a receipt -- what they said
-- would arrive and what we counted -- have never been joined by a foreign key.
--
-- That is a bigger gap than this migration, and it is question 157 rather than a
-- column invented here to make one sentence checkable.
--
-- `despatch_advice` keeps its `resolved_*` columns writable for the same reason:
-- what would freeze them is a receipt against the shipment, and that link is the
-- one 157 is about.
