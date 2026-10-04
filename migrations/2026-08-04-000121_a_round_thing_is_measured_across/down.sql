ALTER TABLE packaging_type DROP CONSTRAINT IF EXISTS packaging_type_one_shape_ck;
ALTER TABLE packaging_type DROP COLUMN IF EXISTS round;
DELETE FROM metric WHERE tenant_id IS NULL AND code IN ('diameter', 'base_diameter', 'top_height')
   AND NOT EXISTS (SELECT 1 FROM observation o WHERE o.metric_id = metric.id);
