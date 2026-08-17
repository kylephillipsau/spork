-- Reverse of 2026-08-04-000043_receipt_disposition.
--
-- The dispositions already written stay: they are facts about what a receiver
-- decided. What goes is the only way to write another one.

DROP FUNCTION IF EXISTS goods_receipt_line_dispose(uuid, uuid, boolean, text, uuid, uuid);
