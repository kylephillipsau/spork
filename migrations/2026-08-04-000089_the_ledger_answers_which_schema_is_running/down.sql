-- Migration 89 down: the application stops being able to say which schema it is
-- running on. The ledger itself is untouched; only the reading goes.
REVOKE SELECT ON schema_migration FROM nylonite_app;
