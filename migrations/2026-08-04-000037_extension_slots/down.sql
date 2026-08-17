-- Reverse of 2026-08-04-000037_extension_slots.
--
-- The declarations go with the tables. What a tenant declared is a statement
-- about their own extension of the model, and no rebuild reconstructs it.

DO $$
DECLARE n bigint; m bigint;
BEGIN
    SELECT count(*) INTO n FROM record_scheme;
    SELECT count(*) INTO m FROM extension_slot WHERE claimed_key IS NOT NULL;
    IF n > 0 OR m > 0 THEN
        RAISE NOTICE 'reversing D80 destroys % scheme declaration(s) and releases % '
                     'claimed slot(s): the ceiling stops being enforceable before the DDL', n, m;
    END IF;
END $$;

DROP FUNCTION IF EXISTS extension_slot_claim(uuid, extension_slot_kind, text);

DROP TABLE IF EXISTS record_scheme_field;
DROP TABLE IF EXISTS record_scheme;
DROP TABLE IF EXISTS extension_slot;

DROP TYPE IF EXISTS record_scheme_field_type;
DROP TYPE IF EXISTS record_scheme_state;
DROP TYPE IF EXISTS record_scheme_source;
DROP TYPE IF EXISTS record_scheme_cardinality;
DROP TYPE IF EXISTS record_scheme_role;
DROP TYPE IF EXISTS record_scheme_provenance;
DROP TYPE IF EXISTS extension_slot_kind;
