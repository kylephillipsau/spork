-- Migration 93: a fulfilment is the document it was raised by (D172).
--
-- At Melbourne the goods are picked on the WMS handheld, which writes an item
-- fulfilment into NetSuite; the packing happens here. One sales order can have
-- several of them -- one shipped from Sydney, one picked at Melbourne, or two
-- at one site -- and each is its own piece of work with its own lines. Keyed by
-- (order, site), as the intake was, two item fulfilments at one site collapse
-- into one row here and neither is what the packer was handed.
--
-- So a fulfilment that arrived through a channel carries that channel's id for
-- it, and the pair is unique: the same item fulfilment sent twice is the same
-- row. The number a person quotes (IF270947) already has a home,
-- fulfilment.reference (migration 71); this is the key the machine matches on,
-- which in NetSuite is the internal id, because a number can be reraised and an
-- internal id cannot.
--
-- **Typed columns, not a polymorphic mapping table.** Open question 4 proposed
-- external_reference(system, entity_type, entity_id, external_id). A table like
-- that cannot carry a foreign key to the row it names, which is S51 and the
-- first principle of this schema, and a second channel (MachShip) is not yet a
-- third case. D157: wait for the third case.

-- ---------------------------------------------------------------------------
-- 1. The fulfilment, and which document it is
-- ---------------------------------------------------------------------------

ALTER TABLE fulfilment
    ADD COLUMN source_channel_id uuid REFERENCES source_channel(id),
    ADD COLUMN external_id text,
    -- Both or neither: an id with no channel is a number from nowhere, and a
    -- channel with no id names a document it cannot find again.
    ADD CONSTRAINT fulfilment_external_pair_ck
        CHECK ((source_channel_id IS NULL) = (external_id IS NULL));

COMMENT ON COLUMN fulfilment.source_channel_id IS
    'The channel whose document this fulfilment is, when it arrived through one. '
    'Set with external_id or not at all. Migration 93, D172.';
COMMENT ON COLUMN fulfilment.external_id IS
    'That channel''s key for the document: a NetSuite item fulfilment''s internal '
    'id. Unique per channel, so a resend finds the same row. The number a person '
    'quotes is reference. Migration 93, D172.';

CREATE UNIQUE INDEX fulfilment_external_key
    ON fulfilment (tenant_id, source_channel_id, external_id)
    WHERE external_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 2. The line, and which line of that document it is
-- ---------------------------------------------------------------------------

ALTER TABLE fulfilment_line ADD COLUMN external_line text;

COMMENT ON COLUMN fulfilment_line.external_line IS
    'The line''s key within its fulfilment''s external document (a NetSuite '
    'item fulfilment line). What a reported pick is matched on. Migration 93, '
    'D172.';

CREATE UNIQUE INDEX fulfilment_line_external_key
    ON fulfilment_line (fulfilment_id, external_line)
    WHERE external_line IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 3. The order, and its id in the system that raised it
-- ---------------------------------------------------------------------------
--
-- "order".external_ref holds the customer's purchase order number, which is
-- what the pack queue is searched by, and it stays. The sales order's own key
-- in NetSuite is a different thing and gets its own column rather than a
-- second meaning for that one.

ALTER TABLE "order" ADD COLUMN external_id text;

COMMENT ON COLUMN "order".external_id IS
    'The order''s key in its source channel: a NetSuite sales order''s internal '
    'id. Distinct from external_ref, the customer''s purchase order number. '
    'Migration 93, D172.';

CREATE INDEX order_external_id_idx
    ON "order" (tenant_id, source_channel_id, external_id)
    WHERE external_id IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 4. Grants (S45)
-- ---------------------------------------------------------------------------
--
-- Insertable, and updatable because a fulfilment or order loaded before this
-- migration (by the userscript that sent no ids) is adopted by the first send
-- that carries them, rather than duplicated.

GRANT INSERT (source_channel_id, external_id), UPDATE (source_channel_id, external_id)
    ON fulfilment TO spork_app;
GRANT INSERT (external_line), UPDATE (external_line) ON fulfilment_line TO spork_app;
GRANT INSERT (external_id), UPDATE (external_id) ON "order" TO spork_app;
