-- Migration 79: `item_barcode`, and one identifier is not the way it was printed.
--
-- D34 was adopted on 2026-08-03 and says of itself that it *"blocks the first
-- migration and every scan."* Seventy-eight migrations later nothing scans,
-- because the table a scan resolves through has never existed. This is that
-- table, built to the decision rather than to the shape a scanner suggests.
--
-- # The scheme is what the identifier is, not how it reached the reader
--
-- The obvious column is `kind`, spelled `gtin13 | gtin14 | itf14 | internal |
-- supplier_ref`. It collapses two axes and each collapse causes a defect.
--
-- ITF-14 is a *symbology* — one of several ways to print a GTIN-14 — so putting
-- it here means a carton read from the ITF-14 and the same carton read from the
-- GS1-128 printed beside it resolve through different rows. How a barcode was
-- printed belongs on the scan, not on the identity.
--
-- GTIN-13 and GTIN-14 are not two schemes either. A GTIN-13 right-justified to
-- 14 with indicator digit 0 **is the same trade item**; indicators 1 to 8 denote
-- higher packaging levels and are genuinely different ones. Store the
-- unnormalised form and `9312345678907` from an EAN-13 and `09312345678907` from
-- AI 01 on the carton become two rows for one product, so the carton scan
-- misses. That is the most common integration defect in this area and it is
-- silent. So `scheme` is `gtin | internal | supplier_reference`, and GTINs are
-- normalised to 14 on write.
--
-- # Why an exclusion over a range rather than `active boolean`
--
-- The shape this wants to be written in is `active boolean` with
-- `UNIQUE (tenant_id, barcode) WHERE active`, and it enforces nothing across the
-- shared catalogue: `tenant_id` is NULL for every shared row, a NULL comparison
-- yields NULL rather than true, and the unique index therefore permits any
-- number of duplicate shared barcodes.
--
-- Written as an exclusion over a range instead, one constraint does three jobs:
-- a barcode resolves to at most one item per scope at any instant;
-- deactivation closes the range rather than flipping a boolean, so a scan
-- recorded in March is still explainable in September; and a barcode that
-- legitimately rebinds does so as a later non-overlapping range, which the same
-- constraint permits while continuing to forbid the overlap.

-- ---------------------------------------------------------------------------
-- `item` offers its tenant, before anything references it
-- ---------------------------------------------------------------------------
--
-- Trivially satisfiable — `id` is already the primary key, so this adds no
-- constraint on the data — and it is the same `(id, tenant_id)` shape
-- `observation_event` already offered migration 78 for exactly this purpose.
--
-- It sits above the table rather than below it because a composite foreign key
-- is resolved when the table is created, not when the migration ends.
ALTER TABLE item ADD CONSTRAINT item_tenant_key UNIQUE (id, tenant_id);

CREATE TABLE item_barcode (
    id                uuid PRIMARY KEY DEFAULT uuidv7(),

    -- NULL is the shared catalogue, per D19's reference shape.
    tenant_id         uuid REFERENCES tenant(id),

    item_id           uuid NOT NULL,

    -- **NULL means the brand owner issued it (a GTIN) or we did (an internal
    -- code); set means it is this party's code and means nothing outside that.**
    --
    -- A supplier's own code on a carton is not globally unique, and two
    -- suppliers will eventually use the same string for different products. As a
    -- row keyed on the barcode alone, `supplier_reference` is wrong the first
    -- time that happens — and wrong by resolving confidently.
    issuer_party_id   uuid,

    -- Text, never numeric: leading zeros are significant and a GTIN-14 beginning
    -- 0 is the commonest case there is.
    barcode           text NOT NULL,

    scheme            text NOT NULL,

    -- D23's unit vocabulary. **Not the packaging level**, which the built
    -- vocabulary does not carry — see the note at the foot of this file.
    unit_id           uuid NOT NULL REFERENCES unit(id),

    -- Base units per scan of this barcode. NULL = variable measure, where the
    -- weight rides in the barcode under AI 310n rather than in a table, which is
    -- why the CHECK below confines a NULL to GTINs: no other scheme has
    -- anywhere to carry one.
    quantity          bigint,

    effective         daterange NOT NULL
                          DEFAULT daterange(CURRENT_DATE, NULL),

    CONSTRAINT item_barcode_scheme_ck
        CHECK (scheme IN ('gtin', 'internal', 'supplier_reference')),

    -- J42 is asserted over the table rather than at the call site, because
    -- normalisation happens in the writer and a second write path will be added.
    -- This is the structural half: the length. The check digit is arithmetic the
    -- register runs.
    CONSTRAINT item_barcode_gtin_width_ck
        CHECK (scheme <> 'gtin' OR barcode ~ '^[0-9]{14}$'),

    CONSTRAINT item_barcode_quantity_ck
        CHECK (quantity IS NOT NULL OR scheme = 'gtin'),
    CONSTRAINT item_barcode_quantity_positive_ck
        CHECK (quantity IS NULL OR quantity > 0),

    -- **Two foreign keys to one table, and both earn their place.**
    --
    -- The composite carries the tenant into the key per S51, so a tenant-scoped
    -- row cannot point at another tenant's item — the hole D55 found open since
    -- migration 1, where RLS filters what a query *reads* and says nothing about
    -- what a row may *point at*.
    --
    -- But a composite FK is MATCH SIMPLE: when any column of the key is NULL the
    -- **entire** check is skipped. `tenant_id` is NULL for every shared row, so
    -- the composite alone would let a shared row name an item that does not
    -- exist at all. The plain key is what keeps existence enforced on that arm.
    CONSTRAINT item_barcode_item_fk
        FOREIGN KEY (item_id) REFERENCES item(id),
    CONSTRAINT item_barcode_item_tenant_fk
        FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),

    CONSTRAINT item_barcode_issuer_fk
        FOREIGN KEY (issuer_party_id, tenant_id) REFERENCES party(id, tenant_id),

    -- **The COALESCE sentinels are load-bearing rather than stylistic**, and
    -- J43 exists to say so from `pg_constraint`. `UNIQUE NULLS NOT DISTINCT`
    -- fixes the shared-catalogue hole for a unique index and has **no
    -- equivalent for an exclusion constraint**, so without the sentinels a NULL
    -- tenant compares as NULL, the exclusion never fires between two shared
    -- rows, and duplicate shared barcodes become insertable while nothing
    -- complains. The first reviewer to read this will try to simplify it back
    -- into the hole.
    CONSTRAINT item_barcode_one_meaning_at_a_time
        EXCLUDE USING gist (
            COALESCE(tenant_id, '00000000-0000-0000-0000-000000000000'::uuid) WITH =,
            COALESCE(issuer_party_id, '00000000-0000-0000-0000-000000000000'::uuid) WITH =,
            barcode WITH =,
            effective WITH &&
        )
);

COMMENT ON TABLE item_barcode IS
    'What a scanned identifier means, per scope and per instant (D34). scheme is '
    'what the identifier IS; how it was printed belongs on the scan. GTINs are '
    'normalised to 14 on write, because a GTIN-13 padded with indicator 0 is the '
    'same trade item and two rows for it make the carton scan miss.';

COMMENT ON CONSTRAINT item_barcode_one_meaning_at_a_time ON item_barcode IS
    'The COALESCE sentinels are load-bearing: UNIQUE NULLS NOT DISTINCT has no '
    'equivalent for an exclusion constraint, so removing them makes duplicate '
    'shared barcodes insertable and nothing complains. J43 asserts they are here.';

-- The scan-resolution path. Deliberately on `barcode` alone and not on
-- `(tenant_id, barcode)`: the scanner does not know whose catalogue it is
-- holding, and the resolver reads both scopes and orders `tenant_id NULLS LAST`.
CREATE INDEX item_barcode_scan_idx ON item_barcode (barcode);

-- Everything a given item answers to, which is the maintenance direction.
CREATE INDEX item_barcode_item_idx ON item_barcode (item_id);

-- ---------------------------------------------------------------------------
-- Reference, so D19's two policy shapes apply unchanged
-- ---------------------------------------------------------------------------
--
-- **A feed may not write here.** A GS1 National Product Catalogue export, a
-- supplier price file or a wholesaler's catalogue is an *assertion* under D21,
-- not reference data: it lands as an assertion and something with a name
-- promotes it. If a feed wrote here directly, a supplier would silently rewrite
-- what a scan means — the poisoning D19 exists to prevent, one level below
-- measurements and with a worse blast radius, because a wrong measurement
-- produces a bad autofill and a wrong barcode binding produces stock movements
-- against the wrong item.
ALTER TABLE item_barcode ENABLE ROW LEVEL SECURITY;
ALTER TABLE item_barcode FORCE ROW LEVEL SECURITY;

CREATE POLICY item_barcode_shared_read ON item_barcode FOR SELECT
    USING (tenant_id IS NULL OR tenant_id = current_tenant());

CREATE POLICY item_barcode_own_write ON item_barcode FOR ALL
    USING (tenant_id = current_tenant()
           OR (tenant_id IS NULL AND is_platform()))
    WITH CHECK (tenant_id = current_tenant()
           OR (tenant_id IS NULL AND is_platform()));

-- Reference takes the full set on the application role (D25). Closing an
-- `effective` range is an UPDATE and not a DELETE, and D31 retains the closed
-- row indefinitely: it is the evidence for what a historical scan meant, and
-- truncating it makes historical resolution quietly start returning nothing.
GRANT SELECT, INSERT, UPDATE, DELETE ON item_barcode TO spork_app;
GRANT SELECT ON item_barcode TO spork_platform;
GRANT SELECT ON item_barcode TO spork_projection_owner;
GRANT SELECT ON item_barcode TO spork_scheduler;

-- ---------------------------------------------------------------------------
-- What this does not do, said here so the next reader does not go looking
-- ---------------------------------------------------------------------------
--
-- **`unit_id` does not imply a packaging level, though D34 assumed it would.**
-- The decision names `item_barcode.unit_id` as the consumer that replaces an
-- `unit_level` enum, *"the unit vocabulary carries packaging levels and measures
-- in one table, as already recorded."* The built vocabulary does not: `unit`
-- holds `ea` in the count dimension and nothing above it — no carton, no inner,
-- no pallet. So a carton GTIN cannot say it is a carton, and the resolver does
-- not pretend otherwise; it returns the item and the operator names the level.
-- Adding packaging levels to `unit` is D23's to decide, not this migration's to
-- assume.
--
-- **No `symbology` table and no failure record.** D28 puts the raw scanned
-- string, the symbology and the four resolution-failure kinds on
-- `activity_event`, which does not exist either. That is a range-partitioned
-- fact table and a reference table of its own, and it is deferred deliberately
-- rather than forgotten — see D136.
