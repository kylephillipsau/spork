-- Reverse of 2026-08-04-000006_discrepancy.

DROP FUNCTION IF EXISTS record_finding(uuid, discrepancy_kind, text, text);
DROP TABLE IF EXISTS discrepancy;
DROP TYPE IF EXISTS discrepancy_state;
DROP TYPE IF EXISTS discrepancy_kind;
