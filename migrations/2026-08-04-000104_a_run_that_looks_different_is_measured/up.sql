-- Migration 104: a run that looks different is measured on its own. D182.
--
-- One product can arrive in cartons printed two ways: the same code, the same
-- bin liners, one carton yellow and printed, the next plain with a sticker,
-- from another factory. Each run is a lot of the item (the lot table and the
-- observation registry's lot arm were both already here), photographed and
-- measured as itself, because a different carton can be a different size.
--
-- The writer refuses a metric that does not apply to the subject's kind, and
-- weight and the three lengths applied to items, styles, parts and packages.
-- They apply to a lot now too.

UPDATE metric
   SET applies_to = array_append(applies_to, 'lot')
 WHERE tenant_id IS NULL
   AND code IN ('gross_weight', 'length', 'width', 'height')
   AND NOT ('lot' = ANY (applies_to));
