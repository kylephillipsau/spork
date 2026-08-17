-- Reverse of 2026-08-04-000008_policy.

DROP VIEW IF EXISTS policy_binding_scope;
DROP FUNCTION IF EXISTS resolve_policy_binding(uuid, policy_kind, uuid, uuid, uuid, uuid, uuid, uuid);

DROP TABLE IF EXISTS shelf_life_policy;
DROP TABLE IF EXISTS receiving_policy;
DROP TABLE IF EXISTS allocation_policy;
DROP TABLE IF EXISTS policy_change;
DROP TABLE IF EXISTS policy_binding;
DROP TYPE IF EXISTS policy_kind;
DROP TYPE IF EXISTS policy_change_kind;

ALTER TABLE party DROP CONSTRAINT IF EXISTS party_class_fk;
ALTER TABLE party DROP COLUMN IF EXISTS party_class_id;

DROP TABLE IF EXISTS party_class_closure;
DROP TABLE IF EXISTS party_class;
