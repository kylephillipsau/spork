-- Migration 115 down: the platform's statuses go, where nothing names them.

DELETE FROM inventory_status s
 WHERE s.tenant_id IS NULL
   AND s.id IN ('57a70000-0000-0000-0000-000000000001', '57a70000-0000-0000-0000-000000000002')
   AND NOT EXISTS (SELECT 1 FROM stock_movement m WHERE s.id IN (m.from_status_id, m.to_status_id))
   AND NOT EXISTS (SELECT 1 FROM stock st WHERE st.status_id = s.id);
