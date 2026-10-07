-- Migration 131: what NetSuite says an item looks like, weighs and measures (D237).
--
-- NetSuite has its own picture of most items, and its own weight and UPC for
-- them. Spork has its own record of the same things: photographs taken on the
-- floor, figures measured here, barcodes scanned off the box. **Neither writes
-- the other.** NetSuite's word is a report, as the balance (migration 86) and
-- the unit (migration 123) are: replaced whole by each load of its feed, never
-- edited here. Spork's word is its observations. A screen showing both says
-- whose each is, and where they disagree, that is a thing to put right in
-- NetSuite, not a reason to overwrite either.
--
-- # NetSuite's figures, beside the unit and the supplier's part
--
-- Its weight and size are of what NetSuite counts one of (D218), so they are
-- compared with the unit's figures here. Held in grams and millimetres, read
-- from whatever unit NetSuite gave them in.
--
-- # NetSuite's picture
--
-- The item details name the picture's file in NetSuite; the bytes are carried
-- separately, a few at a time (`reported_item_picture`), into the same store
-- as photographs, by their content address. It is NetSuite's picture of the
-- item and nothing else: not a look, not a face, not something cut.
--
-- # Spork's word on the pictures
--
-- Which picture is an item's main one, a photograph or NetSuite's, and that a
-- picture NetSuite has is not this product (`item_picture_said`). Said here,
-- by somebody, the newest saying winning, so a new load of NetSuite's feed
-- changes none of it.

ALTER TABLE reported_item
    ADD COLUMN picture_file text,
    ADD COLUMN weight_g integer,
    ADD COLUMN length_mm integer,
    ADD COLUMN width_mm integer,
    ADD COLUMN height_mm integer,
    ADD COLUMN upc text;

COMMENT ON COLUMN reported_item.picture_file IS
    'NetSuite''s reference to the item''s picture: its file. The bytes are reported_item_picture''s. D237.';
COMMENT ON COLUMN reported_item.weight_g IS
    'NetSuite''s weight for one of what it counts, in grams. D237.';
COMMENT ON COLUMN reported_item.upc IS
    'NetSuite''s UPC for the item, as it holds it. D237.';

CREATE TABLE reported_item_picture (
    id          uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id   uuid NOT NULL REFERENCES tenant(id),
    item_id     uuid NOT NULL,
    -- The feed it came with, and the file it is: a new file is a new picture.
    source      text NOT NULL,
    file        text NOT NULL,
    digest      text NOT NULL,
    mime        text NOT NULL,
    byte_count  bigint NOT NULL,
    width_px    integer,
    height_px   integer,
    loaded_at   timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT reported_item_picture_item_fk FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    -- One picture an item, a feed: what NetSuite pictures it by now.
    CONSTRAINT reported_item_picture_once UNIQUE (tenant_id, item_id, source)
);

CREATE INDEX reported_item_picture_digest_idx ON reported_item_picture (digest);

COMMENT ON TABLE reported_item_picture IS
    'NetSuite''s picture of an item, as its feed carried it: a report, replaced by a new file. D237, migration 131.';

ALTER TABLE reported_item_picture ENABLE ROW LEVEL SECURITY;
ALTER TABLE reported_item_picture FORCE ROW LEVEL SECURITY;
CREATE POLICY reported_item_picture_own ON reported_item_picture FOR ALL
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

-- A report is replaced, not amended (migration 86).
GRANT SELECT, INSERT, UPDATE, DELETE ON reported_item_picture TO spork_app;
GRANT SELECT ON reported_item_picture TO spork_platform;

CREATE TABLE item_picture_said (
    id              uuid PRIMARY KEY DEFAULT uuidv7(),
    tenant_id       uuid NOT NULL REFERENCES tenant(id),
    item_id         uuid NOT NULL,
    -- The picture, by its content address; for `main`, none goes back to
    -- choosing as before (D141).
    digest          text,
    -- `main`: the item's picture wherever it is shown. `not_it`: a picture
    -- NetSuite has that is not this product, shown to nobody. `is_it`: that
    -- said again the other way.
    said            text NOT NULL,
    client_event_id uuid NOT NULL,
    recorded_by_id  uuid NOT NULL REFERENCES person(id),
    recorded_at     timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT item_picture_said_kind_ck CHECK (said IN ('main', 'not_it', 'is_it')),
    CONSTRAINT item_picture_said_digest_ck CHECK (said = 'main' OR digest IS NOT NULL),
    CONSTRAINT item_picture_said_item_fk FOREIGN KEY (item_id, tenant_id) REFERENCES item(id, tenant_id),
    CONSTRAINT item_picture_said_event_fk
        FOREIGN KEY (tenant_id, client_event_id) REFERENCES client_event(tenant_id, client_event_id),
    CONSTRAINT item_picture_said_event_key UNIQUE (tenant_id, client_event_id)
);

CREATE INDEX item_picture_said_item_idx ON item_picture_said (item_id, recorded_at DESC);

COMMENT ON TABLE item_picture_said IS
    'Spork''s word on an item''s pictures: its main one, and NetSuite''s that are not it. Newest wins. D237.';

ALTER TABLE item_picture_said ENABLE ROW LEVEL SECURITY;
ALTER TABLE item_picture_said FORCE ROW LEVEL SECURITY;
CREATE POLICY item_picture_said_own ON item_picture_said
    USING (tenant_id = current_tenant())
    WITH CHECK (tenant_id = current_tenant());

GRANT SELECT, INSERT ON item_picture_said TO spork_app;

-- **Where NetSuite and Spork disagree** about an item, read and never stored:
-- both words are kept, so the comparison is a reading of them. NetSuite's
-- newest report against the unit's figures measured here (D219), its own or
-- its family's; and NetSuite's UPC against the barcodes scanned for it.
--
-- - Weight differs by more than 10 g and 5 %.
-- - Size differs where any side, both sorted longest first (so a box turned
--   round is the same box), differs by more than 5 mm and 5 %.
-- - UPC differs where Spork has a GTIN for the item and none is NetSuite's.
--
-- A unit of several single ones that is no package of its own (D233) has no
-- card to compare, so its figures are not.
CREATE VIEW item_netsuite_differs WITH (security_invoker = true) AS
WITH ns AS (
    SELECT DISTINCT ON (r.item_id) r.item_id, r.weight_g, r.length_mm, r.width_mm, r.height_mm, r.upc
      FROM reported_item r
     ORDER BY r.item_id, r.as_at DESC
),
unit AS (
    SELECT ns.*, i.style_id, coalesce(u.level::text, 'each') AS level, coalesce(u.singles, 1) AS singles
      FROM ns
      JOIN item i ON i.id = ns.item_id
      LEFT JOIN item_unit_level u ON u.item_id = ns.item_id
),
measured AS (
    SELECT unit.*,
           (SELECT max(oc.value_numeric) FILTER (WHERE m.code = 'gross_weight')
              FROM observable o
              JOIN observation_current oc ON oc.observable_id = o.id
              JOIN metric m ON m.id = oc.metric_id
             WHERE (o.item_id = unit.item_id OR (unit.style_id IS NOT NULL AND o.item_style_id = unit.style_id))
               AND o.packaging_level::text = unit.level
               AND oc.method IN ('instrument', 'scan', 'keyed', 'derived', 'photographed')
               AND unit.singles = 1) AS spork_weight_g,
           (SELECT array_agg(oc.value_numeric ORDER BY oc.value_numeric DESC)
              FROM observable o
              JOIN observation_current oc ON oc.observable_id = o.id
              JOIN metric m ON m.id = oc.metric_id
             WHERE (o.item_id = unit.item_id OR (unit.style_id IS NOT NULL AND o.item_style_id = unit.style_id))
               AND o.packaging_level::text = unit.level
               AND m.code IN ('length', 'width', 'height')
               AND oc.value_numeric IS NOT NULL
               AND oc.method IN ('instrument', 'scan', 'keyed', 'derived', 'photographed')
               AND unit.singles = 1) AS spork_sides,
           (SELECT array_agg(s ORDER BY s DESC)
              FROM unnest(ARRAY[unit.length_mm, unit.width_mm, unit.height_mm]) s
             WHERE s IS NOT NULL) AS netsuite_sides,
           (SELECT array_agg(b.barcode)
              FROM item_barcode b
             WHERE b.item_id = unit.item_id AND b.scheme = 'gtin') AS spork_gtins
      FROM unit
)
SELECT item_id,
       weight_g AS netsuite_weight_g,
       spork_weight_g::bigint AS spork_weight_g,
       weight_g IS NOT NULL AND spork_weight_g IS NOT NULL
         AND abs(spork_weight_g - weight_g) > greatest(10, 0.05 * weight_g) AS weight_differs,
       cardinality(netsuite_sides) = 3 AND cardinality(spork_sides) = 3
         AND EXISTS (SELECT 1 FROM generate_series(1, 3) k
                      WHERE abs(spork_sides[k] - netsuite_sides[k]) > greatest(5, 0.05 * netsuite_sides[k]))
         AS size_differs,
       upc AS netsuite_upc,
       upc IS NOT NULL AND spork_gtins IS NOT NULL
         AND NOT (lpad(regexp_replace(upc, '[^0-9]', '', 'g'), 14, '0') = ANY (spork_gtins)) AS upc_differs
  FROM measured;

COMMENT ON VIEW item_netsuite_differs IS
    'Where NetSuite''s weight, size or UPC for an item disagree with what Spork measured and scanned: '
    'a reading of both words, never stored. D237, migration 131.';

GRANT SELECT ON item_netsuite_differs TO spork_app;
