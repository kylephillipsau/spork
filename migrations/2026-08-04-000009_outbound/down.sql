-- Reverse of 2026-08-04-000009_outbound.

DROP FUNCTION IF EXISTS projection_order_rebuild(uuid);

DELETE FROM projection_rebuild
 WHERE table_name IN ('fulfilment', 'fulfilment_line');

ALTER TABLE package
    DROP COLUMN IF EXISTS sealed_at,
    DROP COLUMN IF EXISTS dimensions_source,
    DROP COLUMN IF EXISTS gross_weight_g,
    DROP COLUMN IF EXISTS height_mm,
    DROP COLUMN IF EXISTS width_mm,
    DROP COLUMN IF EXISTS length_mm,
    DROP COLUMN IF EXISTS sequence,
    DROP COLUMN IF EXISTS package_type_id,
    DROP COLUMN IF EXISTS fulfilment_id;

ALTER TABLE stock_allocation DROP COLUMN IF EXISTS fulfilment_line_id;

DROP TABLE IF EXISTS consignment_package;
DROP TABLE IF EXISTS consignment;
DROP TABLE IF EXISTS carrier_service;
DROP TABLE IF EXISTS freight_provider;
DROP TABLE IF EXISTS carrier;
DROP TABLE IF EXISTS fulfilment_line;
DROP TABLE IF EXISTS fulfilment;
DROP TABLE IF EXISTS intention_amendment;
DROP TABLE IF EXISTS order_line;
DROP TABLE IF EXISTS "order";
DROP TABLE IF EXISTS package_type;

DROP TYPE IF EXISTS fulfilment_state;
DROP TYPE IF EXISTS order_state;
