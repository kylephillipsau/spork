-- Reverse of 2026-08-04-000044_resolution_freeze.
--
-- The application gets the privilege back, and with it the ability to rewrite an
-- annotation something has already compared against.

DROP FUNCTION IF EXISTS asserted_unit_content_resolve(uuid, uuid, uuid, uuid, text);

GRANT UPDATE (resolved_item_id, resolved_purchase_order_line_id, resolved_at,
              resolved_by_id, resolution_method)
    ON asserted_unit_content TO nylonite_app;
