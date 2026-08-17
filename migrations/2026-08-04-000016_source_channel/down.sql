-- Reverse of 2026-08-04-000016_source_channel.

COMMENT ON TABLE item_class_closure IS
    'PROJECTION of item_class. Maintained by a named function under D35, never written by the application role.';

COMMENT ON COLUMN item_class_closure.tenant_id IS NULL;
COMMENT ON COLUMN item_class_closure.ancestor_id IS NULL;
COMMENT ON COLUMN item_class_closure.descendant_id IS NULL;
COMMENT ON COLUMN item_class_closure.depth IS NULL;
COMMENT ON COLUMN party_class_closure.tenant_id IS NULL;
COMMENT ON COLUMN party_class_closure.ancestor_id IS NULL;
COMMENT ON COLUMN party_class_closure.descendant_id IS NULL;
COMMENT ON COLUMN party_class_closure.depth IS NULL;

DROP INDEX IF EXISTS order_source_channel_idx;

ALTER TABLE "order" ADD COLUMN source_channel text;

UPDATE "order" o
   SET source_channel = sc.code
  FROM source_channel sc
 WHERE sc.id = o.source_channel_id;

ALTER TABLE "order" ALTER COLUMN source_channel SET NOT NULL;
ALTER TABLE "order" DROP COLUMN source_channel_id;

REVOKE INSERT, UPDATE ON "order" FROM nylonite_app;

-- Migration 12's list, plus the currency migration 13 added.
GRANT INSERT (id, tenant_id, site_id, customer_party_id, confirmation_number,
              source_channel, external_ref, supersedes_order_id, placed_at,
              promised_from, promised_to, required_by, state, currency),
      UPDATE (site_id, customer_party_id, confirmation_number, external_ref,
              supersedes_order_id)
    ON "order" TO nylonite_app;

DROP TABLE IF EXISTS source_channel;
DROP TYPE IF EXISTS channel_authority;
