-- Reverse of 2026-08-04-000034_assertions.
--
-- Everything D21 built comes out, and what that costs is worth stating: the
-- artefacts go too. `party_message.payload` is the only copy of what a
-- counterparty actually sent, and no projection can reconstruct bytes.

DO $$
DECLARE n bigint; m bigint;
BEGIN
    SELECT count(*) INTO n FROM assertion;
    SELECT count(*) INTO m FROM party_message;
    IF n > 0 OR m > 0 THEN
        RAISE NOTICE 'reversing D21 destroys % claim(s) and % exchanged message(s): '
                     'the payloads are the only copy of what was sent', n, m;
    END IF;
END $$;

DELETE FROM projection_rebuild
 WHERE function_name = 'projection_inbound_shipment_rebuild';
DELETE FROM projection_step
 WHERE function_name = 'projection_inbound_shipment_rebuild';
DROP FUNCTION IF EXISTS projection_inbound_shipment_rebuild(uuid);

ALTER TABLE discrepancy DROP CONSTRAINT IF EXISTS discrepancy_assertion_check_fk;
ALTER TABLE discrepancy DROP COLUMN IF EXISTS assertion_check_id;

-- Referents before referenced, which is not the same as bodies before envelope:
-- assertion_check points at the hierarchy as well as at the claim, and
-- despatch_advice points at the subject.
DROP TABLE IF EXISTS assertion_check;
DROP TABLE IF EXISTS asserted_unit_content;
DROP TABLE IF EXISTS asserted_unit;
DROP TABLE IF EXISTS document_response;
DROP TABLE IF EXISTS despatch_advice;
DROP TABLE IF EXISTS inbound_shipment;
DROP TABLE IF EXISTS assertion_stance;
DROP TABLE IF EXISTS assertion;
DROP TABLE IF EXISTS party_message;

-- After the tables, because assertion_check's foreign key depends on this index.
ALTER TABLE discrepancy DROP CONSTRAINT IF EXISTS discrepancy_tenant_key;

DROP TYPE IF EXISTS assertion_check_outcome;
DROP TYPE IF EXISTS assertion_stance_kind;
DROP TYPE IF EXISTS assertion_kind;
DROP TYPE IF EXISTS message_parse_status;
DROP TYPE IF EXISTS message_channel;
DROP TYPE IF EXISTS message_direction;
