-- Reverse of 2026-08-04-000050_declared_pallet_meets_scanned.
--
-- The declared pallet and the scanned one come apart again, and S35 goes back to
-- having no column its sentence can be true of.

DO $$
DECLARE n bigint;
BEGIN
    SELECT count(*) INTO n FROM asserted_unit WHERE resolved_package_id IS NOT NULL;
    IF n > 0 THEN
        RAISE NOTICE 'reversing D96 destroys the collapse of % declared unit(s): which pallet '
                     'on the dock was the one they said they sent becomes unanswerable', n;
    END IF;
END $$;

DELETE FROM mediated_write WHERE function_name = 'asserted_unit_collapse';

DROP FUNCTION IF EXISTS asserted_unit_collapse(uuid, uuid, uuid);

REVOKE ALL ON package FROM spork_mediation_owner;
REVOKE ALL ON package_event FROM spork_mediation_owner;
REVOKE ALL ON asserted_unit FROM spork_mediation_owner;

DROP INDEX IF EXISTS asserted_unit_package_idx;

ALTER TABLE asserted_unit
    DROP CONSTRAINT IF EXISTS asserted_unit_collapse_pair_ck,
    DROP CONSTRAINT IF EXISTS asserted_unit_document_has_no_package_ck,
    DROP CONSTRAINT IF EXISTS asserted_unit_package_fk,
    DROP CONSTRAINT IF EXISTS asserted_unit_document_has_no_sscc_ck;

ALTER TABLE asserted_unit
    DROP COLUMN IF EXISTS collapsed_by_id,
    DROP COLUMN IF EXISTS collapsed_at,
    DROP COLUMN IF EXISTS resolved_package_id,
    DROP COLUMN IF EXISTS resolved_physical;
