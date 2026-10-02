-- Migration 104 down: weight and size no longer apply to a lot.
--
-- What was recorded against a lot stays; nothing new of the kind is taken.

UPDATE metric
   SET applies_to = array_remove(applies_to, 'lot')
 WHERE tenant_id IS NULL
   AND code IN ('gross_weight', 'length', 'width', 'height');
