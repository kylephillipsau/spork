-- Migration 50: the pallet they declared meets the pallet we scanned.
--
-- D96, settling question 162, which the inbound walk raised by receiving a
-- delivery end to end and leaving the two pallets unconnected.
--
-- **This is 157 one level up.** D91 joined the two halves of a receipt at the
-- content level -- what they said was in the carton against what we counted --
-- and the unit level stayed open: a despatch advice declaring three SSCCs and
-- three pallets arriving on the dock were two sets of rows with nothing between
-- them. D24's own sketch gave `asserted_unit` a `package_id`, *"nullable; set at
-- receipt"*, and migration 34 built the table without it.
--
-- S35 has been pending on this since it was written, and its blocker was stated
-- as *"the collapse at receipt does not exist yet"* -- which was true of the code
-- and hid that the column it would write into was also missing.

-- ---------------------------------------------------------------------------
-- 1. Our reading of their level vocabulary
-- ---------------------------------------------------------------------------
--
-- `level_code` is raw, in the author's words, and D21 rule 5 keeps it that way:
-- an 856 says `S O T P I`, an EDIFACT message says something else, and D43 stores
-- the order level as a node. **So "non-physical" is not derivable from the raw
-- code**, and S35's sentence -- a non-physical node carries no SSCC and
-- contributes no package -- had no column to be true of.
--
-- This is the same raw-to-resolved shape the table already carries for
-- `raw_package_type_code -> resolved_package_type_id`, and that D93 formalised on
-- the content line. Nullable, because a level we have not read yet is D21's normal
-- state and not an error.

ALTER TABLE asserted_unit
    ADD COLUMN resolved_physical boolean,

    -- S35's first half, and it turns out to be a row-level CHECK rather than the
    -- job it was filed as: a document node has no SSCC because a document is not
    -- a thing you can put a label on.
    ADD CONSTRAINT asserted_unit_document_has_no_sscc_ck
        CHECK (resolved_physical IS DISTINCT FROM false OR sscc IS NULL);

COMMENT ON COLUMN asserted_unit.resolved_physical IS
    'Our reading of level_code: whether this node is a thing on the dock or a '
    'level of the document. NULL until read. The 856''s S and O levels are '
    'documents and its T and P levels are not, which is not derivable from the '
    'raw code and so is recorded rather than inferred. D21, D43, D96.';

-- ---------------------------------------------------------------------------
-- 2. The package it became
-- ---------------------------------------------------------------------------
--
-- S35's second half, now expressible. A document node contributes no package
-- because the CHECK forbids it, rather than because a job notices afterwards.

ALTER TABLE asserted_unit
    ADD COLUMN resolved_package_id uuid,
    ADD COLUMN collapsed_at        timestamptz,
    ADD COLUMN collapsed_by_id     uuid REFERENCES person(id),

    ADD CONSTRAINT asserted_unit_package_fk
        FOREIGN KEY (resolved_package_id, tenant_id)
        REFERENCES package(id, tenant_id),

    ADD CONSTRAINT asserted_unit_document_has_no_package_ck
        CHECK (resolved_package_id IS NULL OR resolved_physical),

    -- The annotation carries its own provenance, as every other one here does.
    ADD CONSTRAINT asserted_unit_collapse_pair_ck
        CHECK ((resolved_package_id IS NULL) = (collapsed_at IS NULL));

COMMENT ON COLUMN asserted_unit.resolved_package_id IS
    'The physical package this declared unit turned out to be, set when a receiver '
    'scanned it. Never derived from the claim: J34 forbids minting a package from '
    'an assertion, so this records a match rather than a creation. D24, D96.';

CREATE INDEX asserted_unit_package_idx
    ON asserted_unit (resolved_package_id)
    WHERE resolved_package_id IS NOT NULL;

-- Ingestion reads their level vocabulary and records what it made of it. The
-- collapse is a later act by a different actor and goes through the function
-- below, so the application never gets UPDATE -- a claim is immutable (D77) and
-- our annotation on it freezes on first use (D21).
GRANT INSERT (resolved_physical), SELECT (resolved_physical, resolved_package_id,
       collapsed_at, collapsed_by_id)
    ON asserted_unit TO spork_app;

-- ---------------------------------------------------------------------------
-- 3. The collapse, as a mediated write
-- ---------------------------------------------------------------------------
--
-- **The first use of D94's pattern since D94 made it work.** The application has
-- no UPDATE on this table and cannot acquire one; the function owns exactly the
-- three columns it writes; and it is a definer owned by `spork_mediation_owner`,
-- which has neither SUPERUSER nor BYPASSRLS, so it stays inside row-level
-- security. S54 checks all of that from the registry rather than from memory.
--
-- Four refusals, and each is a rule that could not otherwise be enforced:

CREATE FUNCTION asserted_unit_collapse(p_unit uuid, p_package uuid, p_actor uuid)
    RETURNS void
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, public
    AS $$
DECLARE
    u          record;
    pkg_sscc   text;
    scans      bigint;
BEGIN
    SELECT * INTO u FROM asserted_unit WHERE id = p_unit FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'no asserted unit %', p_unit;
    END IF;

    -- 1. Freezes on first use, which is D21's rule about our annotations and the
    --    shape D90 built for the content line. Rewriting this after a receipt has
    --    been taken against it changes what the receipt meant.
    IF u.resolved_package_id IS NOT NULL THEN
        RAISE EXCEPTION 'asserted unit % already collapsed onto package %',
            p_unit, u.resolved_package_id
            USING HINT = 'a correction writes a new assertion, which is D21''s rule';
    END IF;

    -- 2. A document level is not a thing on the dock. The CHECK would catch this
    --    too; the function says so in words a receiver can read.
    IF u.resolved_physical IS DISTINCT FROM true THEN
        RAISE EXCEPTION 'asserted unit % is not a physical level (%), so nothing was scanned as it',
            p_unit, u.level_code
            USING HINT = 'S35: a document node carries no SSCC and contributes no package';
    END IF;

    -- 3. The package must be something somebody observed. J34 forbids minting a
    --    package from an assertion and asserts it globally; this refuses the
    --    narrower case at the moment it would be created, which is where a
    --    receiver can still do something about it.
    SELECT count(*) INTO scans FROM package_event
     WHERE package_id = p_package AND source <> 'asn';
    IF scans = 0 THEN
        RAISE EXCEPTION 'package % has no observed event, so collapsing onto it would make the claim its own evidence',
            p_package;
    END IF;

    -- 4. Where both sides carry an SSCC they must be the same SSCC. A pallet
    --    scanned as one licence plate is not the pallet declared under another,
    --    and the whole point of the link is that it is evidenced.
    SELECT sscc INTO pkg_sscc FROM package WHERE id = p_package;
    IF u.sscc IS NOT NULL AND pkg_sscc IS NOT NULL AND u.sscc <> pkg_sscc THEN
        RAISE EXCEPTION 'asserted unit % declares SSCC % and package % carries %',
            p_unit, u.sscc, p_package, pkg_sscc;
    END IF;

    UPDATE asserted_unit
       SET resolved_package_id = p_package,
           collapsed_at = now(),
           collapsed_by_id = p_actor
     WHERE id = p_unit;
END
$$;

ALTER FUNCTION asserted_unit_collapse(uuid, uuid, uuid)
    OWNER TO spork_mediation_owner;

COMMENT ON FUNCTION asserted_unit_collapse(uuid, uuid, uuid) IS
    'Records that a declared unit and a scanned package are the same pallet. '
    'Refuses a second collapse, a document node, a package nobody observed, and '
    'an SSCC that disagrees. D24, D94, D96.';

-- J37: a definer must not grant EXECUTE to PUBLIC.
REVOKE EXECUTE ON FUNCTION asserted_unit_collapse(uuid, uuid, uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION asserted_unit_collapse(uuid, uuid, uuid) TO spork_app;

GRANT SELECT ON package TO spork_mediation_owner;
GRANT SELECT ON package_event TO spork_mediation_owner;
GRANT SELECT ON asserted_unit TO spork_mediation_owner;
GRANT UPDATE (resolved_package_id, collapsed_at, collapsed_by_id)
    ON asserted_unit TO spork_mediation_owner;

-- The declared side of S54's diff, so the mediation is checkable rather than
-- remembered. D94 built this registry for the next use of the pattern; this is it.
INSERT INTO mediated_write (table_name, column_name, function_name) VALUES
    ('asserted_unit', 'resolved_package_id', 'asserted_unit_collapse'),
    ('asserted_unit', 'collapsed_at',        'asserted_unit_collapse'),
    ('asserted_unit', 'collapsed_by_id',     'asserted_unit_collapse');
