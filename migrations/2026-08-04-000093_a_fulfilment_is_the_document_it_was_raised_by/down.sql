-- Migration 93 down: a fulfilment is keyed by its order and site again.

DROP INDEX IF EXISTS order_external_id_idx;
ALTER TABLE "order" DROP COLUMN IF EXISTS external_id;

DROP INDEX IF EXISTS fulfilment_line_external_key;
ALTER TABLE fulfilment_line DROP COLUMN IF EXISTS external_line;

DROP INDEX IF EXISTS fulfilment_external_key;
ALTER TABLE fulfilment DROP CONSTRAINT IF EXISTS fulfilment_external_pair_ck;
ALTER TABLE fulfilment DROP COLUMN IF EXISTS external_id;
ALTER TABLE fulfilment DROP COLUMN IF EXISTS source_channel_id;
