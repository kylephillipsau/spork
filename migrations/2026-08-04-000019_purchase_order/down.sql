-- Reverse of 2026-08-04-000019_purchase_order.

DROP INDEX IF EXISTS purchase_order_supplier_idx;
DROP INDEX IF EXISTS purchase_order_source_channel_idx;
DROP INDEX IF EXISTS purchase_order_open_idx;
DROP INDEX IF EXISTS purchase_order_line_item_idx;
DROP INDEX IF EXISTS purchase_order_line_order_idx;

DROP TABLE IF EXISTS purchase_order_line;
DROP TABLE IF EXISTS purchase_order;

DROP TYPE IF EXISTS purchase_order_state;
