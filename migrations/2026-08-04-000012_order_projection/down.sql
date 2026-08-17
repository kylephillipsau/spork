-- Reverse of 2026-08-04-000012_order_projection.

REVOKE INSERT, UPDATE ON "order" FROM nylonite_app;
GRANT SELECT, INSERT, UPDATE ON "order" TO nylonite_app;

COMMENT ON COLUMN "order".promised_from IS NULL;
COMMENT ON COLUMN "order".promised_to IS NULL;
COMMENT ON COLUMN "order".required_by IS NULL;
COMMENT ON COLUMN "order".state IS NULL;

DELETE FROM projection_rebuild
 WHERE table_name = 'order' AND function_name = 'projection_order_rebuild';

INSERT INTO projection_rebuild (table_name, column_name, function_name) VALUES
    ('fulfilment', 'progress', 'projection_fulfilment_rebuild'),
    ('fulfilment_line', 'allocated_quantity', 'projection_fulfilment_rebuild');

COMMENT ON COLUMN consignment.status IS '@projection of the in-force carrier advice.';
COMMENT ON COLUMN consignment.eta IS '@projection of the in-force carrier advice.';
COMMENT ON COLUMN consignment.price_minor IS '@projection of the in-force carrier advice.';
COMMENT ON COLUMN fulfilment.progress IS '@projection of fulfilment_line.';
COMMENT ON COLUMN fulfilment_line.allocated_quantity IS '@projection of stock_allocation.';
