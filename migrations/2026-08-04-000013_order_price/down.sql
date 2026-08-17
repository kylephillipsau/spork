-- Reverse of 2026-08-04-000013_order_price.

ALTER TABLE order_line
    DROP CONSTRAINT IF EXISTS order_line_price_sign_ck,
    DROP CONSTRAINT IF EXISTS order_line_price_basis_ck,
    DROP CONSTRAINT IF EXISTS order_line_price_pair_ck;

ALTER TABLE order_line
    DROP COLUMN IF EXISTS price_basis_quantity,
    DROP COLUMN IF EXISTS unit_price_minor;

ALTER TABLE consignment DROP CONSTRAINT IF EXISTS consignment_currency_ck;

ALTER TABLE "order" DROP CONSTRAINT IF EXISTS order_currency_ck;
ALTER TABLE "order" DROP COLUMN IF EXISTS currency;
