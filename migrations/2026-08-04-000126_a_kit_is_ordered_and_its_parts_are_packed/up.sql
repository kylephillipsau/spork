-- Migration 126: a kit is ordered, and its parts are packed (D223).
--
-- NetSuite sells some things as kits: a trigger sprayer and its bottle, sold
-- as one code, never stocked as one. An item fulfilment carries the kit's line
-- and then a line for each part, and only the parts are goods: picked from
-- their own bins, packed loose. Read as plain lines, the bench asked for the
-- kit to be packed beside its parts, and the packing plan counted the goods
-- twice.
--
-- # The kit's line is on the order, and no work
--
-- What the customer ordered is the kit, so its line stays on the order. Nothing
-- is committed to pick or pack for it: the intake loads it with nothing
-- outstanding, and records no picks against it. So no reader of the work needs
-- telling what a kit is. A part's line is an ordinary line, and says which kit
-- it is part of, so a screen can show the parts together under their kit.
--
-- # Within the order, and the tenant
--
-- A part's kit is a line of the same order, which the composite key added by
-- migration 14 lets the database hold rather than the application promise; and
-- of the same tenant, which S51 asks every reference to carry.

ALTER TABLE order_line ADD COLUMN kit_line_id uuid;

ALTER TABLE order_line
    ADD CONSTRAINT order_line_kit_fk
        FOREIGN KEY (kit_line_id, order_id) REFERENCES order_line(id, order_id),
    ADD CONSTRAINT order_line_kit_tenant_fk
        FOREIGN KEY (kit_line_id, tenant_id) REFERENCES order_line(id, tenant_id),
    ADD CONSTRAINT order_line_kit_not_itself_ck CHECK (kit_line_id <> id);

CREATE INDEX order_line_kit_idx ON order_line (kit_line_id) WHERE kit_line_id IS NOT NULL;

COMMENT ON COLUMN order_line.kit_line_id IS
    'The kit this line is a part of: a line of the same order whose item is sold '
    'as a kit and never stocked. The kit''s own line is what was ordered and is no '
    'work; its parts are the goods. D223.';

-- Insertable, and updatable because a line loaded before the sender said which
-- kit it is part of learns it from the first send that does (as migration 93's
-- keys are).
GRANT INSERT (kit_line_id), UPDATE (kit_line_id) ON order_line TO spork_app;
