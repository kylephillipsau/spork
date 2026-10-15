-- Migration 146 down: no build rules, no usual carrier, and a pallet is built
-- for nobody. A carrier's services go back to having no workspace of their own.

DO $$
DECLARE sets bigint; pallets bigint;
BEGIN
    SELECT count(*) INTO sets FROM build_rules;
    SELECT count(*) INTO pallets FROM package WHERE carrier_id IS NOT NULL;
    IF sets > 0 OR pallets > 0 THEN
        RAISE NOTICE 'reversing D250 destroys % set(s) of build rules and forgets which carrier % pallet(s) were built for', sets, pallets;
    END IF;
END
$$;

ALTER TABLE pack_plan_parcel DROP COLUMN carrier_id;

ALTER TABLE package DROP CONSTRAINT package_build_rules_fk;
ALTER TABLE package DROP COLUMN build_rules_version_id, DROP COLUMN carrier_id;

ALTER TABLE workspace_setting DROP COLUMN usual_carrier_id;

ALTER TABLE carrier
    DROP CONSTRAINT carrier_rules_own_ck,
    DROP CONSTRAINT carrier_as_one_rules_fk,
    DROP CONSTRAINT carrier_breakdown_rules_fk,
    DROP COLUMN as_one_rules_id,
    DROP COLUMN breakdown_rules_id;

DROP VIEW build_rules_now;
DROP TABLE build_rule;
DROP TABLE build_rules_version;
DROP TABLE build_rules;
DROP TYPE rule_strength;
DROP TYPE build_rule_kind;

DROP POLICY carrier_service_own_write ON carrier_service;
DROP POLICY carrier_service_shared_read ON carrier_service;
ALTER TABLE carrier_service NO FORCE ROW LEVEL SECURITY;
ALTER TABLE carrier_service DISABLE ROW LEVEL SECURITY;
ALTER TABLE carrier_service DROP CONSTRAINT carrier_service_carrier_tenant_fk;
ALTER TABLE carrier_service DROP COLUMN tenant_id;
ALTER TABLE carrier DROP CONSTRAINT carrier_tenant_key;
